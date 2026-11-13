// NoRender's switches, shared between the game crate (which sets them from the
// module's settings) and the render crate (whose particle engine and screen
// effects read them). The game and the renderer are different crates, and
// neither the engine nor the effect builder has access to the mod registry, so
// one atomic carries each answer.
use std::sync::atomic::{AtomicBool, Ordering};

static PARTICLES: AtomicBool = AtomicBool::new(false);

static WATER: AtomicBool = AtomicBool::new(false);

static FIRE: AtomicBool = AtomicBool::new(false);

pub fn set_particles(hidden: bool) {
    PARTICLES.store(hidden, Ordering::Relaxed);
}

pub fn particles_hidden() -> bool {
    PARTICLES.load(Ordering::Relaxed)
}

pub fn set_water(hidden: bool) {
    WATER.store(hidden, Ordering::Relaxed);
}

pub fn water_hidden() -> bool {
    WATER.load(Ordering::Relaxed)
}

pub fn set_fire(hidden: bool) {
    FIRE.store(hidden, Ordering::Relaxed);
}

pub fn fire_hidden() -> bool {
    FIRE.load(Ordering::Relaxed)
}
