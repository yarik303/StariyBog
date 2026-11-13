// AutoXP: throws experience bottles at your feet so Mending repairs your gear.
//
// The reference is Zio's AutoExpModule. Its mechanism is a hotbar slot holding
// bottles, a `UseItem` for each throw and a `SetCarriedItem` either side of
// them when the mode is `Silent`. Horizon can do the same three things:
//
//  * the bottle's `UseItem` packet goes out with the chosen rotation;
//  * `SetCarriedItem` selects the bottle slot for the server and, in `Silent`,
//    puts the old slot back before the client ever sees a change.
//
// The reference also sends a swing packet. 26.3 has no packet-only swing:
// `Punch` took over the main-hand swing, and Horizon sends it only from the
// left-click attack and mining paths. Its `handlePunch` resets the server's
// attack-strength ticker, so sending one after a bottle would tell the server
// you punched and cost you the charge on your next hit. No swing is sent.
//
// The packets go straight into the session, not through `Game::send`. That
// method calls `ensureHasSentCarriedItem` before any `UseItem`, which would
// answer with the player's *own* selected slot and undo the swap between the
// selection and the throw. `Remote::carried` is told about each change so the
// next tick's own check does not send the slot a second time.
//
// `native_tick` runs once a tick, after the tick's movement packet has gone
// out, so a throw cannot be reordered in front of it. The whole cycle is
// delayed by `Delay` ticks rather than milliseconds: Horizon's tick is the
// clock the reference's `delayTicks * 50` was measuring.
//
// What is not here: a client-side rotation request (the rotation is in the
// packet only, so your view stays where you put it), bottles in the off hand
// (only the hotbar and the main inventory are searched), the reference's
// swing packet (see above), and the reference's ElytraFly modes that allow a
// throw while gliding. Gliding pauses the module.
use core::cell::RefCell;

use horizon_client::{items::ItemStack, menu::Input};
use horizon_protocol::packets::{play, play::Serverbound as S};

use crate::game::{Game, Screen, clickgui as m};

/// The item, and the enchantment the whole module is about.
const BOTTLE: &str = "minecraft:experience_bottle";
const MENDING: &str = "minecraft:mending";
/// `ServerboundUseItemPacket`'s hand: 0 is the main hand. Only the hotbar is
/// searched, so the off hand's 1 is never needed.
const MAIN: i32 = 0;
/// Straight down: the bottle breaks at your feet and its orbs land on you.
const DOWN: f32 = 90.0;
/// `sequence` is the client's prediction id. A throw sent outside the game's
/// own use path has no prediction to acknowledge, so there is nothing to
/// number. The server still consumes the bottle and sends the slot update.
const UNPREDICTED: i32 = 0;

#[derive(Default)]
struct State {
    /// Ticks left before the next throw cycle.
    wait: u32,
    /// The (inventory, hotbar) pair the `Inventory` setting swapped, to put
    /// back when the module goes off. Only one pair is tracked, as in the
    /// reference: a second restock waits for the next activation.
    swapped: Option<(usize, usize)>,
    /// `Normal` moves the player's selection to the bottle slot. These two
    /// say where it came from and what we last put there, so turning the
    /// module off only undoes a change the player has not since overridden.
    original: Option<usize>,
    ours: Option<usize>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// One throw cycle's packets, in order. Split out from the game so the order
/// can be pinned by a test: the server has to be holding the bottle for every
/// `UseItem`, and `Silent` has to put the old slot back afterwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sent {
    Carried(usize),
    Use,
}

fn throw_packets(silent: bool, frequency: u32, slot: usize, previous: usize) -> Vec<Sent> {
    let mut out = vec![Sent::Carried(slot)];
    for _ in 0..frequency {
        out.push(Sent::Use);
    }
    if silent {
        out.push(Sent::Carried(previous));
    }
    out
}

/// The window's own game, in play, with a remote session and a player that is
/// not gliding. A cursor stack means a menu click would drop or split it.
fn ready(game: &Game<'_>) -> bool {
    game.player.is_some()
        && game.remote.is_some()
        && game.carried.is_none()
        && !game.spectator()
        && (game.native_playing() || game.screen == Screen::Inventory)
        && !game
            .player
            .as_ref()
            .is_some_and(|player| player.fall_flying)
}

/// Whether a menu click is safe: the player's own inventory is window 0 on a
/// server, and only that menu has the slots this module clicks.
fn menu_ready(game: &Game<'_>) -> bool {
    game.remote.is_some()
        && game.carried.is_none()
        && (game.native_playing() || game.screen == Screen::Inventory)
}

pub(crate) fn tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let config = if game.native_on(m::ID) {
        m::autoxp()
    } else {
        None
    };
    if let Some(config) = config
        && ready(game)
    {
        STATE.with(|state| run(game, &config, &mut state.borrow_mut()));
    } else {
        STATE.with(|state| off(game, &mut state.borrow_mut()));
    }
}

/// Off, or not in a world to act in: undo the two things the module changed
/// about the player's inventory. A deferred menu swap is kept until a tick
/// where clicking is safe, so a death screen cannot silently drop it.
fn off(game: &mut Game<'_>, state: &mut State) {
    state.wait = 0;
    if let Some(ours) = state.ours.take()
        && game.selected == ours
        && let Some(original) = state.original.take()
    {
        game.selected = original;
    }
    state.original = None;
    if let Some((inv, hotbar)) = state.swapped
        && menu_ready(game)
        && click(game, &[(Some(inv), hotbar as i32, Input::Swap)])
    {
        state.swapped = None;
    }
}

fn run(game: &mut Game<'_>, config: &m::AutoXpConfig, state: &mut State) {
    if state.wait > 0 {
        state.wait -= 1;
        return;
    }
    // Nothing to repair: stop instead of spending bottles. With `Stop` off
    // the module throws until the bottles run out.
    if config.stop && !anything_to_mend(game, config) {
        if config.auto_disable {
            m::with(|client| {
                if let Some(module) = client
                    .modules
                    .iter_mut()
                    .find(|module| module.id == m::AUTOXP)
                {
                    module.enabled = false;
                }
            });
        }
        return;
    }
    if !air_ok(game, config) {
        return;
    }
    let Some(slot) = bottle_slot(game, config, state) else {
        return;
    };
    throw(game, config, state, slot);
    state.wait = config.delay;
}

/// Whether anything the XP could repair is still damaged. Vanilla's
/// `ExperienceOrb.repairPlayerItems` considers the main hand, the off hand
/// and then the four armour slots; `Tools` is what brings the hands in, as in
/// the reference.
fn anything_to_mend(game: &Game<'_>, config: &m::AutoXpConfig) -> bool {
    if game
        .armor
        .iter()
        .any(|slot| needs_mending(game, slot.as_ref(), config.stop_at))
    {
        return true;
    }
    config.tools
        && (needs_mending(game, game.held(), config.stop_at)
            || needs_mending(game, game.offhand.as_ref(), config.stop_at))
}

fn needs_mending(game: &Game<'_>, stack: Option<&ItemStack>, stop_at: i32) -> bool {
    let Some(stack) = stack else {
        return false;
    };
    let Some(info) = game.res.items().get(&stack.item) else {
        return false;
    };
    needs_repair(
        info.max_damage,
        stack.damage,
        stack.enchantment_level(MENDING),
        stop_at,
    )
}

/// The reference's `needsMending`: a Mending item that wears out, below the
/// `StopAt` percentage. Pure so the gate can be tested without a game.
fn needs_repair(max_damage: u32, damage: u32, mending: u32, stop_at: i32) -> bool {
    mending > 0 && max_damage > 0 && durability_percent(max_damage, damage) < stop_at
}

/// Remaining durability as a percentage, rounding down. A damage value past
/// the maximum cannot go negative.
fn durability_percent(max_damage: u32, damage: u32) -> i32 {
    if max_damage == 0 {
        return 100;
    }
    ((u64::from(max_damage.saturating_sub(damage)) * 100) / u64::from(max_damage)) as i32
}

/// `Air` off means the reference's guard: only throw while the crosshair is
/// on a solid block, so the bottle does not sail off into the sky. With `Air`
/// on (the default) no such restriction applies.
fn air_ok(game: &Game<'_>, config: &m::AutoXpConfig) -> bool {
    if config.air {
        return true;
    }
    let Some((_, hit)) = game.native_view_hit() else {
        return false;
    };
    let state = game.world.block(hit.block_x, hit.block_y, hit.block_z);
    let facts = game.res.facts();
    !facts.is_air(state) && !facts.get(state).replaceable
}

fn is_bottle(stack: Option<&ItemStack>) -> bool {
    stack.is_some_and(|s| s.count > 0 && &*s.item == BOTTLE)
}

fn hotbar_bottle(game: &Game<'_>) -> Option<usize> {
    (0..9).find(|&slot| is_bottle(game.inventory.get(slot).and_then(|s| s.as_ref())))
}

fn inventory_bottle(game: &Game<'_>) -> Option<usize> {
    (9..game.inventory.len().min(36))
        .find(|&slot| is_bottle(game.inventory.get(slot).and_then(|s| s.as_ref())))
}

/// Where to throw from. A hotbar stack needs no menu click; otherwise one
/// number-key swap brings a stack up from the main inventory, remembered so
/// `off` can put the two slots back.
fn bottle_slot(
    game: &mut Game<'_>,
    config: &m::AutoXpConfig,
    state: &mut State,
) -> Option<usize> {
    if let Some(slot) = hotbar_bottle(game) {
        return Some(slot);
    }
    if !config.inventory || state.swapped.is_some() {
        return None;
    }
    let inv = inventory_bottle(game)?;
    let hotbar = game.selected;
    if !click(game, &[(Some(inv), hotbar as i32, Input::Swap)]) {
        return None;
    }
    state.swapped = Some((inv, hotbar));
    Some(hotbar)
}

fn click(game: &mut Game<'_>, ops: &[(Option<usize>, i32, Input)]) -> bool {
    if game.remote.is_some() {
        game.remote_menu_ops(ops)
    } else {
        game.menu_ops(ops)
    }
}

fn throw(game: &mut Game<'_>, config: &m::AutoXpConfig, state: &mut State, slot: usize) {
    let previous = game.selected;
    if !send_throw(game, config, slot, previous) {
        return;
    }
    if config.mode.silent() {
        return;
    }
    // `Normal` leaves the bottle in hand. Remember where the selection came
    // from the first time, so `off` can put it back — but only if the player
    // has not since chosen another slot themselves.
    if state.original.is_none() {
        state.original = Some(previous);
    }
    state.ours = Some(slot);
    game.selected = slot;
}

fn send_throw(game: &Game<'_>, config: &m::AutoXpConfig, slot: usize, previous: usize) -> bool {
    let Some(remote) = &game.remote else {
        return false;
    };
    // `Rotate` aims the packet straight down. Without it the bottle goes
    // where the player is already looking.
    let (yaw, pitch) = if config.rotate {
        (game.yaw, DOWN)
    } else {
        (game.yaw, game.pitch)
    };
    let silent = config.mode.silent();
    for sent in throw_packets(silent, config.frequency, slot, previous) {
        let packet = match sent {
            Sent::Carried(s) => {
                remote.carried.set(Some(s));
                S::SetCarriedItem(play::SetCarriedItem { slot: s as i16 })
            }
            Sent::Use => S::UseItem(play::UseItem {
                hand: MAIN,
                sequence: UNPREDICTED,
                yaw,
                pitch,
            }),
        };
        remote.session.send(packet);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{
        BOTTLE, Sent, durability_percent, is_bottle, needs_repair, throw_packets,
    };
    use horizon_client::items::ItemStack;

    #[test]
    fn a_silent_throw_puts_the_slot_back_last() {
        let packets = throw_packets(true, 2, 4, 1);
        assert_eq!(
            packets,
            vec![Sent::Carried(4), Sent::Use, Sent::Use, Sent::Carried(1)]
        );
    }

    #[test]
    fn a_normal_throw_leaves_the_bottle_in_hand() {
        let packets = throw_packets(false, 1, 4, 1);
        assert_eq!(packets, vec![Sent::Carried(4), Sent::Use]);
        assert!(!packets.contains(&Sent::Carried(1)));
    }

    #[test]
    fn a_throw_sends_no_attack_side_packet() {
        // 26.3's `Punch` is the left-click packet: it resets attack strength
        // on the server. A bottle throw must never look like a melee swing.
        for packets in [throw_packets(true, 4, 3, 0), throw_packets(false, 4, 3, 0)] {
            assert!(packets.iter().all(|p| matches!(p, Sent::Use | Sent::Carried(_))));
        }
    }

    #[test]
    fn the_server_holds_the_bottle_for_every_use() {
        let packets = throw_packets(true, 16, 3, 0);
        let restored = packets.iter().rposition(|p| *p == Sent::Carried(0)).unwrap();
        let uses: Vec<usize> = packets
            .iter()
            .enumerate()
            .filter(|(_, p)| matches!(**p, Sent::Use))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(uses.len(), 16);
        // Every throw sits between the selection and the restore, so the
        // server is holding the bottle for all sixteen.
        assert!(uses.iter().all(|&i| i > 0 && i < restored));    }

    #[test]
    fn only_a_damaged_mending_item_wants_repair() {
        assert!(needs_repair(100, 40, 1, 100));
        assert!(needs_repair(100, 1, 1, 100));
        assert!(!needs_repair(100, 0, 1, 100), "an undamaged piece is done");
        assert!(!needs_repair(100, 40, 0, 100), "no Mending");
        assert!(!needs_repair(0, 0, 1, 100), "nothing that wears out");
        assert!(!needs_repair(100, 40, 1, 50), "already past StopAt");
    }

    #[test]
    fn durability_rounds_down_and_never_goes_negative() {
        assert_eq!(durability_percent(0, 0), 100);
        assert_eq!(durability_percent(100, 0), 100);
        assert_eq!(durability_percent(100, 40), 60);
        assert_eq!(durability_percent(3, 1), 66);
        assert_eq!(durability_percent(100, 140), 0);
    }

    #[test]
    fn only_a_nonempty_bottle_stack_counts() {
        let mut bottle = ItemStack::new(BOTTLE, 1);
        assert!(is_bottle(Some(&bottle)));
        bottle.count = 0;
        assert!(!is_bottle(Some(&bottle)));
        assert!(!is_bottle(Some(&ItemStack::new("minecraft:apple", 1))));
        assert!(!is_bottle(None));
    }
}
