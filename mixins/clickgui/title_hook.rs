// Draws the mod's logo 256x64 GUI pixels over the vanilla logo's box when the
// main menu's version line is drawn, the first text after the splash.
fn title_logo(
    batch: &mut crate::hud::HudBatch,
    atlas: &crate::hud::HudAtlas,
    text: &crate::text::Text,
    y: f32,
) {
    crate::hud::clickgui_title_logo::before_text(atlas, batch, text, y);
}
