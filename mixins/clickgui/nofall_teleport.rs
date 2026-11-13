fn clickgui_nofall_teleport(
    game: &mut crate::game::Game<'_>,
    packet: &horizon_protocol::packets::play::PlayerPosition,
) -> core::ops::ControlFlow<Result<()>> {
    crate::game::remote::clickgui_fly::teleport(game, packet);
    crate::game::remote::clickgui_nofall::teleport(game, packet)
}
