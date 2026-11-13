// AutoClicker: presses the game's own click queue at a chosen rate.
//
// The references are eleven autoclickers across Alan, keystrokes, Raven,
// Prestige and Argon. They all do the same thing — decide when to click, then
// click — and differ in the timing shape (a CPS range, a drag-click burst, a
// replay) and in the gates (weapon only, on click, stop on blocks). This port
// keeps the two timing shapes that matter and the gates they share.
//
// A click is a queued press, not a packet. `InteractionInput::press` bumps a
// per-button counter and `input.tick` drains it; `remote_handle_input` runs
// inside `tick_remote_level`, which `tick_remote` calls *before* `native_tick`
// (where this module runs), so a press is drained on the next tick. That is a
// fixed one-tick delay on every click, not a lost click. Either way the game's
// own attack and use paths run — target picking, attack cooldown, swing and
// packet — exactly as a real click would, which is why nothing here builds a
// packet by hand.
//
// The rate is a millisecond interval, as in the references, but a tick can only
// carry one press per button, so the practical ceiling is twenty clicks a
// second. `Stop on block` uses the game's own block ray within its own reach
// rather than a long raycast, so looking at a distant hill does not count.
//
// Not ported: Raven's inventory clicking (its own feature), the recorded-click
// replay (there is nothing to replay from), Prestige's per-target reaction
// delay, and the jitter some references apply to the player's rotation — that
// is the AntiAim/rotation territory, not clicking.
use crate::game::{Game, clickgui as m};
use horizon_client::interaction_input::Button;
use std::cell::{Cell, RefCell};

/// The click queue's slot for the attack button, and for the use button: what
/// `Game::toggles` uses for "the player is holding this".
const ATTACK: usize = 2;
const USE: usize = 3;
/// The reference's drag-click skip: one tick in twenty does nothing.
const DRAG_CHANCE: f32 = 0.95;

#[derive(Default)]
struct State {
    /// When the next `Normal` click is due, in the debug clock's millis.
    next_ms: i64,
    /// Clicks left in the current drag burst.
    drag_len: i32,
    /// Ticks left before the next burst.
    drag_delay: i32,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    /// The jitter's own roll, kept away from `world.random`: that stream is the
    /// level's, and the client's own simulation draws from it every tick.
    static ROLL: Cell<u32> = const { Cell::new(0x9E37_79B9) };
}

pub(crate) fn tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let Some(config) = m::autoclicker() else {
        STATE.with(|state| *state.borrow_mut() = State::default());
        return;
    };
    if !game.native_playing() || game.player.is_none() || game.remote.is_none() {
        STATE.with(|state| *state.borrow_mut() = State::default());
        return;
    }
    let now = game.debug_millis();
    let mut state = STATE.with(|state| std::mem::take(&mut *state.borrow_mut()));
    match config.mode {
        m::AutoClickerMode::Normal => normal(game, &config, &mut state, now),
        m::AutoClickerMode::Drag => drag(game, &config, &mut state),
    }
    STATE.with(|slot| *slot.borrow_mut() = state);
}

/// A steady rate: click when the interval is up, then pick the next one.
fn normal(game: &mut Game<'_>, config: &m::AutoClickerConfig, state: &mut State, now: i64) {
    if now < state.next_ms {
        return;
    }
    // A gated click leaves the deadline in the past, so the next tick that
    // passes the gates clicks at once instead of waiting out a fresh interval.
    if !click(game, config) {
        return;
    }
    state.next_ms = now + m::AutoClickerConfig::interval_ms(config.cps(roll()));
}

/// A drag-click burst: `Drag length` clicks, one tick apart, then `Drag delay`
/// ticks of nothing — the shape the references simulate.
fn drag(game: &mut Game<'_>, config: &m::AutoClickerConfig, state: &mut State) {
    if state.drag_len < 0 {
        state.drag_delay -= 1;
        if state.drag_delay < 0 {
            state.drag_delay = config.drag_delay;
            state.drag_len = config.drag_length;
        }
        return;
    }
    if roll() >= DRAG_CHANCE {
        return;
    }
    if click(game, config) {
        state.drag_len -= 1;
    }
}

/// Queues a press for each enabled button that passes its gates. False when
/// neither did, so the caller does not count it as a click.
fn click(game: &mut Game<'_>, config: &m::AutoClickerConfig) -> bool {
    let left = config.left && left_allowed(game, config);
    let right = config.right && right_allowed(game, config);
    if !left && !right {
        return false;
    }
    let Some(remote) = game.remote.as_mut() else {
        return false;
    };
    if left {
        remote.input.press(Button::Attack);
    }
    if right {
        remote.input.press(Button::Use);
    }
    true
}

fn left_allowed(game: &Game<'_>, config: &m::AutoClickerConfig) -> bool {
    if config.on_click && !game.toggles.is_down(ATTACK) {
        return false;
    }
    if config.weapon_only && !holds_weapon(game) {
        return false;
    }
    if config.stop_on_block && block_in_reach(game) {
        return false;
    }
    true
}

fn right_allowed(game: &Game<'_>, config: &m::AutoClickerConfig) -> bool {
    !config.on_click || game.toggles.is_down(USE)
}

/// A sword or an axe, by the item tags with the id suffix as the fallback, the
/// way AutoWeapon classifies its weapons.
fn holds_weapon(game: &Game<'_>) -> bool {
    let Some(stack) = game.held() else {
        return false;
    };
    let item = &*stack.item;
    let tagged = |tag: &str| {
        game.res
            .items()
            .item_tags
            .get(tag)
            .is_some_and(|items| items.contains(item))
    };
    tagged("minecraft:swords")
        || tagged("minecraft:axes")
        || item.ends_with("_sword")
        || (item.ends_with("_axe") && !item.ends_with("_pickaxe"))
}

/// Whether the crosshair is on a block the player could reach. The game's own
/// ray, not a long one: a hill on the horizon is not something a click would
/// dig.
fn block_in_reach(game: &Game<'_>) -> bool {
    game.native_raycast(game.block_reach as f32).is_some()
}

/// xorshift32, mapped to `0..1`.
fn roll() -> f32 {
    ROLL.with(|state| {
        let mut x = state.get();
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        state.set(x);
        (x >> 8) as f32 / 16_777_216.0
    })
}

#[cfg(test)]
mod tests {
    use super::roll;
    use crate::game::clickgui::AutoClickerConfig;

    #[test]
    fn the_roll_stays_inside_one_and_moves() {
        let first = roll();
        let mut moved = false;
        for _ in 0..32 {
            let next = roll();
            assert!((0.0..1.0).contains(&next), "{next}");
            moved |= next != first;
        }
        assert!(moved, "the roll never changed");
    }

    #[test]
    fn a_click_interval_matches_the_rate() {
        let config = AutoClickerConfig::default();
        // Eight to fourteen clicks a second, so 125 ms down to about 71 ms.
        assert_eq!(
            AutoClickerConfig::interval_ms(config.cps(0.0)),
            125
        );
        assert_eq!(
            AutoClickerConfig::interval_ms(config.cps(1.0)),
            71
        );
        // Twenty a second is the fastest a tick can carry.
        assert_eq!(AutoClickerConfig::interval_ms(20.0), 50);
    }
}
