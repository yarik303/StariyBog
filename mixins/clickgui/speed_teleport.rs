fn clickgui_speed_teleport(
    game: &mut crate::game::Game<'_>,
    packet: &horizon_protocol::packets::play::PlayerPosition,
) {
    let _ = packet;
    crate::game::remote::clickgui_speed::teleport(game);
}
