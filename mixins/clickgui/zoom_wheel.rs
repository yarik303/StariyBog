// While the ClickGUI Zoom module's key is held, the wheel zooms instead of
// moving the hotbar.
fn clickgui_zoom_wheel(game: &crate::game::Game<'_>, steps: f32, claimed: &mut bool) {
    use crate::game::clickgui as m;

    if game.native_on(m::ID) && m::zoom_wheel(steps) {
        *claimed = true;
    }
}
