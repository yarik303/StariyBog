// AimAssist: turns the view toward a nearby target, a step at a time.
//
// Seven references (Raven/Xd, keystrokes' Normal/Tejas/Grim, Prestige, Argon,
// Alan) are the same three steps: pick a target inside a cone and a range,
// work out the rotation to a point on its body, then turn toward that rotation
// by a limited amount each tick. The differences are the gates and the human
// touches, so this keeps the gates they share and leaves out the anti-cheat
// trickery that Horizon cannot reproduce honestly (see the tail of this file).
//
// The seam is `Game::yaw` / `Game::pitch`. `remote_send_state` reads exactly
// those two for the movement packet (`remote.rs:7684`), so changing them turns
// the view *and* tells the server. That function runs inside `tick_remote_level`,
// which `tick_remote` calls before `native_tick`, so a change made here is drawn
// at once and reaches the server on the next tick — invisible at the speed this
// module turns, but worth knowing.
//
// `native_camera_pose` is NOT this hook: it moves only the picture, and its
// docs say the player's aim and first-person hand stay where they were.
//
// Not ported: silent rotations (Scaffold's `sync_sender` can send a look the
// camera does not keep, but that is a one-tick placement look, not a persistent
// aim), GCD/rotMove simulation, mouse-movement gating, reaction delays, target
// prediction, and the per-client noise/jitter models. All of those exist to
// shape the packet stream for an anti-cheat; none of them is aiming.
use crate::game::{Game, clickgui as m};
use horizon_client::mobs::{Mob, living::wrap};
use std::cell::RefCell;

const PLAYER: &str = "minecraft:player";
/// The mannequin uses the player model everywhere else in the client.
const MANNEQUIN: &str = "minecraft:mannequin";
/// `Game::toggles` slot for the attack button.
const ATTACK: usize = 2;

#[derive(Default)]
struct State {
    /// The entity Sticky last chose, by server id.
    target: Option<u32>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// One entity the view could turn toward, and the rotation that would face it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Candidate {
    index: usize,
    yaw: f32,
    pitch: f32,
    /// The larger of the two angular errors, used to rank candidates.
    angle: f32,
}

pub(crate) fn tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let Some(config) = m::aimassist() else {
        STATE.with(|state| *state.borrow_mut() = State::default());
        return;
    };
    if !ready(game, &config) {
        STATE.with(|state| *state.borrow_mut() = State::default());
        return;
    }
    let Some(eye) = game.eye_position() else {
        return;
    };
    let own = game.remote.as_ref().map_or(-1, |r| r.entity_id);
    let friends = m::friend_names();
    let friend_ids: Vec<u32> = game
        .remote_player_names()
        .iter()
        .filter(|(_, name)| m::is_friend(&friends, name))
        .map(|(id, _)| *id)
        .collect();

    let sticky = config
        .sticky
        .then(|| {
            STATE.with(|state| state.borrow().target)
                .and_then(|id| game.mobs.iter().position(|mob| mob.id == id))
        })
        .flatten();
    let pick = |only: Option<usize>| best(game, &config, eye, own, &friend_ids, only);
    let chosen = if sticky.is_some() {
        // Sticky keeps its target while it is still valid, then falls back to
        // a fresh scan rather than sticking to an entity that left the cone.
        pick(sticky).or_else(|| pick(None))
    } else {
        pick(None)
    };

    let Some(candidate) = chosen else {
        STATE.with(|state| state.borrow_mut().target = None);
        return;
    };
    STATE.with(|state| state.borrow_mut().target = Some(game.mobs[candidate.index].id));

    if config.stop_on_target
        && game
            .mob_hit_within(game.entity_reach)
            .is_some_and(|(index, _, _)| index == candidate.index)
    {
        return;
    }
    apply(game, &config, candidate.yaw, candidate.pitch);
}

fn ready(game: &Game<'_>, config: &m::AimAssistConfig) -> bool {
    game.player.is_some()
        && game.remote.is_some()
        && game.native_playing()
        && !game.spectator()
        && (!config.click_aim || game.toggles.is_down(ATTACK))
        && (!config.weapon_only || holds_weapon(game))
}

/// The smallest-angle target, optionally restricted to one entity (Sticky).
fn best(
    game: &Game<'_>,
    config: &m::AimAssistConfig,
    eye: [f64; 3],
    own: i32,
    friends: &[u32],
    only: Option<usize>,
) -> Option<Candidate> {
    let half = config.fov * 0.5;
    let mut found: Option<Candidate> = None;
    for (index, mob) in game.mobs.iter().enumerate() {
        if only.is_some_and(|wanted| wanted != index) || !eligible(config, mob, own, friends) {
            continue;
        }
        let point = aim_point(mob, config.point);
        let delta = [
            point[0] - eye[0],
            point[1] - eye[1],
            point[2] - eye[2],
        ];
        let distance = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
        if distance > f64::from(config.range) {
            continue;
        }
        let (yaw, pitch) = look(eye, point);
        let dyaw = wrap(yaw - game.yaw);
        let dpitch = pitch - game.pitch;
        if dyaw.abs() > half || dpitch.abs() > half {
            continue;
        }
        if !config.through_walls && blocked(game, eye, delta, distance) {
            continue;
        }
        let angle = dyaw.abs().max(dpitch.abs());
        if found.is_none_or(|c| angle < c.angle) {
            found = Some(Candidate {
                index,
                yaw,
                pitch,
                angle,
            });
        }
    }
    found
}

fn eligible(config: &m::AimAssistConfig, mob: &Mob, own: i32, friends: &[u32]) -> bool {
    if mob.id as i32 == own || mob.death_time > 0 || friends.contains(&mob.id) {
        return false;
    }
    if mob.invisible && !config.invisibles {
        return false;
    }
    let player = &*mob.kind.id == PLAYER || &*mob.kind.id == MANNEQUIN;
    if player {
        config.players
    } else {
        config.mobs
    }
}

/// The point on a body the rotations aim at.
fn aim_point(mob: &Mob, point: m::AimPoint) -> [f64; 3] {
    let height = f64::from(mob.kind.height);
    [
        mob.position.x,
        mob.position.y + height_offset(point, height, f64::from(mob.kind.eye_height)),
        mob.position.z,
    ]
}

/// Split out so the three body points can be pinned by a test.
fn height_offset(point: m::AimPoint, height: f64, eye_height: f64) -> f64 {
    match point {
        m::AimPoint::Head => eye_height,
        m::AimPoint::Chest => height * 0.6,
        m::AimPoint::Legs => height * 0.25,
    }
}

/// The game's own block-outline ray, cast from the eyes toward the target. Its
/// reach stops short of the target so a block the target stands beside does not
/// count as cover.
fn blocked(game: &Game<'_>, eye: [f64; 3], delta: [f64; 3], distance: f64) -> bool {
    if distance < 0.5 {
        return false;
    }
    let dir = [delta[0] / distance, delta[1] / distance, delta[2] / distance];
    game.clip_ray(eye, dir, distance - 0.25).is_some()
}

fn apply(game: &mut Game<'_>, config: &m::AimAssistConfig, yaw: f32, pitch: f32) {
    if config.yaw {
        game.yaw = advance(game.yaw, wrap(yaw - game.yaw), config.speed);
    }
    if config.pitch {
        game.pitch = advance(game.pitch, pitch - game.pitch, config.speed).clamp(-90.0, 90.0);
    }
}

/// Yaw and pitch in degrees from `from` to `to`, in Horizon's convention:
/// yaw 0 faces +Z and pitch is positive looking down. Scaffold's `aim` uses
/// the same pair of `atan2` calls.
fn look(from: [f64; 3], to: [f64; 3]) -> (f32, f32) {
    let dx = to[0] - from[0];
    let dy = to[1] - from[1];
    let dz = to[2] - from[2];
    let yaw = (-dx).atan2(dz).to_degrees() as f32;
    let pitch = (-dy).atan2(dx.hypot(dz)).to_degrees() as f32;
    (yaw, pitch)
}

/// Moves `current` toward `current + delta` by at most `speed` degrees.
fn advance(current: f32, delta: f32, speed: f32) -> f32 {
    current + delta.clamp(-speed, speed)
}

/// A sword or an axe, by the item tags with the id suffix as the fallback, the
/// same classification AutoWeapon and AutoClicker use.
fn holds_weapon(game: &Game<'_>) -> bool {
    let Some(stack) = game.held() else {
        return false;
    };
    let item = &*stack.item;
    let tagged = |tag: &str| {
        game.res
            .items()
            .item_tags
            .get(tag)
            .is_some_and(|items| items.contains(item))
    };
    tagged("minecraft:swords")
        || tagged("minecraft:axes")
        || item.ends_with("_sword")
        || (item.ends_with("_axe") && !item.ends_with("_pickaxe"))
}

#[cfg(test)]
mod tests {
    use super::{advance, height_offset, look, wrap};
    use crate::game::clickgui::AimPoint;

    #[test]
    fn looking_straight_ahead_is_zero_zero() {
        let (yaw, pitch) = look([0.0, 1.62, 0.0], [0.0, 1.62, 3.0]);
        assert!(yaw.abs() < 1e-4, "{yaw}");
        assert!(pitch.abs() < 1e-4, "{pitch}");
    }

    #[test]
    fn yaw_and_pitch_follow_the_game_convention() {
        // +X is east: yaw -90. Straight down is pitch +90.
        let (yaw, _) = look([0.0, 0.0, 0.0], [3.0, 0.0, 0.0]);
        assert!((yaw + 90.0).abs() < 1e-4, "{yaw}");
        let (_, pitch) = look([0.0, 1.0, 0.0], [0.0, 0.0, 0.0]);
        assert!((pitch - 90.0).abs() < 1e-4, "{pitch}");
        // A target at the same height and 3 blocks away needs a small pitch.
        let (_, pitch) = look([0.0, 1.62, 0.0], [0.0, 1.0, 3.0]);
        assert!(pitch > 0.0 && pitch < 20.0, "{pitch}");
    }

    #[test]
    fn an_angle_wraps_to_the_short_way_round() {
        assert!((wrap(370.0) - 10.0).abs() < 1e-4);
        assert!((wrap(-370.0) + 10.0).abs() < 1e-4);
        assert!((wrap(180.0) + 180.0).abs() < 1e-4);
        assert!(wrap(0.0).abs() < 1e-4);
    }

    #[test]
    fn a_step_never_overshoots_or_exceeds_the_speed() {
        assert!((advance(0.0, 100.0, 4.0) - 4.0).abs() < 1e-4);
        assert!((advance(0.0, -100.0, 4.0) + 4.0).abs() < 1e-4);
        assert!((advance(0.0, 2.0, 4.0) - 2.0).abs() < 1e-4);
        assert!((advance(30.0, 0.0, 4.0) - 30.0).abs() < 1e-4);
    }

    #[test]
    fn the_three_body_points_sit_in_order() {
        let (height, eye) = (1.8, 1.62);
        let head = height_offset(AimPoint::Head, height, eye);
        let chest = height_offset(AimPoint::Chest, height, eye);
        let legs = height_offset(AimPoint::Legs, height, eye);
        assert!((head - eye).abs() < 1e-6);
        assert!(head > chest && chest > legs && legs > 0.0);
    }
}
