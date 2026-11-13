// NoRender's particles: skip the particle engine's culled quads, the path
// the game takes when particle culling is on.
fn clickgui_norender_particles_culled() -> core::ops::ControlFlow<()> {
    if crate::clickgui_norender::particles_hidden() {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
