fn clickgui_animations_pose(
    game: &crate::game::Game<'_>,
    swing: f32,
    z: &mut f32,
    pitch: &mut f32,
    roll: &mut f32,
) {
    crate::game::clickgui_animations::pose(game, swing, pitch, roll, z);
}
