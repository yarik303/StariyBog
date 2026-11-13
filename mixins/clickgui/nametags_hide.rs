// Cancel Horizon's own name tags (players, named mobs and holograms) while
// NameTags is on, so the HUD layer is the only tag source, and while
// NoRender's name tags setting is on. Enters the cancellable HEAD of
// `Game::name_tag_geometry`, which never calls the per-tag native hook on a
// mixin mod (its `hooks` flag counts runtime mods only). With the HUD hidden
// (F1) nothing draws the mod's tags, so the vanilla ones stay for NameTags.
fn clickgui_nametags_hide(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    use crate::game::clickgui as m;

    if !game.native_on(m::ID) || !game.native_playing() {
        return core::ops::ControlFlow::Continue(());
    }
    // NoRender hides every tag, HUD or not.
    if m::norender().is_some_and(|config| config.nametags) {
        return core::ops::ControlFlow::Break(());
    }
    if !game.ui.hide_gui && m::nametags().is_some() {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
