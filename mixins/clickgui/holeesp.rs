// HoleESP: the sealed obsidian and bedrock holes around the player, drawn
// with the game's world-primitive hook so the boxes are real 3D geometry with
// the world's depth (or seen through walls, as the setting asks).
//
// The scan runs once a tick and is cached in the registry: blocks change at
// tick speed, and the camera moves between ticks without changing which
// blocks are holes.
fn clickgui_holeesp(
    game: &crate::game::Game<'_>,
    _partial: f32,
    camera: [f64; 3],
    primitives: &mut horizon_client::world_primitives::WorldPrimitives,
) {
    use crate::game::clickgui as m;

    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    let Some(config) = m::holeesp() else {
        return;
    };
    let Some(player) = game.player.as_ref() else {
        return;
    };
    let feet = [
        player.position.x.floor() as i32,
        player.position.y.floor() as i32,
        player.position.z.floor() as i32,
    ];
    let scan = m::HoleScan::of(game.world.game_time, feet, &config);
    let reach = m::hole_max_distance(&config);

    m::with_holes(
        scan,
        || {
            let facts = game.res.facts();
            m::scan_holes(feet, &config, |x, y, z| {
                let state = game.world.block(x, y, z);
                if facts.is_air(state) {
                    return m::HoleMaterial::Air;
                }
                let block = facts.get(state);
                if block.name.as_ref() == "minecraft:bedrock" {
                    m::HoleMaterial::Bedrock
                } else if block.explosion_resistance >= 600.0 {
                    // Obsidian, crying obsidian, ancient debris, a respawn
                    // anchor: the blast-resistant blocks a hole is made of.
                    m::HoleMaterial::Obsidian
                } else if block.full_collision {
                    m::HoleMaterial::Solid
                } else {
                    m::HoleMaterial::Other
                }
            })
        },
        |holes| {
            for hole in holes {
                let (width, depth) = (f64::from(hole.width), f64::from(hole.depth));
                let fade = if config.fade {
                    let dx = camera[0] - (f64::from(hole.x) + width * 0.5);
                    let dy = camera[1] - (f64::from(hole.y) + 0.5);
                    let dz = camera[2] - (f64::from(hole.z) + depth * 0.5);
                    m::hole_fade((dx * dx + dy * dy + dz * dz).sqrt(), reach)
                } else {
                    1.0
                };
                let rgb = m::hole_color(&config, hole.kind);
                let min = [
                    f64::from(hole.x),
                    f64::from(hole.y) + 0.01,
                    f64::from(hole.z),
                ];
                let max = [
                    f64::from(hole.x) + width,
                    f64::from(hole.y) + f64::from(config.height),
                    f64::from(hole.z) + depth,
                ];

                if config.filled {
                    let alpha = (f32::from(config.fill_alpha) * fade).round() as u32;
                    if alpha > 0 {
                        let primitive = primitives.filled_box(min, max, (alpha << 24) | rgb);
                        if config.through_walls {
                            primitive.through_walls();
                        }
                    }
                }
                if config.outline {
                    let alpha = (f32::from(config.outline_alpha) * fade).round() as u32;
                    if alpha > 0 {
                        let primitive = primitives.box_outline(min, max, (alpha << 24) | rgb, 1.5);
                        if config.through_walls {
                            primitive.through_walls();
                        }
                    }
                }
            }
        },
    );
}
