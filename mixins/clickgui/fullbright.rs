// FullBright: a port of LiquidBounce's ModuleFullBright.
//
// Copyright (c) 2015-2026 CCBlueX. LiquidBounce is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed in the
// hope that it will be useful, but WITHOUT ANY WARRANTY; without even the
// implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the LICENSE file or <https://www.gnu.org/licenses/>.
//
// Gamma mode drives the renderer's brightness the way the original drove
// `options.gamma`, with an optional smooth ramp. NightVision mode keeps a
// night vision effect on the player. Both are settings in the registry, so the
// GUI edits them and they are kept in the mod's data folder.
use crate::game::{Game, clickgui as m};
use horizon_client::effects::EffectInstance;

const GAMMA: u8 = 0;
const NIGHT_VISION: u8 = 1;

pub(super) fn tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let Some((enabled, config)) = m::fullbright() else {
        return;
    };
    let mut state = m::with(|c| c.fullbright);
    let on = enabled && game.native_on(m::ID);

    if !on {
        if state.applied {
            restore_gamma(game, &mut state);
            if state.night_vision {
                remove_night_vision(game);
                state.night_vision = false;
            }
            state.applied = false;
            state.saved_gamma = None;
            state.current_gamma = 0.0;
        }
        m::with(|c| c.fullbright = state);
        return;
    }

    if game.native_player().is_none() {
        m::with(|c| c.fullbright = state);
        return;
    }

    let first = !state.applied;
    if first {
        state.applied = true;
        // The renderer's gamma is what draws; settings.gamma is left alone so
        // the player's options.txt never gets a 15 written into it.
        let gamma = renderer_gamma(game);
        state.current_gamma = gamma;
        // Kept in memory only: it is the value to restore when the module is
        // turned off, and the renderer's copy is never written to options.txt.
        state.saved_gamma = Some(gamma);
    } else if state.mode != config.mode {
        // Undo what the previous mode changed before the new one starts.
        if state.mode == GAMMA {
            restore_gamma(game, &mut state);
        } else if state.night_vision {
            remove_night_vision(game);
            state.night_vision = false;
        }
    }
    state.mode = config.mode;

    if config.mode == NIGHT_VISION {
        game.add_player_effect(EffectInstance::new("night_vision", 1337, config.amplifier));
        state.night_vision = true;
    } else {
        if state.night_vision {
            remove_night_vision(game);
            state.night_vision = false;
        }
        let step = if config.smooth { config.speed } else { 999.0 };
        state.current_gamma = m::gamma_step(state.current_gamma, config.brightness as f32, step);
        set_gamma(game, state.current_gamma);
    }

    m::with(|c| c.fullbright = state);
}

fn renderer_gamma(game: &Game<'_>) -> f32 {
    game.renderer
        .as_ref()
        .map_or(game.settings.gamma, |renderer| renderer.gamma)
}

fn set_gamma(game: &mut Game<'_>, gamma: f32) {
    if let Some(renderer) = game.renderer.as_mut() {
        renderer.gamma = gamma;
    }
}

fn restore_gamma(game: &mut Game<'_>, state: &mut m::FullBrightState) {
    if let Some(gamma) = state.saved_gamma {
        set_gamma(game, gamma);
        state.current_gamma = gamma;
    }
}

fn remove_night_vision(game: &mut Game<'_>) {
    game.potions
        .effects
        .list
        .retain(|effect| effect.id != "night_vision");
    game.apply_effect_status();
}
