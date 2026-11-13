// Google Sans for the ClickGUI and the ArrayList, as a texture.
//
// `font_data` holds the glyph masks rasterized at 4x the 10px em and packed
// into one coverage atlas. Horizon's HUD atlas samples with nearest filtering
// and a native mod cannot upload a texture of its own, so this module filters
// the atlas to one texel per physical pixel for the frame's GUI scale and
// uploads it into `horizon/ui-text` (the blank sprite the game reserves for
// run-time glyphs), anchored at its bottom-right corner so the game's own
// shelf-packed glyphs, which grow from the top left, do not reach it. Each
// glyph is then one quad.
//
// The sprite is re-uploaded when the GUI scale changes or the atlas is
// replaced (a resource reload allocates the new atlas before dropping the
// old, so its address changes).
use crate::game::{Game, clickgui_font_data as data};
use horizon_client::font::Patch;
use horizon_client::hud::{HudAtlas, HudBatch, Rect, rgba};
use std::cell::RefCell;

/// The em was rasterized at this many times the GUI-pixel size.
const SUPERSAMPLE: u32 = 4;
/// Puts the 10px em on the built-in 8px font's baseline.
const Y_SHIFT: f32 = -2.0;
const SHADOW_OFFSET: f32 = 1.0;
const SHADOW_SHADE: f32 = 0.25;

struct State {
    /// The atlas the upload belongs to.
    atlas: usize,
    /// Physical pixels per GUI pixel the atlas was filtered for.
    scale: u32,
    /// One atlas rect per ASCII 32..126.
    glyphs: Vec<Rect>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub(super) fn width(text: &str) -> f32 {
    let mut pen = 0.0;
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\u{a7}' {
            chars.next();
            continue;
        }
        pen += data::GLYPHS[index(ch)].advance;
    }
    pen
}

pub(super) fn draw(
    game: &Game<'_>,
    batch: &mut HudBatch,
    text: &str,
    at: [f32; 2],
    rgb: u32,
    shadow: bool,
) {
    let [_, r, g, b] = rgb.to_be_bytes();
    let scale = batch.scale;
    if scale <= 0.0 {
        return;
    }
    let density = (scale.round() as u32).clamp(1, SUPERSAMPLE);
    if !ensure(&game.gui.atlas, batch, density) {
        return;
    }
    STATE.with(|state| {
        let state = state.borrow();
        let Some(state) = state.as_ref() else {
            return;
        };
        if shadow {
            let shade = |c: u8| (f32::from(c) * SHADOW_SHADE) as u8;
            pass(
                batch,
                text,
                at,
                scale,
                [shade(r), shade(g), shade(b)],
                SHADOW_OFFSET,
                state,
            );
        }
        pass(batch, text, at, scale, [r, g, b], 0.0, state);
    });
}

fn pass(
    batch: &mut HudBatch,
    text: &str,
    at: [f32; 2],
    scale: f32,
    rgb: [u8; 3],
    offset: f32,
    state: &State,
) {
    let color = rgba(rgb[0], rgb[1], rgb[2], 0xFF);
    let em = SUPERSAMPLE as f32;
    let mut pen = at[0];
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\u{a7}' {
            chars.next();
            continue;
        }
        let i = index(ch);
        let glyph = &data::GLYPHS[i];
        if glyph.aw != 0 {
            let snap = |v: f32| (v * scale).round() / scale;
            let x = snap(pen + glyph.x) + offset;
            let y = snap(at[1] + glyph.y + Y_SHIFT) + offset;
            batch.rect_uv(
                x,
                y,
                glyph.aw as f32 / em,
                glyph.ah as f32 / em,
                state.glyphs[i],
                color,
            );
        }
        pen += glyph.advance;
    }
}

fn index(ch: char) -> usize {
    let code = ch as u32;
    if (32..=126).contains(&code) {
        (code - 32) as usize
    } else {
        ('?' as u32 - 32) as usize
    }
}

/// Filters and uploads the atlas when the scale changed or the atlas was
/// replaced. Returns whether a state is ready.
fn ensure(atlas: &HudAtlas, batch: &mut HudBatch, density: u32) -> bool {
    let id = atlas as *const HudAtlas as usize;
    let ready = STATE.with(|s| {
        s.borrow()
            .as_ref()
            .is_some_and(|s| s.atlas == id && s.scale == density)
    });
    if ready {
        return true;
    }

    let Some(region) = atlas.sprites.get("horizon/ui-text").copied() else {
        return false;
    };
    let (w, h, coverage) = resample(density);
    if w > region.w || h > region.h {
        return false;
    }
    let aw = atlas.width as f32;
    let ah = atlas.height as f32;
    let x = (region.u0 * aw).round() as u32 + region.w - w;
    let y = (region.v0 * ah).round() as u32 + region.h - h;

    let mut pixels = Vec::with_capacity(coverage.len() * 4);
    for alpha in coverage {
        pixels.extend_from_slice(&[255, 255, 255, alpha]);
    }
    batch.patches.push(Patch {
        x,
        y,
        width: w,
        height: h,
        rgba: pixels,
    });

    let rect = Rect {
        u0: x as f32 / aw,
        v0: y as f32 / ah,
        u1: (x + w) as f32 / aw,
        v1: (y + h) as f32 / ah,
        w,
        h,
    };
    let texels = density as f32 / SUPERSAMPLE as f32;
    let glyphs = data::GLYPHS
        .iter()
        .map(|g| {
            let gx = g.ax as f32 * texels;
            let gy = g.ay as f32 * texels;
            Rect {
                u0: rect.u0 + gx / aw,
                v0: rect.v0 + gy / ah,
                u1: rect.u0 + (gx + g.aw as f32 * texels) / aw,
                v1: rect.v0 + (gy + g.ah as f32 * texels) / ah,
                w: u32::from(g.aw),
                h: u32::from(g.ah),
            }
        })
        .collect();
    STATE.with(|s| {
        *s.borrow_mut() = Some(State {
            atlas: id,
            scale: density,
            glyphs,
        })
    });
    true
}

/// Area-averages the 4x atlas down to `density` physical pixels per GUI pixel.
fn resample(density: u32) -> (u32, u32, Vec<u8>) {
    let (sw, sh) = (data::ATLAS_W as usize, data::ATLAS_H as usize);
    if density >= SUPERSAMPLE {
        return (data::ATLAS_W, data::ATLAS_H, data::PIXELS.to_vec());
    }
    let factor = SUPERSAMPLE as f32 / density as f32;
    let ow = (sw as f32 / factor).round() as usize;
    let oh = (sh as f32 / factor).round() as usize;
    let mut out = vec![0u8; ow * oh];
    for oy in 0..oh {
        let y0 = oy as f32 * factor;
        let y1 = (y0 + factor).min(sh as f32);
        for ox in 0..ow {
            let x0 = ox as f32 * factor;
            let x1 = (x0 + factor).min(sw as f32);
            let (mut sum, mut area) = (0.0f32, 0.0f32);
            for sy in y0.floor() as usize..(y1.ceil() as usize).min(sh) {
                let wy = (y1.min(sy as f32 + 1.0) - y0.max(sy as f32)).max(0.0);
                for sx in x0.floor() as usize..(x1.ceil() as usize).min(sw) {
                    let wx = (x1.min(sx as f32 + 1.0) - x0.max(sx as f32)).max(0.0);
                    sum += f32::from(data::PIXELS[sy * sw + sx]) * wx * wy;
                    area += wx * wy;
                }
            }
            if area > 0.0 {
                out[oy * ow + ox] = (sum / area).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    (ow as u32, oh as u32, out)
}
