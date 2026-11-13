// AutoWeapon: a port of LiquidBounce's ModuleAutoWeapon.
//
// Copyright (c) 2015-2026 CCBlueX. LiquidBounce is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed in the
// hope that it will be useful, but WITHOUT ANY WARRANTY; without even the
// implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the LICENSE file or <https://www.gnu.org/licenses/>.
//
// When the player attacks (OnAttack) or aims at (OnTarget) a living entity,
// the module switches the hotbar to the best weapon for that moment: a mace
// while falling, an axe when the target blocks with a shield, or otherwise the
// item matching Preferred and ranked by Priority. The held slot goes back
// after `SwitchBack` idle ticks. The selection is a normal hotbar switch (the
// server is told with `SetCarriedItem`), not LiquidBounce's silent one.
use crate::game::{Game, clickgui as m};
use horizon_client::items::ItemStack;
use std::cell::RefCell;

const MACE: &str = "minecraft:mace";
/// `MaceItem.SMASH_ATTACK_FALL_THRESHOLD`.
const FALL_THRESHOLD: f64 = 1.5;

#[derive(Default)]
struct State {
    /// The slot the player held before AutoWeapon first switched.
    restore_slot: Option<usize>,
    /// The slot AutoWeapon last selected, so a manual switch is left alone.
    switched_to: Option<usize>,
    /// Ticks since the last time a weapon was selected.
    idle: i32,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub(super) fn tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    // No player: the world was left, so forget everything without clicking.
    if game.player.is_none() {
        STATE.with(|s| *s.borrow_mut() = State::default());
        return;
    }
    STATE.with(|s| {
        let state = &mut *s.borrow_mut();
        let config = if game.native_on(m::ID) {
            m::autoweapon()
        } else {
            None
        };
        let Some(config) = config else {
            // Off: put the player's slot back.
            restore(game, state);
            return;
        };
        if !game.native_playing() || game.spectator() {
            return;
        }

        let target = target_mob(game);
        let attack = target.is_some() && game.attack_clicks > 0 && config.on_attack();
        let on_target = target.is_some() && config.on_target();
        if !attack && !on_target {
            idle(game, &config, state);
            return;
        }
        // Food and potions own the main hand: do not take it away.
        if is_busy(game) {
            idle(game, &config, state);
            return;
        }

        let Some(slot) = determine_weapon(game, &config, target) else {
            idle(game, &config, state);
            return;
        };
        select(game, state, slot);
        state.idle = 0;
    });
}

/// The attack click, before the game handles it. In a server world (the
/// default single player included) the click is consumed before `native_tick`
/// runs, so OnAttack has to switch the slot here to have it held for the hit.
/// The input is whatever `key.attack` is bound to, mouse or keyboard.
pub(super) fn on_key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed
        || !game.native_window()
        || !game
            .settings
            .keys
            .mappings_for(key)
            .any(|mapping| mapping == "key.attack")
    {
        return;
    }
    if !game.native_playing() || game.spectator() || !game.native_on(m::ID) {
        return;
    }
    let Some(config) = m::autoweapon() else {
        return;
    };
    if !config.on_attack() || is_busy(game) {
        return;
    }
    let target = target_mob(game);
    if target.is_none() {
        return;
    }
    let Some(slot) = determine_weapon(game, &config, target) else {
        return;
    };
    STATE.with(|s| {
        let state = &mut *s.borrow_mut();
        select(game, state, slot);
        state.idle = 0;
    });
}

/// The living entity under the crosshair, as a mob id.
fn target_mob(game: &Game<'_>) -> Option<u32> {
    let Some(Some(id)) = game.pick_entity_hit() else {
        return None;
    };
    let id = u32::try_from(id).ok()?;
    game.mobs
        .iter()
        .find(|m| m.id == id && !m.removed)
        .map(|m| m.id)
}

/// Whether the target is using a shield (or another `blocks_attacks` item).
fn target_blocking(game: &Game<'_>, target: Option<u32>) -> bool {
    target.is_some_and(|id| game.mobs.iter().any(|m| m.id == id && m.extra.blocking))
}

/// LiquidBounce's `isBusy`: the player is consuming the item in the main hand.
fn is_busy(game: &Game<'_>) -> bool {
    let Some((slot, _)) = game.using else {
        return false;
    };
    slot == game.selected
        && game.held().is_some_and(|stack| {
            game.res
                .items()
                .get(&stack.item)
                .is_some_and(|info| info.consumable.is_some())
        })
}

/// `MaceItem.canSmashAttack`: falling more than one and a half blocks, not
/// gliding.
fn can_smash(game: &Game<'_>) -> bool {
    game.player
        .as_ref()
        .is_some_and(|p| p.fall_distance > FALL_THRESHOLD && !p.fall_flying)
}

/// The hotbar slot LiquidBounce's `determineWeaponSlot` would pick.
fn determine_weapon(
    game: &Game<'_>,
    config: &m::AutoWeaponConfig,
    target: Option<u32>,
) -> Option<usize> {
    let slots: Vec<usize> = (0..9)
        .filter(|&slot| stack(game, slot).is_some_and(|stack| is_weapon_candidate(game, stack)))
        .collect();

    // A mace's smash cannot be blocked, and an axe stuns a shielding target;
    // both beat the normal preference order, mace first.
    if config.auto_mace && can_smash(game) {
        return best_by(
            game,
            slots.iter().copied().filter(|&slot| {
                stack(game, slot).is_some_and(|s| type_matches(game, s, m::WeaponKind::Mace))
            }),
            |stack| estimate_damage(game, stack),
        );
    }
    if config.auto_shield_break && target_blocking(game, target) {
        return best_by(
            game,
            slots.iter().copied().filter(|&slot| {
                stack(game, slot).is_some_and(|s| type_matches(game, s, m::WeaponKind::Axe))
            }),
            |stack| estimate_damage(game, stack),
        );
    }

    let preferred: Vec<usize> = slots
        .into_iter()
        .filter(|&slot| {
            stack(game, slot).is_some_and(|stack| {
                config
                    .preferred
                    .iter()
                    .any(|&kind| type_matches(game, stack, kind))
            })
        })
        .collect();
    if preferred.is_empty() {
        // An empty Preferred list only lets AutoShieldBreak and AutoMace pick.
        return None;
    }
    match config.priority {
        1 => best_by(game, preferred, knockback_level),
        2 => best_by(game, preferred, |stack| attack_damage(game, stack)),
        3 => best_by(game, preferred, |stack| attack_speed(game, stack)),
        _ => best_by(game, preferred, |stack| estimate_damage(game, stack)),
    }
}

/// The highest-scoring slot; ties keep the lower slot, as `maxOrNull` keeps
/// the first candidate.
fn best_by(
    game: &Game<'_>,
    slots: impl IntoIterator<Item = usize>,
    mut score: impl FnMut(&ItemStack) -> f64,
) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for slot in slots {
        let Some(stack) = stack(game, slot) else {
            continue;
        };
        let value = score(stack);
        if !value.is_finite() {
            continue;
        }
        if best.is_none_or(|(_, top)| value > top) {
            best = Some((slot, value));
        }
    }
    best.map(|(slot, _)| slot)
}

/// A stack LiquidBounce's `WeaponItemFacet` would consider better than a fist.
fn is_weapon_candidate(game: &Game<'_>, stack: &ItemStack) -> bool {
    game.res
        .items()
        .get(&stack.item)
        .is_some_and(|info| info.attack_damage > 0.0 || info.weapon_damage.is_some())
        || [
            m::WeaponKind::Sword,
            m::WeaponKind::Axe,
            m::WeaponKind::Mace,
            m::WeaponKind::Spear,
            m::WeaponKind::Pickaxe,
            m::WeaponKind::Shovel,
            m::WeaponKind::Hoe,
        ]
        .into_iter()
        .any(|kind| type_matches(game, stack, kind))
        || stack.enchantment_level("minecraft:knockback") > 0
        || stack.enchantment_level("minecraft:fire_aspect") > 0
}

/// LiquidBounce's `WeaponType` predicates. Tags decide the tool families;
/// the id suffix is the fallback when a tag is missing from the data.
fn type_matches(game: &Game<'_>, stack: &ItemStack, kind: m::WeaponKind) -> bool {
    let item = &*stack.item;
    match kind {
        m::WeaponKind::Any => true,
        m::WeaponKind::Sword => in_tag(game, "minecraft:swords", item) || named(item, "_sword"),
        m::WeaponKind::Axe => {
            in_tag(game, "minecraft:axes", item)
                || (named(item, "_axe") && !named(item, "_pickaxe"))
        }
        m::WeaponKind::Mace => item == MACE,
        m::WeaponKind::Spear => in_tag(game, "minecraft:spears", item) || named(item, "_spear"),
        m::WeaponKind::Pickaxe => {
            in_tag(game, "minecraft:pickaxes", item) || named(item, "_pickaxe")
        }
        m::WeaponKind::Shovel => in_tag(game, "minecraft:shovels", item) || named(item, "_shovel"),
        m::WeaponKind::Hoe => in_tag(game, "minecraft:hoes", item) || named(item, "_hoe"),
        m::WeaponKind::Knockback => stack.enchantment_level("minecraft:knockback") > 0,
        m::WeaponKind::FireAspect => stack.enchantment_level("minecraft:fire_aspect") > 0,
    }
}

fn in_tag(game: &Game<'_>, tag: &str, item: &str) -> bool {
    game.res
        .items()
        .item_tags
        .get(tag)
        .is_some_and(|items| items.contains(item))
}

fn named(item: &str, suffix: &str) -> bool {
    item.rsplit(':')
        .next()
        .is_some_and(|name| name.ends_with(suffix))
}

fn stack<'a>(game: &'a Game<'_>, slot: usize) -> Option<&'a ItemStack> {
    game.inventory.get(slot).and_then(|s| s.as_ref())
}

/// LiquidBounce's `ItemStack.attackDamage`: the main-hand attack damage
/// attribute (base 1 plus the item's modifier) and the Sharpness bonus.
fn attack_damage(game: &Game<'_>, stack: &ItemStack) -> f64 {
    let base = 1.0
        + game
            .res
            .items()
            .get(&stack.item)
            .map_or(0.0, |info| info.attack_damage) as f64;
    let sharpness = f64::from(stack.enchantment_level("minecraft:sharpness"));
    let sharpness_bonus = if sharpness > 0.0 {
        0.5 * sharpness + 0.5
    } else {
        0.0
    };
    base + sharpness_bonus
}

/// LiquidBounce's `ItemStack.attackSpeed`: the player's base 4.0 plus the
/// item's attack-speed modifier.
fn attack_speed(game: &Game<'_>, stack: &ItemStack) -> f64 {
    4.0 + game
        .res
        .items()
        .get(&stack.item)
        .map_or(0.0, |info| info.attack_speed) as f64
}

fn knockback_level(stack: &ItemStack) -> f64 {
    f64::from(stack.enchantment_level("minecraft:knockback"))
}

/// `WeaponItemFacet.estimateDamage`: speed-adjusted damage, with Fire Aspect
/// and the Smite, Bane of Arthropods and Knockback weights.
fn estimate_damage(game: &Game<'_>, stack: &ItemStack) -> f64 {
    let damage = attack_damage(game, stack);
    let speed = attack_speed(game, stack).max(0.1);
    let p = 0.85f64.powf(1.0 / 20.0);
    let big_t = 20.0 / speed;
    let probability = p.powf((big_t * 0.9).ceil());
    let fire_aspect = f64::from(stack.enchantment_level("minecraft:fire_aspect"));
    let fire = (fire_aspect * 4.0 - 1.0).max(0.0) * 0.33;
    let extra = 0.2 * f64::from(stack.enchantment_level("minecraft:smite"))
        + 0.2 * f64::from(stack.enchantment_level("minecraft:bane_of_arthropods"))
        + 0.2 * knockback_level(stack);
    damage * speed * probability * (1.0 + extra) + fire
}

/// Selects `slot`, remembering the player's own slot the first time.
fn select(game: &mut Game<'_>, state: &mut State, slot: usize) {
    if game.selected == slot {
        return;
    }
    if state.restore_slot.is_none() {
        state.restore_slot = Some(game.selected);
    }
    state.switched_to = Some(slot);
    game.selected = slot;
}

/// Counts an idle tick; after `SwitchBack` of them, the slot goes back.
fn idle(game: &mut Game<'_>, config: &m::AutoWeaponConfig, state: &mut State) {
    state.idle += 1;
    if state.idle > config.switch_back {
        restore(game, state);
    }
}

fn restore(game: &mut Game<'_>, state: &mut State) {
    if !game.native_playing() {
        return;
    }
    // A slot the player (or another module) chose since is left alone.
    if state.switched_to == Some(game.selected)
        && let Some(slot) = state.restore_slot.take()
        && slot < 9
    {
        game.selected = slot;
    }
    state.restore_slot = None;
    state.switched_to = None;
    state.idle = 0;
}
