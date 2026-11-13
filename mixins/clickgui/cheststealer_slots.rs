// Runs from `draw_carried`: the screen's items are already drawn, the carried
// stack is not, so the highlight and bar sit over the icons.
fn clickgui_cheststealer_slots(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
) {
    crate::game::clickgui_cheststealer::draw(game, batch);
}
