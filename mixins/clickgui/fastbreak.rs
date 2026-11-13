// FastBreak: a port of LiquidBounce's FastBreak and Alan's FastBreak.
//
// Copyright (c) 2015-2026 CCBlueX. LiquidBounce is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed in the
// hope that it will be useful, but WITHOUT ANY WARRANTY; without even the
// implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the LICENSE file or <https://www.gnu.org/licenses/>.
//
// Both references remove the client's five-tick destroy delay and finish the
// dug block early. LiquidBounce (Damage) completes once the stored progress
// passes BreakDamage; Alan (Percentage, Ticks) finishes at a point computed
// from a speed percentage or from the per-tick hardness, optionally first
// dividing the progress and the finish point the way its EqualAirGroundDig
// does under water and in the air.
//
// Horizon keeps the stored progress and the delay of the block being dug in
// the remote client (`Remote.destroying` and `destroy_delay`), so the head of
// `remote_mining` stands in for both references' per-tick events: the delay is
// cleared, and the mode's decision is written back before the tick's own
// progress is added. `remote_mining` holds closures, which interior hooks
// reject, so AbortAnother (LiquidBounce, Grim's issue 1296) also runs from the
// head: when this call is the one that will send STOP-DESTROY, it sends the
// abort for the block above first, with the face the stop reports.
// IgnoreMiningFatigue drops the dig-slowdown effect from the client's list
// each tick, as Alan does; the server still sees the effect.
//
// Like the remote client it hooks, this only runs against a server: the
// single-player client mines through its own path.
use crate::game::clickgui::FastBreakConfig;
use crate::game::{Game, clickgui as m};
use horizon_client::blocks::Fluid;
use horizon_protocol::packets::play::{self, Serverbound as S};

/// `ServerboundPlayerActionPacket.Action.ABORT_DESTROY_BLOCK`.
const ABORT_DESTROY_BLOCK: i32 = 2;

/// The head of `remote_mining`, before the tick's progress lands.
pub(super) fn mining(game: &mut Game<'_>, start: bool) {
    let Some(config) = m::fastbreak() else {
        return;
    };
    if config.only_tool && !holds_tool(game) {
        return;
    }
    if let Some(remote) = game.remote.as_mut() {
        // LiquidBounce: `interaction.destroyDelay = 0` every tick.
        remote.destroy_delay = 0;
    }
    apply(game, &config);
    if config.abort_another && !start {
        abort(game);
    }
}

/// The mode's opinion of the stored progress: complete now (Damage) or move
/// the finish point (Percentage, Ticks).
fn apply(game: &mut Game<'_>, config: &FastBreakConfig) {
    let Some((pos, done)) = game
        .remote
        .as_ref()
        .and_then(|remote| remote.destroying)
        .map(|(pos, _, done)| (pos, done))
    else {
        return;
    };
    if config.mode == m::FASTBREAK_DAMAGE {
        if done > config.break_damage {
            store(game, 1.0);
        }
        return;
    }
    let (margin, threshold) = if config.mode == m::FASTBREAK_TICKS {
        (0.81, 1.0 - game.destroy_progress(pos) * config.ticks as f32)
    } else {
        (0.8, 1.0 - config.speed as f32 / 100.0)
    };
    let (in_water, ahead_air, on_ground) = if config.equal_air_ground {
        surroundings(game)
    } else {
        (false, true, true)
    };
    let (progress, threshold) =
        m::fastbreak_air_ground(done, threshold, margin, in_water, ahead_air, on_ground);
    store(game, m::fastbreak_finish(progress, threshold).unwrap_or(progress));
}

/// AbortAnother: the abort for the block above, before the STOP the break
/// about to complete sends.
fn abort(game: &mut Game<'_>) {
    let Some((pos, face)) = stopping(game) else {
        return;
    };
    game.send(S::PlayerAction(play::PlayerAction {
        action: ABORT_DESTROY_BLOCK,
        pos: [pos[0], pos[1].saturating_add(1), pos[2]],
        direction: face,
        sequence: 0,
    }));
}

/// The block and face of the STOP this call will send, with the same gates
/// `remote_mining` checks before it reaches `done >= 1.0`.
fn stopping(game: &mut Game<'_>) -> Option<([i32; 3], u8)> {
    if !game.res.items().can_destroy_block(game.held(), game.creative)
        || game.remote_break_restricted()
    {
        return None;
    }
    let (pos, done) = game
        .remote
        .as_ref()
        .and_then(|remote| remote.destroying)
        .map(|(pos, _, done)| (pos, done))?;
    let hit = game.pick()?;
    if hit.pos != pos {
        return None;
    }
    let state = game.world.block(pos[0], pos[1], pos[2]);
    if game.res.facts().is_air(state) {
        return None;
    }
    if game.remote.as_ref()?.destroying_item != game.main_hand_key() {
        return None;
    }
    let progress = game.destroy_progress(pos);
    (done + progress >= 1.0).then_some((pos, super::face_direction(hit.face)))
}

/// Once per tick, from the window's game.
pub(crate) fn tick(game: &mut Game<'_>) {
    let Some(config) = m::fastbreak() else {
        return;
    };
    if !config.ignore_mining_fatigue {
        return;
    }
    let effects = &mut game.potions.effects.list;
    let before = effects.len();
    effects.retain(|effect| effect.id != "mining_fatigue");
    if effects.len() != before {
        game.apply_effect_status();
    }
}

/// Stores the progress `remote_mining` will add this tick's damage to.
fn store(game: &mut Game<'_>, progress: f32) {
    if let Some((_, _, stored)) = game.remote.as_mut().and_then(|remote| remote.destroying.as_mut()) {
        *stored = progress;
    }
}

/// OnlyTool: the main hand's item has tool rules.
fn holds_tool(game: &Game<'_>) -> bool {
    game.held()
        .and_then(|stack| game.res.items().get(&stack.item))
        .is_some_and(|item| !item.tool.is_empty())
}

/// Alan's water and air checks: the feet's fluid, whether the block the
/// player's motion leads into is air (the reference's `PlayerUtil.p`), and
/// the ground flag.
fn surroundings(game: &Game<'_>) -> (bool, bool, bool) {
    let Some(player) = game.player.as_ref() else {
        return (false, true, true);
    };
    let position = player.position;
    let (x, z) = (position.x.floor() as i32, position.z.floor() as i32);
    let feet = game.world.block(x, position.y.floor() as i32, z);
    let ahead = game.world.block(x, (position.y + player.velocity.y).floor() as i32, z);
    let facts = game.res.facts();
    (
        facts.get(feet).fluid == Fluid::Water,
        facts.is_air(ahead),
        player.ground.on_ground,
    )
}
