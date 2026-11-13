// ViewModel: the first-person hands, moved, turned and resized.
//
// `native_render_hand_pose` is handed the frame's own hand transform as seven
// numbers — an offset in blocks, pitch/yaw/roll in degrees, and one scale —
// and the game composes them onto the hand before drawing it. That composition
// covers the arm and the item in it together, so one hook cannot give the arm
// and the item separate transforms the way the references' two events do.
// What it can do is tell them apart: the hook is also handed the stack the
// hand holds, and an empty one is the bare arm. That is the Arm group — the
// same pose, added only while the hand is empty.
//
// The swing and equip progress arrive by value, so the animation itself is out
// of reach here; that is the Animations module's.
//
// The values are added to whatever the frame already has, and the scale is
// multiplied, so another mod's pose still applies and the two compose.
use crate::game::{Game, clickgui as m};
use horizon_client::{first_person::Hand, items::ItemStack};

/// Adds ViewModel's pose for `hand` to the frame's own.
pub(super) fn pose(
    game: &Game<'_>,
    hand: Hand,
    stack: Option<&ItemStack>,
    x: &mut f32,
    y: &mut f32,
    z: &mut f32,
    pitch: &mut f32,
    yaw: &mut f32,
    roll: &mut f32,
    scale: &mut f32,
) {
    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    let Some(config) = m::viewmodel() else {
        return;
    };
    let pose = match hand {
        Hand::Main => &config.main,
        Hand::Off => &config.off,
    };
    apply(pose, x, y, z, pitch, yaw, roll, scale);
    // Nothing in the hand is the arm itself, which the references pose on its
    // own. Added to the hand's pose rather than replacing it, so a main-hand
    // offset still moves the empty hand and the arm group only refines it.
    if stack.is_none() {
        apply(&config.arm, x, y, z, pitch, yaw, roll, scale);
    }
}

fn apply(
    pose: &m::HandPose,
    x: &mut f32,
    y: &mut f32,
    z: &mut f32,
    pitch: &mut f32,
    yaw: &mut f32,
    roll: &mut f32,
    scale: &mut f32,
) {
    if pose.is_identity() {
        return;
    }
    *x += pose.x;
    *y += pose.y;
    *z += pose.z;
    *pitch += pose.pitch;
    *yaw += pose.yaw;
    *roll += pose.roll;
    *scale *= pose.scale;
}
