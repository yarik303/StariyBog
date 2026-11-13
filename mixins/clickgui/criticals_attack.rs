fn clickgui_criticals_attack(game: &mut crate::game::Game<'_>) {
    crate::game::remote::clickgui_criticals::attack(game);
    crate::game::remote::clickgui_velocity::attack(game);
}
