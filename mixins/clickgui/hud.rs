// The mod's layer in the HUD, drawn after Horizon's own extras and boxes. The
// ArrayList is behind the ClickGUI so the menu's dim covers it.
fn clickgui_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use crate::game::clickgui as m;

    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    crate::game::clickgui_esp::draw(game, batch);
    crate::game::clickgui_tracers::draw(game, batch);
    crate::game::clickgui_nametags::draw(game, batch);
    crate::game::clickgui_watermark::draw(game, batch);
    crate::game::clickgui_arraylist::draw(game, batch, corners);
    crate::game::clickgui_ui::draw(game, batch);
}
