// Chams' filled figures, in the world-primitive hook: a thin call so the body
// stays out of the inlined hooks (`native_hooks.rs` must stay macro-free).
fn clickgui_chams_draw(
    game: &crate::game::Game<'_>,
    partial: f32,
    camera: [f64; 3],
    primitives: &mut horizon_client::world_primitives::WorldPrimitives,
) {
    crate::game::clickgui_chams::draw(game, partial, camera, primitives);
}
