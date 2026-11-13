// Phase: moves the player into a block, then through it.
//
// The references act at their client's PreMotion or player-update event. In
// Horizon the same point is the head of `remote_send_state`: after the tick's
// physics, before the movement packet is built. Whatever this module writes
// into `player.position` there is the position the packet carries and the
// position the next tick's physics starts from, so a mode that walks the
// player into a block only has to move them; the packet follows on its own.
//
// The camera is drawn from that same field, interpolated from
// `previous_position`, which the player tick saves at its start. A write here
// therefore does move the view, but as a slide across the tick, so the modes
// that jump move `previous_position` with the body and snap instead.
//
// The modes split by what the server is told. Vanilla, Skip and Mineplex send
// the references' own bursts of position packets through `session.send`, not
// `Game::send`: those packets have already been shaped here, and the movement
// chain would run them through NoFall, Criticals, Speed, Step, Fly and Blink a
// second time. Blink would hold them, which is the one thing a burst must not
// do.
//
// The collision-shape family (the two BlocksMC ports, Seppuku's SAND,
// Shoreline's SAND and CLIMB, ThunderHack's Vanilla, Wurst's NoClip) removes or
// replaces the block's collision box, and the packet-hold family (Celestial's
// Sunrise New/Old and Lorent Craft, Grim's and LiquidBounce's PhaseBlink,
// Alan's WatchdogBlink) keeps the player's packets back. Horizon's collision
// seam, `native_block_collision`, is read-only and server-side, so the first
// family has no equivalent here — Nudge is the reachable half of it, a position
// push that ignores the box. The second family is the Blink module's, and
// duplicating it here would only give two modules the same queue.
//
// The teleport-ID desync family (earthhack's, Wurst+'s, Konas's, Phobos's,
// Celestial's Lorent Packet and New Lorent, Alan's WatchdogPrediction) spoofs
// `AcceptTeleportation` ids around an out-of-bounds position. That is PacketFly
// with a different name, and it is not a way through a block; those references
// are not modes.
use core::cell::RefCell;

use horizon_core::geometry::Vec3;
use horizon_protocol::packets::play::{self, Serverbound as S};
use winit::keyboard::KeyCode;

use crate::game::{Game, clickgui as m};

/// The ground bit of `ServerboundMovePlayerPacket`'s flags.
const ON_GROUND: u8 = 1;
/// `ServerboundPlayerActionPacket.Action`: 0 start, 2 stop.
const START_DESTROY_BLOCK: i32 = 0;
const STOP_DESTROY_BLOCK: i32 = 2;
/// `Direction.UP`, the face the references break and confirm against.
const UP: u8 = 1;
/// The distance the Hypixel 2025-02-04 mode drops the player before the
/// server's own gravity check accepts the position.
const GRAVITY: f64 = 0.078_400_001_52;
/// Intave's descent per tick while the block under the feet is being mined.
const SINK_RATE: f64 = 0.005_2;
/// An ender pearl's own use cooldown, in ticks. Throwing one per attempt
/// without waiting for it only spends the player's pearls.
const PEARL_COOLDOWN: u64 = 20;

/// What the module keeps between ticks.
struct State {
    /// The mode the counters belong to (a change resets them).
    mode: Option<m::PhaseMode>,
    /// The tick the next attempt is due on.
    next: u64,
    /// Mineplex's stage in its three-tick burst.
    stage: i32,
    /// Mineplex has latched on a hit and runs its burst to the end.
    clipping: bool,
    /// Center has already settled this activation.
    settled: bool,
}

impl State {
    const fn new() -> Self {
        Self {
            mode: None,
            next: 0,
            stage: 0,
            clipping: false,
            settled: false,
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::new()) };
}

/// The tick's movement facts, read once so every mode works from the same
/// snapshot.
#[derive(Clone, Copy, Default)]
struct Ctx {
    position: [f64; 3],
    /// The yaw the movement input faces.
    yaw: f32,
    on_ground: bool,
    /// The body is held against a block this tick.
    collided: bool,
    sneak: bool,
    jump: bool,
}

fn context(game: &Game<'_>) -> Ctx {
    let Some(player) = game.player.as_ref() else {
        return Ctx::default();
    };
    let held = |key: KeyCode| game.physical_keys.contains(&key);
    let dir = m::input_direction(
        held(KeyCode::KeyW),
        held(KeyCode::KeyS),
        held(KeyCode::KeyA),
        held(KeyCode::KeyD),
    );
    Ctx {
        position: [player.position.x, player.position.y, player.position.z],
        // Standing still, the module works along the way the player looks.
        yaw: if dir[0] == 0.0 && dir[1] == 0.0 {
            game.yaw
        } else {
            m::movement_yaw(game.yaw, dir)
        },
        on_ground: player.ground.on_ground,
        collided: player.collision.horizontal,
        sneak: held(KeyCode::ShiftLeft) || held(KeyCode::ShiftRight),
        jump: held(KeyCode::Space),
    }
}

/// The horizontal offset `distance` along `yaw`, in the references' sign
/// convention: `-sin` on x, `+cos` on z.
fn forward(yaw: f32, distance: f64) -> [f64; 2] {
    let (sin, cos) = f64::from(yaw.to_radians()).sin_cos();
    [-sin * distance, cos * distance]
}

/// Vanilla's six packets: home, the corner at the world origin, home again, a
/// fifth of a block up, the corner, and half a block over on both axes. The
/// server takes the last one, so it now stands half a block into the wall.
fn vanilla_burst(position: [f64; 3]) -> [[f64; 3]; 6] {
    let [px, py, pz] = position;
    [
        [px, py, pz],
        [0.5, 0.0, 0.5],
        [px, py, pz],
        [px, py + 0.2, pz],
        [0.5, 0.0, 0.5],
        [px + 0.5, py, pz + 0.5],
    ]
}

/// Skip's six packets: each of three steps lifts the head 0.06 and then
/// reaches further along the way, to 0.3 and then 0.6.
fn skip_burst(position: [f64; 3], yaw: f32) -> [[f64; 3]; 6] {
    let [px, py, pz] = position;
    let [dx, dz] = forward(yaw, 0.3);
    let mut out = [[0.0; 3]; 6];
    for step in 0..3_usize {
        let reach = step as f64;
        out[step * 2] = [px, py + 0.06, pz];
        out[step * 2 + 1] = [px + dx * reach, py, pz + dz * reach];
    }
    out
}

fn send_position(game: &Game<'_>, position: [f64; 3], on_ground: bool) {
    let Some(remote) = &game.remote else {
        return;
    };
    let packet = S::MovePlayerPos(play::MovePlayerPos {
        x: position[0],
        y: position[1],
        z: position[2],
        flags: if on_ground { ON_GROUND } else { 0 },
    });
    // Blink's marker is drawn from the last position the server was told, and
    // this packet skips the chain that would normally note it.
    crate::game::clickgui_blink::noted(&packet);
    remote.session.send(packet);
}

fn send_position_rotation(
    game: &Game<'_>,
    position: [f64; 3],
    yaw: f32,
    pitch: f32,
    on_ground: bool,
) {
    let Some(remote) = &game.remote else {
        return;
    };
    let packet = S::MovePlayerPosRot(play::MovePlayerPosRot {
        x: position[0],
        y: position[1],
        z: position[2],
        yaw,
        pitch,
        flags: if on_ground { ON_GROUND } else { 0 },
    });
    crate::game::clickgui_blink::noted(&packet);
    remote.session.send(packet);
}

/// Moves the body without asking the collision solver: the next tick's physics
/// starts from here, and this tick's packet says so.
fn shift(game: &mut Game<'_>, delta: [f64; 3]) {
    if let Some(player) = game.player.as_mut() {
        player.position.x += delta[0];
        player.position.y += delta[1];
        player.position.z += delta[2];
    }
}

fn place(game: &mut Game<'_>, position: [f64; 3]) {
    if let Some(player) = game.player.as_mut() {
        player.position.x = position[0];
        player.position.y = position[1];
        player.position.z = position[2];
    }
}

/// The engine's own convention for an instant move: `previous_position` travels
/// with the body, so the camera and the model are drawn at the new spot instead
/// of sliding there across the tick. `Game::tp`, going to sleep and every
/// teleport the server sends all set both. Only the modes that jump use it — the
/// camera interpolates from `previous_position`, which the player tick saves at
/// its start, so a mode that nudges every tick has to leave it alone or walking
/// would render as twenty steps a second.
fn snap(game: &mut Game<'_>) {
    let Some(at) = game.player.as_ref().map(|player| player.position) else {
        return;
    };
    game.previous_position = at;
}

/// Whether `from` can be this tick's start for a body now at `at`. A tick's own
/// walk is well under a block — a sprint jump reaches about 0.6 — so a bigger
/// gap means `previous_position` is not this tick's start: a level change, or a
/// player that has not ticked yet. Rewinding to one of those would drag the
/// player somewhere they never were.
fn same_tick(from: Vec3, at: Vec3) -> bool {
    let (dx, dz) = (at.x - from.x, at.z - from.z);
    dx * dx + dz * dz < 1.0
}

/// A block a body can stand in: air, or something replaceable like grass.
fn open(game: &Game<'_>, x: i32, y: i32, z: i32) -> bool {
    let state = game.world.block(x, y, z);
    let facts = game.res.facts();
    facts.is_air(state) || facts.get(state).replaceable
}

fn due(now: u64) -> bool {
    STATE.with(|state| now >= state.borrow().next)
}

fn schedule(now: u64, config: &m::PhaseConfig) {
    STATE.with(|state| {
        state.borrow_mut().next = now.saturating_add(u64::from(config.delay));
    });
}

/// The first hotbar slot holding an ender pearl.
fn pearl_slot(game: &Game<'_>) -> Option<usize> {
    (0..9).find(|&slot| {
        game.inventory
            .get(slot)
            .and_then(|stack| stack.as_ref())
            .is_some_and(|stack| &*stack.item == "minecraft:ender_pearl")
    })
}

/// `remote_send_state`'s head, after the physics and before the packet. Every
/// mode works here, so what it does to the body is what goes out.
pub(super) fn step(game: &mut Game<'_>) {
    let config = if game.native_on(m::ID) { m::phase() } else { None };
    let Some(config) = config else {
        STATE.with(|state| *state.borrow_mut() = State::new());
        return;
    };
    // A rider's position is its vehicle's to send: `remote_send_state` builds a
    // vehicle packet instead, so moving the body here would only desync the two.
    if game.player.is_none() || game.remote.is_none() || game.riding.is_some() {
        return;
    }
    let ctx = context(game);
    // A fresh activation, or a new mode, starts its counters over: Center's
    // one shot is armed again and Mineplex's burst begins at its first stage.
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.mode != Some(config.mode) {
            state.mode = Some(config.mode);
            state.next = game.ticks;
            state.stage = 0;
            state.clipping = false;
            state.settled = false;
        }
    });
    match config.mode {
        m::PhaseMode::Vanilla => vanilla(game, &config, &ctx),
        m::PhaseMode::Skip => skip(game, &config, &ctx),
        m::PhaseMode::Mineplex => mineplex(game, &ctx),
        m::PhaseMode::Clip => clip(game, &config, &ctx),
        m::PhaseMode::Center => center(game, &ctx),
        m::PhaseMode::Nudge => nudge(game, &config, &ctx),
        m::PhaseMode::Break => break_block(game, &config, &ctx),
        m::PhaseMode::Sink => sink(game, &ctx),
        m::PhaseMode::Pearl => pearl(game, &config, &ctx),
    }
}

/// Alan's and FDPClient's Vanilla: the 0.5 burst, then a 0.04 nudge so the
/// client's own copy of the player follows the server's.
fn vanilla(game: &mut Game<'_>, config: &m::PhaseConfig, ctx: &Ctx) {
    if !ctx.on_ground || !ctx.collided || !due(game.ticks) {
        return;
    }
    for point in vanilla_burst(ctx.position) {
        send_position(game, point, true);
    }
    let [dx, dz] = forward(ctx.yaw, 0.04);
    shift(game, [dx, 0.0, dz]);
    schedule(game.ticks, config);
}

/// FDPClient's Skip and Seppuku's SKIP: three rising steps reaching 0.3 and
/// then 0.6 along the way, then the client's own copy of the player takes the
/// first step so it does not snap back.
fn skip(game: &mut Game<'_>, config: &m::PhaseConfig, ctx: &Ctx) {
    if !ctx.on_ground || !ctx.collided || !due(game.ticks) {
        return;
    }
    for point in skip_burst(ctx.position, ctx.yaw) {
        send_position(game, point, true);
    }
    let [dx, dz] = forward(ctx.yaw, 0.3);
    shift(game, [dx, 0.0, dz]);
    schedule(game.ticks, config);
}

/// FDPClient's Mineplex: a hit latches the mode on, then three ticks carry the
/// player 0.06 and then 1.6 into the wall while their own movement is zeroed.
fn mineplex(game: &mut Game<'_>, ctx: &Ctx) {
    let stage = STATE.with(|state| {
        let mut state = state.borrow_mut();
        if ctx.collided {
            state.clipping = true;
        }
        if !state.clipping {
            return 0;
        }
        state.stage += 1;
        state.stage
    });
    if stage == 0 {
        return;
    }
    // The reference zeroes the tick's own movement, so the only offset the
    // server sees is the one the burst sends. Its event runs before the
    // physics; this seam is after, so the walk step is taken back out
    // instead: the packet, and the next tick's physics, both start from where
    // the tick began. `previous_position` is the tick's own start, saved by
    // the player tick before the physics ran.
    let from = game.previous_position;
    if let Some(player) = game.player.as_mut() {
        player.velocity.x = 0.0;
        player.velocity.z = 0.0;
        if same_tick(from, player.position) {
            player.position.x = from.x;
            player.position.z = from.z;
        }
    }
    if stage >= 3 {
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.stage = 0;
            state.clipping = false;
        });
        return;
    }
    let offset = if stage >= 2 { 1.6 } else { 0.06 };
    let [dx, dz] = forward(ctx.yaw, offset);
    shift(game, [dx, 0.0, dz]);
    if stage >= 2 {
        // 1.6 blocks in one tick: the camera would otherwise be drawn inside
        // the wall for the frames the slide takes.
        snap(game);
    }
}

/// FDPClient's Clip: scan forward for a column with room for the body and
/// stand in it. Shoreline's AutoClip, Zio's Clip and Alan's Watchdog full-block
/// port all resolve to this search.
fn clip(game: &mut Game<'_>, config: &m::PhaseConfig, ctx: &Ctx) {
    if !ctx.collided || !due(game.ticks) {
        return;
    }
    let [px, py, pz] = ctx.position;
    let (sin, cos) = f64::from(ctx.yaw.to_radians()).sin_cos();
    let feet = py.floor() as i32;
    let mut target = None;
    for step in 1..=config.scan {
        let reach = f64::from(step);
        let x = px - sin * reach;
        let z = pz + cos * reach;
        let (bx, bz) = (x.floor() as i32, z.floor() as i32);
        if open(game, bx, feet, bz) && open(game, bx, feet + 1, bz) {
            target = Some([x, py, z]);
            break;
        }
    }
    let Some(target) = target else {
        return;
    };
    place(game, target);
    // Two or more blocks in one tick: without the snap the camera is drawn
    // travelling through the wall it is being skipped past.
    snap(game);
    schedule(game.ticks, config);
}

/// LiquidBounce's Hypixel 2025-02-04: one position packet to the centre of the
/// block below, a hair under the feet, which the server's own gravity check
/// then accepts as a fall. The reference disables itself afterwards; here the
/// mode settles once per activation.
fn center(game: &mut Game<'_>, ctx: &Ctx) {
    if STATE.with(|state| state.borrow().settled) {
        return;
    }
    let target = [
        ctx.position[0].floor() + 0.5,
        ctx.position[1] - GRAVITY,
        ctx.position[2].floor() + 0.5,
    ];
    send_position_rotation(game, target, game.yaw, game.pitch, false);
    place(game, target);
    // The descent is a fifth of a block, but the horizontal half block is a
    // jump: the camera should land on the centre rather than drift onto it.
    snap(game);
    STATE.with(|state| state.borrow_mut().settled = true);
}

/// Alan's Normal, Vulcan and WatchdogSemiBlock: push into the block every tick
/// it is hit. The references reach 0.005 a tick because their block-shape hook
/// also stops the client pushing back; without that hook the push has to beat
/// vanilla's 0.1 `moveTowardsClosestSpace` nudge, which is why the setting
/// starts above it.
fn nudge(game: &mut Game<'_>, config: &m::PhaseConfig, ctx: &Ctx) {
    if !ctx.collided {
        return;
    }
    let [dx, dz] = forward(ctx.yaw, config.push);
    shift(game, [dx, 0.0, dz]);
}

/// Alan's BreakBlock and ThunderHack's Sunrise: break the block in the way and
/// stand in its place. Sneak aims at the floor, jump at the block overhead.
fn break_block(game: &mut Game<'_>, config: &m::PhaseConfig, ctx: &Ctx) {
    if !ctx.collided || !due(game.ticks) {
        return;
    }
    let [px, py, pz] = ctx.position;
    let [dx, dz] = forward(ctx.yaw, 0.75);
    let feet = py.floor() as i32;
    let y = if ctx.sneak {
        feet - 1
    } else if ctx.jump {
        feet + 2
    } else {
        feet
    };
    let target = [(px + dx).floor() as i32, y, (pz + dz).floor() as i32];
    if open(game, target[0], target[1], target[2]) {
        return;
    }
    // START then STOP in the same tick: the server breaks the block without
    // waiting for the client's progress.
    for action in [START_DESTROY_BLOCK, STOP_DESTROY_BLOCK] {
        game.send(S::PlayerAction(play::PlayerAction {
            action,
            pos: target,
            direction: UP,
            sequence: 0,
        }));
    }
    place(
        game,
        [f64::from(target[0]) + 0.5, py, f64::from(target[2]) + 0.5],
    );
    // Up to a block and a half into the block that was just broken.
    snap(game);
    schedule(game.ticks, config);
}

/// FDPClient's Intave and Celestial's Lorent Down: descend through the floor.
/// The STOP on the block under the feet is what the server reads as the player
/// finishing a break there; the descent carries them below it.
fn sink(game: &mut Game<'_>, ctx: &Ctx) {
    if !ctx.sneak {
        return;
    }
    let [px, py, pz] = ctx.position;
    game.send(S::PlayerAction(play::PlayerAction {
        action: STOP_DESTROY_BLOCK,
        pos: [
            px.floor() as i32,
            (py - 0.1).floor() as i32,
            pz.floor() as i32,
        ],
        direction: UP,
        sequence: 0,
    }));
    shift(game, [0.0, -SINK_RATE, 0.0]);
}

/// Shoreline's, ThunderHack's and Zio's Pearl: throw an ender pearl into the
/// block in the way and let the server's own pearl teleport carry the player
/// through it. The held slot has to reach the server before the use does, or it
/// throws whatever the player was holding.
fn pearl(game: &mut Game<'_>, config: &m::PhaseConfig, ctx: &Ctx) {
    if !ctx.collided || !ctx.on_ground || !due(game.ticks) {
        return;
    }
    let Some(slot) = pearl_slot(game) else {
        return;
    };
    let [px, _, pz] = ctx.position;
    let [dx, dz] = forward(ctx.yaw, 1.0);
    let to_x = (px + dx).floor() + 0.5 - px;
    let to_z = (pz + dz).floor() + 0.5 - pz;
    let yaw = (-to_x).atan2(to_z).to_degrees() as f32;
    let previous = game.selected;
    let Some(remote) = &game.remote else {
        return;
    };
    // `carried` is told too: the tick's own carried-slot check compares it with
    // `selected`, and a stale value would send the swap a second time.
    remote.session.send(S::SetCarriedItem(play::SetCarriedItem {
        slot: slot as i16,
    }));
    remote.carried.set(Some(slot));
    remote.session.send(S::UseItem(play::UseItem {
        hand: 0,
        sequence: 0,
        yaw,
        pitch: config.pitch,
    }));
    if previous != slot {
        remote.session.send(S::SetCarriedItem(play::SetCarriedItem {
            slot: previous as i16,
        }));
        remote.carried.set(Some(previous));
    }
    // The pearl's cooldown, not the module's: a second use inside it is
    // dropped by the server and the pearl is spent for nothing.
    let wait = u64::from(config.delay).max(PEARL_COOLDOWN);
    let now = game.ticks;
    STATE.with(|state| {
        state.borrow_mut().next = now.saturating_add(wait);
    });
}

/// `remote_teleport`'s head: the server is moving the player, so an attempt in
/// flight and Mineplex's burst belong to the position that was left. Center's
/// one shot stays spent — a server that refused it would refuse it again.
pub(super) fn teleport(_game: &mut Game<'_>) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.next = 0;
        state.stage = 0;
        state.clipping = false;
    });
}

#[cfg(test)]
mod tests {
    use super::{forward, same_tick, skip_burst, vanilla_burst};
    use horizon_core::geometry::Vec3;

    fn at(x: f64, z: f64) -> Vec3 {
        Vec3::new(x, 64.0, z)
    }

    #[test]
    fn a_walk_step_is_the_same_tick() {
        // Standing still, a walk, and a sprint jump all stay inside the bound.
        for (dx, dz) in [(0.0, 0.0), (0.21, 0.0), (0.4, 0.4), (0.55, 0.25)] {
            assert!(same_tick(at(10.0, 10.0), at(10.0 + dx, 10.0 + dz)), "{dx},{dz}");
        }
    }

    #[test]
    fn a_teleport_is_not_the_same_tick() {
        // A gap `previous_position` cannot explain: rewinding would drag the
        // player back to a spot they are not standing in.
        assert!(!same_tick(at(10.0, 10.0), at(11.0, 10.0)));
        assert!(!same_tick(at(0.0, 0.0), at(100.0, -240.0)));
        // Vertical movement is not what this asks about: a fall of any height
        // is still the same tick, and only the horizontal axis is rewound.
        assert!(same_tick(Vec3::new(10.0, 300.0, 10.0), at(10.0, 10.0)));
    }

    #[test]
    fn forward_follows_the_reference_convention() {
        let [x, z] = forward(0.0, 1.0);
        assert!(x.abs() < 1.0e-6, "facing +z has no x offset: {x}");
        assert!((z - 1.0).abs() < 1.0e-6, "facing +z moves +z: {z}");
        let [x, z] = forward(90.0, 2.0);
        assert!((x + 2.0).abs() < 1.0e-6, "facing +x moves -x: {x}");
        assert!(z.abs() < 1.0e-6, "facing +x has no z offset: {z}");
    }

    #[test]
    fn the_vanilla_burst_ends_half_a_block_over() {
        let burst = vanilla_burst([10.0, 64.0, -3.0]);
        assert_eq!(burst[0], [10.0, 64.0, -3.0], "it starts at the feet");
        assert_eq!(burst[5], [10.5, 64.0, -2.5], "it ends half a block over");
        assert_eq!(burst[3], [10.0, 64.2, -3.0], "the fourth packet rises");
        assert_eq!(burst[1], [0.5, 0.0, 0.5], "the second visits the corner");
    }

    #[test]
    fn the_skip_burst_rises_before_every_step() {
        let burst = skip_burst([0.0, 64.0, 0.0], 0.0);
        for step in 0..3_usize {
            assert_eq!(burst[step * 2][1], 64.06, "step {step} lifts the head");
            assert_eq!(
                burst[step * 2 + 1][1],
                64.0,
                "step {step} lands at the feet"
            );
        }
        assert_eq!(burst[1], [0.0, 64.0, 0.0], "the first step stays put");
        assert!(
            (burst[3][2] - 0.3).abs() < 1.0e-6,
            "the second step reaches 0.3: {}",
            burst[3][2]
        );
        assert!(
            (burst[5][2] - 0.6).abs() < 1.0e-6,
            "the third step reaches 0.6: {}",
            burst[5][2]
        );
    }
}
