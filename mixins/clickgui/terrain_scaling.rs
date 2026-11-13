// The Speed module's terrain mode scales the player's travel input before the
// movement tick, the way Alan's StrafeEvent changes the movement friction:
// vanilla acceleration is the input scaled by the friction speed, so scaling
// the input scales that acceleration. The Fly modes that set their own
// horizontal velocity zero the input instead, so the physics does not add
// vanilla acceleration on top of their motion. Both factors last one game tick.
use horizon_core::geometry::Vec3;
use std::cell::Cell;

const MIN_FACTOR: f32 = 0.0;
const MAX_FACTOR: f32 = 8.0;

thread_local! {
    static FACTOR: Cell<f32> = const { Cell::new(1.0) };
    /// The Fly modes' input gate: 1 keeps it, 0 removes it.
    static FLY: Cell<f32> = const { Cell::new(1.0) };
}

/// Starts a tick at the unmodified movement speed.
pub fn begin_tick() {
    FACTOR.with(|factor| factor.set(1.0));
}

/// Starts a tick with the Fly input gate open.
pub fn begin_fly_tick() {
    FLY.with(|factor| factor.set(1.0));
}

/// Removes this tick's travel input, for the Fly modes that set their own
/// horizontal velocity.
pub fn block_fly_input() {
    FLY.with(|factor| factor.set(0.0));
}

fn fly_factor() -> f32 {
    FLY.with(Cell::get)
}

/// Multiplies this tick's movement acceleration by `factor`.
pub fn multiply(factor: f32) {
    if factor.is_finite() {
        FACTOR.with(|current| {
            current.set((current.get() * factor).clamp(MIN_FACTOR, MAX_FACTOR));
        });
    }
}

pub fn factor() -> f32 {
    FACTOR.with(Cell::get)
}

/// `MotionWorld::dry_tick`'s input, scaled on the horizontal axes.
pub fn scale_input(input: Vec3) -> Vec3 {
    let factor = f64::from(factor() * fly_factor());
    Vec3::new(input.x * factor, input.y, input.z * factor)
}
