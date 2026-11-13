// The mod's settings page: move the window back to the centre, or turn
// FullBright on and off. The menu key is listed above these buttons.
//
// The body lives here rather than in the `native_settings_page` hook point:
// those helpers are inlined into `native_hooks`, and `format!` contains `!(`,
// which Horizon's `hook_points_stay_hookable` reads as a macro in a hook point.
pub(super) fn handle(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    use crate::game::clickgui as m;

    let _ = game;
    if mod_id != m::ID {
        return;
    }
    buttons.push("Reset StariyBog position".into());
    buttons.push(format!(
        "FullBright: {}",
        if m::is_enabled(m::FULLBRIGHT) {
            "ON"
        } else {
            "OFF"
        }
    ));
}
