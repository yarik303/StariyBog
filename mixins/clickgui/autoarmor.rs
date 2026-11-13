// AutoArmor: puts the best armour you carry onto your body.
//
// The references are Zio's ArmorSwapAction and ArmorSwapStrategy. They are the
// mechanical half of an auto-armour module — find the source slot, find the
// armour slot the item's `equippable` component names, swap the two with menu
// clicks — and that half is what Horizon can back. The choosing half is here:
// a piece is worth what its `minecraft:armor` modifiers say, with the
// protection enchantment and toughness breaking ties.
//
// Every click goes through `remote_menu_ops` (a server's window) or
// `menu_ops` (the local menu engine), so the server receives the same
// `ContainerClick` a player's click would send. The player's own inventory
// menu is always window 0 on the server, so this works without opening a
// screen — and it is why the module stops while another menu is open: a
// chest's slot numbering has no armour slots in it.
//
// Three things are deliberately not candidates. An elytra is worn in the chest
// slot but is not armour, and swapping it out would end a glide. A carved
// pumpkin or a mob head is equippable in the helmet slot but carries no armour
// points, so it is not a piece this module puts on. A piece with the binding
// curse cannot come off (`Slot.mayPickup`), so a slot wearing one is left alone
// instead of clicked at forever.
use crate::game::{Game, Screen, clickgui as m};
use horizon_client::items::ItemStack;
use horizon_client::menu::Input;
use std::cell::RefCell;

#[derive(Default)]
struct State {
    /// Ticks left before the next click.
    wait: u32,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub(super) fn tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let config = if game.native_on(m::ID) {
        m::autoarmor()
    } else {
        None
    };
    let Some(config) = config else {
        STATE.with(|state| *state.borrow_mut() = State::default());
        return;
    };
    // The player's own inventory counts as in play: its menu is the one the
    // module clicks, and it is window 0 on a server.
    let open = game.native_playing() || game.screen == Screen::Inventory;
    if game.player.is_none() || !open || game.spectator() {
        STATE.with(|state| *state.borrow_mut() = State::default());
        return;
    }
    STATE.with(|state| run(game, &config, &mut state.borrow_mut()));
}

fn run(game: &mut Game<'_>, config: &m::AutoArmorConfig, state: &mut State) {
    if state.wait > 0 {
        state.wait -= 1;
        return;
    }
    // The cursor holds a stack: a click now would drop or split it.
    if game.carried.is_some() {
        return;
    }
    let Some((slot, armor)) = choose_swap(game, config) else {
        return;
    };
    if swap_to(game, slot, armor) {
        state.wait = config.delay;
    }
}

/// The best swap the player's inventory offers: the slot to take from and the
/// armour slot it belongs in.
fn choose_swap(game: &Game<'_>, config: &m::AutoArmorConfig) -> Option<(usize, m::ArmorSlot)> {
    let mut best: Option<(usize, m::ArmorSlot, Score)> = None;
    let slots = game.inventory.len().min(36);
    for slot in 0..slots {
        let Some(stack) = game.inventory.get(slot).and_then(|held| held.as_ref()) else {
            continue;
        };
        let Some(armor) = armor_of(game, &stack.item) else {
            continue;
        };
        let Some(candidate) = stats(game, stack, armor) else {
            continue;
        };
        // A carved pumpkin or a mob head is worn in the helmet slot but is
        // not armour; this is not an AutoHat.
        if !is_armour(candidate) {
            continue;
        }
        let worn = match &game.armor[armor.index()] {
            None => None,
            Some(worn) => {
                // A cursed piece cannot be taken off, so clicking at it would
                // only repeat a click the menu refuses.
                if worn.enchantment_level("minecraft:binding_curse") > 0 {
                    continue;
                }
                // A worn piece the module cannot weigh (an elytra) stays on.
                match stats(game, worn, armor) {
                    Some(worn) => Some(worn),
                    None => continue,
                }
            }
        };
        if !wanted(
            config.mode,
            config.compare,
            config.min_durability,
            candidate,
            worn,
        ) {
            continue;
        }
        let score = score(config.compare, candidate);
        if best.is_none_or(|(_, _, best)| score > best) {
            best = Some((slot, armor, score));
        }
    }
    best.map(|(slot, armor, _)| (slot, armor))
}

/// What a piece is worth, as the module sees it. Every field is an integer so
/// the whole thing sorts: armour points and toughness in tenths (the game's
/// are whole numbers, but a mod's may not be), protection in levels, and
/// durability as a percentage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Score {
    /// The comparison's own first key: points or protection.
    primary: i32,
    points: i32,
    protection: i32,
    toughness: i32,
    durability: i32,
}

/// The facts a piece is scored from, read out of the item data so the
/// decision can be tested without a game.
#[derive(Clone, Copy, Debug)]
struct Stats {
    points: f64,
    toughness: f64,
    protection: u32,
    /// Remaining durability as a percentage.
    durability: i32,
}

fn score(compare: m::AutoArmorCompare, stats: Stats) -> Score {
    let points = (stats.points * 10.0).round() as i32;
    let protection = stats.protection as i32;
    Score {
        primary: match compare {
            m::AutoArmorCompare::Armor => points,
            m::AutoArmorCompare::Protection => protection,
        },
        points,
        protection,
        toughness: (stats.toughness * 10.0).round() as i32,
        durability: stats.durability,
    }
}

/// Whether a carried piece should go on: an empty slot takes anything the
/// module would wear, a worn piece gives way only to a better one, and only
/// in `Best`. Equal pieces are left alone, so two of the same do not swap
/// back and forth every tick.
fn wanted(
    mode: m::AutoArmorMode,
    compare: m::AutoArmorCompare,
    min_durability: i32,
    candidate: Stats,
    worn: Option<Stats>,
) -> bool {
    if candidate.durability < min_durability {
        return false;
    }
    match worn {
        None => true,
        Some(worn) => {
            mode == m::AutoArmorMode::Best && score(compare, candidate) > score(compare, worn)
        }
    }
}

/// Whether a piece is armour at all. A carved pumpkin and a mob head carry
/// `equippable.slot: head` but no `minecraft:armor` or
/// `minecraft:armor_toughness`, and putting one on is not what this module is
/// for. The test is on the carried piece only: a worn pumpkin is fair game to
/// replace, and it loses the comparison on its own score anyway.
fn is_armour(stats: Stats) -> bool {
    stats.points > 0.0 || stats.toughness > 0.0
}

/// The armour slot `item` goes in, or None for something the module will not
/// wear: an elytra (a glider in the chest slot, not armour) and a piece
/// restricted to another kind of wearer.
fn armor_of(game: &Game<'_>, item: &str) -> Option<m::ArmorSlot> {
    let info = game.res.items().get(item)?;
    if info.glider {
        return None;
    }
    let allowed = info
        .equip_allowed
        .as_ref()
        .is_none_or(|allowed| allowed.contains("minecraft:player"));
    if !allowed {
        return None;
    }
    m::ArmorSlot::from_name(info.equip_slot.as_deref()?)
}

/// What the item data says a piece is worth in `armor`'s slot. None when the
/// item is not a piece for that slot at all.
fn stats(game: &Game<'_>, stack: &ItemStack, armor: m::ArmorSlot) -> Option<Stats> {
    let info = game.res.items().get(&stack.item)?;
    if info.glider || info.equip_slot.as_deref() != Some(armor.name()) {
        return None;
    }
    let allowed = info
        .equip_allowed
        .as_ref()
        .is_none_or(|allowed| allowed.contains("minecraft:player"));
    if !allowed {
        return None;
    }
    let sum = |attribute: &str| -> f64 {
        info.modifiers
            .iter()
            .filter(|(name, _, _, operation, group)| {
                name.as_str() == attribute
                    && operation.as_str() == "add_value"
                    && group_matches(group, armor)
            })
            .map(|(_, _, amount, _, _)| *amount)
            .sum()
    };
    Some(Stats {
        points: sum("minecraft:armor"),
        toughness: sum("minecraft:armor_toughness"),
        protection: stack.enchantment_level("minecraft:protection"),
        durability: remaining(info.max_damage, stack.damage),
    })
}

/// `EquipmentSlotGroup.test`: whether a modifier the item records for `group`
/// counts in `armor`. `any` and `armor` are the two vanilla armour pieces use;
/// the slot's own name is there for a mod's item.
fn group_matches(group: &str, armor: m::ArmorSlot) -> bool {
    match group {
        "any" | "armor" => true,
        "head" => armor == m::ArmorSlot::Helmet,
        "chest" => armor == m::ArmorSlot::Chest,
        "legs" => armor == m::ArmorSlot::Legs,
        "feet" => armor == m::ArmorSlot::Boots,
        _ => false,
    }
}

/// The remaining durability as a percentage; 100 for a piece that does not
/// wear out.
fn remaining(max_damage: u32, damage: u32) -> i32 {
    if max_damage == 0 {
        return 100;
    }
    let left = max_damage.saturating_sub(damage);
    ((u64::from(left) * 100) / u64::from(max_damage)) as i32
}

/// The clicks that move `slot`'s stack onto `armor`, split out from the game
/// so the order can be pinned by a test. The engine cannot be driven from here
/// — `may_place` on an armour slot needs the item data, which only a prepared
/// resource tree has — so this is the part that is checkable without one.
fn ops_for(slot: usize, armor: m::ArmorSlot, worn: bool) -> Vec<(Option<usize>, i32, Input)> {
    // A hotbar stack goes over in one click: the number key on the armour
    // slot swaps the two, and the worn piece lands in the hotbar slot.
    if slot < 9 {
        return vec![(Some(armor.slot()), slot as i32, Input::Swap)];
    }
    // A free slot takes a shift click. An occupied one is a pickup swap: pick
    // the new piece up, click the armour slot (the worn piece comes back on
    // the cursor), then put it back where the new one came from.
    if !worn {
        return vec![(Some(slot), 0, Input::QuickMove)];
    }
    vec![
        (Some(slot), 0, Input::Pickup),
        (Some(armor.slot()), 0, Input::Pickup),
        (Some(slot), 0, Input::Pickup),
    ]
}

/// Moves `slot`'s stack onto `armor`.
fn swap_to(game: &mut Game<'_>, slot: usize, armor: m::ArmorSlot) -> bool {
    let worn = game.armor[armor.index()].is_some();
    click(game, &ops_for(slot, armor, worn))
}

fn click(game: &mut Game<'_>, ops: &[(Option<usize>, i32, Input)]) -> bool {
    if game.remote.is_some() {
        game.remote_menu_ops(ops)
    } else {
        game.menu_ops(ops)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(points: f64, protection: u32, durability: i32) -> Stats {
        Stats {
            points,
            toughness: 0.0,
            protection,
            durability,
        }
    }

    #[test]
    fn an_empty_slot_takes_the_only_piece() {
        assert!(wanted(
            m::AutoArmorMode::Best,
            m::AutoArmorCompare::Armor,
            5,
            stats(2.0, 0, 100),
            None
        ));
        assert!(wanted(
            m::AutoArmorMode::Empty,
            m::AutoArmorCompare::Armor,
            5,
            stats(2.0, 0, 100),
            None
        ));
    }

    #[test]
    fn a_better_piece_replaces_the_worn_one() {
        assert!(wanted(
            m::AutoArmorMode::Best,
            m::AutoArmorCompare::Armor,
            5,
            stats(8.0, 0, 100),
            Some(stats(6.0, 0, 100))
        ));
    }

    #[test]
    fn an_equal_piece_does_not() {
        // Two of the same must not swap back and forth every tick.
        assert!(!wanted(
            m::AutoArmorMode::Best,
            m::AutoArmorCompare::Armor,
            5,
            stats(6.0, 2, 100),
            Some(stats(6.0, 2, 100))
        ));
    }

    #[test]
    fn a_worse_piece_does_not() {
        assert!(!wanted(
            m::AutoArmorMode::Best,
            m::AutoArmorCompare::Armor,
            5,
            stats(3.0, 0, 100),
            Some(stats(8.0, 0, 100))
        ));
    }

    #[test]
    fn empty_mode_never_replaces_a_worn_piece() {
        assert!(!wanted(
            m::AutoArmorMode::Empty,
            m::AutoArmorCompare::Armor,
            5,
            stats(8.0, 4, 100),
            Some(stats(1.0, 0, 100))
        ));
    }

    #[test]
    fn a_piece_below_the_durability_floor_is_left_alone() {
        assert!(!wanted(
            m::AutoArmorMode::Best,
            m::AutoArmorCompare::Armor,
            25,
            stats(8.0, 0, 24),
            None
        ));
        assert!(wanted(
            m::AutoArmorMode::Best,
            m::AutoArmorCompare::Armor,
            25,
            stats(8.0, 0, 25),
            None
        ));
    }

    #[test]
    fn protection_first_beats_armour_points() {
        let iron = stats(6.0, 4, 100);
        let diamond = stats(8.0, 0, 100);
        assert!(wanted(
            m::AutoArmorMode::Best,
            m::AutoArmorCompare::Protection,
            5,
            iron,
            Some(diamond)
        ));
        // The other way round, armour points win.
        assert!(!wanted(
            m::AutoArmorMode::Best,
            m::AutoArmorCompare::Armor,
            5,
            iron,
            Some(diamond)
        ));
    }

    #[test]
    fn armour_points_break_a_protection_tie() {
        assert!(wanted(
            m::AutoArmorMode::Best,
            m::AutoArmorCompare::Protection,
            5,
            stats(8.0, 2, 100),
            Some(stats(6.0, 2, 100))
        ));
    }

    #[test]
    fn toughness_breaks_a_points_and_protection_tie() {
        let tougher = Stats {
            points: 8.0,
            toughness: 2.0,
            protection: 0,
            durability: 100,
        };
        assert!(wanted(
            m::AutoArmorMode::Best,
            m::AutoArmorCompare::Armor,
            5,
            tougher,
            Some(stats(8.0, 0, 100))
        ));
    }

    #[test]
    fn an_indestructible_piece_is_at_full_durability() {
        assert_eq!(remaining(0, 0), 100);
    }

    #[test]
    fn a_piece_worn_to_nothing_is_at_zero() {
        assert_eq!(remaining(100, 100), 0);
        assert_eq!(remaining(100, 50), 50);
        assert_eq!(remaining(100, 99), 1);
        // A damage value past the maximum cannot go negative.
        assert_eq!(remaining(100, 140), 0);
    }

    #[test]
    fn a_pumpkin_is_not_armour() {
        // `equippable.slot: head`, but nothing to protect the head with.
        assert!(!is_armour(stats(0.0, 0, 100)));
    }

    #[test]
    fn a_leather_cap_is_armour() {
        assert!(is_armour(stats(1.0, 0, 100)));
    }

    #[test]
    fn a_piece_with_only_toughness_counts() {
        let tough = Stats {
            points: 0.0,
            toughness: 2.0,
            protection: 0,
            durability: 100,
        };
        assert!(is_armour(tough));
    }

    #[test]
    fn a_hotbar_piece_is_one_swap_click() {
        // The number key over the armour slot: the button is the hotbar index.
        assert_eq!(
            ops_for(4, m::ArmorSlot::Helmet, true),
            vec![(Some(500), 4, Input::Swap)]
        );
        assert_eq!(
            ops_for(0, m::ArmorSlot::Boots, false),
            vec![(Some(503), 0, Input::Swap)]
        );
    }

    #[test]
    fn a_free_slot_is_one_shift_click() {
        assert_eq!(
            ops_for(20, m::ArmorSlot::Chest, false),
            vec![(Some(20), 0, Input::QuickMove)]
        );
    }

    #[test]
    fn a_worn_piece_is_three_pickups_that_end_where_they_started() {
        // New piece, armour slot, source slot: the last click is what puts
        // the worn piece back, so the source slot must bookend the sequence.
        assert_eq!(
            ops_for(20, m::ArmorSlot::Legs, true),
            vec![
                (Some(20), 0, Input::Pickup),
                (Some(502), 0, Input::Pickup),
                (Some(20), 0, Input::Pickup),
            ]
        );
    }

    #[test]
    fn the_armour_slot_is_clicked_once_whatever_the_source() {
        for slot in 0..36 {
            let ops = ops_for(slot, m::ArmorSlot::Helmet, true);
            let on_armour = ops.iter().filter(|(at, _, _)| *at == Some(500)).count();
            assert_eq!(on_armour, 1, "slot {slot}");
        }
    }

    #[test]
    fn a_modifier_counts_only_in_the_group_it_names() {
        assert!(group_matches("armor", m::ArmorSlot::Helmet));
        assert!(group_matches("any", m::ArmorSlot::Boots));
        assert!(group_matches("head", m::ArmorSlot::Helmet));
        assert!(!group_matches("head", m::ArmorSlot::Chest));
        assert!(!group_matches("feet", m::ArmorSlot::Legs));
        // The hands and the body are not armour slots.
        assert!(!group_matches("mainhand", m::ArmorSlot::Helmet));
        assert!(!group_matches("body", m::ArmorSlot::Chest));
    }
}
