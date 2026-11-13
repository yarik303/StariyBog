// Step: a port of Alan's Step, FDPClient's step modes and LiquidBounce's Step.
//
// Copyright (c) 2015-2026 CCBlueX. LiquidBounce is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed in
// the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the
// implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the LICENSE file or <https://www.gnu.org/licenses/>.
//
// The references act at three points of a tick: a pre-motion event that sets
// the step height and starts jump sequences, the step confirmation the
// movement engine fires after a step, and the movement packet itself. The
// mod's tick runs at the head of `tick_remote_level`, before the input and
// physics; the confirm runs at the head of `remote_send_state`, after the
// physics and before the movement packet is built, which is where the step
// confirmation's fake position packets belong.
//
// The step height is the `minecraft:step_height` attribute (0.6 by default):
// a transient modifier raises it for the tick's collision solve, and it is
// removed again whenever the module is off or the mode does not use it.
//
// The duplicates were folded into one entry each: FDPClient's AAC and OldAAC
// send NCP's packets; its Vanilla, NCP, Jump and Vulcan copies and Celestial's
// Matrix, NCP and Vanilla repeat modes already here; LiquidBounce's Legit only
// permits the vanilla auto-jump Jump covers. FDPClient's BlocksMC sends a fake
// stone placement in its C08, which Horizon cannot reproduce, so LiquidBounce's
// 1.9+ BlocksMC is the one ported; FDPClient's timer-based BlocksMCTimer stays
// its own mode.
use crate::game::clickgui as m;
use crate::game::clickgui::{StepConfig, StepMode as Mode};
use crate::game::{Game, Screen};
use horizon_client::clock::clickgui_timer as timer;
use horizon_core::geometry::{Aabb, Vec3};
use horizon_protocol::packets::play::{self, Serverbound as S};
use std::cell::RefCell;
use winit::keyboard::KeyCode;

/// The ground bit of `ServerboundMovePlayerPacket`'s flags.
const ON_GROUND: u8 = 1;
/// LiquidBounce Instant's simulated jump heights, index 0 meaning nothing.
const JUMP_ORDER: [f64; 9] = [
    0.0,
    0.419_999_986_886_98,
    0.753_199_980_521_2,
    1.001_335_979_112_15,
    1.166_109_260_938_214,
    1.249_187_078_744_68,
    1.252_203_340_253_73,
    1.176_759_275_064_24,
    1.024_424_088_213_685,
];

/// The counters and timers the modes keep between ticks and packets.
struct State {
    /// The mode the state belongs to (a change resets the counters).
    mode: Option<Mode>,
    /// Ticks since leaving the ground (the references' `airTicks`).
    air_ticks: i32,
    /// The previous tick's ground state, for the AAC step gate.
    last_on_ground: bool,
    /// AAC4.4.0, NCPNew and Matrix6.7.0 step only when the wall was hit on
    /// two ground ticks in a row.
    can_step: bool,
    /// A step this tick may confirm, and the position it started from.
    is_step: bool,
    step_pos: [f64; 3],
    /// When the last step confirmed, for the delay setting.
    last_step_ms: i64,
    /// Matrix's continuing jump after a two-block step.
    matrix_ticks: i32,
    matrix_jump: bool,
    /// MotionNCP's position in its three-tick motion sequence.
    ncp_next: i32,
    /// Spartan's legit/unlegit packet alternation.
    spartan_switch: bool,
    /// NCPPacketless' jump and its tick count.
    packetless_pending: bool,
    packetless_ticks: i32,
    /// AAC3.3.4's slowly sinking jump.
    aac_step: bool,
    /// Vulcan286's and BlocksMC's multi-tick sequences.
    v286_stage: i32,
    v286_wait: i32,
    v286_counter: i32,
    v286_stepping: bool,
    blocksmc_stage: i32,
    blocksmc_wait: i32,
    blocksmc_stepping: bool,
    /// BlocksMCTimer's timer cycle.
    bmct_stage: i32,
    /// Hypixel's sequence, its height and whether it is running.
    hypixel_stage: i32,
    hypixel_wait: i32,
    hypixel_height: f32,
    hypixel_stepping: bool,
    /// The coming movement packet's spoofs: OldNCP's height offset and
    /// Hypixel's ground bit.
    spoof_y: bool,
    spoof_ground: bool,
    /// LiquidBounce Instant's wait before the next step.
    wait_ticks: i32,
    wait_cycle: u32,
}

impl State {
    fn new(mode: Option<Mode>) -> Self {
        Self {
            mode,
            air_ticks: 0,
            last_on_ground: false,
            can_step: false,
            is_step: false,
            step_pos: [0.0; 3],
            last_step_ms: 0,
            matrix_ticks: 0,
            matrix_jump: false,
            ncp_next: 0,
            spartan_switch: false,
            packetless_pending: false,
            packetless_ticks: 0,
            aac_step: false,
            v286_stage: 0,
            v286_wait: 0,
            v286_counter: 0,
            v286_stepping: false,
            blocksmc_stage: 0,
            blocksmc_wait: 0,
            blocksmc_stepping: false,
            bmct_stage: 0,
            hypixel_stage: 0,
            hypixel_wait: 0,
            hypixel_height: 0.0,
            hypixel_stepping: false,
            spoof_y: false,
            spoof_ground: false,
            wait_ticks: 0,
            wait_cycle: 0,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new(None)
    }
}

/// The parts of the tick the modes read, taken before anything changes.
#[derive(Clone, Copy, Default)]
struct Ctx {
    on_ground: bool,
    horizontal: bool,
    position: [f64; 3],
    yaw: f32,
    dir: [f32; 2],
    moving: bool,
    jump_held: bool,
    sneak_held: bool,
    in_liquid: bool,
    in_web: bool,
    on_ladder: bool,
    riding: bool,
    air_ticks: i32,
    now: i64,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn take_state() -> State {
    STATE.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

fn put_state(state: State) {
    STATE.with(|s| *s.borrow_mut() = state);
}

/// The module's conditions: enabled, playing, in the world with no screen up.
fn running_config(game: &Game<'_>) -> Option<StepConfig> {
    if !game.native_window() || !game.native_playing() || !game.native_on(m::ID) {
        return None;
    }
    if game.spectator() || game.screen != Screen::Playing {
        return None;
    }
    game.player.as_ref()?;
    m::step()
}

fn context(game: &Game<'_>, state: &mut State) -> Ctx {
    let Some(player) = game.player.as_ref() else {
        return Ctx::default();
    };
    let held = |key: KeyCode| game.physical_keys.contains(&key);
    let dir = m::input_direction(
        held(KeyCode::KeyW),
        held(KeyCode::KeyS),
        held(KeyCode::KeyA),
        held(KeyCode::KeyD),
    );
    let on_ground = player.ground.on_ground;
    if on_ground {
        state.air_ticks = 0;
    } else {
        state.air_ticks += 1;
    }
    let in_water = player.fluids.was_touching_water;
    Ctx {
        on_ground,
        horizontal: player.collision.horizontal,
        position: [player.position.x, player.position.y, player.position.z],
        yaw: game.yaw,
        dir,
        moving: dir[0] != 0.0 || dir[1] != 0.0,
        jump_held: held(KeyCode::Space),
        sneak_held: held(KeyCode::ShiftLeft),
        in_liquid: in_water || player.is_in_lava(),
        in_web: in_web(game),
        on_ladder: player.last_climbable.is_some(),
        riding: game.riding.is_some(),
        air_ticks: state.air_ticks,
        now: game.debug_millis(),
    }
}

/// The tick hook, before the player's input and physics move.
pub(super) fn tick(game: &mut Game<'_>) {
    timer::begin_step_tick();
    let mut state = take_state();
    let Some(config) = running_config(game) else {
        cleanup(game);
        put_state(State::default());
        return;
    };
    if state.mode != Some(config.mode) {
        cleanup(game);
        state = State::new(Some(config.mode));
    }
    let ctx = context(game, &mut state);
    state.spoof_y = false;
    state.spoof_ground = config.mode == Mode::Hypixel && config.hypixel_spoof && ctx.air_ticks == 8;
    if ctx.riding {
        state.is_step = false;
        apply_height(game, 0.6);
        put_state(state);
        return;
    }
    if state.wait_ticks > 0 {
        state.wait_ticks -= 1;
        state.is_step = false;
        apply_height(game, 0.6);
        put_state(state);
        return;
    }

    if matches!(config.mode, Mode::Aac440 | Mode::NcpNew | Mode::Matrix670) {
        state.can_step = ctx.horizontal && ctx.on_ground && state.last_on_ground;
    }
    state.last_on_ground = ctx.on_ground;

    let ready = delay_passed(&config, &state, ctx.now);
    let height = target_height(&config, &ctx, &state, ready);
    apply_height(game, height);
    state.is_step = false;
    if height > 0.6 && ctx.on_ground && confirmable(config.mode) {
        state.is_step = true;
        state.step_pos = ctx.position;
    }

    match config.mode {
        Mode::Jump => {
            if ctx.on_ground && ctx.horizontal && !ctx.jump_held && !ctx.sneak_held {
                jump(game, config.jump_height, 0);
            }
        }
        Mode::NcpPacketless => packetless(game, &ctx, &mut state),
        Mode::MotionNcp => motion_ncp(game, &ctx, &mut state),
        Mode::Laac => laac(game, &config, &ctx, &mut state),
        Mode::Aac334 => aac334(game, &ctx, &mut state),
        Mode::Vulcan286 => vulcan286(game, &ctx, &mut state),
        Mode::BlocksMc => blocks_mc(game, &config, &ctx, &mut state),
        Mode::BlocksMcTimer => blocks_mc_timer(game, &ctx, &mut state),
        Mode::Hypixel => hypixel(game, &config, &ctx, &mut state),
        Mode::Matrix => matrix(game, &ctx, &mut state),
        Mode::Vanilla
        | Mode::Ncp
        | Mode::NcpNew
        | Mode::OldNcp
        | Mode::Aac440
        | Mode::Spartan
        | Mode::Rewinside
        | Mode::Vulcan
        | Mode::Verus
        | Mode::Watchdog
        | Mode::Instant
        | Mode::Matrix670 => {}
    }
    put_state(state);
}

/// Whether the mode's step is confirmed with fake position packets (the
/// others move the player themselves and need no confirmation).
fn confirmable(mode: Mode) -> bool {
    !matches!(
        mode,
        Mode::Jump
            | Mode::NcpPacketless
            | Mode::MotionNcp
            | Mode::Laac
            | Mode::Aac334
            | Mode::Vulcan286
            | Mode::BlocksMc
            | Mode::BlocksMcTimer
            | Mode::Hypixel
    )
}

/// Whether the mode's step height honours the delay settings.
fn uses_delay(mode: Mode) -> bool {
    !matches!(
        mode,
        Mode::Jump
            | Mode::NcpPacketless
            | Mode::MotionNcp
            | Mode::Laac
            | Mode::Aac334
            | Mode::Vulcan286
            | Mode::Watchdog
            | Mode::BlocksMc
            | Mode::BlocksMcTimer
            | Mode::Hypixel
            | Mode::Matrix
    )
}

fn delay_passed(config: &StepConfig, state: &State, now: i64) -> bool {
    if !uses_delay(config.mode) {
        return true;
    }
    now - state.last_step_ms >= i64::from(config.delay + config.extra_delay)
}

/// The step height the mode asks for this tick, 0.6 when it wants none.
fn target_height(config: &StepConfig, ctx: &Ctx, state: &State, ready: bool) -> f32 {
    let default = 0.6;
    if ctx.in_liquid || ctx.in_web {
        return default;
    }
    match config.mode {
        Mode::Jump
        | Mode::NcpPacketless
        | Mode::MotionNcp
        | Mode::Laac
        | Mode::Aac334
        | Mode::Vulcan286
        | Mode::BlocksMc
        | Mode::BlocksMcTimer
        | Mode::Hypixel => default,
        Mode::Watchdog => {
            if ctx.on_ground && !ctx.jump_held && !ctx.on_ladder {
                1.0
            } else {
                default
            }
        }
        Mode::Matrix => {
            if config.matrix_two_block {
                2.0
            } else {
                1.0
            }
        }
        Mode::Ncp => {
            if ctx.on_ground && ready {
                config.height
            } else {
                default
            }
        }
        Mode::Aac440 | Mode::NcpNew | Mode::Matrix670 => {
            if state.can_step && ready {
                config.height
            } else {
                default
            }
        }
        _ => {
            if ctx.on_ground && ready {
                config.height
            } else {
                default
            }
        }
    }
}

/// Raises the step height through a transient attribute modifier, or removes
/// it for the modes that use the vanilla height.
fn apply_height(game: &mut Game<'_>, height: f32) {
    let present = height > 0.6001;
    let amount = if present {
        f64::from(height) - 0.6
    } else {
        0.0
    };
    if let Some(player) = game.player.as_mut() {
        let _ = player.set_modifier(
            "minecraft:step_height",
            "clickgui:step",
            amount,
            "add_value",
            present,
        );
    }
}

fn cleanup(game: &mut Game<'_>) {
    apply_height(game, 0.6);
}

/// The confirm hook, after the physics moved the player.
pub(super) fn confirm(game: &mut Game<'_>) {
    let Some(config) = running_config(game) else {
        return;
    };
    let mut state = take_state();
    if state.mode != Some(config.mode) || !state.is_step {
        put_state(state);
        return;
    }
    let Some(player) = game.player.as_ref() else {
        put_state(state);
        return;
    };
    let rise = player.position.y - state.step_pos[1];
    let velocity = [player.velocity.x, player.velocity.y, player.velocity.z];
    if rise <= 0.6 {
        state.is_step = false;
        put_state(state);
        return;
    }
    let origin = state.step_pos;
    let now = game.debug_millis();
    state.last_step_ms = now;
    state.is_step = false;

    if config.simulate_jump {
        jump(game, 0.42, 1);
    }
    if uses_timer(config.mode) && config.timer < 1.0 {
        let multiplier = if config.dynamic_timer {
            config.timer / (rise as f32).sqrt()
        } else {
            config.timer
        };
        timer::request_step(multiplier);
    }

    match config.mode {
        Mode::Vanilla | Mode::Jump | Mode::Aac334 => {}
        Mode::Ncp => ncp_step(game, origin, rise, now),
        Mode::NcpNew => ncp_new_step(game, origin, rise),
        Mode::NcpPacketless | Mode::MotionNcp | Mode::Laac => {}
        Mode::OldNcp => state.spoof_y = true,
        Mode::Aac440 => aac440_step(game, origin, rise, velocity),
        Mode::Spartan => spartan_step(game, origin, &mut state),
        Mode::Rewinside => rewinside_step(game, origin),
        Mode::Vulcan => vulcan_step(game, origin, rise),
        Mode::Vulcan286 | Mode::BlocksMc | Mode::BlocksMcTimer | Mode::Hypixel => {}
        Mode::Verus => verus_step(game, origin, rise),
        Mode::Watchdog => watchdog_step(game, origin, rise),
        Mode::Instant => instant_step(game, &config, &mut state, origin, rise),
        Mode::Matrix => matrix_step(game, &config, &mut state, origin, rise),
        Mode::Matrix670 => matrix670_step(game, origin, rise),
    }
    put_state(state);
}

/// The modes whose user timer setting is requested on a step.
fn uses_timer(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::Vanilla
            | Mode::Ncp
            | Mode::NcpNew
            | Mode::OldNcp
            | Mode::Aac440
            | Mode::Spartan
            | Mode::Rewinside
            | Mode::Vulcan
            | Mode::Instant
    )
}

fn send_pos(game: &mut Game<'_>, x: f64, y: f64, z: f64, ground: bool) {
    game.send(S::MovePlayerPos(play::MovePlayerPos {
        x,
        y,
        z,
        flags: if ground { ON_GROUND } else { 0 },
    }));
}

fn send_full(game: &mut Game<'_>, x: f64, y: f64, z: f64, ground: bool) {
    let (yaw, pitch) = (game.yaw, game.pitch);
    game.send(S::MovePlayerPosRot(play::MovePlayerPosRot {
        x,
        y,
        z,
        yaw,
        pitch,
        flags: if ground { ON_GROUND } else { 0 },
    }));
}

/// Alan's NCP step: the packet height depends on how far the step rose.
fn ncp_step(game: &mut Game<'_>, origin: [f64; 3], rise: f64, now: i64) {
    let offsets: &[f64] = if rise > 2.019 {
        &[
            0.425, 0.821, 0.699, 0.599, 1.022, 1.372, 1.652, 1.869, 2.019, 1.919,
        ]
    } else if rise > 1.869 {
        &[0.425, 0.821, 0.699, 0.599, 1.022, 1.372, 1.652, 1.869]
    } else if rise > 1.5 {
        &[0.425, 0.821, 0.699, 0.599, 1.022, 1.372, 1.652]
    } else if rise > 1.015 {
        &[0.42, 0.7532, 1.01, 1.093, 1.015]
    } else if rise > 0.875 {
        &[0.42, 0.7532]
    } else {
        &[0.39, 0.6938]
    };
    let jitter = (now % 2_000) as f64 / 2_000_000.0;
    for offset in offsets {
        send_pos(
            game,
            origin[0],
            origin[1] + offset + jitter,
            origin[2],
            false,
        );
    }
}

/// FDPClient's NCPNew step: the longer packet ladders, and a stopped player
/// after a step above one and a half blocks.
fn ncp_new_step(game: &mut Game<'_>, origin: [f64; 3], rise: f64) {
    let offsets: &[f64] = if rise > 2.019 {
        &[
            0.425, 0.821, 0.699, 0.599, 1.022, 1.372, 1.652, 1.869, 2.019, 1.919,
        ]
    } else if rise > 1.869 {
        &[0.425, 0.821, 0.699, 0.599, 1.022, 1.372, 1.652, 1.869]
    } else if rise > 1.5 {
        &[0.425, 0.821, 0.699, 0.599, 1.022, 1.372, 1.652]
    } else if rise > 1.015 {
        &[0.42, 0.7532, 1.01, 1.093, 1.015]
    } else if rise > 0.875 {
        &[0.419_999_986_886_98, 0.753_199_980_521_2]
    } else {
        &[0.39, 0.6938]
    };
    for offset in offsets {
        send_pos(game, origin[0], origin[1] + offset, origin[2], false);
    }
    if rise > 1.5
        && let Some(player) = game.player.as_mut()
    {
        player.velocity.x = 0.0;
        player.velocity.z = 0.0;
    }
}

/// FDPClient's AAC4.4.0 step: three ladders picked by the risen height.
fn aac440_step(game: &mut Game<'_>, origin: [f64; 3], rise: f64, velocity: [f64; 3]) {
    let edge = 0.015_625;
    if rise >= 1.0 - edge && rise < 1.5 - edge {
        for offset in [0.4, 0.7, 0.9] {
            send_pos(game, origin[0], origin[1] + offset, origin[2], false);
        }
        send_pos(game, origin[0], origin[1] + 1.0, origin[2], true);
    } else if rise >= 1.5 - edge && rise < 2.0 - edge {
        for offset in [0.42, 0.7718, 1.0556, 1.2714, 1.412] {
            send_pos(game, origin[0], origin[1] + offset, origin[2], false);
        }
        send_pos(game, origin[0], origin[1] + 1.50, origin[2], true);
    } else if rise >= 2.0 - edge {
        for offset in [0.45, 0.84375, 1.18125, 1.4625, 1.6875, 1.85625, 1.96875] {
            send_pos(game, origin[0], origin[1] + offset, origin[2], false);
        }
        send_pos(
            game,
            origin[0] + velocity[0] * 0.5,
            origin[1] + 2.0,
            origin[2] + velocity[2] * 0.5,
            true,
        );
    }
}

/// FDPClient's Spartan step: alternate a forced half-block packet with the
/// vanilla three-packet ladder.
fn spartan_step(game: &mut Game<'_>, origin: [f64; 3], state: &mut State) {
    if state.spartan_switch {
        for offset in [
            0.419_999_986_886_98,
            0.753_199_980_521_2,
            1.001_335_979_112_147,
        ] {
            send_pos(game, origin[0], origin[1] + offset, origin[2], false);
        }
    } else {
        send_pos(game, origin[0], origin[1] + 0.6, origin[2], false);
    }
    state.spartan_switch = !state.spartan_switch;
}

fn rewinside_step(game: &mut Game<'_>, origin: [f64; 3]) {
    for offset in [
        0.419_999_986_886_98,
        0.753_199_980_521_2,
        1.001_335_979_112_147,
    ] {
        send_pos(game, origin[0], origin[1] + offset, origin[2], false);
    }
}

/// FDPClient's Vulcan step: half-block ladder, ground bit on every packet.
fn vulcan_step(game: &mut Game<'_>, origin: [f64; 3], rise: f64) {
    let offsets: &[f64] = if rise > 2.0 {
        &[0.5, 1.0, 1.5, 2.0]
    } else if rise > 1.5 {
        &[0.5, 1.0, 1.5]
    } else if rise > 1.0 {
        &[0.5, 1.0]
    } else {
        &[0.5]
    };
    for offset in offsets {
        send_pos(game, origin[0], origin[1] + offset, origin[2], true);
    }
}

/// FDPClient's Verus step: half-block packets and the matching timer.
fn verus_step(game: &mut Game<'_>, origin: [f64; 3], rise: f64) {
    let steps = (rise * 2.0).ceil() as i32;
    if steps <= 0 {
        return;
    }
    timer::request_step(1.0 / steps as f32);
    let mut offset = 0.0;
    for _ in 0..(steps - 1) {
        offset += 0.5;
        send_pos(game, origin[0], origin[1] + offset, origin[2], true);
    }
}

/// Alan's Watchdog step: a single-block step repeats the jump arc at a
/// quarter timer.
fn watchdog_step(game: &mut Game<'_>, origin: [f64; 3], rise: f64) {
    if (rise - 1.0).abs() > 1.0e-6 {
        return;
    }
    for offset in [0.42, 0.75, 1.0] {
        send_pos(game, origin[0], origin[1] + offset, origin[2], false);
    }
    timer::request_step(0.25);
}

/// LiquidBounce Instant: send the configured slice of the jump arc.
fn instant_step(
    game: &mut Game<'_>,
    config: &StepConfig,
    state: &mut State,
    _origin: [f64; 3],
    rise: f64,
) {
    let (min, max) = (
        config.instant_order_min.min(8) as usize,
        config.instant_order_max.min(8) as usize,
    );
    let (min, max) = (min.min(max), max);
    if min == 0 && max == 0 {
        return;
    }
    let Some(player) = game.player.as_ref() else {
        return;
    };
    let (x, z) = (player.position.x, player.position.z);
    let base = player.position.y;
    let limit = base + rise;
    for additional in &JUMP_ORDER[min..=max] {
        if *additional == 0.0 {
            continue;
        }
        let mut y = base + additional;
        if config.instant_trim {
            y = y.min(limit);
        }
        if config.instant_full {
            send_full(game, x, y, z, true);
        } else {
            send_pos(game, x, y, z, true);
        }
    }
    let (wait_min, wait_max) = (
        config.instant_wait_min.min(60),
        config.instant_wait_max.min(60),
    );
    let (wait_min, wait_max) = (wait_min.min(wait_max), wait_max);
    let span = wait_max - wait_min + 1;
    state.wait_ticks = (wait_min + state.wait_cycle % span) as i32;
    state.wait_cycle = state.wait_cycle.wrapping_add(1);
}

/// Alan's Matrix step: the one-block case sends a short ladder, the two-block
/// case either its long ladder or a continuing jump.
fn matrix_step(
    game: &mut Game<'_>,
    config: &StepConfig,
    state: &mut State,
    origin: [f64; 3],
    rise: f64,
) {
    if rise > 1.0 {
        if config.matrix_instant {
            let offsets = [
                0.419_999_986_886_98,
                0.753_199_980_521_2,
                1.001_335_979_112_15,
                1.421_335_965_999_13,
                1.754_535_959_633_35,
                2.002_671_958_224_3,
            ];
            for (index, offset) in offsets.iter().enumerate() {
                send_pos(game, origin[0], origin[1] + offset, origin[2], index == 2);
            }
            timer::request_step(0.142_857_15);
        } else {
            state.matrix_jump = true;
            state.matrix_ticks = 0;
        }
    } else {
        timer::request_step(0.333_33);
        send_pos(game, origin[0], origin[1] + 0.42, origin[2], false);
        send_pos(game, origin[0], origin[1] + 0.42, origin[2], true);
    }
}

/// FDPClient's Matrix6.7.0 step: a ladder per tenth of a block.
fn matrix670_step(game: &mut Game<'_>, origin: [f64; 3], rise: f64) {
    const GROUND_99990: f64 = 0.999_9;
    const GROUND_75320: f64 = 0.753_199_980_521_2;
    let (offsets, speed, ground): (&[f64], f32, f64) = if rise > 2.95 {
        (
            &[
                0.419_51, 0.752_23, 0.999_9, 1.429_89, 1.772_89, 2.040_32, 2.233_71, 2.354_53,
                2.404_23,
            ],
            0.11,
            GROUND_99990,
        )
    } else if rise > 2.83 {
        (
            &[
                0.419_51, 0.752_23, 0.999_9, 1.429_89, 1.772_89, 2.040_32, 2.233_71, 2.354_53,
            ],
            0.12,
            GROUND_99990,
        )
    } else if rise > 2.64 {
        (
            &[
                0.419_51, 0.752_23, 0.999_9, 1.429_89, 1.772_89, 2.040_32, 2.233_71,
            ],
            0.13,
            GROUND_99990,
        )
    } else if rise > 2.37 {
        (
            &[0.419_51, 0.752_23, 0.999_9, 1.429_89, 1.772_89, 2.040_32],
            0.14,
            GROUND_99990,
        )
    } else if rise > 2.02 {
        (
            &[0.419_51, 0.752_23, 0.999_9, 1.429_89, 1.772_89],
            0.16,
            GROUND_99990,
        )
    } else if rise > 1.77 {
        (&[0.419_51, 0.752_23, 0.999_9, 1.429_89], 0.21, GROUND_99990)
    } else if rise > 1.6 {
        (
            &[
                0.419_999_986_886_98,
                0.753_199_980_521_2,
                1.173_199_967_408_18,
            ],
            0.28,
            GROUND_75320,
        )
    } else if rise > 1.3525 {
        (
            &[
                0.419_999_986_886_98,
                0.753_199_980_521_2,
                1.001_335_979_112_147,
            ],
            0.28,
            -1.0,
        )
    } else if rise > 1.02 {
        (&[0.419_999_986_886_98, 0.753_199_980_521_2], 0.34, -1.0)
    } else {
        (&[0.419_999_986_886_98], 0.5, -1.0)
    };
    for offset in offsets {
        send_pos(
            game,
            origin[0],
            origin[1] + offset,
            origin[2],
            (*offset - ground).abs() < 1.0e-9,
        );
    }
    timer::request_step(speed);
}

/// Jumps this tick with `power`, the sprint impulse and the same-tick jump
/// guard the references' jump does.
fn jump(game: &mut Game<'_>, power: f32, impulses: i32) {
    if !(power > 1.0e-5) {
        return;
    }
    let yaw = game.yaw;
    let Some(player) = game.player.as_mut() else {
        return;
    };
    player.velocity.y = f64::from(power).max(player.velocity.y);
    if player.sprinting {
        let rad = f64::from(yaw.to_radians());
        for _ in 0..impulses.max(0) {
            player.velocity.x -= rad.sin() * 0.2;
            player.velocity.z += rad.cos() * 0.2;
        }
    }
    player.no_jump_delay = 2;
}

/// Alan's NCPPacketless: jump at the wall, then restore NCP's predicted
/// motion and a little under the allowed distance three ticks later.
fn packetless(game: &mut Game<'_>, ctx: &Ctx, state: &mut State) {
    if state.packetless_pending {
        state.packetless_ticks += 1;
        if state.packetless_ticks >= 3 {
            let current = game.player.as_ref().map_or(0.0, |p| p.velocity.y) as f32;
            set_y(game, m::predicted_motion(current, 2));
            let speed_amp = game.potions.effects.amplifier("speed").unwrap_or(-1);
            let slowness = game.potions.effects.amplifier("slowness").unwrap_or(-1);
            let noise = (ctx.now % 100) as f32 / 100.0;
            let speed =
                m::allowed_horizontal_distance(speed_amp, slowness) * 0.6 - noise / 100.0 - 0.05;
            strafe(game, ctx, speed.max(0.0));
            state.packetless_pending = false;
        }
        return;
    }
    let jumping = game.potions.effects.amplifier("jump_boost").is_some();
    if ctx.on_ground && ctx.horizontal && !jumping && !ctx.in_liquid {
        jump(game, 0.42, 1);
        stop_xz(game);
        state.packetless_pending = true;
        state.packetless_ticks = 0;
    }
}

/// FDPClient's MotionNCP: the NCP ladder as real motion, three ticks long.
fn motion_ncp(game: &mut Game<'_>, ctx: &Ctx, state: &mut State) {
    if !ctx.moving || !ctx.horizontal || ctx.jump_held {
        state.ncp_next = 0;
        return;
    }
    match state.ncp_next {
        0 => {
            if ctx.on_ground && could_step(game, ctx) {
                stop_xz(game);
                set_y(game, 0.419_999_986_886_98);
                state.ncp_next = 1;
            }
        }
        1 => {
            set_y(game, 0.333_199_993_634_22);
            state.ncp_next = 2;
        }
        2 => {
            set_y(game, 0.248_135_998_590_947);
            let radar = m::movement_yaw(ctx.yaw, ctx.dir).to_radians();
            let (x, z) = (-radar.sin() * 0.7, radar.cos() * 0.7);
            if let Some(player) = game.player.as_mut() {
                player.velocity.x = f64::from(x);
                player.velocity.z = f64::from(z);
            }
            state.ncp_next = 0;
        }
        _ => state.ncp_next = 0,
    }
}

/// FDPClient's LAAC: a high jump at the wall, on a delay.
fn laac(game: &mut Game<'_>, config: &StepConfig, ctx: &Ctx, state: &mut State) {
    if !ctx.moving || !ctx.horizontal {
        return;
    }
    let ready = ctx.now - state.last_step_ms >= i64::from(config.delay);
    if ctx.on_ground && ready {
        add_y(game, 0.620_000_001_490_116);
        let radar = m::movement_yaw(ctx.yaw, ctx.dir).to_radians();
        if let Some(player) = game.player.as_mut() {
            player.velocity.x -= f64::from(radar.sin()) * 0.2;
            player.velocity.z += f64::from(radar.cos()) * 0.2;
        }
        state.last_step_ms = ctx.now;
    }
}

/// FDPClient's AAC3.3.4: a faster jump that sinks slowly afterwards.
fn aac334(game: &mut Game<'_>, ctx: &Ctx, state: &mut State) {
    if !ctx.horizontal || !ctx.moving {
        state.aac_step = false;
        return;
    }
    if ctx.on_ground && could_step(game, ctx) {
        multiply_xz(game, 1.26);
        jump(game, 0.42, 1);
        state.aac_step = true;
    }
    if state.aac_step {
        add_y(game, -0.015);
    }
}

/// LiquidBounce's Vulcan286: a two-tick hop with a half-block lift, on a
/// per-jump alternate.
fn vulcan286(game: &mut Game<'_>, ctx: &Ctx, state: &mut State) {
    if state.v286_stepping {
        state.v286_wait -= 1;
        if state.v286_wait > 0 {
            return;
        }
        match state.v286_stage {
            1 => {
                if state.v286_counter % 2 == 0 {
                    set_y(game, 0.246_800_019_478_800_04);
                    strafe(game, ctx, 0.2);
                }
                state.v286_stage = 2;
                state.v286_wait = 1;
            }
            2 => {
                if state.v286_counter % 2 == 0 {
                    set_y(game, 0.0);
                }
                state.v286_stage = 3;
                state.v286_wait = 1;
            }
            3 => {
                set_y(game, -0.17);
                state.v286_stage = 0;
                state.v286_stepping = false;
            }
            _ => state.v286_stepping = false,
        }
        return;
    }
    if !ctx.on_ground || !can_step(game, 1.0) {
        return;
    }
    jump(game, 0.42, 1);
    state.v286_counter = state.v286_counter.wrapping_add(1);
    state.v286_stepping = true;
    state.v286_stage = 1;
    state.v286_wait = 2;
}

/// LiquidBounce's BlocksMC (1.9+): a three-tick timer burst carries the step.
fn blocks_mc(game: &mut Game<'_>, config: &StepConfig, ctx: &Ctx, state: &mut State) {
    if state.blocksmc_stepping {
        timer::request_step(config.blocksmc_base_timer);
        state.blocksmc_wait -= 1;
        if state.blocksmc_wait > 0 {
            return;
        }
        match state.blocksmc_stage {
            1 => {
                set_y(game, 0.33);
                state.blocksmc_stage = 2;
                state.blocksmc_wait = 1;
            }
            2 => {
                set_y(game, 0.25);
                state.blocksmc_stage = 3;
                state.blocksmc_wait = 2;
            }
            3 => {
                strafe(game, ctx, 0.281);
                if let Some(player) = game.player.as_mut() {
                    player.velocity.y -= player.position.y % 1.0;
                }
                timer::request_step(config.blocksmc_recovery_timer);
                state.blocksmc_stage = 0;
                state.blocksmc_stepping = false;
            }
            _ => state.blocksmc_stepping = false,
        }
        return;
    }
    // LiquidBounce's mode is off while its Speed module runs.
    if !ctx.on_ground || m::speed().is_some() {
        return;
    }
    if !can_step(game, 1.0) {
        return;
    }
    jump(game, 0.42, 1);
    timer::request_step(config.blocksmc_base_timer);
    state.blocksmc_stepping = true;
    state.blocksmc_stage = 1;
    state.blocksmc_wait = 1;
}

/// FDPClient's BlocksMCTimer: three timed ticks then a strafe onto the block.
/// The reference's nearby-chest bail-out is left out: Horizon has no chest
/// search over the world's block entities.
fn blocks_mc_timer(game: &mut Game<'_>, ctx: &Ctx, state: &mut State) {
    if state.bmct_stage > 0 {
        state.bmct_stage += 1;
        match state.bmct_stage {
            2 => timer::request_step(0.2),
            3 => timer::request_step(4.0),
            4 => {
                strafe(game, ctx, 0.27);
                state.bmct_stage = 0;
            }
            _ => {}
        }
        return;
    }
    if !ctx.moving || !ctx.on_ground || !ctx.horizontal || ctx.jump_held {
        return;
    }
    if !could_step(game, ctx) {
        return;
    }
    jump(game, 0.42, 1);
    timer::request_step(5.0);
    state.bmct_stage = 1;
}

/// LiquidBounce's Hypixel: a real jump raised by 0.061 for tall steps, with
/// an alternate ground spoof or a second jump.
fn hypixel(game: &mut Game<'_>, config: &StepConfig, ctx: &Ctx, state: &mut State) {
    if state.hypixel_stepping {
        state.hypixel_wait -= 1;
        if state.hypixel_wait > 0 {
            if state.hypixel_stage == 3 && config.hypixel_alternate {
                state.spoof_ground = true;
            }
            return;
        }
        match state.hypixel_stage {
            1 => {
                if state.hypixel_height > 1.0 {
                    add_y(game, 0.061);
                }
                state.hypixel_stage = 2;
                state.hypixel_wait = 2;
            }
            2 => {
                if (state.hypixel_height - 1.0).abs() < 1.0e-6 {
                    add_y(game, -0.14);
                    hypixel_finish(game, ctx, state);
                } else {
                    add_y(game, -0.095);
                    if state.hypixel_height > 1.25 {
                        state.hypixel_stage = 3;
                        state.hypixel_wait = 5;
                    } else {
                        hypixel_finish(game, ctx, state);
                    }
                }
            }
            3 => {
                if config.hypixel_alternate {
                    state.spoof_ground = true;
                } else {
                    set_y(game, 0.42);
                }
                hypixel_finish(game, ctx, state);
            }
            _ => {
                state.hypixel_stage = 0;
                state.hypixel_stepping = false;
            }
        }
        return;
    }
    if !ctx.on_ground || !can_step(game, 1.5) {
        return;
    }
    state.hypixel_height = if can_step(game, 1.0) {
        1.0
    } else if can_step(game, 1.25) {
        1.25
    } else {
        1.5
    };
    jump(game, 0.42, 1);
    state.hypixel_stepping = true;
    state.hypixel_stage = 1;
    state.hypixel_wait = 1;
}

fn hypixel_finish(game: &mut Game<'_>, ctx: &Ctx, state: &mut State) {
    strafe(game, ctx, 0.183_860_140_745_907_4);
    state.hypixel_stage = 0;
    state.hypixel_stepping = false;
}

/// Alan's Matrix: after a two-block step, a jump every third tick until the
/// ground is reached again.
fn matrix(game: &mut Game<'_>, ctx: &Ctx, state: &mut State) {
    if !state.matrix_jump {
        return;
    }
    if (state.matrix_ticks > 0 && ctx.on_ground) || state.matrix_ticks > 5 {
        state.matrix_ticks = 0;
        state.matrix_jump = false;
        return;
    }
    if state.matrix_ticks % 3 == 0 {
        jump(game, 0.42, 1);
    }
    state.matrix_ticks += 1;
}

/// LiquidBounce's `canStep`: the player's box above and ahead is free.
fn can_step(game: &Game<'_>, height: f32) -> bool {
    let Some(player) = game.player.as_ref() else {
        return false;
    };
    let bounds = player.bounds().move_by(Vec3::new(
        player.velocity.x,
        f64::from(height),
        player.velocity.z,
    ));
    !box_hits(game, bounds)
}

/// FDPClient's `couldStep`: the box a full block up is free in every
/// direction within eighty degrees of the movement input.
fn could_step(game: &Game<'_>, ctx: &Ctx) -> bool {
    let Some(player) = game.player.as_ref() else {
        return false;
    };
    if ctx.sneak_held || ctx.jump_held {
        return false;
    }
    let yaw = m::movement_yaw(ctx.yaw, ctx.dir);
    for step in -10..=10 {
        let adjusted = (yaw + step as f32 * 8.0).to_radians();
        let bounds = player.bounds().move_by(Vec3::new(
            f64::from(-adjusted.sin()) * 0.2,
            1.001_335_979_112_147,
            f64::from(adjusted.cos()) * 0.2,
        ));
        if box_hits(game, bounds) {
            return false;
        }
    }
    true
}

fn box_hits(game: &Game<'_>, bounds: Aabb) -> bool {
    let min = bounds.min_position();
    let max = bounds.max_position();
    for x in min.x.floor() as i32..=max.x.floor() as i32 {
        for y in min.y.floor() as i32..=max.y.floor() as i32 {
            for z in min.z.floor() as i32..=max.z.floor() as i32 {
                if solid_at(game, x, y, z) {
                    return true;
                }
            }
        }
    }
    false
}

fn solid_at(game: &Game<'_>, x: i32, y: i32, z: i32) -> bool {
    let state = game.world.block(x, y, z);
    let facts = game.res.facts();
    if facts.is_air(state) {
        return false;
    }
    match game.res.collision().cached_shapes().cached_shape(state) {
        Ok(Some(shape)) => !shape.is_empty(),
        Ok(None) => facts.get(state).fluid_level == 0,
        Err(_) => facts.get(state).full_collision,
    }
}

fn in_web(game: &Game<'_>) -> bool {
    let Some(player) = game.player.as_ref() else {
        return false;
    };
    let x = player.position.x.floor() as i32;
    let y = player.position.y.floor() as i32;
    let z = player.position.z.floor() as i32;
    let web = |x: i32, y: i32, z: i32| {
        let state = game.world.block(x, y, z);
        game.res.facts().get(state).name.as_ref() == "minecraft:cobweb"
    };
    web(x, y, z) || web(x, y + 1, z)
}

fn set_y(game: &mut Game<'_>, y: f32) {
    if let Some(player) = game.player.as_mut() {
        player.velocity.y = f64::from(y);
    }
}

fn add_y(game: &mut Game<'_>, delta: f32) {
    if let Some(player) = game.player.as_mut() {
        player.velocity.y += f64::from(delta);
    }
}

fn multiply_xz(game: &mut Game<'_>, factor: f32) {
    if let Some(player) = game.player.as_mut() {
        player.velocity.x *= f64::from(factor);
        player.velocity.z *= f64::from(factor);
    }
}

fn stop_xz(game: &mut Game<'_>) {
    if let Some(player) = game.player.as_mut() {
        player.velocity.x = 0.0;
        player.velocity.z = 0.0;
    }
}

fn strafe(game: &mut Game<'_>, ctx: &Ctx, speed: f32) {
    let Some(player) = game.player.as_mut() else {
        return;
    };
    let current = [player.velocity.x as f32, player.velocity.z as f32];
    let [x, z] = m::with_strafe(current, ctx.dir, ctx.yaw, Some(speed), 1.0);
    player.velocity.x = f64::from(x);
    player.velocity.z = f64::from(z);
}

/// The movement packet rewrite: OldNCP lifts the reported height, Hypixel
/// forces the ground bit while its bypass or spoof asks for it.
pub(super) fn spoof(packet: Option<S>) -> Option<S> {
    let Some(config) = m::step() else {
        return packet;
    };
    let packet = packet?;
    let (mode, spoof_y, spoof_ground) = STATE.with(|s| {
        let state = s.borrow();
        (state.mode, state.spoof_y, state.spoof_ground)
    });
    if mode != Some(config.mode) {
        return Some(packet);
    }
    let mut packet = packet;
    if spoof_y {
        packet = match packet {
            S::MovePlayerPos(mut p) => {
                p.y += 0.07;
                S::MovePlayerPos(p)
            }
            S::MovePlayerPosRot(mut p) => {
                p.y += 0.07;
                S::MovePlayerPosRot(p)
            }
            other => other,
        };
    }
    if spoof_ground {
        packet = match packet {
            S::MovePlayerPos(mut p) => {
                p.flags |= ON_GROUND;
                S::MovePlayerPos(p)
            }
            S::MovePlayerPosRot(mut p) => {
                p.flags |= ON_GROUND;
                S::MovePlayerPosRot(p)
            }
            S::MovePlayerRot(mut p) => {
                p.flags |= ON_GROUND;
                S::MovePlayerRot(p)
            }
            S::MovePlayerStatus(mut p) => {
                p.flags |= ON_GROUND;
                S::MovePlayerStatus(p)
            }
            other => other,
        };
    }
    Some(packet)
}
