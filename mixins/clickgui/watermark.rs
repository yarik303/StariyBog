// The Watermark: the player's name, FPS and ping in a rounded gradient pill,
// placed in the corner and at the offset its settings pick. The example
// Watermark's layout, without its icon.
use crate::game::{Game, clickgui as m, clickgui_font as font};
use horizon_client::extras::Corner;
use horizon_client::hud::HudBatch;

const H: f32 = 16.0;
const PAD_X: f32 = 5.0;
const RADIUS: f32 = 3.0;
const GAP: f32 = 4.0;
/// The separator dot's side.
const DOT: f32 = 2.0;
/// The space between a number and its label.
const LABEL_GAP: f32 = 1.0;
const TEXT_Y: f32 = 3.0;
/// The client's tag in the pill.
const ROLE: &str = "StariyBog";

const NAME_INK: u32 = 0xFFED_ED_F2;
const ACCENT: u32 = 0xFFA3_9B_D6;
const BRACKET: u32 = 0xFF8D_8C_8F;
const SEPARATOR: u32 = 0xFF4B_4B_50;
/// The pill's gradient, top to bottom.
const TOP: [u8; 3] = [48, 47, 57];
const BOTTOM: [u8; 3] = [17, 17, 17];

pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch) {
    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    let Some(config) = m::watermark() else {
        return;
    };
    let corner = Corner::ALL[config.corner.min(3)];
    if !game.native_corner_shown(corner) {
        return;
    }

    let name = match game.remote.as_ref() {
        Some(remote) => remote.profile_name.clone(),
        None => game.options.player_name(),
    };
    let fps = game.native_frame_rate().0.to_string();
    let ping = game.clickgui_ping().to_string();

    let name_w = font::width(&name);
    let bracket_w = font::width("[");
    let role_w = font::width(ROLE);
    let fps_w = font::width(&fps);
    let pulse_w = font::width("FPS");
    let ping_w = font::width(&ping);
    let ms_w = font::width("MS");
    let w = PAD_X * 2.0
        + name_w
        + bracket_w * 2.0
        + role_w
        + fps_w
        + LABEL_GAP
        + pulse_w
        + ping_w
        + LABEL_GAP
        + ms_w
        + DOT * 3.0
        + GAP * 6.0;

    let left = matches!(corner, Corner::TopLeft | Corner::BottomLeft);
    let top = matches!(corner, Corner::TopLeft | Corner::TopRight);
    let x = if left {
        config.offset_x
    } else {
        batch.width - w - config.offset_x
    }
    .floor();
    let y = if top {
        config.offset_y
    } else {
        batch.height - H - config.offset_y
    }
    .floor();

    // One row per GUI pixel so the gradient stays smooth and the corners round.
    for row in 0..H as i32 {
        let t = (row as f32 + 0.5) / H;
        let argb = 0xFF00_0000
            | lerp(TOP[0], BOTTOM[0], t) << 16
            | lerp(TOP[1], BOTTOM[1], t) << 8
            | lerp(TOP[2], BOTTOM[2], t);
        let inset = row_inset(row as f32, H, RADIUS);
        game.native_fill(
            batch,
            [x + inset, y + row as f32, w - inset * 2.0, 1.0],
            argb,
        );
    }

    let dot_y = y + (H - DOT) * 0.5;
    let text_y = y + TEXT_Y;
    let mut pen = x + PAD_X;
    font::draw(game, batch, &name, [pen, text_y], NAME_INK, config.shadow);
    pen += name_w + GAP;
    game.native_fill(batch, [pen, dot_y, DOT, DOT], SEPARATOR);
    pen += DOT + GAP;
    font::draw(game, batch, "[", [pen, text_y], BRACKET, config.shadow);
    pen += bracket_w;
    font::draw(game, batch, ROLE, [pen, text_y], ACCENT, config.shadow);
    pen += role_w;
    font::draw(game, batch, "]", [pen, text_y], BRACKET, config.shadow);
    pen += bracket_w + GAP;
    game.native_fill(batch, [pen, dot_y, DOT, DOT], SEPARATOR);
    pen += DOT + GAP;
    font::draw(game, batch, &fps, [pen, text_y], NAME_INK, config.shadow);
    pen += fps_w + LABEL_GAP;
    font::draw(game, batch, "FPS", [pen, text_y], ACCENT, config.shadow);
    pen += pulse_w + GAP;
    game.native_fill(batch, [pen, dot_y, DOT, DOT], SEPARATOR);
    pen += DOT + GAP;
    font::draw(game, batch, &ping, [pen, text_y], NAME_INK, config.shadow);
    pen += ping_w + LABEL_GAP;
    font::draw(game, batch, "MS", [pen, text_y], ACCENT, config.shadow);
}

fn lerp(a: u8, b: u8, t: f32) -> u32 {
    (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u32
}

/// How far a one-pixel row is inset at each end inside a corner of radius `r`.
fn row_inset(row: f32, h: f32, r: f32) -> f32 {
    let d = (row + 0.5).min(h - row - 0.5);
    if d >= r {
        0.0
    } else {
        r - (r * r - (r - d) * (r - d)).max(0.0).sqrt()
    }
}
