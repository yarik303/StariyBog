// A click on the settings page's buttons (0: reset the position, 1: FullBright).
//
// Only the call is here. The body lives in `clickgui_clicks` because this
// helper is inlined into `native_hooks`, which has to stay free of macros.
fn clickgui_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::clickgui_clicks::handle(game, mod_id, button);
}
