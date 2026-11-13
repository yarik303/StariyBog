// Google Sans text for the ClickGUI and the ArrayList, the font SixEyes uses.
//
// The rasterized 4x atlas and the upload/draw path live in
// `clickgui_font_atlas`; this is the name the mod's widgets call.
use crate::game::Game;
use horizon_client::hud::HudBatch;

pub(super) fn width(text: &str) -> f32 {
    crate::game::clickgui_font_atlas::width(text)
}

pub(super) fn draw(
    game: &Game<'_>,
    batch: &mut HudBatch,
    text: &str,
    at: [f32; 2],
    rgb: u32,
    shadow: bool,
) {
    crate::game::clickgui_font_atlas::draw(game, batch, text, at, rgb, shadow);
}
