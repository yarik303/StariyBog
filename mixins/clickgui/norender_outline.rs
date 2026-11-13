// NoRender's glowing: take away every entity's glowing outline when the
// setting is on.
fn clickgui_norender_outline(game: &crate::game::Game<'_>, outline: &mut Option<u32>) {
    use crate::game::clickgui as m;

    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    if m::norender().is_some_and(|config| config.glowing) {
        *outline = None;
    }
}
