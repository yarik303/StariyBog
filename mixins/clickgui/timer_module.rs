// The Timer module: multiplies the game clock, optionally alternating a boost
// speed with the normal multiplier. The clock itself lives in
// `horizon_client::clock`; `set_module` hands it this tick's multiplier. The
// Speed modes' own timer requests are separate and multiply with this one.
use crate::game::clickgui as m;
use crate::game::{Game, Screen};
use horizon_client::clock::clickgui_timer as timer;
use std::cell::Cell;
use winit::keyboard::KeyCode;

thread_local! {
    /// The boost cycle's position, in ticks.
    static CYCLE: Cell<u32> = const { Cell::new(0) };
}

fn config(game: &Game<'_>) -> Option<m::TimerConfig> {
    if !game.native_window() || !game.native_playing() || game.screen != Screen::Playing {
        return None;
    }
    game.player.as_ref()?;
    m::timer()
}

fn moving(game: &Game<'_>) -> bool {
    let Some(player) = game.player.as_ref() else {
        return false;
    };
    let held = |key: KeyCode| game.physical_keys.contains(&key);
    player.velocity.x.abs() > 0.01
        || player.velocity.z.abs() > 0.01
        || held(KeyCode::KeyW)
        || held(KeyCode::KeyS)
        || held(KeyCode::KeyA)
        || held(KeyCode::KeyD)
}

/// The multiplier for this tick, advancing the boost cycle. The cycle holds
/// while the mode's condition is off, as the reference timer does.
fn effective(config: &m::TimerConfig) -> f32 {
    if config.boost_ticks == 0 {
        return config.multiplier;
    }
    let total = config.boost_ticks + config.normal_ticks;
    let phase = CYCLE.with(Cell::get) % total;
    CYCLE.with(|cycle| cycle.set((phase + 1) % total));
    if phase < config.boost_ticks {
        config.boost_speed
    } else {
        config.multiplier
    }
}

pub(super) fn tick(game: &mut Game<'_>) {
    let Some(config) = config(game) else {
        CYCLE.with(|cycle| cycle.set(0));
        timer::set_module(1.0);
        return;
    };
    let moving = moving(game);
    let active = match config.mode {
        1 => moving,
        2 => !moving,
        _ => true,
    };
    if !active {
        timer::set_module(1.0);
        return;
    }
    timer::set_module(effective(&config));
}
