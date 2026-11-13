// Velocity: a port of LiquidBounce's Modify, Reduce, Reversal, Strafe, Lag and
// JumpReset velocity modes, plus the FDPClient, Alan, Sigma, Tenacity, Flux,
// Expensive, Azura, Augustus and Cel implementations and the 1.21 "New Grim"
// fragment.
//
// Copyright (c) 2015-2026 CCBlueX. LiquidBounce is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed in
// the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the
// implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the LICENSE file or <https://www.gnu.org/licenses/>.
//
// Three hooks make the port work. `handle_packet` is a macro and cannot be
// hooked, so the knockback is reconstructed instead: `snapshot` runs at the
// head of `remote_send_state`, after the tick's physics and before the
// movement packet, and remembers the velocity the player starts the next tick
// with; `tick` runs at the head of `tick_remote_level`, before the physics,
// and a velocity that differs from that snapshot (and from the explosion
// pushes `explode` recorded) is a server `SetEntityMotion` applied between the
// ticks. The tick rewrites that velocity the way the mode's reference client
// rewrites its packet or motion. Explosions reach the player by pushing
// `player.velocity` inside `remote_explode`, so the recorded push is taken
// back out, the rest is handled, and the push re-applied scaled.
//
// The differences from the originals: Horizon has no transaction queue or
// fake-packet bus, so the queue/transaction/blink-stack modes and their
// settings are not ported. Block break checks are not exposed, so the Grim
// modes send the stop-destroy action themselves (below the feet facing up for
// Grim, at the feet facing down for NewGrim); GrimC03 sends it into the air
// block at the feet and ramps the clickgui timer, as Spoof does for its one
// tick. Sneaking is the input flag in 1.21, so BlocksMC sneaks through the key
// set for its ticks.
use crate::game::clickgui as m;
use crate::game::clickgui::{VelocityConfig, VelocityMode as Mode};
use crate::game::{Game, Screen};
use horizon_client::clock::clickgui_timer as timer;
use horizon_protocol::packets::play::{self, Serverbound as S};
use std::cell::{Cell, RefCell};
use winit::keyboard::KeyCode;

/// The ground bit of `ServerboundMovePlayerPacket`'s flags.
const ON_GROUND: u8 = 1;
/// `ServerboundPlayerActionPacket.Action.STOP_DESTROY_BLOCK`.
const STOP_DESTROY_BLOCK: i32 = 3;
/// The face value pointing up (`Direction.UP.get3DDataValue`).
const UP: u8 = 1;
/// `Direction.DOWN`, which the New Grim fragment sends.
const DOWN: u8 = 0;

/// One knockback the server sent, with the player's motion before it.
#[derive(Clone, Copy)]
struct Knock {
    velocity: [f64; 3],
    pre: [f64; 3],
    position: [f64; 3],
    on_ground: bool,
    /// Hypixel: whether this is the first air velocity and gets absorbed.
    absorb: bool,
}

/// A knockback the Delayed/Reverse queue holds for a number of ticks.
#[derive(Clone, Copy)]
struct Held {
    /// The velocity to restore, already scaled by the mode's modifiers, so the
    /// flush below needs no config.
    velocity: [f64; 3],
    ticks: i32,
}

/// The counters and timers the modes keep between packets and ticks.
struct State {
    /// The mode the state belongs to (a change resets the counters).
    mode: Option<Mode>,
    /// The last knockback this tick's modes act on.
    pending: Option<Knock>,
    held: Vec<Held>,
    /// The explosion pushes the blast hook recorded since the last tick.
    explosions: Vec<[f64; 3]>,
    /// Tick mode's countdown.
    tick_wait: i32,
    tick_factor: f64,
    tick_reset_y: bool,
    /// JumpReset/Intave's next jump (-1 idle, 0 jump this tick).
    jump_in: i32,
    /// Whether this module inserted Space into the game's key set.
    space: bool,
    /// BlocksMC's sneaking.
    sneak_ticks: i32,
    sneaked: bool,
    /// Hypixel's first air velocity.
    air_absorbed: bool,
    /// Click's interval countdown.
    click_wait: i32,
    /// SideStrafe's position before the knockback.
    saved: Option<[f64; 3]>,
    /// Per-hurt flags.
    aac_applied: bool,
    polar_applied: bool,
    hurt_seen: bool,
    hurt_since: i64,
    /// Grim's packets wait for the next tick (they follow the velocity).
    grim_pending: bool,
    /// NewGrim's PosRot and action packet, likewise.
    newgrim_pending: bool,
    grim_position: [f64; 3],
    grim_rotation: (f32, f32),
    grim_ground: bool,
    /// GrimC03's slow-timer countdown and a knockback waiting for the feet
    /// block to be air.
    grimc03_ticks: i32,
    grimc03_pending: bool,
    rng: u64,
}

impl State {
    fn new(mode: Option<Mode>) -> Self {
        Self {
            mode,
            pending: None,
            held: Vec::new(),
            explosions: Vec::new(),
            tick_wait: 0,
            tick_factor: 0.0,
            tick_reset_y: false,
            jump_in: -1,
            space: false,
            sneak_ticks: 0,
            sneaked: false,
            air_absorbed: false,
            click_wait: 0,
            saved: None,
            aac_applied: false,
            polar_applied: false,
            hurt_seen: false,
            hurt_since: 0,
            grim_pending: false,
            newgrim_pending: false,
            grim_position: [0.0; 3],
            grim_rotation: (0.0, 0.0),
            grim_ground: false,
            grimc03_ticks: 0,
            grimc03_pending: false,
            rng: 0,
        }
    }

    fn random(&mut self) -> f64 {
        if self.rng == 0 {
            self.rng = 0x9E37_79B9_7F4A_7C15;
        }
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.rng >> 11) as f64) / ((1u64 << 53) as f64)
    }

    fn roll(&mut self, chance: i32) -> bool {
        chance >= 100 || self.random() * 100.0 < f64::from(chance.max(0))
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new(None)
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    /// The velocity `remote_send_state` saw after the last tick's physics:
    /// the motion the next tick starts with unless a packet replaces it.
    static LAST: Cell<Option<[f64; 3]>> = const { Cell::new(None) };
}

fn take_state() -> State {
    STATE.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

fn put_state(state: State) {
    STATE.with(|s| *s.borrow_mut() = state);
}

fn last_velocity() -> Option<[f64; 3]> {
    LAST.with(|l| l.get())
}

/// The module's conditions: enabled, playing, in the world with no screen up.
fn running_config(game: &Game<'_>) -> Option<VelocityConfig> {
    if !game.native_window() || !game.native_playing() || !game.native_on(m::ID) {
        return None;
    }
    if game.spectator() || game.screen != Screen::Playing {
        return None;
    }
    game.player.as_ref()?;
    let config = m::velocity()?;
    (config.mode != Mode::Off).then_some(config)
}

fn filter_allows(filter: u8, on_ground: bool) -> bool {
    match filter {
        1 => on_ground,
        2 => !on_ground,
        _ => true,
    }
}

fn movement_input(game: &Game<'_>) -> bool {
    [KeyCode::KeyW, KeyCode::KeyS, KeyCode::KeyA, KeyCode::KeyD]
        .into_iter()
        .any(|key| game.keys.contains(&key))
}

fn is_air(game: &Game<'_>, x: i32, y: i32, z: i32) -> bool {
    game.res.facts().is_air(game.world.block(x, y, z))
}

fn set_velocity(game: &mut Game<'_>, velocity: [f64; 3]) {
    if let Some(player) = game.player.as_mut() {
        player.velocity.x = velocity[0];
        player.velocity.y = velocity[1];
        player.velocity.z = velocity[2];
    }
}

/// The snapshot hook, at the head of `remote_send_state`: remember the
/// velocity this tick's physics left, before the movement packet is built.
pub(super) fn snapshot(game: &Game<'_>) {
    let velocity = game
        .player
        .as_ref()
        .map(|player| [player.velocity.x, player.velocity.y, player.velocity.z]);
    LAST.with(|l| l.set(velocity));
}

/// The explosion hook, at the head of `remote_explode`: the blast's push is
/// added to the player's velocity at its end; remember it so the tick can
/// take it back out and re-add it scaled.
pub(crate) fn explode(game: &mut Game<'_>, packet: &play::Explode) {
    let Some(push) = packet.knockback else {
        return;
    };
    if game.player.is_none() {
        return;
    }
    let mut state = take_state();
    state.explosions.push(push);
    put_state(state);
}

fn differs(a: [f64; 3], b: [f64; 3]) -> bool {
    (a[0] - b[0]).abs() > 1.0e-7 || (a[1] - b[1]).abs() > 1.0e-7 || (a[2] - b[2]).abs() > 1.0e-7
}

/// Whether the mode acts on this knockback, and whether Hypixel absorbs it.
fn eligible(
    config: &VelocityConfig,
    state: &mut State,
    on_ground: bool,
    hurt: bool,
) -> Option<bool> {
    match config.mode {
        Mode::Standard | Mode::Delayed | Mode::Reverse | Mode::SideStrafe => Some(false),
        Mode::Hypixel => {
            if config.air_only && on_ground {
                return None;
            }
            let absorb = !state.air_absorbed;
            state.air_absorbed = true;
            Some(absorb)
        }
        Mode::Phase => {
            if config.phase_ground && !on_ground {
                return None;
            }
            Some(false)
        }
        Mode::AacPush => {
            if !hurt {
                return None;
            }
            Some(false)
        }
        Mode::Motion
        | Mode::Tick
        | Mode::JumpReset
        | Mode::Verus
        | Mode::Grim
        | Mode::NewGrim
        | Mode::BlocksMc
        | Mode::Click
        | Mode::Aac
        | Mode::Polar
        | Mode::Intave
        | Mode::Spoof
        | Mode::GrimC03 => Some(false),
        Mode::Off => None,
    }
}

/// The tick hook, before the player's physics move: rewrite the velocity the
/// packets applied.
pub(super) fn tick(game: &mut Game<'_>) {
    timer::begin_velocity_tick();
    let mut state = take_state();
    let Some(config) = running_config(game) else {
        flush_held(game, &mut state);
        release_keys(game, &state);
        put_state(State::default());
        return;
    };
    if state.mode != Some(config.mode) {
        flush_held(game, &mut state);
        release_keys(game, &state);
        state = State::new(Some(config.mode));
    }

    let Some((position, on_ground, hurt_time, sprinting, velocity, now)) = player_state(game)
    else {
        put_state(state);
        return;
    };

    drop_space(game, &mut state);
    blocksmc_sneak(game, &config, &mut state);
    track_hurt(&mut state, hurt_time, on_ground, now);

    let pushes = std::mem::take(&mut state.explosions);
    let pre = last_velocity().unwrap_or(velocity);
    // Without the blast pushes, what is left is either the motion the last
    // tick left or the velocity a packet replaced it with.
    let mut base = velocity;
    for push in &pushes {
        base[0] -= push[0];
        base[1] -= push[1];
        base[2] -= push[2];
    }
    if differs(base, pre) || !pushes.is_empty() {
        if let Some(absorb) = eligible(&config, &mut state, on_ground, hurt_time > 0) {
            state.pending = Some(Knock {
                velocity: base,
                pre,
                position,
                on_ground,
                absorb,
            });
        }
    }

    apply_pending(game, &config, &mut state);
    apply_blasts(game, &config, &mut state, &pushes, on_ground);
    release_held(game, &mut state);

    if state.grim_pending {
        state.grim_pending = false;
        grim_packets(
            game,
            &config,
            state.grim_position,
            state.grim_rotation,
            state.grim_ground,
        );
    }
    if state.newgrim_pending {
        state.newgrim_pending = false;
        newgrim_packets(
            game,
            state.grim_position,
            state.grim_rotation,
            state.grim_ground,
        );
    }

    match config.mode {
        Mode::Motion => {
            if config.motion_hurt_time > 0 && hurt_time == config.motion_hurt_time {
                if let Some(player) = game.player.as_mut() {
                    player.velocity.x *= f64::from(config.scale_horizontal);
                    player.velocity.z *= f64::from(config.scale_horizontal);
                    player.velocity.y *= f64::from(config.scale_vertical);
                }
            }
        }
        Mode::Tick => {
            if state.tick_wait > 0 {
                state.tick_wait -= 1;
                if state.tick_wait == 0 {
                    if let Some(player) = game.player.as_mut() {
                        player.velocity.x *= state.tick_factor;
                        player.velocity.z *= state.tick_factor;
                        if state.tick_reset_y {
                            player.velocity.y = 0.0;
                        }
                    }
                }
            }
        }
        Mode::JumpReset | Mode::Intave => {
            let chance = if config.mode == Mode::Intave {
                config.intave_jump_chance
            } else {
                config.chance
            };
            if hurt_time == 9
                && on_ground
                && (!config.only_sprinting || sprinting)
                && state.jump_in < 0
                && state.roll(chance)
            {
                state.jump_in = config.jump_delay;
            }
            if state.jump_in > 0 {
                state.jump_in -= 1;
            }
            if state.jump_in == 0 {
                state.jump_in = -1;
                if on_ground && !game.physical_keys.contains(&KeyCode::Space) {
                    game.keys.insert(KeyCode::Space);
                    state.space = true;
                }
            }
        }
        Mode::Click => {
            if state.click_wait > 0 {
                state.click_wait -= 1;
            } else if hurt_time > 0 && hurt_time == config.click_hurt_time {
                for _ in 0..config.click_count {
                    game.remote_start_attack();
                }
                state.click_wait = config.click_interval;
            }
        }
        Mode::SideStrafe => {
            if hurt_time > 0
                && let Some(saved) = state.saved
            {
                let dx = saved[0] - position[0];
                let dz = saved[2] - position[2];
                let length = dx.hypot(dz);
                if length > 1.0e-4 {
                    let pull = f64::from(config.side_strength) / length;
                    if let Some(player) = game.player.as_mut() {
                        player.velocity.x += dx * pull;
                        player.velocity.z += dz * pull;
                    }
                }
            }
        }
        Mode::Aac => {
            if hurt_time > 0 && (!config.aac_only_air || !on_ground) && !state.aac_applied {
                if let Some(player) = game.player.as_mut() {
                    player.velocity.x *= f64::from(config.aac_factor);
                    player.velocity.z *= f64::from(config.aac_factor);
                }
                state.aac_applied = true;
            }
        }
        Mode::Polar => {
            if hurt_time >= config.polar_hurt_min
                && hurt_time <= config.polar_hurt_max
                && !state.polar_applied
            {
                let divisor = f64::from(config.polar_divisor.max(1.0));
                if let Some(player) = game.player.as_mut()
                    && player.velocity.x.abs() < 0.1
                    && player.velocity.z.abs() < 0.1
                {
                    player.velocity.x /= divisor;
                    player.velocity.z /= divisor;
                }
                state.polar_applied = true;
            }
        }
        Mode::GrimC03 => {
            if state.grimc03_ticks > 0 {
                let ramp = 0.8 + 0.2 * f64::from(20 - state.grimc03_ticks) / 20.0;
                timer::request_velocity(ramp as f32);
                state.grimc03_ticks -= 1;
            }
            if state.grimc03_pending {
                let x = position[0].floor() as i32;
                let y = position[1].floor() as i32;
                let z = position[2].floor() as i32;
                if is_air(game, x, y, z) {
                    game.send(S::MovePlayerStatus(play::MovePlayerStatus {
                        flags: ON_GROUND,
                    }));
                    game.send(S::PlayerAction(play::PlayerAction {
                        action: STOP_DESTROY_BLOCK,
                        pos: [x, y, z],
                        direction: DOWN,
                        sequence: 0,
                    }));
                    state.grimc03_pending = false;
                    state.grimc03_ticks = 20;
                }
            }
        }
        Mode::Off
        | Mode::Standard
        | Mode::Delayed
        | Mode::Reverse
        | Mode::Hypixel
        | Mode::Verus
        | Mode::Grim
        | Mode::NewGrim
        | Mode::Spoof
        | Mode::BlocksMc
        | Mode::Phase
        | Mode::AacPush => {}
    }

    put_state(state);
}

/// The attack click, at the head of `remote_start_attack`: Intave slows the
/// player when they hit while still hurt.
pub(crate) fn attack(game: &mut Game<'_>) {
    let Some(config) = running_config(game) else {
        return;
    };
    if config.mode != Mode::Intave {
        return;
    }
    let Some(player) = game.player.as_ref() else {
        return;
    };
    let hurt_time = player.health.hurt.hurt_time;
    let state = take_state();
    let now = game.debug_millis();
    if hurt_time >= config.intave_hurt_min
        && hurt_time <= config.intave_hurt_max
        && state.hurt_seen
        && now - state.hurt_since <= i64::from(config.intave_window)
    {
        if let Some(player) = game.player.as_mut() {
            player.velocity.x *= f64::from(config.intave_factor);
            player.velocity.z *= f64::from(config.intave_factor);
        }
    }
    put_state(state);
}

fn player_state(game: &Game<'_>) -> Option<([f64; 3], bool, i32, bool, [f64; 3], i64)> {
    let player = game.player.as_ref()?;
    Some((
        [player.position.x, player.position.y, player.position.z],
        player.ground.on_ground,
        player.health.hurt.hurt_time,
        player.sprinting,
        [player.velocity.x, player.velocity.y, player.velocity.z],
        game.debug_millis(),
    ))
}

/// Drops the jump key this module inserted last tick.
fn drop_space(game: &mut Game<'_>, state: &mut State) {
    if state.space {
        if !game.physical_keys.contains(&KeyCode::Space) {
            game.keys.remove(&KeyCode::Space);
        }
        state.space = false;
    }
}

/// Drops every key this module inserted (turning off, or changing modes).
fn release_keys(game: &mut Game<'_>, state: &State) {
    if state.space && !game.physical_keys.contains(&KeyCode::Space) {
        game.keys.remove(&KeyCode::Space);
    }
    if state.sneaked && !game.physical_keys.contains(&KeyCode::ShiftLeft) {
        game.keys.remove(&KeyCode::ShiftLeft);
    }
}

/// BlocksMC sneaks for the mode's ticks after the knockback.
fn blocksmc_sneak(game: &mut Game<'_>, config: &VelocityConfig, state: &mut State) {
    if config.mode != Mode::BlocksMc {
        return;
    }
    if state.sneak_ticks > 0 {
        if !game.physical_keys.contains(&KeyCode::ShiftLeft) {
            game.keys.insert(KeyCode::ShiftLeft);
            state.sneaked = true;
        }
        state.sneak_ticks -= 1;
    } else if state.sneaked {
        if !game.physical_keys.contains(&KeyCode::ShiftLeft) {
            game.keys.remove(&KeyCode::ShiftLeft);
        }
        state.sneaked = false;
    }
}

/// Starts a new hurt window, and clears the per-hurt flags when it ends.
fn track_hurt(state: &mut State, hurt_time: i32, on_ground: bool, now: i64) {
    if hurt_time >= 10 {
        state.hurt_seen = true;
        state.hurt_since = now;
        state.aac_applied = false;
        state.polar_applied = false;
        state.jump_in = -1;
        state.click_wait = 0;
    } else if hurt_time == 0 {
        state.hurt_seen = false;
        state.aac_applied = false;
        state.polar_applied = false;
        state.jump_in = -1;
        state.saved = None;
    }
    if on_ground {
        state.air_absorbed = false;
    }
}

/// Puts the blast pushes back: scaled for the Standard mode's explosions,
/// untouched for the modes whose reference clients cancel only the velocity
/// packet.
fn apply_blasts(
    game: &mut Game<'_>,
    config: &VelocityConfig,
    state: &mut State,
    pushes: &[[f64; 3]],
    on_ground: bool,
) {
    if pushes.is_empty() {
        return;
    }
    let scaled = config.mode == Mode::Standard
        && config.explosions
        && state.roll(config.chance)
        && filter_allows(config.filter, on_ground);
    let (horizontal, vertical) = if scaled {
        (f64::from(config.horizontal), f64::from(config.vertical))
    } else {
        (1.0, 1.0)
    };
    let Some(player) = game.player.as_mut() else {
        return;
    };
    for push in pushes {
        player.velocity.x += push[0] * horizontal;
        player.velocity.y += push[1] * vertical;
        player.velocity.z += push[2] * horizontal;
    }
}

/// Applies the mode's handling of the knockback the packets delivered.
fn apply_pending(game: &mut Game<'_>, config: &VelocityConfig, state: &mut State) {
    let Some(knock) = state.pending.take() else {
        return;
    };
    match config.mode {
        Mode::Standard => {
            if !state.roll(config.chance)
                || !filter_allows(config.filter, knock.on_ground)
                || (config.only_move && !movement_input(game))
            {
                return;
            }
            let mut velocity = knock.velocity;
            if config.horizontal != 0.0 {
                velocity[0] *= f64::from(config.horizontal);
                velocity[2] *= f64::from(config.horizontal);
            } else {
                velocity[0] = knock.pre[0] * f64::from(config.motion_horizontal);
                velocity[2] = knock.pre[2] * f64::from(config.motion_horizontal);
            }
            if config.vertical != 0.0 {
                velocity[1] *= f64::from(config.vertical);
            } else {
                velocity[1] = knock.pre[1] * f64::from(config.motion_vertical);
            }
            set_velocity(game, velocity);
        }
        Mode::Delayed => {
            state.held.push(Held {
                velocity: knock.velocity,
                ticks: config.delay_ticks,
            });
            set_velocity(game, [0.0; 3]);
        }
        Mode::Reverse => {
            if !state.roll(config.chance) || (config.only_move && !movement_input(game)) {
                return;
            }
            let mut velocity = knock.velocity;
            velocity[0] *= f64::from(config.x_modifier);
            velocity[2] *= f64::from(config.z_modifier);
            state.held.push(Held {
                velocity,
                ticks: config.delay_ticks,
            });
            set_velocity(game, [0.0; 3]);
        }
        Mode::Tick => {
            state.tick_wait = config.wait_ticks;
            state.tick_factor = f64::from(config.reduction);
            state.tick_reset_y = config.reset_vertical;
        }
        Mode::Hypixel => {
            if knock.absorb {
                set_velocity(game, [0.0; 3]);
            } else {
                set_velocity(game, [knock.pre[0], knock.velocity[1], knock.pre[2]]);
            }
        }
        Mode::Verus => set_velocity(game, [0.0, f64::from(config.y_boost), 0.0]),
        Mode::Grim => {
            set_velocity(game, [0.0; 3]);
            state.grim_pending = true;
            state.grim_position = knock.position;
            state.grim_rotation = (game.yaw, game.pitch);
            state.grim_ground = knock.on_ground;
        }
        // The reference cancels the velocity packet outright: the player
        // keeps the motion they had before it.
        Mode::NewGrim => {
            set_velocity(game, knock.pre);
            state.newgrim_pending = true;
            state.grim_position = knock.position;
            state.grim_rotation = (game.yaw, game.pitch);
            state.grim_ground = knock.on_ground;
        }
        // The reference cancels the velocity packet outright: the player
        // keeps the motion they had before it, and reports the packet's
        // motion as a step with a one-tick slow timer.
        Mode::Spoof => {
            set_velocity(game, knock.pre);
            game.send(S::MovePlayerPos(play::MovePlayerPos {
                x: knock.position[0] + knock.velocity[0],
                y: knock.position[1] + knock.velocity[1],
                z: knock.position[2] + knock.velocity[2],
                flags: 0,
            }));
            if config.spoof_modify_timer {
                timer::request_velocity(config.spoof_timer);
            }
        }
        // The reference leaves the knockback applied; the tick's timer ramp
        // and ghost stop-destroy are the mode.
        Mode::GrimC03 => state.grimc03_pending = true,
        Mode::BlocksMc => {
            set_velocity(game, [0.0; 3]);
            state.sneak_ticks = config.blocksmc_ticks;
        }
        Mode::Phase => {
            set_velocity(game, [0.0; 3]);
            if let Some(player) = game.player.as_mut() {
                player.position.y = knock.position[1] - f64::from(config.phase_height);
            }
        }
        Mode::AacPush => {
            let reducer = f64::from(config.push_reducer.max(1.0));
            let vertical = if config.push_reduce_y {
                knock.velocity[1] - f64::from(config.push_y)
            } else {
                knock.velocity[1]
            };
            set_velocity(
                game,
                [
                    knock.velocity[0] / reducer,
                    vertical,
                    knock.velocity[2] / reducer,
                ],
            );
        }
        Mode::SideStrafe => state.saved = Some(knock.position),
        Mode::Off
        | Mode::Motion
        | Mode::JumpReset
        | Mode::Click
        | Mode::Aac
        | Mode::Polar
        | Mode::Intave => {}
    }
}

/// Releases the Delayed/Reverse queue's due knockbacks.
fn release_held(game: &mut Game<'_>, state: &mut State) {
    if state.held.is_empty() {
        return;
    }
    let mut due = Vec::new();
    for held in &mut state.held {
        if held.ticks > 0 {
            held.ticks -= 1;
        }
        if held.ticks == 0 {
            due.push(*held);
        }
    }
    state.held.retain(|held| held.ticks > 0);
    for held in due {
        set_velocity(game, held.velocity);
    }
}

/// Applies every held knockback now. The velocity was zeroed when the knockback
/// arrived, so dropping the entries would leave the client out of step with a
/// server that already applied it — a rubber-band on the next correction.
fn flush_held(game: &mut Game<'_>, state: &mut State) {
    for held in state.held.drain(..) {
        set_velocity(game, held.velocity);
    }
}

/// Grim's stop-destroy and duplicate position packets, after the knockback.
fn grim_packets(
    game: &mut Game<'_>,
    config: &VelocityConfig,
    position: [f64; 3],
    rotation: (f32, f32),
    ground: bool,
) {
    if config.grim_block_action {
        game.send(S::PlayerAction(play::PlayerAction {
            action: STOP_DESTROY_BLOCK,
            pos: [
                position[0].floor() as i32,
                (position[1] - 1.0).floor() as i32,
                position[2].floor() as i32,
            ],
            direction: UP,
            sequence: 0,
        }));
    }
    let flags = if ground { ON_GROUND } else { 0 };
    for _ in 0..config.grim_duplicates {
        game.send(S::MovePlayerPosRot(play::MovePlayerPosRot {
            x: position[0],
            y: position[1],
            z: position[2],
            yaw: rotation.0,
            pitch: rotation.1,
            flags,
        }));
    }
}

/// NewGrim's packet pair, after the knockback: one PosRot without the
/// horizontal-collision bit, then a stop-destroy at the feet facing down.
fn newgrim_packets(game: &mut Game<'_>, position: [f64; 3], rotation: (f32, f32), ground: bool) {
    let flags = if ground { ON_GROUND } else { 0 };
    game.send(S::MovePlayerPosRot(play::MovePlayerPosRot {
        x: position[0],
        y: position[1],
        z: position[2],
        yaw: rotation.0,
        pitch: rotation.1,
        flags,
    }));
    game.send(S::PlayerAction(play::PlayerAction {
        action: STOP_DESTROY_BLOCK,
        pos: [
            position[0].floor() as i32,
            position[1].floor() as i32,
            position[2].floor() as i32,
        ],
        direction: DOWN,
        sequence: 0,
    }));
}
