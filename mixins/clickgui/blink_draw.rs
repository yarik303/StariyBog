fn clickgui_blink_draw(
    game: &crate::game::Game<'_>,
    partial: f32,
    camera: [f64; 3],
    primitives: &mut horizon_client::world_primitives::WorldPrimitives,
) {
    crate::game::clickgui_blink::draw(game, partial, camera, primitives);
}
