// Chams' outline, in the entity-outline hook: a thin call so the body stays out
// of the inlined hooks (`native_hooks.rs` must stay macro-free).
fn clickgui_chams_outline(
    game: &crate::game::Game<'_>,
    entity_id: u32,
    kind: &str,
    outline: &mut Option<u32>,
) {
    crate::game::clickgui_chams::outline(game, entity_id, kind, outline);
}
