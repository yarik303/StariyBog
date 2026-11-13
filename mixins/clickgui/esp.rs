// ESP: a dashed gradient box around players, friends, the player's own body
// and mobs (a port of the client's ESP render module). The boxes are projected
// with the same camera NameTags uses and drawn as HUD quads, so they show
// through the world like the reference's line layer.
//
// The colour follows the entity: friends use their own colour, everyone else
// gets a slowly cycling hue offset by entity id (the reference's animated
// theme colour). A hurt entity fades to red over ten ticks; the whole box
// fades with distance down to a fifth of its alpha at the maximum range.
use crate::game::{Game, clickgui as m, clickgui_nametags::View};
use horizon_client::hud::{HudAtlas, HudBatch};
use horizon_client::mobs::Mob;
use horizon_core::geometry::{Aabb, Axis, Vec3};

/// `HURT_COLOR` and `HURT_FADE_TICKS`.
const HURT_COLOR: u32 = 0xFFFF_2020;
const HURT_FADE_TICKS: f32 = 10.0;
/// `PLAYER_BOX_PADDING` and `MOB_BOX_PADDING`.
const PLAYER_PADDING: f64 = 0.09;
const MOB_PADDING: f64 = 0.06;
/// `BRIGHT_RGB_SCALE`, `MID_RGB_SCALE` and `DARK_RGB_SCALE`.
const BRIGHT_SCALE: f32 = 1.086_956_5;
const MID_SCALE: f32 = 0.8;
const DARK_SCALE: f32 = 0.62;
/// `MIN_DISTANCE_ALPHA_FACTOR`.
const MIN_ALPHA: f32 = 0.2;
/// The reference's 96-entity cap.
const MAX_ENTITIES: usize = 96;
/// `DASH_STRIDE` and `DASH_LENGTH` (blocks), and the line width in GUI pixels.
const DASH_STRIDE: f64 = 0.24;
const DASH_LENGTH: f64 = 0.18;
const WIDTH: f32 = 1.5;
/// How fast the animated hue turns, in degrees per tick.
const HUE_PER_TICK: f32 = 1.5;

pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch) {
    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    let Some(config) = m::esp() else {
        return;
    };
    if !config.outline {
        return;
    }
    let Some(player) = game.player.as_ref() else {
        return;
    };
    let Some(view) = View::from_game(game, batch) else {
        return;
    };
    let atlas = &game.gui.atlas;
    let eye = player.position;
    let time = game.world.game_time.max(0) as u64;
    let alpha = game.clock.camera_fraction();
    let max_distance = f64::from(config.max_distance);
    let own_id = game.remote.as_ref().map_or(-1, |remote| remote.entity_id);
    let remote = game.remote_player_names();
    let mut drawn = 0usize;

    // The other players a server sends.
    if config.players || config.friends {
        for (id, name) in &remote {
            if drawn >= MAX_ENTITIES {
                break;
            }
            let friend = m::is_friend(&config.friend_names, name);
            if (friend && !config.friends) || (!friend && !config.players) {
                continue;
            }
            let Some(mob) = game.mobs.iter().find(|m| m.id == *id) else {
                continue;
            };
            let base = if friend {
                config.friend_color
            } else {
                animated(time, *id)
            };
            if append_mob(
                batch,
                atlas,
                &view,
                mob,
                alpha,
                eye,
                max_distance,
                base,
                PLAYER_PADDING,
            ) {
                drawn += 1;
            }
        }
    }

    // The players this game hosts over LAN.
    if let Some(host) = &game.lan {
        for guest in host.hub.guests.iter().filter(|g| host.hub.here(g)) {
            if drawn >= MAX_ENTITIES {
                break;
            }
            let friend = m::is_friend(&config.friend_names, &guest.name);
            if (friend && !config.friends) || (!friend && !config.players) {
                continue;
            }
            let base = if friend {
                config.friend_color
            } else {
                animated(time, guest.entity_id as u32)
            };
            if append_guest(
                batch,
                atlas,
                &view,
                guest,
                alpha,
                eye,
                max_distance,
                base,
                PLAYER_PADDING,
            ) {
                drawn += 1;
            }
        }
    }

    // The player's own body, in third person only.
    if config.self_body
        && !game.camera_type.first_person()
        && let Some(mob) = &game.player_model
        && drawn < MAX_ENTITIES
        && append_mob(
            batch,
            atlas,
            &view,
            mob,
            alpha,
            eye,
            max_distance,
            animated(time, mob.id),
            PLAYER_PADDING,
        )
    {
        drawn += 1;
    }

    // The mobs and the other living entities, minus the players above.
    if config.mobs {
        for mob in &game.mobs {
            if drawn >= MAX_ENTITIES {
                break;
            }
            if mob.id as i32 == own_id || remote.iter().any(|(id, _)| *id == mob.id) {
                continue;
            }
            if append_mob(
                batch,
                atlas,
                &view,
                mob,
                alpha,
                eye,
                max_distance,
                animated(time, mob.id),
                MOB_PADDING,
            ) {
                drawn += 1;
            }
        }
    }
}

/// Appends one entity's dashed box, unless it is dead, removed, behind the
/// camera or beyond `max_distance`. Returns whether it drew.
#[allow(clippy::too_many_arguments)]
/// The same box for a LAN guest, whose body the host keeps in its hub
/// instead of in the mob list.
fn append_guest(
    batch: &mut HudBatch,
    atlas: &HudAtlas,
    view: &View,
    guest: &horizon_client::lan::guest::Guest,
    alpha: f64,
    eye: Vec3,
    max_distance: f64,
    base: u32,
    padding: f64,
) -> bool {
    if guest.dead {
        return false;
    }
    let position = Vec3::new(
        lerp(guest.previous_position[0], guest.position[0], alpha),
        lerp(guest.previous_position[1], guest.position[1], alpha),
        lerp(guest.previous_position[2], guest.position[2], alpha),
    );
    let distance = eye.vector_to(position).length();
    if distance > max_distance {
        return false;
    }
    let dimensions = guest.dimensions();
    let half = f64::from(dimensions.width) / 2.0;
    let bounds = Aabb::new(
        Vec3::new(position.x - half, position.y, position.z - half),
        Vec3::new(
            position.x + half,
            position.y + f64::from(dimensions.height),
            position.z + half,
        ),
    )
    .inflate(Vec3::new(padding, padding, padding));
    let fade = 1.0 - (distance / max_distance).clamp(0.0, 1.0);
    let alpha_byte = (fade.clamp(f64::from(MIN_ALPHA), 1.0) * 255.0).round() as u8;
    let corners = [
        with_alpha(scale_rgb(base, BRIGHT_SCALE), alpha_byte),
        with_alpha(scale_rgb(base, MID_SCALE), alpha_byte),
        with_alpha(scale_rgb(base, DARK_SCALE), alpha_byte),
        with_alpha(scale_rgb(base, MID_SCALE), alpha_byte),
    ];
    outline(batch, atlas, view, bounds, corners);
    true
}

fn append_mob(
    batch: &mut HudBatch,
    atlas: &HudAtlas,
    view: &View,
    mob: &Mob,
    alpha: f64,
    eye: Vec3,
    max_distance: f64,
    base: u32,
    padding: f64,
) -> bool {
    if !mob.alive() || mob.removed {
        return false;
    }
    let position = Vec3::new(
        lerp(mob.previous.x, mob.position.x, alpha),
        lerp(mob.previous.y, mob.position.y, alpha),
        lerp(mob.previous.z, mob.position.z, alpha),
    );
    let distance = eye.vector_to(position).length();
    if distance > max_distance {
        return false;
    }
    let bounds = mob
        .bounds_at(position)
        .inflate(Vec3::new(padding, padding, padding));
    let fade = 1.0 - (distance / max_distance).clamp(0.0, 1.0);
    let alpha_byte = (fade.clamp(f64::from(MIN_ALPHA), 1.0) * 255.0).round() as u8;
    let color = if mob.hurt_time > 0 {
        let hurt = ((mob.hurt_time as f32) - alpha as f32) / HURT_FADE_TICKS;
        if hurt > 0.0 {
            blend(base, HURT_COLOR, hurt.clamp(0.0, 1.0))
        } else {
            base
        }
    } else {
        base
    };
    let corners = [
        with_alpha(scale_rgb(color, BRIGHT_SCALE), alpha_byte),
        with_alpha(scale_rgb(color, MID_SCALE), alpha_byte),
        with_alpha(scale_rgb(color, DARK_SCALE), alpha_byte),
        with_alpha(scale_rgb(color, MID_SCALE), alpha_byte),
    ];
    outline(batch, atlas, view, bounds, corners);
    true
}

/// The twelve edges in the reference's order: bottom, top, then the four
/// vertical corners.
fn outline(batch: &mut HudBatch, atlas: &HudAtlas, view: &View, bounds: Aabb, corners: [u32; 4]) {
    let min = [
        bounds.min(Axis::X),
        bounds.min(Axis::Y),
        bounds.min(Axis::Z),
    ];
    let max = [
        bounds.max(Axis::X),
        bounds.max(Axis::Y),
        bounds.max(Axis::Z),
    ];
    let (x0, y0, z0) = (min[0], min[1], min[2]);
    let (x1, y1, z1) = (max[0], max[1], max[2]);
    let edges: [([f64; 3], [f64; 3], usize, usize); 12] = [
        ([x0, y0, z0], [x1, y0, z0], 0, 1),
        ([x1, y0, z0], [x1, y0, z1], 1, 2),
        ([x1, y0, z1], [x0, y0, z1], 2, 3),
        ([x0, y0, z1], [x0, y0, z0], 3, 0),
        ([x0, y1, z0], [x1, y1, z0], 0, 1),
        ([x1, y1, z0], [x1, y1, z1], 1, 2),
        ([x1, y1, z1], [x0, y1, z1], 2, 3),
        ([x0, y1, z1], [x0, y1, z0], 3, 0),
        ([x0, y0, z0], [x0, y1, z0], 0, 0),
        ([x1, y0, z0], [x1, y1, z0], 1, 1),
        ([x1, y0, z1], [x1, y1, z1], 2, 2),
        ([x0, y0, z1], [x0, y1, z1], 3, 3),
    ];
    for (start, end, from, to) in edges {
        dashed(batch, atlas, view, start, end, corners[from], corners[to]);
    }
}

/// One edge, split into dashes whose colour walks from `from` to `to`. The
/// dash lengths are measured in blocks and scaled to the projected length.
fn dashed(
    batch: &mut HudBatch,
    atlas: &HudAtlas,
    view: &View,
    start: [f64; 3],
    end: [f64; 3],
    from: u32,
    to: u32,
) {
    let Some((a, b)) = view.project_segment(start, end) else {
        return;
    };
    let length = screen_length(a, b);
    if length < 0.5 {
        return;
    }
    let world = {
        let delta = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
        (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt()
    };
    if world < 0.001 {
        return;
    }
    let scale = f64::from(length) / world;
    let stride = DASH_STRIDE * scale;
    let dash = DASH_LENGTH * scale;
    let length = f64::from(length);
    let mut along = 0.0;
    while along < length {
        let stop = (along + dash).min(length);
        if stop > along {
            let colour = blend(from, to, ((along + stop) * 0.5 / length) as f32);
            let at = |distance: f64| {
                let t = (distance / length) as f32;
                [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
            };
            line(batch, atlas, at(along), at(stop), WIDTH, hud(colour));
        }
        along += stride;
    }
}

/// A thick line segment between two GUI-pixel points.
pub(super) fn line(
    batch: &mut HudBatch,
    atlas: &HudAtlas,
    a: [f32; 2],
    b: [f32; 2],
    width: f32,
    color: u32,
) {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let length = (dx * dx + dy * dy).sqrt();
    if length < 0.01 {
        return;
    }
    let nx = -dy / length * width * 0.5;
    let ny = dx / length * width * 0.5;
    batch.fill_quad(
        atlas,
        [
            [a[0] + nx, a[1] + ny],
            [b[0] + nx, b[1] + ny],
            [b[0] - nx, b[1] - ny],
            [a[0] - nx, a[1] - ny],
        ],
        color,
    );
}

/// An ARGB colour as the HUD vertex format (`0xAABBGGRR`).
pub(super) fn hud(color: u32) -> u32 {
    let [a, r, g, b] = color.to_be_bytes();
    horizon_client::hud::rgba(r, g, b, a)
}

fn screen_length(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
}

fn lerp(previous: f64, current: f64, alpha: f64) -> f64 {
    previous + (current - previous) * alpha
}

/// The entity's hue at this game time, fully saturated (the reference's
/// animated theme colour, offset by entity id).
fn animated(time: u64, id: u32) -> u32 {
    let hue = (time as f32 * HUE_PER_TICK + id as f32 * 17.0).rem_euclid(360.0);
    hsv(hue, 1.0, 1.0)
}

fn hsv(hue: f32, saturation: f32, value: f32) -> u32 {
    let h = hue.rem_euclid(360.0) / 60.0;
    let sector = h.floor() as i32;
    let f = h - h.floor();
    let p = value * (1.0 - saturation);
    let q = value * (1.0 - saturation * f);
    let t = value * (1.0 - saturation * (1.0 - f));
    let (r, g, b) = match sector {
        0 => (value, t, p),
        1 => (q, value, p),
        2 => (p, value, t),
        3 => (p, q, value),
        4 => (t, p, value),
        _ => (value, p, q),
    };
    let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u32;
    0xFF00_0000 | byte(r) << 16 | byte(g) << 8 | byte(b)
}

fn with_alpha(color: u32, alpha: u8) -> u32 {
    (color & 0x00FF_FFFF) | (u32::from(alpha) << 24)
}

fn scale_rgb(color: u32, scale: f32) -> u32 {
    let channel = |shift: u32| {
        let value = ((color >> shift) & 0xFF) as f32 * scale;
        (value.round() as u32).min(255)
    };
    (color & 0xFF00_0000) | channel(16) << 16 | channel(8) << 8 | channel(0)
}

fn blend(from: u32, to: u32, t: f32) -> u32 {
    let channel = |shift: u32| {
        let a = ((from >> shift) & 0xFF) as f32;
        let b = ((to >> shift) & 0xFF) as f32;
        (a + (b - a) * t).round().clamp(0.0, 255.0) as u32
    };
    channel(24) << 24 | channel(16) << 16 | channel(8) << 8 | channel(0)
}
