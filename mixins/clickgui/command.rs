// /clickgui [open|close|reset], /fullbright [on|off], /bind <module> <key> and
// /config [save|load|list|path] [name].
//
// Only the call is here. The body lives in `clickgui_commands` because this
// helper is inlined into `native_hooks`, which has to stay free of macros —
// `format!` is one, and Horizon's `hook_points_stay_hookable` asserts it.
fn clickgui_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::clickgui_commands::handle(game, command, ran);
}
