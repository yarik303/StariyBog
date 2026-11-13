// The mod's row in Options > Mods, with its rebindable menu key.
//
// Only the call is here. The body lives in `clickgui_listing` because this
// helper is inlined into `native_hooks`, which has to stay free of macros.
fn clickgui_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::clickgui_listing::push(mods);
}
