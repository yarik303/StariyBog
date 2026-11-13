// NoRender's toasts: keep an advancement, recipe or system toast from being
// queued when the setting is on.
fn clickgui_norender_toast(game: &mut crate::game::Game<'_>, cancelled: &mut bool) {
    use crate::game::clickgui as m;

    if !game.native_on(m::ID) {
        return;
    }
    if m::norender().is_some_and(|config| config.toasts) {
        *cancelled = true;
    }
}
