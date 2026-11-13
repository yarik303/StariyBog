fn clickgui_ambience_fog(
    game: &crate::game::Game<'_>,
    fog_type: horizon_client::fog::FogType,
    fog: &mut horizon_client::fog::FogData,
) {
    crate::game::clickgui_ambience::fog(game, fog_type, fog);
}
