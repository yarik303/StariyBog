// The ArrayList: the enabled modules stacked in the top-right corner.
//
// The box reserves its full height with `Corners::place`, so another overlay in
// the same corner (status effects are already reserved by the caller) does not
// land on it. The list is drawn before the ClickGUI, so it dims with the world
// while the menu is open.
use crate::game::{Game, clickgui as m, clickgui_font as font};
use horizon_client::{extras::Corner, extras_hud::Corners, hud::HudBatch};

const ROW_H: f32 = 12.0;
const PAD_X: f32 = 4.0;
/// The accent stripe on the right edge, in GUI pixels.
const BAR_W: f32 = 2.0;
const INK: u32 = 0xFFED_ED_F2;
const ACCENT: u32 = 0xFF9B_7BFF;
const BACKGROUND: u32 = 0x9010_1018;

pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch, corners: &mut Corners) {
    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    let Some((config, names)) = m::arraylist() else {
        return;
    };
    if names.is_empty() || !game.native_corner_shown(Corner::TopRight) {
        return;
    }

    let text_w = names
        .iter()
        .map(|name| font::width(name))
        .fold(0.0, f32::max);
    let w = text_w + PAD_X * 2.0 + BAR_W;
    let h = ROW_H * names.len() as f32;
    let (x, y) = corners.place(Corner::TopRight, w, h);

    if config.background {
        game.native_fill(batch, [x, y, w, h], BACKGROUND);
        game.native_fill(batch, [x + w - BAR_W, y, BAR_W, h], ACCENT);
    }
    for (i, name) in names.iter().enumerate() {
        let row_y = y + i as f32 * ROW_H;
        let text_x = x + w - BAR_W - PAD_X - font::width(name);
        font::draw(game, batch, name, [text_x, row_y + 1.0], INK, config.shadow);
    }
}
