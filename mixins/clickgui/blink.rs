// Blink: holds the tick's movement packet back, then releases everything it
// held in one burst, so the server keeps seeing the player where they were
// until the moment the trail goes out.
//
// The seam is the movement chain in `remote::clickgui_spoof`. Blink is its
// last link, so a packet it keeps has already been through NoFall, Criticals,
// Speed, Step and Fly — holding it does not undo their rewrites, and the
// packet that finally arrives is the one they produced. The chain is asked
// exactly once per tick: from `Game::send` when the position sender produced
// a packet, or from `native_tick` with `None` when it did not. Holding is
// therefore just answering `None` and keeping the packet.
//
// Only the movement packet is held: positions, look, and the ground flag.
// Keepalive, chat, inventory clicks and the extra packets the other modules
// send themselves still go out, which is what keeps the connection alive
// while the server believes the player is standing still.
//
// A held packet is released through `session.send` directly, not through
// `Game::send`. It has already been through the chain, and running it again
// would rewrite it twice.
use core::cell::{Cell, RefCell};

use horizon_client::world_primitives::WorldPrimitives;

use crate::game::{Dim, Game, clickgui as m};

type Packet = horizon_protocol::packets::play::Serverbound;

/// Where the server last had the player: the position and yaw of the newest
/// movement packet that actually left. Held packets do not move it, which is
/// the whole point of the marker.
#[derive(Clone, Copy, Default)]
struct Known {
    position: [f64; 3],
    yaw: f32,
}

/// The most packets held at once. A vanilla client sends about twenty a
/// second, so this is a minute of blinking; at the cap the burst is released
/// early instead of the queue growing without bound.
const CAP: usize = 1200;

struct Held {
    packets: Vec<Packet>,
    /// The dimension the held packets name positions in. A portal throws them
    /// away: replayed in the next world they would be a teleport across it.
    dim: Option<Dim>,
    /// The tick the pulse timer runs out on, and whether it has been armed.
    next: u64,
    running: bool,
}

thread_local! {
    static HELD: RefCell<Held> = const {
        RefCell::new(Held {
            packets: Vec::new(),
            dim: None,
            next: 0,
            running: false,
        })
    };
    /// Whether `spoof` keeps the packet. Set at the tick's head, so a toggle
    /// takes effect on the next tick rather than in the middle of a send.
    static HOLDING: Cell<bool> = const { Cell::new(false) };
    /// The last position the server was told about, for the marker. None until
    /// a packet with a position has gone out, and again after a teleport,
    /// which moves the player without one.
    static KNOWN: Cell<Option<Known>> = const { Cell::new(None) };
}

/// Notes where the server now has the player. `MovePlayerRot` and
/// `MovePlayerStatus` carry no position, so they leave the note as it was.
fn remember(packet: &Packet) {
    match packet {
        Packet::MovePlayerPos(p) => {
            let yaw = KNOWN.get().map_or(0.0, |known| known.yaw);
            KNOWN.set(Some(Known {
                position: [p.x, p.y, p.z],
                yaw,
            }));
        }
        Packet::MovePlayerPosRot(p) => KNOWN.set(Some(Known {
            position: [p.x, p.y, p.z],
            yaw: p.yaw,
        })),
        _ => {}
    }
}

/// Notes a movement packet another module sent straight into the session,
/// past this chain: the server has the position it carries, so the marker
/// moves to it. Phase's bursts are the case this exists for — they go out
/// through `session.send` on purpose, so that a hold cannot swallow them.
pub(crate) fn noted(packet: &Packet) {
    remember(packet);
}

/// The chain's last link: keep the tick's packet, or let it go. A packet that
/// goes out moves the server, so it is also what the marker is drawn from.
pub(super) fn spoof(packet: Option<Packet>) -> Option<Packet> {
    if !HOLDING.get() {
        if let Some(packet) = &packet {
            remember(packet);
        }
        return packet;
    }
    let Some(packet) = packet else {
        return None;
    };
    HELD.with(|held| held.borrow_mut().packets.push(packet));
    None
}

/// A new level: the held packets and the marker both name the world that was
/// left behind, so both go. Called every tick with the level the player is in,
/// whether or not the module is on, so that turning Blink on is not mistaken
/// for a world change and does not blank the marker.
///
/// A portal and a respawn reach the game through the respawn handler, which
/// sets `Game::dim` and rebuilds the level without going through
/// `remote_teleport`, so `teleport` alone does not cover this.
fn level(dim: Dim) {
    if HELD.with(|held| held.borrow().dim == Some(dim)) {
        return;
    }
    HELD.with(|held| {
        let mut held = held.borrow_mut();
        held.packets.clear();
        held.dim = Some(dim);
        // A trail that no longer exists has no timer to keep.
        held.running = false;
    });
    KNOWN.set(None);
}

/// `native_tick`'s head: starts and ends a hold, and releases on the pulse
/// timer. It runs before `remote::clickgui_spoof::flush` (priority 105
/// against its 100), so a burst never arrives after the packet of the tick
/// that released it.
pub(super) fn tick(game: &mut Game<'_>) {
    // No session to hold a packet for: drop the trail rather than replay it
    // into whatever server comes next, and drop the marker with it — its
    // position belongs to a connection that has ended.
    if game.remote.is_none() || game.player.is_none() {
        HOLDING.set(false);
        forget();
        HELD.with(|held| {
            let mut held = held.borrow_mut();
            held.dim = None;
            held.running = false;
        });
        return;
    }
    // Before the module is consulted: a world change must be noticed even
    // while Blink is off, or the next activation would look like one.
    level(game.dim);
    let config = if game.native_on(m::ID) {
        m::blink()
    } else {
        None
    };
    let Some(config) = config else {
        // Off: what is held is the player's own trail, and dropping it would
        // leave the server standing where the module was turned on.
        HOLDING.set(false);
        release(game);
        HELD.with(|held| held.borrow_mut().running = false);
        return;
    };
    let delay = config.delay.max(1) as u64;
    let now = game.ticks;
    let due = HELD.with(|held| {
        let mut held = held.borrow_mut();
        if !held.running {
            held.running = true;
            held.next = now.saturating_add(delay);
            return false;
        }
        if config.pulse && now >= held.next {
            held.next = now.saturating_add(delay);
            return true;
        }
        false
    });
    let full = HELD.with(|held| held.borrow().packets.len() >= CAP);
    if due || full {
        release(game);
    }
    HOLDING.set(true);
}

/// Throws the trail and the marker away: the held positions, and the position
/// the server was last told about. Used when the player leaves the world, and
/// when the server moves them without a movement packet.
fn forget() {
    HELD.with(|held| held.borrow_mut().packets.clear());
    KNOWN.set(None);
}

/// `remote_teleport`'s head: the server is moving the player, so every held
/// position is behind the new one. Replaying them afterwards would drag the
/// player back to where the teleport found them, and a setback or a `/home`
/// is exactly when that is most visible. The marker is dropped too: until the
/// next packet goes out there is no position the server and the client agree
/// on, and a stale one would point at the world the teleport left.
pub(super) fn teleport(_game: &mut Game<'_>) {
    forget();
}

/// Sends everything held, oldest first, and empties the queue. With no remote
/// the packets are dropped: there is no connection they belong to any more.
fn release(game: &Game<'_>) {
    let packets = HELD.with(|held| core::mem::take(&mut held.borrow_mut().packets));
    let Some(remote) = &game.remote else {
        return;
    };
    for packet in packets {
        remember(&packet);
        remote.session.send(packet);
    }
}

/// The marker: the player's own figure, filled, where the server last had
/// them. It is the same body Chams' `Filled` draws, so the ghost reads as a
/// player rather than a box, and it is only drawn while the module is on —
/// with Blink off, the server's position and the player's are the same and the
/// marker would sit inside the model.
pub(super) fn draw(
    game: &Game<'_>,
    _partial: f32,
    _camera: [f64; 3],
    primitives: &mut WorldPrimitives,
) {
    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    let Some(config) = m::blink() else {
        return;
    };
    if !config.marker || config.marker_color >> 24 == 0 {
        return;
    }
    let Some(known) = KNOWN.get() else {
        return;
    };
    for (from, to) in
        crate::game::clickgui_chams::player_figure(known.position, f64::from(known.yaw))
    {
        let primitive = primitives.filled_box(from, to, config.marker_color);
        if config.marker_walls {
            primitive.through_walls();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HELD, HOLDING, KNOWN, Packet, forget, level, spoof};
    use crate::game::Dim;
    use horizon_protocol::packets::play::{MovePlayerPos, MovePlayerPosRot};

    fn moved(x: f64) -> Packet {
        Packet::MovePlayerPos(MovePlayerPos {
            x,
            y: 64.0,
            z: 0.0,
            flags: 0,
        })
    }

    fn marker() -> Option<([f64; 3], f32)> {
        KNOWN.get().map(|known| (known.position, known.yaw))
    }

    fn held() -> usize {
        HELD.with(|held| held.borrow().packets.len())
    }

    #[test]
    fn a_packet_that_leaves_moves_the_marker() {
        forget();
        HOLDING.set(false);
        spoof(Some(moved(10.0)));
        assert_eq!(marker(), Some(([10.0, 64.0, 0.0], 0.0)));
        spoof(Some(moved(12.0)));
        assert_eq!(marker(), Some(([12.0, 64.0, 0.0], 0.0)));
    }

    #[test]
    fn a_held_packet_moves_neither_the_marker_nor_the_server() {
        forget();
        HOLDING.set(false);
        spoof(Some(moved(10.0)));
        HOLDING.set(true);
        assert!(spoof(Some(moved(20.0))).is_none());
        assert_eq!(marker(), Some(([10.0, 64.0, 0.0], 0.0)));
        assert_eq!(held(), 1);
        HOLDING.set(false);
        forget();
    }

    #[test]
    fn a_burst_sent_past_the_chain_still_moves_the_marker() {
        forget();
        HOLDING.set(false);
        spoof(Some(moved(10.0)));
        // Phase's bursts skip `spoof` on purpose, so that a hold cannot
        // swallow them; they note themselves instead.
        HOLDING.set(true);
        super::noted(&moved(10.5));
        assert_eq!(marker(), Some(([10.5, 64.0, 0.0], 0.0)));
        // Noting one is not holding one: nothing joins the trail.
        assert_eq!(held(), 0);
        HOLDING.set(false);
        forget();
    }

    #[test]
    fn a_turn_only_packet_keeps_the_position_but_takes_the_yaw() {
        forget();
        HOLDING.set(false);
        spoof(Some(moved(10.0)));
        spoof(Some(Packet::MovePlayerPosRot(MovePlayerPosRot {
            x: 10.0,
            y: 64.0,
            z: 0.0,
            yaw: 90.0,
            pitch: 0.0,
            flags: 0,
        })));
        assert_eq!(marker(), Some(([10.0, 64.0, 0.0], 90.0)));
        // A packet with no position in it leaves the marker where it was.
        spoof(Some(Packet::MovePlayerRot(
            horizon_protocol::packets::play::MovePlayerRot {
                yaw: 180.0,
                pitch: 0.0,
                flags: 0,
            },
        )));
        assert_eq!(marker(), Some(([10.0, 64.0, 0.0], 90.0)));
    }

    #[test]
    fn forgetting_drops_the_trail_and_the_marker() {
        forget();
        HOLDING.set(false);
        spoof(Some(moved(10.0)));
        HOLDING.set(true);
        spoof(Some(moved(30.0)));
        assert_eq!(held(), 1);
        forget();
        assert_eq!(held(), 0);
        assert_eq!(marker(), None);
        HOLDING.set(false);
    }

    #[test]
    fn a_new_level_drops_the_trail_and_the_marker() {
        forget();
        level(Dim::Overworld);
        HOLDING.set(false);
        spoof(Some(moved(10.0)));
        HOLDING.set(true);
        spoof(Some(moved(20.0)));
        assert_eq!(held(), 1);
        assert!(marker().is_some());
        // The same level: nothing is dropped, which is what makes turning
        // the module on safe.
        level(Dim::Overworld);
        assert_eq!(held(), 1);
        assert!(marker().is_some());
        // A portal: the trail and the marker both name the level left behind,
        // and the server has not been told about the new one yet.
        level(Dim::Nether);
        assert_eq!(held(), 0);
        assert_eq!(marker(), None);
        HOLDING.set(false);
    }
}
