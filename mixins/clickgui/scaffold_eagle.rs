fn clickgui_scaffold_eagle(game: &crate::game::Game<'_>, sneak: &mut bool) {
    crate::game::remote::clickgui_scaffold::movement_input(game, sneak);
}
