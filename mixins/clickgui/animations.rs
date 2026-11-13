// Animations: how fast the first-person hand swings, how far it travels, and
// whether an item in use comes to the mouth.
//
// The whole first-person transform is built by `first_person::item_pose` out
// of the swing progress the game hands it, so nothing here can replace a
// swing. What is reachable is the swing's own duration (the state the game
// reads the progress from), the use animation (asked every frame), and the one
// additive transform the pose hook allows.

use crate::game::{Game, clickgui as m};
use horizon_world::consumable::UseAnimation;

/// The swing's duration in ticks after `percent`, the reference client's
/// `Swing Speed`: `duration * (1 - percent / 100)`, so 50 is twice as fast and
/// -200 three times as slow. Never below one tick, or the swing would end in
/// the tick it began.
fn swing_ticks(base: i32, percent: f32) -> i32 {
    let factor = 1.0 - f64::from(percent) / 100.0;
    let ticks = (f64::from(base) * factor).round();
    if ticks < 1.0 {
        return 1;
    }
    ticks.min(2400.0) as i32
}

/// `native_tick`: the swing the hand is in, before `tick_hand` advances it.
///
/// The duration is recomputed from the animation and the player's effects
/// every tick instead of being scaled in place, so the setting cannot compound
/// on itself across the six ticks a swing lasts.
pub(super) fn tick(game: &mut Game<'_>) {
    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    let Some(config) = m::animations() else {
        return;
    };
    if config.swing_speed == 0.0 {
        return;
    }
    let effects = &game.potions.effects;
    let dig_speed = effects
        .amplifier("haste")
        .max(effects.amplifier("conduit_power"));
    let fatigue = effects.amplifier("mining_fatigue");
    let Some(current) = game.swing.current.as_mut() else {
        return;
    };
    let base = horizon_client::first_person::swing_duration(current.animation, dig_speed, fatigue);
    current.duration_ticks = swing_ticks(base, config.swing_speed);
}

/// `native_render_hand_pose`: an extra arc on top of the vanilla swing.
///
/// The curve is the game's own (`item_pose` turns the hand by
/// `sin(sqrt(progress) * pi)` about X and Z), and it leans the same way, so the
/// hand travels further along the arc it already takes instead of fighting it.
/// `swing` is the hand's own progress: the off hand has none while the main
/// hand swings.
fn sway(config: &m::AnimationsConfig, swing: f32, pitch: &mut f32, roll: &mut f32, z: &mut f32) {
    let amount = config.sway / 100.0;
    if amount <= 0.0 || swing <= 0.0 {
        return;
    }
    let arc = (swing.clamp(0.0, 1.0).sqrt() * std::f32::consts::PI).sin();
    *pitch -= arc * 18.0 * amount;
    *roll -= arc * 6.0 * amount;
    *z -= arc * 0.04 * amount;
}

/// The pose entry point for the hand hook.
pub(super) fn pose(
    game: &Game<'_>,
    swing: f32,
    pitch: &mut f32,
    roll: &mut f32,
    z: &mut f32,
) {
    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    let Some(config) = m::animations() else {
        return;
    };
    if config.is_vanilla() {
        return;
    }
    sway(&config, swing, pitch, roll, z);
}

/// `native_use_animation`: the pose the hand holds a used item in. `Eat` and
/// `Drink` are the two that bring the item to the mouth; `None` holds it still.
pub(super) fn using(game: &Game<'_>, animation: &mut UseAnimation) {
    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    let Some(config) = m::animations() else {
        return;
    };
    if config.eating {
        return;
    }
    if matches!(*animation, UseAnimation::Eat | UseAnimation::Drink) {
        *animation = UseAnimation::None;
    }
}

#[cfg(test)]
mod tests {
    use super::swing_ticks;

    #[test]
    fn zero_leaves_the_swing_where_the_game_put_it() {
        // Six ticks is the default swing (`SwingAnimation.DEFAULT`).
        assert_eq!(swing_ticks(6, 0.0), 6);
        // Whatever the game computed, haste and fatigue included, is kept.
        for base in 1..=20 {
            assert_eq!(swing_ticks(base, 0.0), base);
        }
    }

    #[test]
    fn the_reference_curve_is_a_percentage_of_the_duration() {
        // 50 halves the swing, the reference's fastest setting.
        assert_eq!(swing_ticks(6, 50.0), 3);
        // -200 triples it, its slowest.
        assert_eq!(swing_ticks(6, -200.0), 18);
        // -50 is half again as slow, +25 a quarter faster.
        assert_eq!(swing_ticks(8, -50.0), 12);
        assert_eq!(swing_ticks(8, 25.0), 6);
    }

    #[test]
    fn a_swing_never_ends_before_it_starts() {
        // A one-tick swing scaled to nothing still lasts a tick, or `tick`
        // would clear it before the hand ever drew it.
        assert_eq!(swing_ticks(1, 50.0), 1);
        assert_eq!(swing_ticks(2, 50.0), 1);
        // The slowest setting on the longest base stays a sane tick count.
        assert_eq!(swing_ticks(20, -200.0), 60);
    }
}
