// A click on the settings page's buttons (0: reset the position, 1: FullBright).
//
// Split out of the `native_settings_click` helper for the same reason as
// `command_impl`: `format!` is a macro and the hook points have to stay free
// of them.
pub(super) fn handle(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use crate::game::{clickgui as m, clickgui_ui as ui};

    if mod_id != m::ID {
        return;
    }
    match button {
        0 => {
            ui::reset_position();
            game.native_chat("StariyBog moved back to the centre");
        }
        1 => {
            let on = ui::toggle_module(game, m::FULLBRIGHT);
            game.native_chat(&format!("FullBright {}", if on { "on" } else { "off" }));
        }
        _ => {}
    }
}
