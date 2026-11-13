// NoRender's HUD layers: cancel a vanilla layer (the crosshair, a boss bar,
// the scoreboard, a title...) when its setting is on.
fn clickgui_norender_layer(game: &mut crate::game::Game<'_>, layer: &str, canceled: &mut bool) {
    use crate::game::clickgui as m;

    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    if m::norender().is_some_and(|config| config.hides_layer(layer)) {
        *canceled = true;
    }
}
