// NoRender's camera overlays: publish the two switches the render crate's
// screen effects read, once per frame, before they are built.
fn clickgui_norender_overlay_publish(game: &crate::game::Game<'_>) {
    use crate::game::clickgui as m;

    let config = game
        .native_on(m::ID)
        .then(m::norender)
        .flatten()
        .unwrap_or_default();
    horizon_client_render::clickgui_norender::set_water(config.water);
    horizon_client_render::clickgui_norender::set_fire(config.fire);
}
