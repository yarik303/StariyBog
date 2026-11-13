// The mod's settings page: move the window back to the centre, or turn
// FullBright on and off. The menu key is listed above these buttons.
//
// Only the call is here. The body lives in `clickgui_settings` because this
// helper is inlined into `native_hooks`, which has to stay free of macros.
fn clickgui_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::clickgui_settings::handle(game, mod_id, buttons);
}
