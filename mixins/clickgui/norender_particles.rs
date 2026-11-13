// NoRender's particles: publish the switch the render crate's particle
// engine reads while it submits its geometry.
fn clickgui_norender_particles(game: &mut crate::game::Game<'_>) {
    use crate::game::clickgui as m;

    let hidden = game.native_on(m::ID)
        && game.native_playing()
        && m::norender().is_some_and(|config| config.particles);
    horizon_client_render::clickgui_norender::set_particles(hidden);
}
