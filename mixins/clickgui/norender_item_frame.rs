// NoRender's item frames: hide the item or map in a frame when the setting
// is on. The frame itself stays.
fn clickgui_norender_item_frame(game: &crate::game::Game<'_>, canceled: &mut bool) {
    use crate::game::clickgui as m;

    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    if m::norender().is_some_and(|config| config.item_frames) {
        *canceled = true;
    }
}
