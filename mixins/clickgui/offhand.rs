// OffHand: keeps one item in the off hand — a totem, a golden apple or an end
// crystal — and swaps it in when the fight needs it.
//
// The references are LiquidBounce's ModuleOffhand and its Totem group,
// Meteor's Offhand, Zio's OffhandModule, and ThunderHack's AutoTotem and
// AutoGApple. What they share is one mechanism: a wanted item, a health (or
// fall, or blast) gate, and a menu click that moves the stack into the off
// hand. That is what this module is; the reference-specific extras are left
// out where Horizon cannot back them (see the README section).
//
// The swap is a menu click, not a hotbar switch. Two shapes cover every case:
//
//  * the stack is in a hotbar slot: one `Swap` click on the off hand slot with
//    that hotbar index, which is vanilla's number key over the slot;
//  * the stack is in the main inventory: pick it up, pick the off hand slot
//    up (the cursor stack goes in, the old off hand stack comes out), then put
//    the old stack back into the slot the new one came from.
//
// Every click goes through `remote_menu_ops` (a server's window) or
// `menu_ops` (the local menu engine), so the server receives the same
// `ContainerClick` a player's click would send. The player's own inventory
// menu is always window 0 on the server, so this works without opening a
// screen — and it is why the module stops while a screen is open: another
// menu's slot numbering has no off hand slot in it.
//
// The one reference behaviour that needs the game's input rather than a menu
// is eating. A gapple in the off hand is used by pressing the use key, so
// `Eat gapple` sets `Game::right` for as long as the health is below `Eat at`
// and clears it when it is not — the same thing AutoGApple does with
// `mc.options.useKey`. That runs the game's own use path, so a main hand that
// can act on what you are looking at still acts first, exactly as a manual
// right click would.
use crate::game::{Game, Screen, clickgui as m};
use horizon_client::items::ItemStack;
use horizon_client::menu::Input;
use horizon_core::geometry::Vec3;
use std::cell::RefCell;

/// The off hand's id in the game's slot numbering (`slots`, `slot_view`):
/// `InventoryMenu.SHIELD_SLOT` is menu slot 45, and 504 is the same slot in
/// the game's own numbering.
const OFFHAND: usize = 504;

/// How close an end crystal or a primed TNT has to be to count as danger.
/// This is proximity, not a damage prediction: a vanilla explosion's damage
/// depends on the blocks between it and the body, which is more than a tick
/// can answer for every crystal in the world.
const BLAST_REACH: f64 = 8.0;

/// The item the module keeps in the off hand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Item {
    Totem,
    GApple,
    Crystal,
}

#[derive(Default)]
struct State {
    /// Ticks left before the next click.
    wait: u32,
    /// The module set `right` itself, so releasing it is the module's job.
    forced_right: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub(super) fn tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let config = if game.native_on(m::ID) {
        m::offhand()
    } else {
        None
    };
    let Some(config) = config else {
        STATE.with(|state| release(game, &mut *state.borrow_mut()));
        return;
    };
    if game.player.is_none() || !game.native_playing() || game.spectator() {
        STATE.with(|state| release(game, &mut *state.borrow_mut()));
        return;
    }
    STATE.with(|state| run(game, &config, &mut *state.borrow_mut()));
}

fn run(game: &mut Game<'_>, config: &m::OffHandConfig, state: &mut State) {
    let facts = Facts::of(game, config);
    // A gapple in the off hand is eaten while the health is below `Eat at`,
    // whether the module put it there or the player did.
    let eat = config.eat && is_gapple(game.offhand.as_ref()) && facts.health <= config.eat_at;
    set_right(game, state, eat);
    let Some(item) = choose(
        config.item,
        config.sword_only,
        holding_sword(game),
        in_danger(config, facts),
    ) else {
        return;
    };
    if holds(game, item) {
        return;
    }
    if state.wait > 0 {
        state.wait -= 1;
        return;
    }
    // A screen with another menu in it numbers its slots differently and has
    // no off hand slot at all, so the swap waits until you are back in play.
    if game.screen != Screen::Playing {
        return;
    }
    let Some(slot) = find_slot(game, item, config.hotbar) else {
        return;
    };
    if swap_to(game, slot) {
        state.wait = config.delay;
    }
}

/// The player facts the gates read, pulled out of `Game` so the decision can
/// be tested without one.
#[derive(Clone, Copy, Debug)]
struct Facts {
    /// Health plus absorption, or health alone.
    health: f32,
    /// Blocks fallen since the last landing.
    fall: f64,
    /// Gliding (elytra), which is not a fall.
    gliding: bool,
    /// Creative flight, which is not a fall either.
    flying: bool,
    /// An end crystal or primed TNT is close. Only filled when the
    /// `Explosion` setting is on, because the scan is the expensive part.
    blast: bool,
}

impl Facts {
    fn of(game: &Game<'_>, config: &m::OffHandConfig) -> Self {
        let Some(player) = game.player.as_ref() else {
            return Self {
                health: 0.0,
                fall: 0.0,
                gliding: false,
                flying: false,
                blast: false,
            };
        };
        let absorption = if config.absorption {
            player.health.absorption()
        } else {
            0.0
        };
        Self {
            health: player.health.health() + absorption,
            fall: player.fall_distance,
            gliding: player.fall_flying,
            flying: player.flying,
            blast: config.explosion && blast_near(game, player.position),
        }
    }
}

/// The choice itself: the totem when the fight calls for one, else the item
/// the module wants — and nothing at all when a gapple is only wanted while a
/// sword is held and none is.
fn choose(item: m::OffHandItem, sword_only: bool, sword: bool, danger: bool) -> Option<Item> {
    if item != m::OffHandItem::Totem && danger {
        return Some(Item::Totem);
    }
    match item {
        m::OffHandItem::Totem => Some(Item::Totem),
        m::OffHandItem::GApple if sword_only && !sword => None,
        m::OffHandItem::GApple => Some(Item::GApple),
        m::OffHandItem::Crystal => Some(Item::Crystal),
    }
}

/// Whether the fight calls for a totem instead of the preferred item.
fn in_danger(config: &m::OffHandConfig, facts: Facts) -> bool {
    if facts.health <= config.health {
        return true;
    }
    if config.elytra && facts.gliding {
        return true;
    }
    // A glide is not a fall: the distance is the dive's, not a drop's.
    if config.fall && !facts.gliding && !facts.flying && fall_damage(facts.fall) >= facts.health {
        return true;
    }
    config.explosion && facts.blast
}

/// `LivingEntity.calculateFallDamage`: nothing under three blocks, then a
/// point a block, rounded up.
fn fall_damage(distance: f64) -> f32 {
    if distance <= 3.0 {
        0.0
    } else {
        (distance - 3.0).ceil() as f32
    }
}

/// Whether an end crystal or a primed TNT is close enough to matter.
fn blast_near(game: &Game<'_>, at: Vec3) -> bool {
    let reach = BLAST_REACH * BLAST_REACH;
    let near = |x: f64, y: f64, z: f64| {
        let (dx, dy, dz) = (x - at.x, y - at.y, z - at.z);
        dx * dx + dy * dy + dz * dz <= reach
    };
    game.crystals
        .iter()
        .any(|crystal| near(crystal.position.x, crystal.position.y, crystal.position.z))
        || game
            .tnts
            .iter()
            .any(|tnt| near(tnt.position.x, tnt.position.y, tnt.position.z))
}

fn holding_sword(game: &Game<'_>) -> bool {
    game.held().is_some_and(|stack| stack.item.ends_with("_sword"))
}

fn is_gapple(stack: Option<&ItemStack>) -> bool {
    stack.is_some_and(|stack| is_item(stack, Item::GApple))
}

fn holds(game: &Game<'_>, item: Item) -> bool {
    game.offhand
        .as_ref()
        .is_some_and(|stack| is_item(stack, item))
}

/// Whether `stack` is the item this module looks for. The totem is matched by
/// its id: the component it is recognised by (`minecraft:death_protection`)
/// lives in the game's item data, which the module cannot read from here.
fn is_item(stack: &ItemStack, item: Item) -> bool {
    match item {
        Item::Totem => &*stack.item == "minecraft:totem_of_undying",
        Item::GApple => {
            &*stack.item == "minecraft:golden_apple"
                || &*stack.item == "minecraft:enchanted_golden_apple"
        }
        Item::Crystal => &*stack.item == "minecraft:end_crystal",
    }
}

/// The first slot holding `item`: the hotbar first when `Hotbar` is on (a
/// hotbar stack is one click), else the main inventory.
fn find_slot(game: &Game<'_>, item: Item, hotbar: bool) -> Option<usize> {
    let slots = game.inventory.len().min(36);
    let first = if hotbar { 0 } else { 9 };
    (first..slots).find(|&slot| {
        game.inventory
            .get(slot)
            .and_then(|held| held.as_ref())
            .is_some_and(|stack| is_item(stack, item))
    })
}

/// The clicks that move `slot`'s stack into the off hand, split out from the
/// game so the order can be pinned by a test. The engine cannot be driven from
/// here — `may_place` on the off hand slot needs the item data, which only a
/// prepared resource tree has — so this is the part that is checkable without
/// one.
fn ops_for(slot: usize, full: bool) -> Vec<(Option<usize>, i32, Input)> {
    // The number key over the off hand slot swaps the two slots.
    if slot < 9 {
        return vec![(Some(OFFHAND), slot as i32, Input::Swap)];
    }
    // Pick the new stack up, then click the off hand slot: the cursor stack
    // goes in and the old one comes out. With nothing to take out the second
    // click only puts the stack in, so the source slot is not clicked again.
    let mut ops = vec![
        (Some(slot), 0, Input::Pickup),
        (Some(OFFHAND), 0, Input::Pickup),
    ];
    if full {
        ops.push((Some(slot), 0, Input::Pickup));
    }
    ops
}

/// Moves `slot`'s stack into the off hand.
fn swap_to(game: &mut Game<'_>, slot: usize) -> bool {
    // The cursor holds a stack: clicking now would drop or split it.
    if game.carried.is_some() {
        return false;
    }
    click(game, &ops_for(slot, game.offhand.is_some()))
}

fn click(game: &mut Game<'_>, ops: &[(Option<usize>, i32, Input)]) -> bool {
    if game.remote.is_some() {
        game.remote_menu_ops(ops)
    } else {
        game.menu_ops(ops)
    }
}

/// Holds or releases the use key, without touching a click the player made.
/// Releasing leaves the input alone while the use key is down — held, or
/// latched by the toggle-use option — because that click is the player's.
fn set_right(game: &mut Game<'_>, state: &mut State, want: bool) {
    if want {
        if !game.right {
            game.right = true;
            state.forced_right = true;
        }
    } else if state.forced_right {
        if !game.toggles.is_down(3) {
            game.right = false;
        }
        state.forced_right = false;
    }
}

fn release(game: &mut Game<'_>, state: &mut State) {
    if state.forced_right && !game.toggles.is_down(3) {
        game.right = false;
    }
    *state = State::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item: &str) -> ItemStack {
        ItemStack::new(item, 1)
    }

    #[test]
    fn a_fall_under_three_blocks_hurts_nothing() {
        assert_eq!(fall_damage(3.0), 0.0);
        assert_eq!(fall_damage(2.4), 0.0);
    }

    #[test]
    fn a_fall_past_three_blocks_hurts_a_point_a_block() {
        assert_eq!(fall_damage(4.0), 1.0);
        assert_eq!(fall_damage(9.0), 6.0);
    }

    #[test]
    fn a_fall_between_blocks_rounds_up() {
        assert_eq!(fall_damage(4.2), 2.0);
        assert_eq!(fall_damage(10.1), 8.0);
    }

    #[test]
    fn the_totem_is_recognised_by_its_item() {
        assert!(is_item(&stack("minecraft:totem_of_undying"), Item::Totem));
        assert!(!is_item(&stack("minecraft:golden_apple"), Item::Totem));
    }

    #[test]
    fn both_golden_apples_count() {
        assert!(is_item(&stack("minecraft:golden_apple"), Item::GApple));
        assert!(is_item(
            &stack("minecraft:enchanted_golden_apple"),
            Item::GApple
        ));
        assert!(!is_item(&stack("minecraft:golden_carrot"), Item::GApple));
    }

    #[test]
    fn a_crystal_is_only_the_end_crystal() {
        assert!(is_item(&stack("minecraft:end_crystal"), Item::Crystal));
        assert!(!is_item(&stack("minecraft:diamond"), Item::Crystal));
    }

    fn facts(health: f32) -> Facts {
        Facts {
            health,
            fall: 0.0,
            gliding: false,
            flying: false,
            blast: false,
        }
    }

    #[test]
    fn a_totem_is_wanted_whatever_the_health() {
        // The totem mode has no preferred item to fall back from, so the gates
        // do not apply to it.
        assert_eq!(
            choose(m::OffHandItem::Totem, false, false, false),
            Some(Item::Totem)
        );
        assert_eq!(
            choose(m::OffHandItem::Totem, false, false, true),
            Some(Item::Totem)
        );
    }

    #[test]
    fn a_gapple_gives_way_to_a_totem_when_the_fight_needs_one() {
        assert_eq!(
            choose(m::OffHandItem::GApple, false, false, true),
            Some(Item::Totem)
        );
        assert_eq!(
            choose(m::OffHandItem::GApple, false, false, false),
            Some(Item::GApple)
        );
        assert_eq!(
            choose(m::OffHandItem::Crystal, false, false, true),
            Some(Item::Totem)
        );
        assert_eq!(
            choose(m::OffHandItem::Crystal, false, false, false),
            Some(Item::Crystal)
        );
    }

    #[test]
    fn sword_only_leaves_the_off_hand_alone_without_a_sword() {
        assert_eq!(choose(m::OffHandItem::GApple, true, false, false), None);
        assert_eq!(
            choose(m::OffHandItem::GApple, true, true, false),
            Some(Item::GApple)
        );
        // A sword in hand does not make a gapple win over a needed totem.
        assert_eq!(
            choose(m::OffHandItem::GApple, true, true, true),
            Some(Item::Totem)
        );
    }

    #[test]
    fn the_health_gate_is_inclusive() {
        let config = m::OffHandConfig::default();
        assert!(in_danger(&config, facts(10.0)));
        assert!(!in_danger(&config, facts(10.5)));
        assert!(!in_danger(&config, facts(20.0)));
    }

    #[test]
    fn a_lethal_fall_wants_a_totem() {
        // The health gate is lowered out of the way, so only the fall decides.
        let mut config = m::OffHandConfig::default();
        config.elytra = false;
        config.health = 0.0;
        let mut falling = facts(6.0);
        // Nine blocks is six damage: exactly the health.
        falling.fall = 9.0;
        assert!(in_danger(&config, falling));
        // Eight blocks is five: survivable, so the gapple stays.
        falling.fall = 8.0;
        assert!(!in_danger(&config, falling));
    }

    #[test]
    fn a_glide_is_not_a_fall() {
        let mut config = m::OffHandConfig::default();
        config.elytra = false;
        let mut gliding = facts(20.0);
        gliding.fall = 60.0;
        gliding.gliding = true;
        assert!(!in_danger(&config, gliding));
        // With Elytra on, the glide itself is the reason for the totem.
        config.elytra = true;
        assert!(in_danger(&config, gliding));
        // Creative flight is not a fall either.
        let mut flying = facts(20.0);
        flying.fall = 60.0;
        flying.flying = true;
        assert!(!in_danger(&config, flying));
    }

    #[test]
    fn a_nearby_blast_wants_a_totem() {
        let mut config = m::OffHandConfig::default();
        let mut close = facts(20.0);
        close.blast = true;
        assert!(in_danger(&config, close));
        // With Explosion off, the same blast is not a reason.
        config.explosion = false;
        assert!(!in_danger(&config, close));
    }

    #[test]
    fn a_hotbar_piece_is_one_swap_click() {
        // The number key over the off hand slot: the button is the hotbar
        // index, and the two slots trade places.
        assert_eq!(ops_for(3, false), vec![(Some(504), 3, Input::Swap)]);
        assert_eq!(ops_for(8, true), vec![(Some(504), 8, Input::Swap)]);
    }

    #[test]
    fn an_empty_off_hand_takes_two_clicks() {
        // Nothing comes out, so the source slot is not clicked a second time.
        assert_eq!(
            ops_for(20, false),
            vec![
                (Some(20), 0, Input::Pickup),
                (Some(504), 0, Input::Pickup),
            ]
        );
    }

    #[test]
    fn a_full_off_hand_takes_three_and_ends_where_it_started() {
        // The old off hand stack lands on the cursor, so the source slot is
        // clicked again to put it back: the sequence bookends on it.
        assert_eq!(
            ops_for(20, true),
            vec![
                (Some(20), 0, Input::Pickup),
                (Some(504), 0, Input::Pickup),
                (Some(20), 0, Input::Pickup),
            ]
        );
    }

    #[test]
    fn the_off_hand_slot_is_clicked_once_whatever_the_source() {
        for slot in 0..36 {
            for full in [false, true] {
                let ops = ops_for(slot, full);
                let on_offhand = ops.iter().filter(|(at, _, _)| *at == Some(504)).count();
                assert_eq!(on_offhand, 1, "slot {slot}, full {full}");
            }
        }
    }
}
