// Chams: entity bodies seen through walls.
//
// Horizon shades an entity's own model with its own texture, and no hook
// reaches that shading, so a mod cannot recolour the model itself. Two things
// are within reach, and Chams offers each as a mode:
//
// * `native_entity_outline`: the model's own silhouette, which the game draws
//   through walls and edges with its outline pass, in the colour asked for.
// * `native_world_primitives`: filled boxes, the only filled shape that hook
//   draws, assembled into the Minecraft player model's own boxes.
//
// A box cannot turn, so the figure turns the way the model does by moving its
// parts along the body's own axis: the arms and legs are square in plan and
// sit where the turn puts them, and the torso, the one part that is not
// square, takes the box its turned footprint needs. The outline keeps the
// model's true shape.
use crate::game::{Game, clickgui as m};
use horizon_client::mobs::{Mob, ai::is_monster, living::wrap};
use horizon_client::world_primitives::WorldPrimitives;
use horizon_core::geometry::Axis;

/// The figure is the Minecraft player model, which is itself six boxes: two
/// legs, a torso, two arms and a head. These are the model's own proportions.
///
/// * `MODEL_WIDTH` is the model's width as a fraction of its height. The parts
///   are one block wide over two blocks tall, and the model is wider than the
///   hitbox because the arms hang outside the body.
/// * `LEGS_H` and `TORSO_H` are the legs and torso as fractions of the height;
///   the head takes what is left, a quarter of it.
/// * `TORSO_W` is the torso as a fraction of the model's width, `LIMB_W` an arm
///   or a leg, and the two arms fill the width beside the torso.
/// * `BODY_D` and `HEAD_D` are their depths, again of the model's width.
///
/// There is no gap between neighbouring parts: the model has none, and a gap
/// would pull the arms off the body.
const MODEL_WIDTH: f64 = 0.5;
const LEGS_H: f64 = 0.375;
const TORSO_H: f64 = 0.375;
const TORSO_W: f64 = 0.5;
const LIMB_W: f64 = 0.25;
const BODY_D: f64 = 0.25;
const HEAD_D: f64 = 0.5;

/// The model's own units: 16 pixels to a block, 32 pixels from the feet to the
/// top of the head.
const PIXELS_PER_BLOCK: f64 = 16.0;
const MODEL_HEIGHT_PX: f64 = 32.0;

/// `AvatarRenderer.scale`: a player's model is drawn a sixteenth smaller than
/// its pixels say, so 32 px at scale 1 cover 1.875 blocks, not 2. This is why
/// the figure cannot be sized from the hitbox — a player's box is 1.8 tall and
/// 0.6 wide, while the model it draws is 1.875 by 0.9375.
const AVATAR_SCALE: f64 = 0.9375;

/// `minecraft:player`, the one kind the players switch owns.
const PLAYER: &str = "minecraft:player";
/// `minecraft:mannequin`, drawn by the same avatar model and scale.
const MANNEQUIN: &str = "minecraft:mannequin";

/// Adds every targeted entity's figure to the frame, `partial` ticks past the
/// last tick, seen from `camera`.
pub(super) fn draw(
    game: &Game<'_>,
    partial: f32,
    camera: [f64; 3],
    primitives: &mut WorldPrimitives,
) {
    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    let Some(config) = m::chams() else {
        return;
    };
    if !config.mode.fills() {
        return;
    }
    let alpha = config.color >> 24;
    if alpha == 0 {
        return;
    }
    let rgb = config.color & 0x00FF_FFFF;
    let own_id = game.remote.as_ref().map_or(0, |remote| remote.entity_id);
    for mob in &game.mobs {
        // The player's own body is the game's separate model, drawn below.
        if mob.id as i32 == own_id || !targets(&config, mob) {
            continue;
        }
        push_figure(primitives, mob, partial, camera, &config, alpha, rgb);
    }
    // `player_model` is the local player's body in third person; first person
    // has no body to draw.
    if config.self_body
        && !game.camera_type.first_person()
        && let Some(mob) = &game.player_model
    {
        push_figure(primitives, mob, partial, camera, &config, alpha, rgb);
    }
    // End crystals are not mobs: they live in their own list, the outline hook
    // never sees them, and they have no model to copy. Their own bounds are the
    // whole shape, so `Filled` and `Both` draw them and `Outline` cannot.
    if config.crystals {
        let color = (alpha << 24) | rgb;
        for crystal in &game.crystals {
            let bounds = crystal.bounds();
            let centre = [
                (bounds.min(Axis::X) + bounds.max(Axis::X)) * 0.5,
                (bounds.min(Axis::Y) + bounds.max(Axis::Y)) * 0.5,
                (bounds.min(Axis::Z) + bounds.max(Axis::Z)) * 0.5,
            ];
            if !in_range(camera, &config, centre) {
                continue;
            }
            push_box(
                primitives,
                [
                    bounds.min(Axis::X),
                    bounds.min(Axis::Y),
                    bounds.min(Axis::Z),
                ],
                [
                    bounds.max(Axis::X),
                    bounds.max(Axis::Y),
                    bounds.max(Axis::Z),
                ],
                &config,
                color,
            );
        }
    }
}

/// Asks the game for `entity_id`'s outline in the Chams colour: its own model's
/// silhouette, drawn through walls.
pub(super) fn outline(game: &Game<'_>, entity_id: u32, _kind: &str, outline: &mut Option<u32>) {
    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    let Some(config) = m::chams() else {
        return;
    };
    if !config.mode.outlines() {
        return;
    }
    let own_id = game.remote.as_ref().map_or(0, |remote| remote.entity_id);
    if entity_id as i32 == own_id {
        // The local player's own body: the game asks with the remote entity id,
        // while `player_model` itself is id 0. `Self` owns it, not `Players`.
        if !config.self_body {
            return;
        }
        if let Some(player) = game.player.as_ref()
            && !near(game, &config, player.position.components())
        {
            return;
        }
        *outline = Some(config.color & 0x00FF_FFFF);
        return;
    }
    let Some(mob) = game.mobs.iter().find(|m| m.id == entity_id) else {
        // A dropped item or a projectile: the reference leaves those alone.
        return;
    };
    if !targets(&config, mob) || !near(game, &config, mob.position.components()) {
        return;
    }
    *outline = Some(config.color & 0x00FF_FFFF);
}

/// Whether Chams draws `mob`: alive, not hidden by the invisibles switch, and
/// in one of the switches its kind belongs to.
fn targets(config: &m::ChamsConfig, mob: &Mob) -> bool {
    if mob.death_time > 0 {
        return false;
    }
    if mob.invisible && !config.invisibles {
        return false;
    }
    // A mannequin is drawn by the avatar model and treated as a player
    // everywhere else in the client (`mobs::render`, `living`, `riders`), so it
    // answers to the same switch rather than falling through to `Animals` —
    // which the README defines as everything else *alive*, and a mannequin is
    // not.
    if avatar(mob) {
        return config.players;
    }
    if is_monster(mob.species()) {
        config.monsters
    } else {
        config.animals
    }
}

/// Whether `at` is within the configured range of the camera. The outline hook
/// is handed no camera, so the frame's own is read instead.
fn near(game: &Game<'_>, config: &m::ChamsConfig, at: [f64; 3]) -> bool {
    let Some(camera) = camera_at(game) else {
        return true;
    };
    let (dx, dy, dz) = (at[0] - camera[0], at[1] - camera[1], at[2] - camera[2]);
    (dx * dx + dy * dy + dz * dz).sqrt() <= config.range
}

/// Whether `mob` is drawn by the avatar model: a player or a mannequin.
fn avatar(mob: &Mob) -> bool {
    &*mob.kind.id == PLAYER || &*mob.kind.id == MANNEQUIN
}

/// The avatar model's height at `scale`, in blocks: 32 pixels at 16 a block,
/// a sixteenth smaller for `AvatarRenderer.scale`.
fn avatar_height(scale: f32) -> f64 {
    MODEL_HEIGHT_PX / PIXELS_PER_BLOCK * f64::from(scale) * AVATAR_SCALE
}

/// The height the figure is drawn at. For an avatar that is the model's own
/// height, which is what the game draws; for any other species the figure is
/// only a stand-in for a body it does not have, so it fills the hitbox.
fn figure_height(mob: &Mob) -> f64 {
    if avatar(mob) {
        return avatar_height(mob.model_scale());
    }
    mob.dims().1
}

/// The camera this frame is drawn from, or the player's eyes when the frame has
/// not set one yet.
fn camera_at(game: &Game<'_>) -> Option<[f64; 3]> {
    if let Some((camera, ..)) = game.locator_camera {
        return Some(camera);
    }
    let player = game.player.as_ref()?;
    Some([
        player.position.x,
        player.position.y + f64::from(player.dimensions.eye_height),
        player.position.z,
    ])
}

/// Whether `at` is within the configured range of `camera`.
fn in_range(camera: [f64; 3], config: &m::ChamsConfig, at: [f64; 3]) -> bool {
    let (dx, dy, dz) = (at[0] - camera[0], at[1] - camera[1], at[2] - camera[2]);
    (dx * dx + dy * dy + dz * dz).sqrt() <= config.range
}

/// Adds one filled box to the frame, through walls when the switch asks for it.
fn push_box(
    primitives: &mut WorldPrimitives,
    from: [f64; 3],
    to: [f64; 3],
    config: &m::ChamsConfig,
    color: u32,
) {
    let primitive = primitives.filled_box(from, to, color);
    if config.through_walls {
        primitive.through_walls();
    }
}

/// Adds `mob`'s figure of boxes to the frame, unless it is out of range.
fn push_figure(
    primitives: &mut WorldPrimitives,
    mob: &Mob,
    partial: f32,
    camera: [f64; 3],
    config: &m::ChamsConfig,
    alpha: u32,
    rgb: u32,
) {
    let at = mob.previous.lerp(mob.position, f64::from(partial));
    // The whole figure shares one check, so a body at the edge of the range
    // never loses its arms before its torso.
    if !in_range(camera, config, [at.x, at.y, at.z]) {
        return;
    }
    let height = figure_height(mob);
    let color = (alpha << 24) | rgb;
    // The body's own turn, interpolated as the model's is (`mobs::render`
    // does the same with `wrap`).
    let turn = mob.previous_body_yaw + wrap(mob.body_yaw - mob.previous_body_yaw) * partial;
    for (from, to) in figure_boxes([at.x, at.y, at.z], height, f64::from(turn)) {
        push_box(primitives, from, to, config, color);
    }
}

/// The player's own figure, feet at `centre`: the boxes `Filled` draws for a
/// player, at the avatar's own height and scale. Blink's server marker is the
/// same body, drawn where the server last had the player.
pub(super) fn player_figure(centre: [f64; 3], turn: f64) -> [([f64; 3], [f64; 3]); 6] {
    figure_boxes(centre, avatar_height(1.0), turn)
}

/// The figure's six boxes, feet at `centre`: two legs, a torso, two arms and a
/// head, in the player model's proportions, turned with the body by `turn`
/// degrees. The order is legs, legs, torso, arms, arms, head.
///
/// The model is wider than the hitbox — its arms stick out past the body — so
/// the figure is laid out from `height` alone rather than from `dims()`'s
/// width. A box cannot turn, so the parts turn by moving along the body's own
/// x axis instead: the arms and legs are square in plan, so moving them is
/// exact, and the torso, the one part that is not, takes the box its turned
/// footprint needs.
fn figure_boxes(centre: [f64; 3], height: f64, turn: f64) -> [([f64; 3], [f64; 3]); 6] {
    let width = height * MODEL_WIDTH;
    let torso_half = width * TORSO_W * 0.5;
    let body_half_z = width * BODY_D * 0.5;
    let head_half_z = width * HEAD_D * 0.5;
    let limb = width * LIMB_W;
    // The legs fill the torso's width, split at the middle; the arms sit
    // against it, a limb wide.
    let leg_centre = torso_half * 0.5;
    let leg_half = torso_half * 0.5;
    let arm_centre = torso_half + limb * 0.5;
    let arm_half = limb * 0.5;
    let (x, y, z) = (centre[0], centre[1], centre[2]);
    let legs_top = y + height * LEGS_H;
    let torso_top = legs_top + height * TORSO_H;
    // Whatever is left above the torso is the head, a quarter of the height.
    let head_top = y + height;

    let (sin, cos) = turn.to_radians().sin_cos();
    let torso_half_x = torso_half * cos.abs() + body_half_z * sin.abs();
    let torso_half_z = torso_half * sin.abs() + body_half_z * cos.abs();
    let part = |offset: f64, half_x: f64, half_z: f64, bottom: f64, top: f64| {
        let (centre_x, centre_z) = (x + offset * cos, z + offset * sin);
        (
            [centre_x - half_x, bottom, centre_z - half_z],
            [centre_x + half_x, top, centre_z + half_z],
        )
    };

    [
        // The two legs, from the feet up to the hip, split at the middle.
        part(-leg_centre, leg_half, body_half_z, y, legs_top),
        part(leg_centre, leg_half, body_half_z, y, legs_top),
        // The torso, straight off the legs, as the model has it.
        part(0.0, torso_half_x, torso_half_z, legs_top, torso_top),
        // An arm down either side of it, shoulder to hip.
        part(-arm_centre, arm_half, body_half_z, legs_top, torso_top),
        part(arm_centre, arm_half, body_half_z, legs_top, torso_top),
        // The head on top, as deep as it is wide.
        part(0.0, torso_half, head_half_z, torso_top, head_top),
    ]
}

#[cfg(test)]
mod tests {
    use super::{avatar_height, figure_boxes};

    /// The avatar's height, which is what the figure is drawn at: 32 model
    /// pixels at 16 a block, a sixteenth short of two blocks.
    const AVATAR: f64 = 1.875;

    /// The six boxes' extents, as (x0, y0, z0, x1, y1, z1).
    fn boxes(height: f64, turn: f64) -> [[f64; 6]; 6] {
        let mut out = [[0.0; 6]; 6];
        for (slot, (from, to)) in figure_boxes([0.0, 64.0, 0.0], height, turn)
            .into_iter()
            .enumerate()
        {
            out[slot] = [from[0], from[1], from[2], to[0], to[1], to[2]];
        }
        out
    }

    fn width(b: &[f64; 6]) -> f64 {
        b[3] - b[0]
    }
    fn depth(b: &[f64; 6]) -> f64 {
        b[5] - b[2]
    }
    fn span_x(bs: &[[f64; 6]; 6]) -> f64 {
        let lo = bs.iter().fold(f64::MAX, |a, b| a.min(b[0]));
        let hi = bs.iter().fold(f64::MIN, |a, b| a.max(b[3]));
        hi - lo
    }
    fn span_z(bs: &[[f64; 6]; 6]) -> f64 {
        let lo = bs.iter().fold(f64::MAX, |a, b| a.min(b[2]));
        let hi = bs.iter().fold(f64::MIN, |a, b| a.max(b[5]));
        hi - lo
    }
    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn the_avatar_is_drawn_at_the_models_own_height() {
        // 32 pixels at 16 a block, a sixteenth short of two blocks.
        assert!(close(avatar_height(1.0), AVATAR));
        // The entity's own `scale` multiplies it.
        assert!(close(avatar_height(2.0), 3.75));
        assert!(close(avatar_height(0.5), 0.9375));
    }

    #[test]
    fn the_figure_is_the_player_model() {
        let b = boxes(AVATAR, 0.0);
        // 16 pixels wide over 32 tall: half the height, and wider than the
        // 0.6 hitbox the old figure was sized from.
        assert!(close(span_x(&b), AVATAR / 2.0), "{:?}", span_x(&b));
        assert!(span_x(&b) > 0.6);
        // Feet on the entity's own y, head at the model's full height.
        assert!(close(b[0][1], 64.0));
        assert!(close(b[5][4], 64.0 + AVATAR));
        // The head is a cube of a quarter the height...
        assert!(close(width(&b[5]), AVATAR / 4.0));
        assert!(close(width(&b[5]), depth(&b[5])));
        // ...and the torso is half the width, a quarter of it deep.
        assert!(close(width(&b[2]), AVATAR / 4.0));
        assert!(close(depth(&b[2]), AVATAR / 8.0));
    }

    #[test]
    fn the_arms_hang_against_the_torso_without_a_gap() {
        let b = boxes(AVATAR, 0.0);
        let (torso, left_arm) = (&b[2], &b[3]);
        // The arm starts where the torso ends, as the model has it: no seam
        // between them, so the hand cannot drift off the body.
        assert!(close(left_arm[3], torso[0]));
        // A limb is a quarter of the model's width.
        assert!(close(width(left_arm), AVATAR / 8.0));
        // The arms reach from the shoulder to the hip, level with the torso.
        assert!(close(left_arm[1], torso[1]));
        assert!(close(left_arm[4], torso[4]));
        // The two of them fill the model's whole width.
        assert!(close(span_x(&b), AVATAR / 2.0));
    }

    #[test]
    fn the_legs_meet_the_torso_and_split_at_the_middle() {
        let b = boxes(AVATAR, 0.0);
        // The legs end where the torso begins, with no gap.
        assert!(close(b[0][4], b[2][1]));
        // They split at the middle and fill the torso's width between them.
        assert!(close(b[0][3], 0.0));
        assert!(close(b[1][0], 0.0));
        assert!(close(width(&b[0]) * 2.0, width(&b[2])));
        // The head sits straight on the torso.
        assert!(close(b[5][1], b[2][4]));
    }

    #[test]
    fn the_figure_scales_with_the_entity() {
        let tall = boxes(AVATAR, 0.0);
        let small = boxes(AVATAR / 2.0, 0.0);
        assert!(close(span_x(&small), span_x(&tall) * 0.5));
        assert!(close(small[5][4], 64.0 + AVATAR / 2.0));
        assert!(close(small[0][1], 64.0));
    }

    #[test]
    fn the_figure_turns_with_the_body() {
        let front = boxes(AVATAR, 0.0);
        let side = boxes(AVATAR, 90.0);
        // Facing along z, the arms are on x; facing along x, on z.
        assert!(span_x(&front) > span_z(&front));
        assert!(span_x(&side) < span_z(&side));
        // Side on, the head is the widest thing left.
        assert!(close(span_x(&side), AVATAR / 4.0));
        // The head is square, so it keeps its width either way.
        assert!(close(width(&side[5]), AVATAR / 4.0));
        // Halfway round, the torso's box is the widest its footprint needs.
        let half = boxes(AVATAR, 45.0);
        let want = (AVATAR / 4.0 + AVATAR / 8.0) * std::f64::consts::FRAC_1_SQRT_2;
        assert!(close(width(&half[2]), want));
    }
}
