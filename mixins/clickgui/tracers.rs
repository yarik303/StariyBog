// Tracers: a line from a point far along the view direction to every other
// player and, optionally, every dropped item (a port of the client's Tracers
// render module). The lines are projected with the camera NameTags uses and
// drawn as HUD quads over the world, like ESP's boxes.
//
// The view point is 150 blocks ahead (the reference's fixed distance); the
// camera's own yaw and pitch are reused from `locator_camera`.
use crate::game::{Game, clickgui as m, clickgui_esp, clickgui_nametags::View};
use horizon_client::hud::HudBatch;

/// The reference's `150.0`: how far ahead the lines start.
const VIEW_DISTANCE: f64 = 150.0;
const WIDTH: f32 = 1.5;

pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch) {
    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    let Some(config) = m::tracers() else {
        return;
    };
    if !config.players && !config.items {
        return;
    }
    if game.player.is_none() {
        return;
    }
    let Some(view) = View::from_game(game, batch) else {
        return;
    };
    let Some((camera, yaw, pitch, _)) = game.locator_camera else {
        return;
    };
    let forward = horizon_client::gizmos::view_vector(pitch, yaw);
    let start = [
        camera[0] + forward[0] * VIEW_DISTANCE,
        camera[1] + forward[1] * VIEW_DISTANCE,
        camera[2] + forward[2] * VIEW_DISTANCE,
    ];
    let atlas = &game.gui.atlas;
    let alpha = game.clock.camera_fraction();

    if config.players {
        for (id, name) in game.remote_player_names() {
            let Some(mob) = game.mobs.iter().find(|m| m.id == id) else {
                continue;
            };
            if !mob.alive() || mob.removed {
                continue;
            }
            let end = [
                lerp(mob.previous.x, mob.position.x, alpha),
                lerp(mob.previous.y, mob.position.y, alpha),
                lerp(mob.previous.z, mob.position.z, alpha),
            ];
            let color = if m::is_friend(&config.friend_names, &name) {
                config.friend_color
            } else {
                config.color
            };
            line(batch, atlas, &view, start, end, color);
        }
        // The players this game hosts over LAN.
        if let Some(host) = &game.lan {
            for guest in host
                .hub
                .guests
                .iter()
                .filter(|g| host.hub.here(g) && !g.dead)
            {
                let end = [
                    lerp(guest.previous_position[0], guest.position[0], alpha),
                    lerp(guest.previous_position[1], guest.position[1], alpha),
                    lerp(guest.previous_position[2], guest.position[2], alpha),
                ];
                let color = if m::is_friend(&config.friend_names, &guest.name) {
                    config.friend_color
                } else {
                    config.color
                };
                line(batch, atlas, &view, start, end, color);
            }
        }
    }

    if config.items {
        for item in &game.dropped {
            let end = [
                lerp(item.previous.x, item.position.x, alpha),
                lerp(item.previous.y, item.position.y, alpha),
                lerp(item.previous.z, item.position.z, alpha),
            ];
            line(batch, atlas, &view, start, end, config.color);
        }
    }
}

fn line(
    batch: &mut HudBatch,
    atlas: &horizon_client::hud::HudAtlas,
    view: &View,
    start: [f64; 3],
    end: [f64; 3],
    color: u32,
) {
    let Some((a, b)) = view.project_segment(start, end) else {
        return;
    };
    clickgui_esp::line(batch, atlas, a, b, WIDTH, clickgui_esp::hud(color));
}

fn lerp(previous: f64, current: f64, alpha: f64) -> f64 {
    previous + (current - previous) * alpha
}
