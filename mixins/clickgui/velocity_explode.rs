fn clickgui_velocity_explode(
    game: &mut crate::game::Game<'_>,
    packet: &horizon_protocol::packets::play::Explode,
) {
    crate::game::remote::clickgui_velocity::explode(game, packet);
    crate::game::remote::clickgui_fly::explode(game, packet);
}
