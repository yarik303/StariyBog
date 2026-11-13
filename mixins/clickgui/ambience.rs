// Ambience: the time of day, the weather, and the world fog.
//
// All three are the game's own state rather than a picture drawn over it, and
// the first two are written once a frame from `native_frame`, which the client
// runs after its ticks and before it renders. That order is the whole trick for
// the time: a server's time update lands during the tick, so writing the day
// time any earlier would be undone for a frame every second.
//
// The fog is the one thing here that is not state: `native_render_fog` hands a
// mod the frame's fog after the game has computed it, so the colour and the two
// distances can be replaced without touching the world.
use crate::game::{Dim, Game, clickgui as m};
use horizon_client_render::fog::{FogData, FogType};

thread_local! {
    /// The rain and thunder flags the world had before Ambience first forced
    /// them. A server sends weather only when it changes, so without this the
    /// client would keep the forced weather for as long as the sky holds.
    static SAVED_WEATHER: core::cell::Cell<Option<(bool, bool)>> =
        const { core::cell::Cell::new(None) };
}

/// The day time `seconds` after midnight maps to, vanilla's own clock: 06:00 is
/// tick 0, 12:00 is 6000, 18:00 is 12000 and midnight is 18000.
fn day_time_at(seconds: i64) -> i64 {
    let offset = (seconds - 21_600).rem_euclid(86_400);
    (offset as f64 / 86_400.0 * 24_000.0).round() as i64
}

/// The day time of the wall clock where the player is.
fn real_time_ticks() -> i64 {
    use chrono::Timelike;
    let now = chrono::Local::now();
    let seconds = i64::from(now.hour() * 3600 + now.minute() * 60 + now.second());
    day_time_at(seconds)
}

/// The day time the module fixes, or None when it fixes none.
fn fixed_day_time(config: &m::AmbienceConfig) -> Option<i64> {
    if config.time.real_time() {
        return Some(real_time_ticks());
    }
    config.time.ticks()
}

/// `native_frame`: the time of day and the weather, before the frame draws.
///
/// Deliberately not gated on the playing screen: the world keeps being drawn
/// behind an open menu, and a sky that snapped back to the server's time every
/// time the inventory opened would read as a bug.
pub(super) fn frame(game: &mut Game<'_>) {
    let config = if game.native_on(m::ID) {
        m::ambience()
    } else {
        None
    };
    let Some(config) = config else {
        restore_weather(game);
        return;
    };
    if config.is_vanilla() {
        // Every setting off: nothing to force, and anything forced before
        // goes back.
        restore_weather(game);
        return;
    }
    // Only a client of a server draws a sky of its own, and only the Overworld
    // has one; `Game::own_view` says the same and is not reachable from a mod.
    let own_sky = game.remote.is_some() && game.dim == Dim::Overworld;
    if own_sky
        && let Some(ticks) = fixed_day_time(&config)
    {
        // The day the world is on is kept: only the time within it is fixed,
        // so the moon keeps its phase and nothing that counts days jumps.
        let day = game.world.day_time.div_euclid(24_000);
        game.world.day_time = day * 24_000 + ticks;
    }
    match config.weather.levels() {
        Some((raining, thundering)) => {
            SAVED_WEATHER.with(|saved| {
                if saved.get().is_none() {
                    let weather = &game.world.weather;
                    saved.set(Some((weather.raining, weather.thundering)));
                }
            });
            game.world.weather.raining = raining;
            game.world.weather.thundering = thundering;
            // The fade is skipped: the reference sets the strength outright,
            // and waiting a hundred ticks for it would read as a bug.
            game.world.weather.settle_levels();
        }
        None => restore_weather(game),
    }
}

/// Puts back the weather the world had before the module forced one.
fn restore_weather(game: &mut Game<'_>) {
    let Some((raining, thundering)) = SAVED_WEATHER.with(|saved| saved.take()) else {
        return;
    };
    game.world.weather.raining = raining;
    game.world.weather.thundering = thundering;
    game.world.weather.settle_levels();
}

/// `native_render_fog`: the fog's colour and where it starts and ends.
///
/// Only the open air's fog is touched. Water, lava and powder snow fog are the
/// game's way of saying the camera is inside something, and replacing them
/// would take away the only cue that says so. Blindness and darkness keep their
/// own fog for the same reason.
pub(super) fn fog(
    game: &Game<'_>,
    fog_type: FogType,
    fog: &mut FogData,
) {
    if fog_type != FogType::Atmospheric {
        return;
    }
    if !game.native_on(m::ID) {
        return;
    }
    let Some(config) = m::ambience() else {
        return;
    };
    if !config.fog.overrides() {
        return;
    }
    let effects = &game.potions.effects;
    if effects.get("blindness").is_some() || effects.get("darkness").is_some() {
        return;
    }
    let [r, g, b] = m::rgb(config.fog_color);
    fog.color = [r, g, b, 1.0];
    // The reference's two sliders are fractions of the view distance, which is
    // the render distance in blocks.
    let view = fog.render_distance_end.max(1.0);
    let start = view * config.fog_start;
    let end = (view * config.fog_end).max(start + 5.0);
    fog.environmental_start = start;
    fog.environmental_end = end;
    fog.render_distance_start = start;
    fog.render_distance_end = end;
    // The sky and the clouds fade where the ground does, or the horizon shows a
    // clear sky above fogged ground.
    fog.sky_end = end;
    fog.cloud_end = end;
}

#[cfg(test)]
mod tests {
    use super::day_time_at;

    #[test]
    fn the_clock_maps_to_the_day_the_game_draws() {
        // 06:00 is tick 0, the sunrise; noon is 6000; dusk is 12000.
        assert_eq!(day_time_at(6 * 3600), 0);
        assert_eq!(day_time_at(12 * 3600), 6_000);
        assert_eq!(day_time_at(18 * 3600), 12_000);
        // Midnight is the middle of the night, not the start of the day, and a
        // whole day later the clock is at midnight again.
        assert_eq!(day_time_at(0), 18_000);
        assert_eq!(day_time_at(24 * 3600), 18_000);
    }

    #[test]
    fn the_clock_wraps_and_stays_inside_one_day() {
        // The last second of the day is still a tick of that day.
        assert_eq!(day_time_at(86_399), 18_000);
        // An hour before sunrise is just before tick 0, i.e. the end of it.
        assert_eq!(day_time_at(5 * 3600), 23_000);
        for hour in 0..48 {
            let t = day_time_at(hour * 3600);
            assert!((0..24_000).contains(&t), "{hour}h gave {t}");
        }
    }
}
