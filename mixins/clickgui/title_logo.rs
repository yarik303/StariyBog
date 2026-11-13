// The title screen's logo: the mod's own 1024x256 image, drawn 256x64 GUI
// pixels where the vanilla logo sits.
//
// The HUD atlas samples with nearest filtering and a native mod cannot upload
// a texture of its own, so `title_data`'s PNG is decoded once, area-averaged
// to one texel per physical pixel for the frame's GUI scale, and uploaded into
// `horizon/ui-text` (the blank sprite the game reserves for run-time pixels),
// anchored at its bottom-left corner. The font atlas uses the bottom right, so
// the two never overlap. The image's background is transparent, so the vanilla
// logo's and edition banner's atlas regions are blanked at the same time;
// otherwise they would show through the image. The upload is repeated only when
// the GUI scale changes or a resource reload gives the game a new atlas (it
// allocates the new one before dropping the old, so its address changes).
use super::clickgui_title_data as data;
use crate::font::Patch;
use crate::hud::{HudAtlas, HudBatch, Rect, decode_bytes};
use std::cell::RefCell;
use std::sync::OnceLock;

/// The sprite the vanilla logo is blitted from.
const VANILLA_LOGO: &str = "gui/title/minecraft";

/// `title_data`'s image is this many times the box it is drawn in.
const SUPERSAMPLE: u32 = 4;
/// The vanilla logo's box, in GUI pixels.
const BOX_W: f32 = 256.0;
const BOX_H: f32 = 64.0;
const BOX_Y: f32 = 30.0;

struct State {
    /// The atlas the upload belongs to.
    atlas: usize,
    /// Physical pixels per GUI pixel the image was filtered for.
    scale: u32,
    rect: Rect,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

#[cfg(test)]
pub(super) fn reset() {
    STATE.with(|s| *s.borrow_mut() = None);
}

/// Called before every `HudBatch::text_styled`. The main menu's version line
/// (`"Horizon {version} (Rust)"`, bottom left) is drawn every frame after the
/// logo and the splash, and no other screen draws it, so it is the trigger.
/// It does not depend on the vanilla logo or edition sprites, whose atlas
/// regions may be absent or blank. The splash cannot be the trigger:
/// `Title::draw` captures the batch's length before drawing it and then tilts
/// every vertex from there on, which would throw a quad added in its call
/// off-screen.
pub(super) fn before_text(
    atlas: &HudAtlas,
    batch: &mut HudBatch,
    text: &crate::text::Text,
    y: f32,
) {
    if (y - (batch.height.ceil() - 10.0)).abs() >= 0.5
        || !text.plain().starts_with("Horizon ")
    {
        return;
    }
    draw(atlas, batch, BOX_Y);
}

fn draw(atlas: &HudAtlas, batch: &mut HudBatch, top: f32) {
    let scale = batch.scale;
    if scale <= 0.0 {
        return;
    }
    let density = (scale.round() as u32).clamp(1, SUPERSAMPLE);
    if !ensure(atlas, batch, density) {
        return;
    }
    let x = (batch.width.ceil() / 2.0).floor() - BOX_W * 0.5;
    STATE.with(|state| {
        let state = state.borrow();
        let Some(state) = state.as_ref() else {
            return;
        };
        batch.rect_uv(x, top, BOX_W, BOX_H, state.rect, u32::MAX);
    });
}

/// Filters and uploads the image when the scale changed or the atlas was
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
    let Some((w, h, pixels)) = resample(density) else {
        return false;
    };
    if w > region.w || h > region.h {
        return false;
    }
    let aw = atlas.width as f32;
    let ah = atlas.height as f32;
    let x = (region.u0 * aw).round() as u32;
    let y = (region.v0 * ah).round() as u32 + region.h - h;
    batch.patches.push(Patch {
        x,
        y,
        width: w,
        height: h,
        rgba: pixels,
    });
    // The image has a transparent background, so the vanilla logo and its
    // edition banner would show through it. Blank both in the atlas.
    for name in [VANILLA_LOGO, crate::title_edition::SPRITE] {
        let Some(sprite) = atlas.sprites.get(name).copied() else {
            continue;
        };
        let (sw, sh) = sprite.texels((atlas.width, atlas.height));
        batch.patches.push(Patch {
            x: (sprite.u0 * aw).round() as u32,
            y: (sprite.v0 * ah).round() as u32,
            width: sw,
            height: sh,
            rgba: vec![0; (sw * sh * 4) as usize],
        });
    }

    let rect = Rect {
        u0: x as f32 / aw,
        v0: y as f32 / ah,
        u1: (x + w) as f32 / aw,
        v1: (y + h) as f32 / ah,
        w,
        h,
    };
    STATE.with(|s| {
        *s.borrow_mut() = Some(State {
            atlas: id,
            scale: density,
            rect,
        })
    });
    true
}

/// The decoded image, kept for the process: decoding 181 KB of PNG again on
/// every scale change buys nothing.
fn source() -> Option<&'static (u32, u32, Vec<u8>)> {
    static SOURCE: OnceLock<Option<(u32, u32, Vec<u8>)>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            let (w, h, rgba) = decode_bytes(data::PNG).ok()?;
            if w != data::WIDTH || h != data::HEIGHT || rgba.len() != (w * h * 4) as usize {
                return None;
            }
            Some((w, h, rgba))
        })
        .as_ref()
}

/// The image area-averaged to `density` physical pixels per GUI pixel, with
/// the colour weighted by alpha so transparent texels do not darken edges.
fn resample(density: u32) -> Option<(u32, u32, Vec<u8>)> {
    let (sw, sh, src) = source()?;
    if density >= SUPERSAMPLE {
        return Some((*sw, *sh, src.clone()));
    }
    let (sw, sh) = (*sw as usize, *sh as usize);
    let factor = SUPERSAMPLE as f32 / density as f32;
    let ow = (sw as f32 / factor).round() as usize;
    let oh = (sh as f32 / factor).round() as usize;
    let mut out = vec![0u8; ow * oh * 4];
    for oy in 0..oh {
        let y0 = oy as f32 * factor;
        let y1 = (y0 + factor).min(sh as f32);
        for ox in 0..ow {
            let x0 = ox as f32 * factor;
            let x1 = (x0 + factor).min(sw as f32);
            let (mut r, mut g, mut b, mut a, mut area) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
            for sy in y0.floor() as usize..(y1.ceil() as usize).min(sh) {
                let wy = (y1.min(sy as f32 + 1.0) - y0.max(sy as f32)).max(0.0);
                for sx in x0.floor() as usize..(x1.ceil() as usize).min(sw) {
                    let wx = (x1.min(sx as f32 + 1.0) - x0.max(sx as f32)).max(0.0);
                    let weight = wx * wy;
                    let p = (sy * sw + sx) * 4;
                    let alpha = f32::from(src[p + 3]) * weight;
                    r += f32::from(src[p]) * alpha;
                    g += f32::from(src[p + 1]) * alpha;
                    b += f32::from(src[p + 2]) * alpha;
                    a += alpha;
                    area += weight;
                }
            }
            let o = (oy * ow + ox) * 4;
            if a > 0.0 && area > 0.0 {
                out[o] = (r / a).round().clamp(0.0, 255.0) as u8;
                out[o + 1] = (g / a).round().clamp(0.0, 255.0) as u8;
                out[o + 2] = (b / a).round().clamp(0.0, 255.0) as u8;
                out[o + 3] = (a / area).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Some((ow as u32, oh as u32, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An atlas with the run-time slots but no textures at all: the vanilla
    /// logo and the edition banner are absent, as on a game whose title
    /// sprites did not make it into the atlas.
    fn bare_atlas() -> HudAtlas {
        let atlas = HudAtlas::build(std::path::Path::new(
            "/nonexistent/horizon-test-assets",
        ))
        .unwrap();
        assert!(!atlas.sprites.contains_key(VANILLA_LOGO));
        reset();
        atlas
    }

    #[test]
    fn the_version_line_draws_the_image_without_the_vanilla_sprites() {
        let atlas = bare_atlas();
        let mut batch = HudBatch::new([1920, 1080]);
        let y = batch.height.ceil() - 10.0;
        let version = crate::text::Text::literal("Horizon 26.3 (Rust)");
        before_text(&atlas, &mut batch, &version, y);

        let x = (batch.width.ceil() / 2.0).floor() - BOX_W * 0.5;
        assert_eq!(batch.vertices.len(), 6);
        let v = &batch.vertices;
        assert_eq!(v[0].pos, [x, BOX_Y]);
        assert_eq!(v[2].pos, [x + BOX_W, BOX_Y + BOX_H]);
        assert_eq!(v[3].pos, [x, BOX_Y]);
        assert_eq!(v[5].pos, [x, BOX_Y + BOX_H]);

        assert_eq!(batch.patches.len(), 1, "the image is uploaded once");
        let patch = &batch.patches[0];
        assert_eq!(patch.width * 4, patch.height * 16, "the image stays 4:1");
        assert_eq!(patch.x, 0);
        assert_eq!(patch.y + patch.height, 1024, "anchored at ui-text's bottom");
        assert!(patch.rgba.iter().any(|&b| b != 0), "the image is not blank");
    }

    #[test]
    fn other_text_and_other_positions_do_not_draw_it() {
        let atlas = bare_atlas();
        let mut batch = HudBatch::new([1920, 1080]);
        let y = batch.height.ceil() - 10.0;
        let version = crate::text::Text::literal("Horizon 26.3 (Rust)");
        // The same string elsewhere, and other text on the version line's row.
        before_text(&atlas, &mut batch, &version, 10.0);
        let notice = crate::text::Text::literal("Not affiliated with Mojang or Microsoft");
        before_text(&atlas, &mut batch, &notice, y);
        assert!(batch.vertices.is_empty());
        assert!(batch.patches.is_empty());
    }
}
