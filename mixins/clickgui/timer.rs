// The clickgui timer: the multiplier `Timer::advance_game_time` scales its
// per-frame delta ticks by. It follows `DeltaTracker.Timer`: the frame's delta
// ticks are scaled before they are accumulated, so the world, partial ticks and
// movement all run at the requested speed.
//
// Five sources add up. The Speed modes ask for a value for one game tick: the
// Speed tick resets that request and the mode asks for what it needs, so frames
// between ticks use the last tick's value, the way a client timer does. The
// Velocity modes' timer tricks (Spoof, GrimC03), the Step modes' step timers and
// the Fly modes' timers ask the same way through their own one-tick request.
// The Timer module sets its own multiplier every tick and keeps it until the
// next one. Only the clock's delta ticks are scaled; its virtual time, UI and
// render frame times keep their real length.
use std::cell::Cell;

/// The multiplier range. A timer below it would stall the clock, and far above
/// it the ten-ticks-a-frame clamp already bounds what a frame can run.
const MIN_MULTIPLIER: f32 = 0.1;
const MAX_MULTIPLIER: f32 = 15.0;

thread_local! {
    /// The Speed mode's request for this tick.
    static SPEED: Cell<f32> = const { Cell::new(1.0) };
    /// The Velocity mode's request for this tick.
    static VELOCITY: Cell<f32> = const { Cell::new(1.0) };
    /// The Step mode's request for this tick.
    static STEP: Cell<f32> = const { Cell::new(1.0) };
    /// The Fly mode's request for this tick.
    static FLY: Cell<f32> = const { Cell::new(1.0) };
    /// The Timer module's multiplier.
    static MODULE: Cell<f32> = const { Cell::new(1.0) };
}

fn clamp(multiplier: f32) -> f32 {
    multiplier.clamp(MIN_MULTIPLIER, MAX_MULTIPLIER)
}

/// Starts a tick with the Speed modes' request off.
pub fn begin_tick() {
    SPEED.with(|multiplier| multiplier.set(1.0));
}

/// Turns the Speed request off now (Speed disabled, or no active game).
pub fn reset() {
    begin_tick();
}

/// A Speed mode's request for this tick. Non-finite values leave it off.
pub fn request(multiplier: f32) {
    if multiplier.is_finite() {
        SPEED.with(|current| current.set(clamp(multiplier)));
    }
}

/// Starts a tick with the Velocity modes' request off.
pub fn begin_velocity_tick() {
    VELOCITY.with(|multiplier| multiplier.set(1.0));
}

/// A Velocity mode's request for this tick. Non-finite values leave it off.
pub fn request_velocity(multiplier: f32) {
    if multiplier.is_finite() {
        VELOCITY.with(|current| current.set(clamp(multiplier)));
    }
}

/// Starts a tick with the Step modes' request off.
pub fn begin_step_tick() {
    STEP.with(|multiplier| multiplier.set(1.0));
}

/// A Step mode's request for this tick. Non-finite values leave it off.
pub fn request_step(multiplier: f32) {
    if multiplier.is_finite() {
        STEP.with(|current| current.set(clamp(multiplier)));
    }
}

/// Starts a tick with the Fly modes' request off.
pub fn begin_fly_tick() {
    FLY.with(|multiplier| multiplier.set(1.0));
}

/// A Fly mode's request for this tick. Non-finite values leave it off.
pub fn request_fly(multiplier: f32) {
    if multiplier.is_finite() {
        FLY.with(|current| current.set(clamp(multiplier)));
    }
}

/// The Timer module's multiplier; 1.0 turns it off. Non-finite values do too.
pub fn set_module(multiplier: f32) {
    MODULE.with(|current| {
        current.set(if multiplier.is_finite() {
            clamp(multiplier)
        } else {
            1.0
        })
    });
}

/// Every source together, which is what the clock runs at.
pub fn multiplier() -> f32 {
    clamp(
        SPEED.with(Cell::get)
            * VELOCITY.with(Cell::get)
            * STEP.with(Cell::get)
            * FLY.with(Cell::get)
            * MODULE.with(Cell::get),
    )
}

/// `Timer::advance_game_time`'s delta ticks, scaled for the current request.
pub fn scale_delta(delta: f32) -> f32 {
    delta * multiplier()
}
