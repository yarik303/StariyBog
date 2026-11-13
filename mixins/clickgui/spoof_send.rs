fn send(
    game: &crate::game::Game<'_>,
    packet: &horizon_protocol::packets::play::Serverbound,
) -> core::ops::ControlFlow<()> {
    crate::game::remote::clickgui_spoof::send(game, packet)
}
