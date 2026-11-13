fn clickgui_viewmodel_pose(
    game: &crate::game::Game<'_>,
    hand: horizon_client::first_person::Hand,
    stack: Option<&horizon_client::items::ItemStack>,
    x: &mut f32,
    y: &mut f32,
    z: &mut f32,
    pitch: &mut f32,
    yaw: &mut f32,
    roll: &mut f32,
    scale: &mut f32,
) {
    crate::game::clickgui_viewmodel::pose(game, hand, stack, x, y, z, pitch, yaw, roll, scale);
}
