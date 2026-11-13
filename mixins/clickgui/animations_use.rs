fn clickgui_animations_use(
    game: &crate::game::Game<'_>,
    animation: &mut horizon_world::consumable::UseAnimation,
) {
    crate::game::clickgui_animations::using(game, animation);
}
