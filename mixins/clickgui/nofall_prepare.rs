fn clickgui_nofall_prepare(game: &mut crate::game::Game<'_>, input: u8) {
    crate::game::remote::clickgui_nofall::prepare(game, input);
    crate::game::remote::clickgui_criticals::prepare(game);
    crate::game::remote::clickgui_scaffold::tick(game);
}
