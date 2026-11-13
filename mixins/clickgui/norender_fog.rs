// NoRender's fog: push the fog bands past the drawn world so the shader's
// fog factor stays at zero. The sky and cloud ends are left alone, so the
// sky still fades at Horizon's own distance.
fn clickgui_norender_fog(
    game: &crate::game::Game<'_>,
    _partial: f32,
    _camera: [f64; 3],
    fog_type: horizon_client::fog::FogType,
    fog: &mut horizon_client::fog::FogData,
) {
    use crate::game::clickgui as m;

    if !game.native_on(m::ID) {
        return;
    }
    let Some(config) = m::norender() else {
        return;
    };
    let hidden = match config.fog {
        m::NoRenderFogMode::Off => false,
        m::NoRenderFogMode::Clear => true,
        m::NoRenderFogMode::Lava => fog_type == horizon_client::fog::FogType::Lava,
        m::NoRenderFogMode::Water => fog_type == horizon_client::fog::FogType::Water,
    };
    if !hidden {
        return;
    }
    const FAR: f32 = 1.0e6;
    fog.environmental_start = FAR;
    fog.environmental_end = FAR * 2.0;
    fog.render_distance_start = FAR;
    fog.render_distance_end = FAR * 2.0;
    // Water fog also closes the sky and the clouds in, to its own 96 blocks
    // (times the biome's multiplier and the water vision): lift those too.
    if fog_type == horizon_client::fog::FogType::Water {
        fog.sky_end = FAR * 2.0;
        fog.cloud_end = FAR * 2.0;
    }
}
