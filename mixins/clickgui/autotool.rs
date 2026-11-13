// AutoTool: a port of LiquidBounce's ModuleAutoTool.
//
// Copyright (c) 2015-2026 CCBlueX. LiquidBounce is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed in the
// hope that it will be useful, but WITHOUT ANY WARRANTY; without even the
// implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the LICENSE file or <https://www.gnu.org/licenses/>.
//
// While the player mines, the module selects the hotbar slot holding the
// fastest tool for the block (Dynamic) or a fixed slot (Static), and puts the
// slot back after `SwapBackDelay` idle ticks. With ConsiderInventory on, a
// tool in the main inventory is swapped into the first empty hotbar slot (or
// the held one) and swapped back afterwards. The block lists, durability and
// silk touch rules are the original's.
use crate::game::{Game, clickgui as m};
use horizon_client::{items::ItemStack, menu::Input};
use std::cell::RefCell;
use winit::keyboard::KeyCode;

/// Ticks an attack on an entity counts as combat (LiquidBounce's 40).
const COMBAT_TICKS: i32 = 40;
/// Ticks between RequireNearBed scans of the blocks around the player.
const BED_SCAN_TICKS: u64 = 10;

/// A stack moved from the main inventory into a hotbar slot, so it can go
/// back after the mining session.
#[derive(Clone, PartialEq)]
struct Swap {
    /// The hotbar slot the tool was moved into.
    anchor: usize,
    /// The inventory slot it came from (where the swap is undone).
    source: usize,
    /// The stack now in `anchor`, to tell the tool from a stack the player
    /// put there since.
    placed: ItemStack,
}

#[derive(Default)]
struct State {
    /// The slot the player held before AutoTool first switched.
    restore_slot: Option<usize>,
    swap: Option<Swap>,
    /// Ticks since the last time a tool was selected.
    idle: i32,
    /// The block whose break started the SwitchDelay countdown, and the tick.
    session: Option<([i32; 3], u64)>,
    /// Ticks of combat left (NotDuringCombat).
    combat: i32,
    /// The RequireNearBed answer, refreshed every `BED_SCAN_TICKS`.
    near_bed: bool,
    bed_scan_at: Option<u64>,
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
            m::autotool()
        } else {
            None
        };
        let Some(config) = config else {
            // Off: put the slot and any swapped tool back.
            restore(game, state);
            return;
        };
        if !game.native_playing() || game.spectator() {
            return;
        }

        if game.attack_clicks > 0 && game.entity_targeted() {
            state.combat = COMBAT_TICKS;
        }
        state.combat = state.combat.saturating_sub(1);

        let Some((pos, block)) = target(game) else {
            idle(game, &config, state);
            return;
        };

        // SwitchDelay: a new block re-arms the countdown before any switch.
        if config.switch_delay > 0 {
            match state.session {
                Some((seen, start)) if seen == pos => {
                    if game.ticks.saturating_sub(start) < config.switch_delay as u64 {
                        return;
                    }
                }
                _ => {
                    state.session = Some((pos, game.ticks));
                    return;
                }
            }
        } else {
            state.session = None;
        }

        let in_combat = config.not_during_combat && state.combat > 0;
        let not_sneaking = config.require_sneaking && !game.keys.contains(&KeyCode::ShiftLeft);
        let far_from_bed = config.require_near_bed && !bed_near(game, &config, state);
        if in_combat || not_sneaking || far_from_bed || !m::filter_allows(config.filter, &config.blocks, &block)
        {
            idle(game, &config, state);
            return;
        }

        let switched = if config.mode == 1 {
            // Static: the chosen slot, whatever it holds.
            select(game, state, config.static_slot.min(8));
            true
        } else {
            dynamic(game, &config, state, &block)
        };
        if switched {
            state.idle = 0;
        } else {
            idle(game, &config, state);
        }
    });
}

/// The block being mined: the break in progress, else the block the held
/// attack button is about to start on (a click on an entity starts none).
fn target(game: &Game<'_>) -> Option<([i32; 3], String)> {
    let pos = match &game.mining {
        Some(mining) => mining.pos,
        None => {
            if !game.left || game.entity_targeted() {
                return None;
            }
            game.pick()?.pos
        }
    };
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let facts = game.res.facts();
    if facts.is_air(state) {
        return None;
    }
    Some((pos, facts.get(state).name.to_string()))
}

/// Counts an idle tick; after `SwapBackDelay` of them, the split is undone.
fn idle(game: &mut Game<'_>, config: &m::AutoToolConfig, state: &mut State) {
    state.idle += 1;
    if state.idle > config.swap_back_delay {
        restore(game, state);
    }
}

fn restore(game: &mut Game<'_>, state: &mut State) {
    if !game.native_playing() {
        return;
    }
    if let Some(swap) = state.swap.take() {
        let still_there = game.inventory.get(swap.anchor).and_then(|s| s.as_ref()) == Some(&swap.placed);
        if still_there {
            swap_slots(game, swap.anchor, swap.source);
        }
    }
    if let Some(slot) = state.restore_slot.take()
        && slot < 9
    {
        game.selected = slot;
    }
    state.idle = 0;
    state.session = None;
}

/// Dynamic: the fastest hotbar (and inventory) stack for `block`.
fn dynamic(
    game: &mut Game<'_>,
    config: &m::AutoToolConfig,
    state: &mut State,
    block: &str,
) -> bool {
    // LiquidBounce picks no tool in creative.
    if game.creative {
        return false;
    }
    let slots = if config.consider_inventory {
        game.inventory.len().min(36)
    } else {
        game.inventory.len().min(9)
    };
    let mut candidates = Vec::with_capacity(slots);
    for slot in 0..slots {
        if let Some(speed) = tool_speed(game, config, slot, block) {
            candidates.push((slot, speed));
        }
    }
    let Some(best) = m::best_slot(&candidates, game.selected) else {
        return false;
    };
    if best < 9 {
        select(game, state, best);
        return true;
    }
    // The best tool is in the main inventory: swap it into the anchor slot.
    let Some(tool) = game.inventory.get(best).and_then(|s| s.clone()) else {
        return false;
    };
    let anchor = state
        .swap
        .as_ref()
        .map_or_else(|| anchor_slot(game, state), |swap| swap.anchor);
    if game.inventory.get(anchor).and_then(|s| s.as_ref()) != Some(&tool) {
        if !swap_slots(game, best, anchor) {
            return false;
        }
    }
    match state.swap.as_mut() {
        Some(swap) => swap.placed = tool,
        None => {
            state.swap = Some(Swap {
                anchor,
                source: best,
                placed: tool,
            });
        }
    }
    select(game, state, anchor);
    true
}

/// The stack in `slot`'s speed against `block`, if the durability, silk touch
/// and (empty slots) rules let it be used.
fn tool_speed(
    game: &Game<'_>,
    config: &m::AutoToolConfig,
    slot: usize,
    block: &str,
) -> Option<f32> {
    let stack = game.inventory.get(slot).and_then(|s| s.as_ref());
    let items = game.res.items();
    let (max_damage, damage) = match stack {
        Some(s) => (
            items.get(&s.item).map_or(0, |i| i.max_damage as i32),
            s.damage as i32,
        ),
        None => (0, 0),
    };
    if !m::durability_ok(config.ignore_durability, max_damage, damage) {
        return None;
    }
    let silk = stack.map_or(0, |s| s.enchantment_level("minecraft:silk_touch") as i32);
    if !m::silk_touch_allows(
        config.silk_touch,
        config.silk_touch_filter,
        &config.silk_touch_blocks,
        block,
        silk,
    ) {
        return None;
    }
    let mut speed = items.mining_speed(stack, block);
    if speed > 1.0 {
        // Efficiency is the `mining_efficiency` attribute in this data
        // (level^2 + 1 per level), the same bonus `destroy_progress` adds.
        speed += game
            .res
            .stations()
            .enchantments
            .attribute_add(stack, "minecraft:mining_efficiency") as f32;
    }
    Some(speed)
}

/// The first empty hotbar slot, else the slot the player held (LiquidBounce's
/// `AnchoredHotbarSwapController`).
fn anchor_slot(game: &Game<'_>, state: &State) -> usize {
    (0..9)
        .find(|&slot| game.inventory.get(slot).is_none_or(Option::is_none))
        .unwrap_or_else(|| state.restore_slot.unwrap_or(game.selected).min(8))
}

/// Selects `slot`, remembering the player's own slot the first time.
fn select(game: &mut Game<'_>, state: &mut State, slot: usize) {
    if game.selected == slot {
        return;
    }
    if state.restore_slot.is_none() {
        state.restore_slot = Some(game.selected);
    }
    game.selected = slot;
}

/// Swaps two player-inventory slots with three pickups (a server's menu, or
/// the local one through the menu engine).
fn swap_slots(game: &mut Game<'_>, a: usize, b: usize) -> bool {
    if game.carried.is_some() {
        return false;
    }
    let ops = [
        (Some(a), 0, Input::Pickup),
        (Some(b), 0, Input::Pickup),
        (Some(a), 0, Input::Pickup),
    ];
    if game.remote.is_some() {
        game.remote_menu_ops(&ops)
    } else {
        game.menu_ops(&ops)
    }
}

/// RequireNearBed: whether a bed's centre lies within `distance` of the eyes.
/// The answer is refreshed every `BED_SCAN_TICKS`.
fn bed_near(game: &Game<'_>, config: &m::AutoToolConfig, state: &mut State) -> bool {
    let due = state
        .bed_scan_at
        .is_none_or(|at| game.ticks.saturating_sub(at) >= BED_SCAN_TICKS);
    if due {
        state.near_bed = scan_bed(game, config.bed_distance);
        state.bed_scan_at = Some(game.ticks);
    }
    state.near_bed
}

fn scan_bed(game: &Game<'_>, distance: f32) -> bool {
    let Some(eye) = game.eye_position() else {
        return false;
    };
    let radius = distance.ceil().max(1.0) as i32;
    let reach = f64::from(distance) * f64::from(distance);
    let (xlo, xhi) = (
        eye[0].floor() as i32 - radius,
        eye[0].floor() as i32 + radius,
    );
    let (ylo, yhi) = (
        eye[1].floor() as i32 - radius,
        eye[1].floor() as i32 + radius,
    );
    let (zlo, zhi) = (
        eye[2].floor() as i32 - radius,
        eye[2].floor() as i32 + radius,
    );
    let facts = game.res.facts();
    let is_bed = |state| facts.get(state).name.ends_with("_bed");
    for cx in xlo >> 4..=xhi >> 4 {
        for cz in zlo >> 4..=zhi >> 4 {
            let lo = [cx << 4, ylo, cz << 4];
            let hi = [(cx << 4) + 15, yhi, (cz << 4) + 15];
            if !game.world.may_hold(lo, hi, is_bed) {
                continue;
            }
            for x in lo[0].max(xlo)..=hi[0].min(xhi) {
                for z in lo[2].max(zlo)..=hi[2].min(zhi) {
                    for y in ylo..=yhi {
                        let state = game.world.block(x, y, z);
                        if !facts.get(state).name.ends_with("_bed") {
                            continue;
                        }
                        let dx = f64::from(x) + 0.5 - eye[0];
                        let dy = f64::from(y) + 0.5 - eye[1];
                        let dz = f64::from(z) + 0.5 - eye[2];
                        if dx * dx + dy * dy + dz * dz <= reach {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}
