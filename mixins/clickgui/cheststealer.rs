// ChestStealer: a port of FDPClient's ChestStealer for chest and furnace
// screens.
//
// Copyright (c) 2015-2026 CCBlueX. LiquidBounce is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed in the
// hope that it will be useful, but WITHOUT ANY WARRANTY; without even the
// implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the LICENSE file or <https://www.gnu.org/licenses/>.
//
// The reference runs a coroutine with millisecond delays; Horizon's mods see
// one tick at a time, so the same plan is a small state machine: wait after
// opening (`StartDelay`), shift-click chest slots in `Order`, re-planning
// after every pass until the chest holds nothing the inventory can take, then
// wait `CloseDelay` and close. Each click goes through `menu_ops`, Horizon's
// own menu engine, so single-player and server menus both work and the server
// receives the same predicted `ContainerClick` a player's click would send.
//
// Settings that a pure client cannot honour are dropped: SilentGUI and
// SilentView (Horizon has no hidden container screen), NoMove (there is no
// server-side move check to wait for) and InventoryCleaner's sorting and
// AutoArmor's equip-from-chest (those modules do not exist here). Debug
// messages go to chat, since Horizon has no notification queue.
use crate::game::{Game, Screen, clickgui as m};
use horizon_client::containers::BlockEntity;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::menu::Input;
use std::cell::RefCell;

/// The open container a session belongs to: a server's window, or the block
/// whose menu a single-player world opened.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Key {
    Window(i32),
    Block((i32, i32, i32)),
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Phase {
    #[default]
    Start,
    Steal,
    Close,
}

#[derive(Default)]
struct State {
    /// The container these fields belong to.
    key: Option<Key>,
    phase: Phase,
    /// Ticks left before the next action.
    wait: u32,
    /// The slot being stolen, for the highlight.
    slot: Option<usize>,
    /// 0..=1 while a session runs; None when there is nothing to show.
    progress: Option<f32>,
    /// The bar's eased value, moved towards `progress` every frame.
    eased: f32,
    /// Stacks taken this session, for the progress estimate.
    stolen: u32,
    /// The shuffle's random source.
    seed: u64,
    /// The chest was emptied (or closed): do not start another pass for it.
    finished: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub(super) fn tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let config = if game.native_on(m::ID) {
        m::cheststealer()
    } else {
        None
    };
    let Some(config) = config else {
        STATE.with(|state| *state.borrow_mut() = State::default());
        return;
    };
    if game.player.is_none() {
        STATE.with(|state| *state.borrow_mut() = State::default());
        return;
    }
    STATE.with(|state| run(game, &config, &mut *state.borrow_mut()));
}

fn run(game: &mut Game<'_>, config: &m::ChestStealerConfig, state: &mut State) {
    let Some(key) = open_key(game, config) else {
        *state = State::default();
        return;
    };
    if state.key != Some(key) {
        *state = State {
            key: Some(key),
            phase: Phase::Start,
            wait: ticks(config.start_delay),
            progress: Some(0.0),
            seed: game.ticks ^ 0x9E37_79B9_7F4A_7C15,
            ..State::default()
        };
        if config.debug != 0 {
            let count = panel_slots(game)
                .into_iter()
                .filter(|&slot| game.slot_view(slot).is_some())
                .count();
            game.native_chat(&format!("Chest opened with {count} items"));
        }
        return;
    }
    if state.finished {
        return;
    }
    if state.wait > 0 {
        state.wait -= 1;
        if state.wait > 0 {
            return;
        }
    }
    match state.phase {
        Phase::Start => {
            state.phase = Phase::Steal;
            // The first click comes next tick, as the reference's
            // `clickNextTick` does.
            state.wait = 0;
            if config.debug != 0 {
                game.native_chat("Stealing items..");
            }
        }
        Phase::Steal => steal(game, config, state),
        Phase::Close => close(game, config, state),
    }
}

/// The container this tick may steal from, or None: another screen, a menu
/// that is not a chest or (with Furnace on) a furnace, the plain inventory,
/// or a title the ChestTitle rule rejects.
fn open_key(game: &Game<'_>, config: &m::ChestStealerConfig) -> Option<Key> {
    if game.screen != Screen::Inventory {
        return None;
    }
    let furnace = match game.container_kind()? {
        BlockEntity::Chest(_) => false,
        BlockEntity::Furnace(_) if config.furnace => true,
        _ => return None,
    };
    let key = current_key(game)?;
    if !furnace && config.chest_title && !title_ok(game, key) {
        return None;
    }
    Some(key)
}

/// The open container's session key, whatever it holds: a server window id
/// (never 0, which is the plain inventory) or a single-player block.
fn current_key(game: &Game<'_>) -> Option<Key> {
    if game.screen != Screen::Inventory {
        return None;
    }
    match game.remote.as_ref() {
        Some(remote) => {
            let (window, _) = remote.menu.as_ref()?;
            (*window != 0).then_some(Key::Window(*window))
        }
        None => game.container.map(Key::Block),
    }
}

/// ChestTitle: a real chest's title (as the reference's
/// `Blocks.chest.localizedName` check does), not a custom GUI's. A
/// single-player chest has no menu title, so its block says.
fn title_ok(game: &Game<'_>, key: Key) -> bool {
    match key {
        Key::Window(_) => game
            .remote_menu_title()
            .is_some_and(|title| title.plain().to_lowercase().contains("chest")),
        Key::Block(pos) => {
            if game.ender_open == Some(pos) {
                return true;
            }
            let name = &game.res.facts().get(game.world.block(pos.0, pos.1, pos.2)).name;
            name.contains("chest")
        }
    }
}

fn steal(game: &mut Game<'_>, config: &m::ChestStealerConfig, state: &mut State) {
    // The cursor holds a stack: clicking now would drop or split it.
    if game.carried.is_some() {
        return;
    }
    let Some((slot, stack, delay_ms)) = choose(game, config, state) else {
        // Nothing left the inventory can hold: the chest is done.
        state.progress = Some(1.0);
        state.phase = Phase::Close;
        state.wait = ticks(config.close_delay);
        return;
    };
    let hotbar = config.quick_swaps.then(|| first_empty_hotbar(game)).flatten();
    let clicked = match hotbar {
        Some(hotbar) => game.menu_ops(&[(Some(slot), hotbar as i32, Input::Swap)]),
        None => game.menu_ops(&[(Some(slot), 0, Input::QuickMove)]),
    };
    if !clicked {
        // The open screen is not a menu the engine models: stop here.
        state.progress = Some(1.0);
        state.phase = Phase::Close;
        state.wait = ticks(config.close_delay);
        return;
    }
    state.slot = Some(slot);
    state.stolen += 1;
    let remaining = panel_slots(game)
        .into_iter()
        .filter(|&slot| game.slot_view(slot).is_some())
        .count() as u32;
    let total = state.stolen + remaining;
    state.progress = Some(if total == 0 {
        1.0
    } else {
        state.stolen as f32 / total as f32
    });
    let mut ms = delay_ms;
    // SimulateShortStop: a quarter of the steals pause 150..=500 ms longer.
    if config.simulate_short_stop && random(state) % 4 == 0 {
        ms += 150 + (random(state) % 351) as i32;
    }
    state.wait = ticks(ms);
    if config.debug != 0 && config.item_debug {
        game.native_chat(&format!(
            "item: {} | slot: {slot} | delay: {ms}ms",
            stack.item.trim_start_matches("minecraft:")
        ));
    }
}

/// The next slot to take and the delay after clicking it, in milliseconds.
fn choose(
    game: &mut Game<'_>,
    config: &m::ChestStealerConfig,
    state: &mut State,
) -> Option<(usize, ItemStack, i32)> {
    let mut items: Vec<(usize, ItemStack)> = panel_slots(game)
        .into_iter()
        .filter_map(|slot| game.slot_view(slot).map(|stack| (slot, stack)))
        .collect();
    if items.is_empty() {
        return None;
    }
    if config.order == 1 {
        items.reverse();
    }
    if config.random_slot {
        for i in (1..items.len()).rev() {
            let j = random(state) as usize % (i + 1);
            items.swap(i, j);
        }
    }
    if config.smart_order
        && let Some(last) = state.slot
    {
        let mut best = 0;
        let mut best_distance = i32::MAX;
        for (index, (slot, _)) in items.iter().enumerate() {
            let distance = m::slot_distance_squared(last, *slot);
            if distance < best_distance {
                best_distance = distance;
                best = index;
            }
        }
        items.swap(0, best);
    }
    let index = if can_take(game, &items[0].1) {
        0
    } else if config.on_full == 1 {
        // Continue: skip the stack and try the rest.
        items.iter().position(|(_, stack)| can_take(game, stack))?
    } else {
        return None;
    };
    let (slot, stack) = &items[index];
    let delay = if config.smart_delay && index + 1 < items.len() {
        let jitter = (random(state) % 21) as i32;
        let distance = m::slot_distance_squared(*slot, items[index + 1].0);
        m::smart_delay_ms(distance, config.delay_multiplier, jitter)
    } else {
        config.delay
    };
    Some((*slot, stack.clone(), delay))
}

/// Close the container the way the inventory key does.
fn close(game: &mut Game<'_>, config: &m::ChestStealerConfig, state: &mut State) {
    if config.auto_close {
        game.close_container();
        game.screen = Screen::Playing;
        game.capture(true);
        if config.debug != 0 {
            game.native_chat("Chest closed");
        }
    }
    state.progress = None;
    state.slot = None;
    state.finished = true;
}

/// The open container's own slots (300..; the player's are 0..36).
fn panel_slots(game: &Game<'_>) -> Vec<usize> {
    game.slots()
        .into_iter()
        .filter(|slot| (300..500).contains(slot))
        .collect()
}

/// Whether the inventory can hold `stack`: an empty slot, or room to merge
/// into a stack of the same item and components.
fn can_take(game: &Game<'_>, stack: &ItemStack) -> bool {
    if game.inventory.iter().any(Option::is_none) {
        return true;
    }
    let max = game
        .res
        .items()
        .get(&stack.item)
        .map_or(64, |item| item.max_stack)
        .max(1);
    game.inventory
        .iter()
        .flatten()
        .any(|have| have.same_item_same_components(stack) && have.count < max)
}

/// The first empty hotbar slot, for QuickSwaps.
fn first_empty_hotbar(game: &Game<'_>) -> Option<usize> {
    (0..9).find(|&slot| game.inventory.get(slot).is_none_or(Option::is_none))
}

/// A millisecond delay as ticks (20 a second), at least one.
fn ticks(ms: i32) -> u32 {
    m::steal_delay_ticks(ms).max(1) as u32
}

/// The module's own random source, so the world's RNG is left alone.
fn random(state: &mut State) -> u32 {
    state.seed = state
        .seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (state.seed >> 33) as u32
}

/// The highlight and the progress bar, over the container screen. Runs from
/// `draw_carried`, after the screen's items and before the carried stack.
pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch) {
    let Some(config) = m::cheststealer() else {
        return;
    };
    if !config.highlight_slot && !config.progress_bar {
        return;
    }
    if !matches!(
        game.container_kind(),
        Some(BlockEntity::Chest(_)) | Some(BlockEntity::Furnace(_))
    ) {
        return;
    }
    let Some(key) = current_key(game) else {
        return;
    };
    let (ox, oy) = game.inventory_origin(batch);
    STATE.with(|state| {
        let state = &mut *state.borrow_mut();
        if state.finished || state.key != Some(key) {
            return;
        }
        if config.highlight_slot
            && let Some(slot) = state.slot
        {
            highlight(game, batch, ox, oy, slot, &config);
        }
        if config.progress_bar
            && let Some(progress) = state.progress
        {
            state.eased += (progress - state.eased) / 6.0;
            bar(game, batch, state.eased);
        }
    });
}

fn highlight(
    game: &Game<'_>,
    batch: &mut HudBatch,
    ox: f32,
    oy: f32,
    slot: usize,
    config: &m::ChestStealerConfig,
) {
    let (sx, sy) = game.slot_position(slot);
    let (x, y) = (ox + sx - 1.0, oy + sy - 1.0);
    let size = 18.0;
    game.native_fill(batch, [x, y, size, size], config.background_color);
    let thickness = config.border_strength.max(1) as f32;
    let color = config.border_color;
    game.native_fill(batch, [x, y, size, thickness], color);
    game.native_fill(batch, [x, y + size - thickness, size, thickness], color);
    let inner = size - 2.0 * thickness;
    game.native_fill(batch, [x, y + thickness, thickness, inner], color);
    game.native_fill(batch, [x + size - thickness, y + thickness, thickness, inner], color);
}

/// The reference's bar: 30%..70% of the width at 75% of the height, a light
/// border, a dark trough and an HSV fill that turns from red to yellow.
fn bar(game: &Game<'_>, batch: &mut HudBatch, eased: f32) {
    let (width, height) = (batch.width, batch.height);
    let (min_x, max_x) = (width * 0.3, width * 0.7);
    let min_y = height * 0.75;
    let bar_height = 10.0;
    game.native_fill(
        batch,
        [min_x - 2.0, min_y - 2.0, max_x - min_x + 4.0, bar_height + 4.0],
        0xFFC8_C8C8,
    );
    game.native_fill(batch, [min_x, min_y, max_x - min_x, bar_height], 0xFF32_3232);
    let eased = eased.clamp(0.0, 1.0);
    let filled = (max_x - min_x) * eased;
    if filled > 0.0 {
        game.native_fill(batch, [min_x, min_y, filled, bar_height], hsv(eased * 72.0));
    }
}

/// A fully saturated hue in degrees, as 0xFFRRGGBB.
fn hsv(hue: f32) -> u32 {
    let h = hue.rem_euclid(360.0) / 60.0;
    let sector = h.floor() as i32;
    let f = h - h.floor();
    let q = 1.0 - f;
    let t = f;
    let (r, g, b) = match sector {
        0 => (1.0, t, 0.0),
        1 => (q, 1.0, 0.0),
        2 => (0.0, 1.0, t),
        3 => (0.0, q, 1.0),
        4 => (t, 0.0, 1.0),
        _ => (1.0, 0.0, q),
    };
    let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
    0xFF00_0000 | byte(r) << 16 | byte(g) << 8 | byte(b)
}
