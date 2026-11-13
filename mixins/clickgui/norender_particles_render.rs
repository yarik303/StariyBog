// NoRender's particles: skip the particle engine's camera-facing quads.
fn clickgui_norender_particles_render() -> core::ops::ControlFlow<()> {
    if crate::clickgui_norender::particles_hidden() {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
