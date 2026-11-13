// The mod's row in Options > Mods, with its rebindable menu key.
//
// Split out of the `native_mods` helper for the same reason as `command_impl`:
// `format!` is a macro and the hook points have to stay free of them.
pub(super) fn push(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::clickgui as m;

    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.into(),
        title: format!("StariyBog {}", m::VERSION),
        description: "Right Shift opens the StariyBog menu. Render has FullBright and the ArrayList."
            .into(),
        keys: vec![[m::KEY.into(), "StariyBog".into(), m::DEFAULT_KEY.into()]],
    });
}
