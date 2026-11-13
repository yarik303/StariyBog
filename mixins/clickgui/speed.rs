// Speed: a port of LiquidBounce's Speed modes, the FDPClient speed modes, and
// the Alan, Librecraft, KoksCraft, Spectre and Swift speed implementations.
//
// Copyright (c) 2015-2026 CCBlueX. LiquidBounce is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed in
// the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the
// implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the LICENSE file or <https://www.gnu.org/licenses/>.
//
// The reference clients act from three places a game tick does not expose to
// the mod directly. `tick` runs at the head of `tick_remote_level`, before the
// tick's input and physics, which is where their tick/update/pre-motion events
// fire; every mode shapes the velocity there. The jump is rebuilt by hand
// (jump strength plus the Jump Boost effect, the sprint impulse and a
// `no_jump_delay` so the same tick's held Space cannot jump twice), which is
// what their movement-input and jump events end up doing. `snapshot` runs at
// the head of `remote_send_state`, after the physics, and remembers the
// velocity the next tick starts with; a different velocity at the next tick's
// head is a server `SetEntityMotion` (a knockback). `teleport` watches the
// server's setbacks, which reset the counter-based modes. `spoof` rewrites the
// movement packet the position sender produced: Vulcan288 reports the ground
// while falling and VulcanGround286 nudges the reported height.
//
// WatchdogTerrain is the one mode the velocity hooks cannot express: its
// reference changes the movement friction, so the tick scales the input the
// movement tick consumes through `clickgui_terrain` instead.
use crate::game::clickgui as m;
use crate::game::clickgui::{SpeedConfig, SpeedMode as Mode};
use crate::game::{Game, Screen};
use horizon_client::clock::clickgui_timer as timer;
use horizon_client::motion::clickgui_terrain as terrain;
use horizon_core::geometry::Vec3;
use horizon_protocol::packets::play::{self, Serverbound as S};
use std::cell::{Cell, RefCell};
use winit::keyboard::KeyCode;

/// The ground bit of `ServerboundMovePlayerPacket`'s flags.
const ON_GROUND: u8 = 1;
/// HypixelBHop's fall-damage speed floor (`BASH`).
const BASH: f32 = 0.285_767_2;
/// The low-hop vertical motion the FDPClient modes use.
const LOW_HOP_Y: f64 = -0.098_000_001_907_348_63;

/// The counters and timers the modes keep between ticks and packets.
struct State {
    /// The mode the state belongs to (a change resets the counters).
    mode: Option<Mode>,
    /// Ticks since leaving the ground (the references' `airTicks`).
    air_ticks: i32,
    /// Ticks since the jump the mode started (-1 when none is running).
    jump_tick: i32,
    /// Horizontal distance covered since leaving the ground (`flyDist`).
    air_distance: f32,
    /// HypixelBHop: the server has set the player back since the last jump.
    was_flagged: bool,
    /// NCP's pull-down counter.
    ncp_air: i32,
    /// NCPBHop's level machine and its five-tick timer cycle.
    ncpbhop_level: i32,
    ncpbhop_speed: f64,
    ncpbhop_timer_delay: i32,
    /// VerusB3882's tick until the next 2x timer request.
    verus_timer_wait: i32,
    /// NCPYPort's jumps made.
    ncp_y_jumps: i32,
    /// BlocksMC's state and setback countdown.
    blocksmc_state: i32,
    blocksmc_flag_delay: i32,
    /// KoksCraft's ground jumps.
    koks_jumps: i32,
    /// Librecraft's tick counter.
    libre_ticks: i32,
    /// Swift's alternating ground factor.
    swift_delay: i32,
    /// Matrix's sneak timer and whether the mod holds the sneak key down.
    matrix_sneak_ticks: i32,
    matrix_shift: bool,
    /// Frame's state machine and timer.
    frame_motion: i32,
    frame_move: bool,
    frame_timer: i64,
    /// SpectreOnGround's accumulated pushes.
    spectre_speed_up: i32,
    /// The Verus family's decaying speed.
    verus_speed: f32,
    /// HylexGround's ground ticks.
    hylex_ground_ticks: i32,
    /// SentinelDamage's damage state.
    sentinel_has_been_hurt: bool,
    sentinel_adjusted: bool,
    sentinel_external_adjust: i32,
    sentinel_wait: i32,
    sentinel_last_damage: i64,
    /// Vulcan288's packet ground bit, from the snapshot.
    spoof_ground: bool,
    /// VulcanGround286's reported-height nudge, from the snapshot.
    spoof_y_005: bool,
    /// WatchdogTerrain's ticks off the stairs and slabs, and whether it took
    /// the jump key out of the tick's key set.
    off_stair_ticks: i32,
    off_slab_ticks: i32,
    terrain_jump: bool,
}

impl State {
    fn new(mode: Option<Mode>) -> Self {
        Self {
            mode,
            air_ticks: 0,
            jump_tick: -1,
            air_distance: 0.0,
            was_flagged: false,
            ncp_air: 0,
            ncpbhop_level: 1,
            ncpbhop_speed: 0.2873,
            ncpbhop_timer_delay: 0,
            verus_timer_wait: 0,
            ncp_y_jumps: 0,
            blocksmc_state: 0,
            blocksmc_flag_delay: 0,
            koks_jumps: 0,
            libre_ticks: 0,
            swift_delay: 0,
            matrix_sneak_ticks: 0,
            matrix_shift: false,
            frame_motion: 0,
            frame_move: false,
            frame_timer: 0,
            spectre_speed_up: 0,
            verus_speed: 0.34,
            hylex_ground_ticks: 0,
            sentinel_has_been_hurt: false,
            sentinel_adjusted: false,
            sentinel_external_adjust: 0,
            sentinel_wait: 0,
            sentinel_last_damage: 0,
            spoof_ground: false,
            spoof_y_005: false,
            off_stair_ticks: 0,
            off_slab_ticks: 0,
            terrain_jump: false,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new(None)
    }
}

/// The parts of the tick every mode reads, taken before anything changes.
#[derive(Clone, Copy, Default)]
struct Ctx {
    dir: [f32; 2],
    moving: bool,
    on_ground: bool,
    yaw: f32,
    pitch: f32,
    hurt_time: i32,
    speed_level: Option<i32>,
    amp: i32,
    sprinting: bool,
    fall_distance: f64,
    position: [f64; 3],
    velocity: [f64; 3],
    hspeed: f32,
    last_dist: f32,
    knock: bool,
    air_ticks: i32,
    using: bool,
    riding: bool,
    in_water: bool,
    in_liquid: bool,
    in_web: bool,
    on_ladder: bool,
    leather_boots: bool,
    forward: bool,
    sideways: bool,
    jump_held: bool,
    shift_held: bool,
    raw_forward: f32,
    raw_strafe: f32,
    now: i64,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    /// The velocity `remote_send_state` saw after the last tick's physics.
    static LAST: Cell<Option<[f64; 3]>> = const { Cell::new(None) };
}

fn take_state() -> State {
    STATE.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

fn put_state(state: State) {
    STATE.with(|s| *s.borrow_mut() = state);
}

/// The module's conditions: enabled, playing, in the world with no screen up.
fn running_config(game: &Game<'_>) -> Option<SpeedConfig> {
    if !game.native_window() || !game.native_playing() || !game.native_on(m::ID) {
        return None;
    }
    if game.spectator() || game.screen != Screen::Playing {
        return None;
    }
    game.player.as_ref()?;
    m::speed()
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
    let position = [player.position.x, player.position.y, player.position.z];
    let old = player.old_position;
    let velocity = [player.velocity.x, player.velocity.y, player.velocity.z];
    let on_ground = player.ground.on_ground;
    if on_ground {
        state.air_ticks = 0;
        state.air_distance = 0.0;
    } else {
        state.air_ticks += 1;
        let last_dist = ((position[0] - old.x).powi(2) + (position[2] - old.z).powi(2)).sqrt();
        state.air_distance += last_dist as f32;
    }
    let last_dist = ((position[0] - old.x).powi(2) + (position[2] - old.z).powi(2)).sqrt() as f32;
    let hspeed = (velocity[0] * velocity[0] + velocity[2] * velocity[2]).sqrt() as f32;
    let speed_level = game.potions.effects.amplifier("speed");
    let in_water = player.fluids.was_touching_water;
    Ctx {
        dir,
        moving: dir[0] != 0.0 || dir[1] != 0.0,
        on_ground,
        yaw: game.yaw,
        pitch: game.pitch,
        hurt_time: player.health.hurt.hurt_time,
        speed_level,
        amp: speed_level.unwrap_or(0),
        sprinting: player.sprinting,
        fall_distance: player.fall_distance,
        position,
        velocity,
        hspeed,
        last_dist,
        knock: LAST
            .with(|last| last.get())
            .is_some_and(|last| differs(velocity, last)),
        air_ticks: state.air_ticks,
        using: game.using.is_some(),
        riding: game.riding.is_some(),
        in_water,
        in_liquid: in_water || player.is_in_lava(),
        in_web: in_web(game),
        on_ladder: player.last_climbable.is_some(),
        leather_boots: player.leather_boots,
        forward: held(KeyCode::KeyW),
        sideways: held(KeyCode::KeyA) || held(KeyCode::KeyD),
        jump_held: held(KeyCode::Space),
        shift_held: held(KeyCode::ShiftLeft),
        raw_forward: f32::from(i8::from(held(KeyCode::KeyW)) - i8::from(held(KeyCode::KeyS))),
        raw_strafe: f32::from(i8::from(held(KeyCode::KeyA)) - i8::from(held(KeyCode::KeyD))),
        now: game.debug_millis(),
    }
}

fn differs(a: [f64; 3], b: [f64; 3]) -> bool {
    (a[0] - b[0]).abs() > 1.0e-7 || (a[1] - b[1]).abs() > 1.0e-7 || (a[2] - b[2]).abs() > 1.0e-7
}

/// The tick hook, before the player's input and physics move.
pub(super) fn tick(game: &mut Game<'_>) {
    terrain::begin_tick();
    let mut state = take_state();
    let Some(config) = running_config(game) else {
        timer::reset();
        cleanup(game, &mut state);
        put_state(State::default());
        return;
    };
    timer::begin_tick();
    let now = game.debug_millis();
    if state.mode != Some(config.mode) {
        cleanup(game, &mut state);
        state = State::new(Some(config.mode));
        init_mode(game, &config, &mut state, now);
    }
    if state.jump_tick >= 0 {
        state.jump_tick += 1;
    }
    let ctx = context(game, &mut state);
    if config.mode == Mode::Matrix {
        suppress_sneak(game, &mut state);
    }

    scheduled(game, &config, &mut state, &ctx);

    match config.mode {
        Mode::LegitHop => {}
        Mode::Custom => {
            if ctx.moving {
                if config.custom_horizontal_acceleration != 0.0 {
                    multiply_xz(game, 1.0 + config.custom_horizontal_acceleration);
                }
                let pull = if ctx.velocity[1] <= 0.0 {
                    config.custom_fall_pulldown
                } else {
                    config.custom_pulldown
                };
                add_y(game, -f64::from(pull));
                if config.custom_speed_enabled {
                    with_strafe(
                        game,
                        &ctx,
                        Some(config.custom_speed),
                        config.custom_strafe_strength,
                    );
                } else {
                    with_strafe(game, &ctx, None, config.custom_strafe_strength);
                }
            }
        }
        Mode::YPort => {
            if !ctx.on_ground && ctx.moving {
                set_y(game, -1.0);
            }
        }
        Mode::Vanilla => {
            if ctx.moving {
                if ctx.on_ground {
                    let power = natural_power(game);
                    launch(game, power, 1);
                }
                strafe(game, &ctx, Some(config.vanilla_speed));
            }
        }
        Mode::Aac => aac(game, &ctx),
        Mode::HypixelBHop => {
            if ctx.on_ground {
                strafe(game, &ctx, None);
            } else {
                let horizontal = if config.hypixel_horizontal_acceleration {
                    0.0004 + 0.0007 * ctx.amp as f32
                } else {
                    0.0
                };
                let vertical = if config.hypixel_vertical_acceleration
                    && ctx.velocity[1] < 0.0
                    && ctx.fall_distance < 1.0
                {
                    0.0004
                } else {
                    0.0
                };
                multiply_xyz(game, 1.0 + horizontal, 1.0 + vertical, 1.0 + horizontal);
            }
            if ctx.knock {
                let speed =
                    if ctx.velocity[0] == 0.0 && ctx.velocity[2] == 0.0 && ctx.velocity[1] < 0.0 {
                        ctx.hspeed.max(BASH * ctx.amp as f32)
                    } else {
                        ctx.hspeed
                    };
                strafe(game, &ctx, Some(speed));
            }
        }
        Mode::HypixelLowHop => {
            if ctx.on_ground {
                strafe(game, &ctx, None);
            } else {
                match ctx.air_ticks {
                    1 => {
                        strafe(game, &ctx, None);
                        add_y(game, 0.0568);
                    }
                    3 => {
                        multiply_xz(game, 0.95);
                        add_y(game, -0.13);
                    }
                    4 => add_y(game, -0.2),
                    7 if config.hypixel_lowhop_glide && ground_exempt(game) => set_y(game, 0.0),
                    _ => {}
                }
                if ground_exempt(game) {
                    strafe(game, &ctx, None);
                }
                if ctx.hurt_time == 9 {
                    strafe(game, &ctx, Some(ctx.hspeed.max(0.281)));
                }
                if ctx.amp == 2 && matches!(ctx.air_ticks, 1 | 2 | 5 | 6 | 8) {
                    multiply_xz(game, 1.2);
                }
            }
        }
        Mode::Spartan => {
            if ctx.forward && ctx.on_ground {
                multiply_xz(game, if ctx.leather_boots { 1.8 } else { 1.3 });
                let power = natural_power(game);
                launch(game, power, 4);
            }
        }
        Mode::SpartanFastFall => {
            if ctx.forward {
                if ctx.on_ground {
                    multiply_xz(game, if ctx.leather_boots { 1.2 } else { 1.05 });
                    let jumps = if ctx.leather_boots { 7 } else { 3 };
                    launch(game, 0.42, jumps);
                } else if ctx.air_ticks == 1 {
                    game.send(S::MovePlayerPosRot(play::MovePlayerPosRot {
                        x: ctx.position[0],
                        y: ctx.position[1],
                        z: ctx.position[2],
                        yaw: ctx.yaw,
                        pitch: ctx.pitch,
                        flags: ON_GROUND,
                    }));
                    timer::request(0.5);
                    set_y(game, -0.0784);
                }
            }
        }
        Mode::SentinelDamage => sentinel(game, &config, &mut state, &ctx),
        Mode::Vulcan286 => {}
        Mode::Vulcan288 => {
            if !ctx.on_ground && ctx.fall_distance.abs() > 0.0 && ctx.amp != 0 {
                multiply_xz(game, 1.055);
            }
        }
        Mode::VulcanGround286 => {
            if ctx.moving && collides_bottom(game, 0.005) && !ctx.jump_held {
                let speed = if ctx.amp > 0 {
                    0.59
                } else if ctx.sideways {
                    0.41
                } else {
                    0.42
                };
                strafe(game, &ctx, Some(speed));
                set_y(game, 0.005);
            }
        }
        Mode::GrimCollide => grim_collide(game, &config, &ctx),
        Mode::GrimBHop => {
            if ctx.moving && !ctx.in_liquid && !ctx.in_web && !ctx.on_ladder && ctx.on_ground {
                let power = natural_power(game);
                launch(game, power, 1);
                strafe(game, &ctx, Some(0.36));
            }
        }
        Mode::GrimLowHop => {
            if ctx.moving && !ctx.in_liquid && !ctx.in_web && !ctx.on_ladder && ctx.on_ground {
                set_y(game, 0.33);
                strafe(game, &ctx, Some(0.33));
            }
        }
        Mode::Ncp => {
            timer::request(1.08);
            if config.ncp_pull_down {
                if ctx.on_ground {
                    state.ncp_air = 0;
                } else {
                    state.ncp_air += 1;
                    if state.ncp_air == config.ncp_pull_down_tick {
                        strafe(game, &ctx, None);
                        add_y(
                            game,
                            -(0.152_335_18 * f64::from(config.ncp_motion_multiplier)),
                        );
                    }
                }
                if config.ncp_on_hurt && ctx.hurt_time >= 5 && ctx.velocity[1] >= 0.0 {
                    add_y(game, -0.1);
                }
            }
            if config.ncp_boost && ctx.moving {
                multiply_xz(game, 1.0 + 0.00718 * config.ncp_initial_boost_multiplier);
            }
            if ctx.moving {
                if ctx.on_ground {
                    let min = 0.281 + 0.199_999_999 * ctx.amp as f32;
                    strafe(game, &ctx, Some(ctx.hspeed.max(min)));
                } else if config.ncp_air_strafe {
                    let min = 0.2 + 0.199_999_999 * ctx.amp as f32;
                    with_strafe(game, &ctx, Some(ctx.hspeed.max(min)), 0.7);
                }
            }
            if ctx.hurt_time >= 1 && config.ncp_damage_boost {
                strafe(game, &ctx, Some(ctx.hspeed.max(0.5)));
            }
        }
        Mode::NcpBHop => ncp_bhop(game, &mut state, &ctx),
        Mode::NcpYPort => {
            if !ctx.on_ladder && !ctx.in_liquid && !ctx.in_web && ctx.moving && !ctx.in_water {
                if state.ncp_y_jumps >= 4 && ctx.on_ground {
                    state.ncp_y_jumps = 0;
                }
                if ctx.on_ground {
                    let power = if state.ncp_y_jumps <= 1 { 0.42 } else { 0.4 };
                    let rad = f64::from(ctx.yaw.to_radians());
                    if let Some(player) = game.player.as_mut() {
                        player.velocity.y = power;
                        player.velocity.x -= rad.sin() * 0.2;
                        player.velocity.z += rad.cos() * 0.2;
                        player.no_jump_delay = 2;
                    }
                    state.ncp_y_jumps += 1;
                } else if state.ncp_y_jumps <= 1 {
                    set_y(game, -5.0);
                }
                strafe(game, &ctx, None);
            }
        }
        Mode::Intave14 => {
            if config.intave_strafe && ctx.sprinting && (ctx.on_ground || ctx.air_ticks == 11) {
                with_strafe(game, &ctx, None, config.intave_strafe_strength);
            }
            if config.intave_air_boost && ctx.velocity[1] > 0.003 && ctx.sprinting {
                multiply_xz(game, 1.0 + 0.003 * 0.25);
            }
        }
        Mode::Intave14Fast => {
            timer::request(1.002);
            match ctx.air_ticks {
                1 => multiply_xz(game, 1.04),
                2 | 3 | 4 => multiply_xz(game, 1.02),
                _ => {}
            }
        }
        Mode::HylexLowHop => {
            if ctx.on_ground {
                if ctx.moving && ctx.hspeed < 0.32 {
                    multiply_xz(game, 1.1);
                }
            } else {
                if ctx.air_ticks == 9 && ctx.hspeed < 0.29 {
                    multiply_xz(game, 1.007);
                }
                if ctx.air_ticks == 1 && ctx.hspeed < 0.2 {
                    multiply_xz(game, 1.01);
                }
                if ctx.velocity[1] > 0.0 && ctx.air_ticks <= 2 && ctx.hspeed < 0.2 {
                    multiply_xz(game, 1.02);
                }
            }
        }
        Mode::HylexGround => {
            if !ctx.on_ground {
                state.hylex_ground_ticks = 0;
            } else {
                state.hylex_ground_ticks += 1;
                if state.hylex_ground_ticks > 5 && ctx.hurt_time < 1 && ctx.amp < 1 {
                    multiply_xz(game, if ctx.sideways { 1.214 } else { 1.2174 });
                }
            }
        }
        Mode::BlocksMc => blocksmc(game, &config, &mut state, &ctx),
        Mode::BlocksMcHop => {
            if ctx.moving && !ctx.in_liquid && !ctx.in_web && !ctx.on_ladder && !ctx.on_ground {
                if config.blocksmc_hop_full_strafe {
                    let speed = (ctx.hspeed - 0.004).max(0.0);
                    strafe(game, &ctx, Some(speed));
                } else if ctx.air_ticks >= 6 {
                    strafe(game, &ctx, None);
                }
                if ctx.amp > 0 && ctx.air_ticks == 3 {
                    multiply_xz(game, 1.12);
                }
                if config.blocksmc_hop_low_hop && ctx.air_ticks == 4 {
                    if config.blocksmc_hop_safe_y {
                        if ctx.position[1] % 1.0 == 0.166_109_260_938_213_77 {
                            set_y64(game, LOW_HOP_Y);
                        }
                    } else {
                        set_y64(game, LOW_HOP_Y);
                    }
                }
                if ctx.hurt_time == 9 && config.blocksmc_hop_damage_boost {
                    strafe(game, &ctx, Some(ctx.hspeed.max(0.7)));
                }
                if config.blocksmc_hop_damage_low_hop && ctx.hurt_time >= 1 && ctx.velocity[1] > 0.0
                {
                    add_y(game, -0.15);
                }
            }
        }
        Mode::Matrix7 => {
            if ctx.moving {
                if ctx.on_ground {
                    set_y(game, 0.419652);
                    strafe(game, &ctx, None);
                } else if ctx.velocity[0] * ctx.velocity[0] + ctx.velocity[2] * ctx.velocity[2]
                    < 0.04
                {
                    strafe(game, &ctx, None);
                }
            }
        }
        Mode::MatrixHop => {
            if ctx.moving && !ctx.in_liquid && !ctx.in_web && !ctx.on_ladder {
                if ctx.on_ground {
                    let extra = if m::scaffold().is_none() {
                        config.matrix_extra_boost
                    } else {
                        0.0
                    };
                    strafe(game, &ctx, Some(ctx.hspeed + extra));
                    let power = if config.matrix_low_hop {
                        0.42 - 0.00348
                    } else {
                        0.42
                    };
                    if let Some(player) = game.player.as_mut() {
                        player.velocity.y = f64::from(power).max(player.velocity.y);
                        player.no_jump_delay = 2;
                    }
                } else if m::scaffold().is_none() && ctx.hspeed < 0.19 {
                    strafe(game, &ctx, None);
                }
            }
        }
        Mode::Matrix => matrix(game, &ctx, &mut state),
        Mode::VerusHop => {
            if ctx.moving && !ctx.in_liquid && !ctx.in_web && !ctx.on_ladder {
                if ctx.on_ground {
                    state.verus_speed = if ctx.speed_level.is_some() && ctx.amp >= 1 {
                        0.46
                    } else {
                        0.34
                    };
                    let power = natural_power(game);
                    launch(game, power, 1);
                } else {
                    state.verus_speed *= 0.98;
                }
                strafe(game, &ctx, Some(state.verus_speed));
            }
        }
        Mode::VerusFHop => {
            if ctx.moving {
                let diagonal = ctx.raw_forward != 0.0 && ctx.raw_strafe != 0.0;
                if ctx.on_ground {
                    strafe(game, &ctx, Some(if diagonal { 0.4825 } else { 0.535 }));
                    let power = natural_power(game);
                    launch(game, power, 1);
                } else {
                    strafe(game, &ctx, Some(if diagonal { 0.334 } else { 0.3345 }));
                }
            }
        }
        Mode::VerusLowHop => {
            if ctx.moving && !ctx.in_liquid && !ctx.in_web && !ctx.on_ladder {
                if ctx.on_ground {
                    state.verus_speed = if ctx.speed_level.is_some() && ctx.amp >= 1 {
                        0.5
                    } else {
                        0.36
                    };
                    let power = natural_power(game);
                    launch(game, power, 1);
                } else {
                    if ctx.air_ticks <= 1 {
                        set_y64(game, LOW_HOP_Y);
                    }
                    state.verus_speed *= 0.98;
                }
                strafe(game, &ctx, Some(state.verus_speed));
            }
        }
        Mode::Verus => {
            if state.verus_timer_wait == 0 {
                timer::request(2.0);
                state.verus_timer_wait = 100;
            } else {
                state.verus_timer_wait -= 1;
            }
            if ctx.moving {
                if ctx.on_ground {
                    let power = natural_power(game);
                    launch(game, power, 1);
                    multiply_xz(game, 1.1);
                }
                strafe(game, &ctx, None);
            }
        }
        Mode::Librecraft => {
            state.libre_ticks += 1;
            if state.libre_ticks == 4 {
                set_y64(game, -0.098_000_001_907_348_64);
            }
            if !ctx.on_ground && ctx.hspeed < 0.215 {
                strafe(game, &ctx, Some(0.215));
            }
            if ctx.on_ground && ctx.moving {
                state.libre_ticks = 0;
                timer::request(1.25);
                let power = natural_power(game);
                launch(game, power, 1);
                strafe(
                    game,
                    &ctx,
                    Some(if ctx.speed_level.is_some() {
                        0.5
                    } else {
                        0.425
                    }),
                );
            } else if !ctx.moving {
                stop_xz(game);
            }
            if ctx.hurt_time > 6 {
                strafe(game, &ctx, Some(0.455));
            }
            if ctx.moving && !ctx.on_ground {
                strafe(game, &ctx, None);
            }
        }
        Mode::KoksCraft => {
            if ctx.on_ground {
                if ctx.hurt_time == 0 {
                    let allowed = m::allowed_horizontal_distance(ctx.amp, -1) * 0.99;
                    strafe(game, &ctx, Some(allowed));
                }
                let power = natural_power(game);
                launch(game, power, 1);
                state.koks_jumps += 1;
            }
            if ctx.air_ticks == 1 && ctx.hurt_time == 0 {
                let ticks = if state.koks_jumps % 2 == 0 { 2 } else { 4 };
                let y = m::predicted_motion(ctx.velocity[1] as f32, ticks);
                set_y(game, y);
            }
        }
        Mode::MiJump => {
            if ctx.moving {
                if ctx.on_ground && !ctx.jump_held {
                    if let Some(player) = game.player.as_mut() {
                        player.velocity.y += 0.1;
                        player.velocity.x *= 1.8;
                        player.velocity.z *= 1.8;
                        let speed = (player.velocity.x * player.velocity.x
                            + player.velocity.z * player.velocity.z)
                            .sqrt();
                        if speed > f64::from(0.66) {
                            let factor = f64::from(0.66) / speed;
                            player.velocity.x *= factor;
                            player.velocity.z *= factor;
                        }
                    }
                }
                strafe(game, &ctx, None);
            }
        }
        Mode::SlowHop => {
            if !ctx.in_liquid && !ctx.in_web && !ctx.on_ladder {
                if ctx.moving {
                    if ctx.on_ground {
                        let power = natural_power(game);
                        launch(game, power, 1);
                    } else {
                        multiply_xz(game, 1.011);
                    }
                } else {
                    stop_xz(game);
                }
            }
        }
        Mode::Frame => frame(game, &ctx, &mut state),
        Mode::SpectreHop => {
            if ctx.moving && !ctx.jump_held {
                if ctx.on_ground {
                    strafe(game, &ctx, Some(1.1));
                    set_y(game, config.spectre_jump_motion);
                } else {
                    strafe(game, &ctx, None);
                }
            }
        }
        Mode::SpectreOnGround => {
            if ctx.moving && !ctx.jump_held {
                if state.spectre_speed_up >= 10 {
                    if ctx.on_ground {
                        stop_xz(game);
                        state.spectre_speed_up = 0;
                    }
                } else if ctx.on_ground && ctx.forward {
                    let rad = ctx.yaw.to_radians();
                    if let Some(player) = game.player.as_mut() {
                        player.velocity.x -= f64::from(rad.sin()) * 0.145;
                        player.velocity.z += f64::from(rad.cos()) * 0.145;
                        player.velocity.y = 0.005;
                    }
                    state.spectre_speed_up += 1;
                }
            }
        }
        Mode::Swift => {
            if ctx.on_ground {
                state.swift_delay = (state.swift_delay + 1) % 2;
                if state.swift_delay == 1 {
                    multiply_xz(game, 2.58);
                } else {
                    multiply_xz(game, 1.0 / 1.5);
                }
                set_y(game, 0.001);
            }
        }
        Mode::WatchdogTerrain => watchdog_terrain(game, &ctx, &mut state),
    }

    auto_jump(game, &config, &mut state, &ctx);
    put_state(state);
}

fn init_mode(game: &mut Game<'_>, config: &SpeedConfig, state: &mut State, now: i64) {
    match config.mode {
        Mode::NcpBHop => {
            let collides = box_collides(
                game,
                game.player.as_ref().map_or(0.0, |player| player.velocity.y),
            ) || game
                .player
                .as_ref()
                .is_some_and(|player| player.collision.vertical);
            state.ncpbhop_level = if collides { 1 } else { 4 };
            state.ncpbhop_speed = ncp_base_speed(game);
        }
        Mode::SentinelDamage => {
            state.sentinel_last_damage = now;
            state.sentinel_wait = 0;
        }
        Mode::Frame => state.frame_timer = now,
        _ => {}
    }
}

/// Releases what a mode held, when it turns off or another mode takes over.
/// Alan's TerrainSpeed Watchdog mode: a boosted climb on stairs, the speed
/// kept on bottom slabs, and the stop and downward step when leaving either.
/// Alan multiplies the StrafeEvent friction; here the same factor scales the
/// travel input, and the move's Y is held at -0.0784 while on or just off the
/// stairs. The reference's Scaffold, Speed and KillAura guards are left out:
/// they only kept its own module from fighting those three.
fn watchdog_terrain(game: &mut Game<'_>, ctx: &Ctx, state: &mut State) {
    let x = ctx.position[0].floor() as i32;
    let z = ctx.position[2].floor() as i32;
    let y = (ctx.position[1] - 0.5).floor() as i32;
    let half = (ctx.position[1] - ctx.position[1].floor() - 0.5).abs() < 1.0e-4;
    let slab = block_name_matches(game, x, y, z, |name| name.ends_with("_slab"));
    let stairs = block_name_matches(game, x, y, z, |name| name.ends_with("_stairs"));
    let back = game.physical_keys.contains(&KeyCode::KeyS);

    if slab && half && !back {
        state.off_slab_ticks = 0;
    } else {
        state.off_slab_ticks += 1;
        if state.off_slab_ticks == 1 {
            terrain::multiply(0.9);
        }
    }

    if stairs && ctx.forward {
        if half || state.off_stair_ticks == 0 {
            terrain::multiply(3.98);
            state.off_stair_ticks = 0;
        }
    } else {
        state.off_stair_ticks += 1;
        if state.off_stair_ticks == 1 {
            stop_xz(game);
        }
    }

    if state.off_stair_ticks < 2 {
        set_y(game, -0.0784);
        if game.keys.contains(&KeyCode::Space) {
            game.keys.remove(&KeyCode::Space);
            state.terrain_jump = true;
        }
    } else if state.terrain_jump {
        if game.physical_keys.contains(&KeyCode::Space) {
            game.keys.insert(KeyCode::Space);
        }
        state.terrain_jump = false;
    }
}

fn cleanup(game: &mut Game<'_>, state: &mut State) {
    match state.mode {
        Some(Mode::SentinelDamage) | Some(Mode::BlocksMc) | Some(Mode::SpartanFastFall) => {
            stop_xz(game);
        }
        _ => {}
    }
    if state.matrix_shift {
        if game.physical_keys.contains(&KeyCode::ShiftLeft)
            && !game.keys.contains(&KeyCode::ShiftLeft)
        {
            game.keys.insert(KeyCode::ShiftLeft);
        }
        state.matrix_shift = false;
    }
    if state.terrain_jump {
        if game.physical_keys.contains(&KeyCode::Space) && !game.keys.contains(&KeyCode::Space) {
            game.keys.insert(KeyCode::Space);
        }
        state.terrain_jump = false;
    }
}

/// Matrix sneaks through the input flag: keep the sneak key out of the tick's
/// key set while it is physically held, and remember to give it back.
fn suppress_sneak(game: &mut Game<'_>, state: &mut State) {
    if !game.physical_keys.contains(&KeyCode::ShiftLeft) {
        state.matrix_shift = false;
    } else if !state.matrix_shift {
        game.keys.remove(&KeyCode::ShiftLeft);
        state.matrix_shift = true;
    }
}

fn ncp_base_speed(game: &Game<'_>) -> f64 {
    let level = game.potions.effects.amplifier("speed");
    match level {
        Some(amp) => f64::from(m::allowed_horizontal_distance(amp, -1)),
        None => 0.2873,
    }
}

/// The after-jump sequences, which the references start from their jump event
/// and continue a tick at a time.
fn scheduled(game: &mut Game<'_>, config: &SpeedConfig, state: &mut State, ctx: &Ctx) {
    if state.jump_tick < 0 {
        return;
    }
    match config.mode {
        Mode::Vulcan286 => {
            let amp = ctx.amp as f32;
            match state.jump_tick {
                1 => strafe(
                    game,
                    ctx,
                    Some(if ctx.sideways {
                        0.3345
                    } else {
                        0.3355 * (1.0 + amp * 0.3819)
                    }),
                ),
                2 => {
                    if ctx.sprinting {
                        strafe(
                            game,
                            ctx,
                            Some(if ctx.sideways {
                                0.3235
                            } else {
                                0.3284 * (1.0 + amp * 0.355)
                            }),
                        );
                    }
                }
                4 => set_y(game, -0.376),
                6 => {
                    if state.air_distance > 0.298 {
                        strafe(game, ctx, Some(0.298));
                    }
                    state.jump_tick = -1;
                }
                tick if tick > 6 => state.jump_tick = -1,
                _ => {}
            }
        }
        Mode::Vulcan288 => {
            let has_speed = ctx.amp != 0;
            match state.jump_tick {
                1 => strafe(game, ctx, Some(if has_speed { 0.605 } else { 0.31 })),
                2 => {
                    strafe(game, ctx, Some(if has_speed { 0.57 } else { 0.29 }));
                    set_y(game, if has_speed { -0.5 } else { -0.37 });
                }
                3 => strafe(game, ctx, Some(if has_speed { 0.595 } else { 0.27 })),
                4 => {
                    strafe(game, ctx, Some(if has_speed { 0.595 } else { 0.28 }));
                    state.jump_tick = -1;
                }
                tick if tick > 4 => state.jump_tick = -1,
                _ => {}
            }
        }
        Mode::Custom => {
            if config.custom_jump_off != 0.0 && config.custom_boost_ticks > 0 {
                if state.jump_tick == config.custom_boost_ticks {
                    multiply_xz(game, 1.0 + config.custom_jump_off);
                    state.jump_tick = -1;
                } else if state.jump_tick > config.custom_boost_ticks {
                    state.jump_tick = -1;
                }
            } else {
                state.jump_tick = -1;
            }
        }
        _ => state.jump_tick = -1,
    }
}

/// The modes whose jump the ground tick starts by itself.
fn auto_jump(game: &mut Game<'_>, config: &SpeedConfig, state: &mut State, ctx: &Ctx) {
    let jump = match config.mode {
        Mode::LegitHop
        | Mode::YPort
        | Mode::HypixelBHop
        | Mode::HypixelLowHop
        | Mode::Ncp
        | Mode::Custom
        | Mode::Intave14
        | Mode::Intave14Fast
        | Mode::HylexLowHop
        | Mode::Matrix7
        | Mode::Vulcan286
        | Mode::Vulcan288 => ctx.on_ground && ctx.moving,
        Mode::BlocksMc => ctx.on_ground && ctx.moving && state.blocksmc_state != 0,
        Mode::BlocksMcHop => {
            ctx.on_ground && ctx.moving && !ctx.in_liquid && !ctx.in_web && !ctx.on_ladder
        }
        _ => false,
    };
    if !jump {
        return;
    }
    let power = match config.mode {
        Mode::Custom => config.custom_jump_height,
        Mode::Ncp if config.ncp_low_hop => 0.4,
        Mode::HylexLowHop => 0.33,
        _ => natural_power(game),
    };
    if !(power > 1.0e-5) {
        return;
    }
    launch(game, power, 1);
    match config.mode {
        Mode::YPort => strafe(game, ctx, Some(config.yp_speed)),
        Mode::HypixelBHop => {
            let at_least = if state.was_flagged {
                0.0
            } else {
                m::hypixel_at_least(ctx.amp)
            };
            let speed = hspeed(game).max(at_least);
            strafe(game, ctx, Some(speed));
        }
        Mode::HypixelLowHop => {
            let at_least = 0.247 + 0.15 * ctx.amp as f32;
            let speed = hspeed(game).max(at_least);
            strafe(game, ctx, Some(speed));
        }
        Mode::Custom => {
            if config.custom_jump_off != 0.0 {
                if config.custom_boost_ticks == 0 {
                    multiply_xz(game, 1.0 + config.custom_jump_off);
                } else {
                    state.jump_tick = 0;
                }
            }
        }
        Mode::Vulcan286 => state.jump_tick = 0,
        Mode::Vulcan288 => {
            strafe(game, ctx, Some(if ctx.amp != 0 { 0.771 } else { 0.5 }));
            state.jump_tick = 0;
        }
        _ => {}
    }
}

/// The jump the references perform from the ground.
fn launch(game: &mut Game<'_>, power: f32, impulses: i32) {
    let yaw = game.yaw;
    let Some(player) = game.player.as_mut() else {
        return;
    };
    if !(power > 1.0e-5) {
        return;
    }
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

/// The vanilla jump power: the jump-strength attribute plus Jump Boost.
fn natural_power(game: &mut Game<'_>) -> f32 {
    let boost = game
        .potions
        .effects
        .amplifier("jump_boost")
        .map_or(0.0, |amp| 0.1 * (amp + 1) as f32);
    let strength = game
        .player
        .as_mut()
        .and_then(|player| player.attribute("minecraft:jump_strength").ok())
        .unwrap_or(0.42) as f32;
    strength + boost
}

fn hspeed(game: &Game<'_>) -> f32 {
    game.player.as_ref().map_or(0.0, |player| {
        (player.velocity.x * player.velocity.x + player.velocity.z * player.velocity.z).sqrt()
            as f32
    })
}

fn multiply_xz(game: &mut Game<'_>, factor: f32) {
    if let Some(player) = game.player.as_mut() {
        player.velocity.x *= f64::from(factor);
        player.velocity.z *= f64::from(factor);
    }
}

fn multiply_xyz(game: &mut Game<'_>, x: f32, y: f32, z: f32) {
    if let Some(player) = game.player.as_mut() {
        player.velocity.x *= f64::from(x);
        player.velocity.y *= f64::from(y);
        player.velocity.z *= f64::from(z);
    }
}

fn add_y(game: &mut Game<'_>, delta: f64) {
    if let Some(player) = game.player.as_mut() {
        player.velocity.y += delta;
    }
}

fn set_y(game: &mut Game<'_>, y: f32) {
    if let Some(player) = game.player.as_mut() {
        player.velocity.y = f64::from(y);
    }
}

fn set_y64(game: &mut Game<'_>, y: f64) {
    if let Some(player) = game.player.as_mut() {
        player.velocity.y = y;
    }
}

fn stop_xz(game: &mut Game<'_>) {
    if let Some(player) = game.player.as_mut() {
        player.velocity.x = 0.0;
        player.velocity.z = 0.0;
    }
}

fn with_strafe(game: &mut Game<'_>, ctx: &Ctx, speed: Option<f32>, strength: f32) {
    let Some(player) = game.player.as_mut() else {
        return;
    };
    let current = [player.velocity.x as f32, player.velocity.z as f32];
    let [x, z] = m::with_strafe(current, ctx.dir, ctx.yaw, speed, strength);
    player.velocity.x = f64::from(x);
    player.velocity.z = f64::from(z);
}

fn strafe(game: &mut Game<'_>, ctx: &Ctx, speed: Option<f32>) {
    with_strafe(game, ctx, speed, 1.0);
}

/// The speed with a given movement direction, for the modes that turn their
/// strafe yaw themselves (BlocksMC).
fn strafe_at(game: &mut Game<'_>, yaw: f32, speed: f32) {
    let rad = yaw.to_radians();
    if let Some(player) = game.player.as_mut() {
        player.velocity.x = f64::from(-rad.sin() * speed);
        player.velocity.z = f64::from(rad.cos() * speed);
    }
}

fn aac(game: &mut Game<'_>, ctx: &Ctx) {
    if !ctx.moving || ctx.in_liquid || ctx.on_ladder || ctx.riding || ctx.hurt_time > 0 {
        return;
    }
    let vertical = game
        .player
        .as_ref()
        .is_some_and(|player| player.collision.vertical);
    if ctx.on_ground && vertical {
        let rad = ctx.yaw.to_radians();
        if let Some(player) = game.player.as_mut() {
            player.velocity.x -= f64::from(rad.sin()) * 0.202;
            player.velocity.z += f64::from(rad.cos()) * 0.202;
            player.velocity.y = 0.405;
            player.no_jump_delay = 2;
        }
        strafe(game, ctx, None);
    } else if ctx.fall_distance < 0.31 {
        if on_carpet(game) {
            return;
        }
        multiply_xz(game, 1.001);
        let horizontal = game
            .player
            .as_ref()
            .is_some_and(|player| player.collision.horizontal);
        if !horizontal {
            add_y(game, -0.014_999_993);
        }
    }
}

fn sentinel(game: &mut Game<'_>, config: &SpeedConfig, state: &mut State, ctx: &Ctx) {
    if ctx.hurt_time > 0 && !state.sentinel_has_been_hurt {
        state.sentinel_has_been_hurt = true;
        state.sentinel_adjusted = true;
    } else if ctx.hurt_time == 10 {
        let elapsed = (ctx.now - state.sentinel_last_damage).max(0) as f64;
        state.sentinel_external_adjust = (elapsed / 50.0).ceil() as i32;
    }
    if ctx.moving {
        if state.sentinel_external_adjust != 0 {
            state.sentinel_wait += state.sentinel_external_adjust;
            state.sentinel_external_adjust = 0;
        }
        if state.sentinel_wait > 0 {
            state.sentinel_wait -= 1;
        } else {
            sentinel_boost(game, ctx);
            state.sentinel_has_been_hurt = false;
            state.sentinel_last_damage = ctx.now;
            state.sentinel_wait = config.sentinel_reboost_ticks;
        }
        if ctx.on_ground && state.sentinel_has_been_hurt {
            let power = natural_power(game);
            launch(game, power, 1);
        }
        if state.sentinel_has_been_hurt || state.sentinel_adjusted {
            strafe(game, ctx, Some(config.sentinel_speed));
        }
    }
}

fn sentinel_boost(game: &mut Game<'_>, ctx: &Ctx) {
    let [x, y, z] = ctx.position;
    game.send(S::MovePlayerPos(play::MovePlayerPos { x, y, z, flags: 0 }));
    game.send(S::MovePlayerPos(play::MovePlayerPos {
        x,
        y: y + 3.25,
        z,
        flags: 0,
    }));
    game.send(S::MovePlayerPos(play::MovePlayerPos { x, y, z, flags: 0 }));
    game.send(S::MovePlayerPos(play::MovePlayerPos {
        x,
        y,
        z,
        flags: ON_GROUND,
    }));
}

fn ncp_bhop(game: &mut Game<'_>, state: &mut State, ctx: &Ctx) {
    // FDPClient's `updateBHopTimer`: every fifth tick runs at 1.3x and gets a
    // horizontal push.
    let next = (state.ncpbhop_timer_delay + 1) % 5;
    if next == 0 {
        state.ncpbhop_timer_delay = 0;
        timer::request(1.3);
        if ctx.moving {
            multiply_xz(game, 1.019_999_98);
        }
    } else {
        state.ncpbhop_timer_delay = next;
    }
    let base = ncp_base_speed(game);
    let last_dist = f64::from(ctx.last_dist);
    let mut level = state.ncpbhop_level;
    let mut move_speed = state.ncpbhop_speed;
    if ctx.on_ground && ctx.moving {
        level = 2;
    }
    let fraction = ctx.position[1] - ctx.position[1].floor();
    if round_thousandths(fraction) == 0.138 {
        add_y(game, -0.08);
        if let Some(player) = game.player.as_mut() {
            player.position.y -= 0.093_160_903_259_601_47;
        }
    }
    if level == 1 && ctx.moving {
        level = 2;
        move_speed = 1.35 * base - 0.01;
    } else if level == 2 {
        level = 3;
        set_y(game, 0.399_399_996);
        move_speed *= 2.149;
    } else if level == 3 {
        level = 4;
        let difference = 0.66 * (last_dist - base);
        move_speed = last_dist - difference;
    } else {
        let velocity_y = game.player.as_ref().map_or(0.0, |player| player.velocity.y) as f32;
        let horizontal = game
            .player
            .as_ref()
            .is_some_and(|player| player.collision.vertical);
        if box_collides(game, f64::from(velocity_y)) || horizontal {
            level = 1;
        }
        move_speed = last_dist - last_dist / 159.0;
    }
    move_speed = move_speed.max(base);
    apply_bhop_direction(game, move_speed, ctx.raw_forward, ctx.raw_strafe, ctx.yaw);
    state.ncpbhop_level = level;
    state.ncpbhop_speed = move_speed;
}

/// FDPClient's `applyBHopDirection`: the movement event's own direction, not
/// the already-strafed velocity.
fn apply_bhop_direction(game: &mut Game<'_>, speed: f64, forward: f32, strafe: f32, yaw: f32) {
    let mut forward = forward;
    let mut strafe = strafe;
    let mut rotation = yaw;
    if forward == 0.0 && strafe == 0.0 {
        stop_xz(game);
        return;
    }
    if forward != 0.0 {
        if strafe >= 1.0 {
            rotation += if forward > 0.0 { -45.0 } else { 45.0 };
            strafe = 0.0;
        } else if strafe <= -1.0 {
            rotation += if forward > 0.0 { 45.0 } else { -45.0 };
            strafe = 0.0;
        }
        forward = forward.signum();
    }
    let radians = f64::from((rotation + 90.0).to_radians());
    let cosine = radians.cos();
    let sine = radians.sin();
    if let Some(player) = game.player.as_mut() {
        player.velocity.x = f64::from(forward) * speed * cosine + f64::from(strafe) * speed * sine;
        player.velocity.z = f64::from(forward) * speed * sine - f64::from(strafe) * speed * cosine;
    }
}

fn blocksmc(game: &mut Game<'_>, config: &SpeedConfig, state: &mut State, ctx: &Ctx) {
    if tower_active(game, ctx) {
        state.blocksmc_state = 0;
        return;
    }
    if ctx.on_ground {
        state.blocksmc_state = if m::scaffold().is_some() { 2 } else { 1 };
    }
    if state.blocksmc_flag_delay > 0 {
        state.blocksmc_flag_delay -= 1;
    }
    let speed = m::blocksmc_speed(
        ctx.on_ground,
        ctx.velocity[1] as f32,
        ctx.amp,
        state.blocksmc_flag_delay,
    );
    match state.blocksmc_state {
        1 => {
            if ctx.air_ticks == 4 {
                set_y(game, -0.098_000_002);
            }
        }
        2 => match ctx.air_ticks {
            1 => add_y(game, 0.0568),
            3 => add_y(game, -0.13),
            4 => add_y(game, -0.2),
            _ => {}
        },
        _ => {}
    }
    if !ctx.on_ground && state.blocksmc_state != 0 && ctx.moving {
        let yaw = m::movement_yaw(ctx.yaw, ctx.dir);
        let yaw = if config.blocksmc_round_yaw {
            m::round_yaw(yaw)
        } else {
            yaw
        };
        strafe_at(game, yaw, speed);
    }
}

fn tower_active(game: &Game<'_>, ctx: &Ctx) -> bool {
    let Some(config) = m::scaffold() else {
        return false;
    };
    if config.tower == m::ScaffoldTower::Off || !game.keys.contains(&KeyCode::Space) {
        return false;
    }
    !ctx.moving || config.tower_move
}

fn matrix(game: &mut Game<'_>, ctx: &Ctx, state: &mut State) {
    if ctx.on_ground && !ctx.using {
        let power = natural_power(game);
        launch(game, power, 1);
    }
    let pos_flag = ctx.position[1] % 0.015625 == 0.0;

    if pos_flag || ctx.hspeed < 0.2 {
        strafe(game, ctx, None);
    }
    if ctx.hspeed < 0.195 && !ctx.using {
        strafe(game, ctx, Some(0.195));
    }
    if ctx.on_ground {
        multiply_xz(game, 1.001);
        strafe(game, ctx, None);
    }
    if ctx.air_ticks > 1 {
        add_y(game, -0.00348);
    }
    if ctx.air_ticks == 1 && ctx.moving {
        let speed = hspeed(game) * 0.65;
        if speed > 0.0 {
            strafe(game, ctx, Some(speed));
        }
    }
    let motion_y = game.player.as_ref().map_or(0.0, |player| player.velocity.y);
    if ctx.moving && box_collides(game, motion_y) {
        let speed = hspeed(game) * 0.65;
        if speed > 0.0 {
            strafe(game, ctx, Some(speed));
        }
    }
    if ctx.air_ticks < 12 && ctx.moving {
        strafe(game, ctx, None);
    }
    if !ctx.shift_held {
        state.matrix_sneak_ticks += 1;
        if state.matrix_sneak_ticks == 1 {
            matrix_packets(game, ctx);
        }
    } else {
        state.matrix_sneak_ticks = 0;
        if ctx.moving {
            strafe(game, ctx, Some(0.07));
        }
    }
}

fn matrix_packets(game: &mut Game<'_>, ctx: &Ctx) {
    let [x, y, z] = ctx.position;
    game.send(S::MovePlayerPosRot(play::MovePlayerPosRot {
        x,
        y,
        z,
        yaw: ctx.yaw,
        pitch: ctx.pitch,
        flags: 0,
    }));
    game.send(S::MovePlayerPos(play::MovePlayerPos { x, y, z, flags: 0 }));
    game.send(S::MovePlayerPosRot(play::MovePlayerPosRot {
        x,
        y,
        z,
        yaw: ctx.yaw,
        pitch: ctx.pitch,
        flags: 0,
    }));
}

fn frame(game: &mut Game<'_>, ctx: &Ctx, state: &mut State) {
    if !ctx.moving {
        return;
    }
    if ctx.on_ground {
        let power = natural_power(game);
        launch(game, power, 1);
        if state.frame_motion == 1 {
            state.frame_timer = ctx.now;
            if state.frame_move {
                stop_xz(game);
                state.frame_move = false;
            }
            state.frame_motion = 0;
        } else {
            state.frame_motion = 1;
        }
    } else if !state.frame_move && state.frame_motion == 1 && ctx.now - state.frame_timer >= 5 {
        multiply_xz(game, 4.25);
        state.frame_move = true;
    }
    if !ctx.on_ground {
        strafe(game, ctx, None);
    }
    state.frame_timer = ctx.now;
}

fn grim_collide(game: &mut Game<'_>, config: &SpeedConfig, ctx: &Ctx) {
    if !ctx.moving {
        return;
    }
    let shrink = f64::from(config.grim_shrink);
    let count = {
        let Some(player) = game.player.as_ref() else {
            return;
        };
        let bounds = player.bounds().inflate(Vec3::new(shrink, shrink, shrink));
        game.mobs
            .iter()
            .filter(|mob| !mob.removed && bounds.intersects(mob.bounds()))
            .count()
    };
    if count == 0 {
        return;
    }
    let boost = config.grim_boost * count as f32;
    let rad = ctx.yaw.to_radians();
    if let Some(player) = game.player.as_mut() {
        player.velocity.x -= f64::from(rad.sin()) * f64::from(boost);
        player.velocity.z += f64::from(rad.cos()) * f64::from(boost);
    }
}

/// The snapshot hook, at the head of `remote_send_state`.
pub(super) fn snapshot(game: &Game<'_>) {
    let velocity = game
        .player
        .as_ref()
        .map(|player| [player.velocity.x, player.velocity.y, player.velocity.z]);
    LAST.with(|last| last.set(velocity));
    let config = m::speed();
    let mode = config.map(|config| config.mode);
    let spoof_ground =
        matches!(mode, Some(Mode::Vulcan288)) && velocity.is_some_and(|velocity| velocity[1] < 0.0);
    let spoof_y_005 = matches!(mode, Some(Mode::VulcanGround286))
        && !game.physical_keys.contains(&KeyCode::Space)
        && collides_bottom(game, 0.005);
    let mut state = take_state();
    state.spoof_ground = spoof_ground;
    state.spoof_y_005 = spoof_y_005;
    put_state(state);
}

/// The module's movement packet rewrite, after NoFall and Criticals.
pub(super) fn spoof(packet: Option<S>) -> Option<S> {
    let Some(config) = m::speed() else {
        return packet;
    };
    let packet = packet?;
    let (state_mode, ground, y_nudge) = STATE.with(|s| {
        let state = s.borrow();
        (state.mode, state.spoof_ground, state.spoof_y_005)
    });
    if state_mode != Some(config.mode) {
        return Some(packet);
    }
    match config.mode {
        Mode::Vulcan288 => {
            if !ground {
                return Some(packet);
            }
            Some(match packet {
                S::MovePlayerPos(mut packet) => {
                    packet.flags |= ON_GROUND;
                    S::MovePlayerPos(packet)
                }
                S::MovePlayerPosRot(mut packet) => {
                    packet.flags |= ON_GROUND;
                    S::MovePlayerPosRot(packet)
                }
                S::MovePlayerRot(mut packet) => {
                    packet.flags |= ON_GROUND;
                    S::MovePlayerRot(packet)
                }
                S::MovePlayerStatus(mut packet) => {
                    packet.flags |= ON_GROUND;
                    S::MovePlayerStatus(packet)
                }
                other => other,
            })
        }
        Mode::VulcanGround286 => {
            if !y_nudge {
                return Some(packet);
            }
            Some(match packet {
                S::MovePlayerPos(mut packet) => {
                    packet.y += 0.005;
                    S::MovePlayerPos(packet)
                }
                S::MovePlayerPosRot(mut packet) => {
                    packet.y += 0.005;
                    S::MovePlayerPosRot(packet)
                }
                other => other,
            })
        }
        _ => Some(packet),
    }
}

/// The teleport hook, before the server's setback is applied.
pub(super) fn teleport(_game: &mut Game<'_>) {
    LAST.with(|last| last.set(None));
    let Some(config) = m::speed() else {
        return;
    };
    let mut state = take_state();
    if state.mode == Some(config.mode) {
        state.air_ticks = 0;
        state.air_distance = 0.0;
        state.jump_tick = -1;
        state.ncp_air = 0;
        state.ncp_y_jumps = 0;
        match config.mode {
            Mode::HypixelBHop => state.was_flagged = true,
            Mode::BlocksMc => state.blocksmc_flag_delay = 20,
            _ => {}
        }
    }
    put_state(state);
}

fn on_carpet(game: &Game<'_>) -> bool {
    let Some(player) = game.player.as_ref() else {
        return false;
    };
    let x = player.position.x.floor() as i32;
    let y = player.position.y.floor() as i32;
    let z = player.position.z.floor() as i32;
    block_name_matches(game, x, y, z, |name| name.ends_with("_carpet"))
}

fn in_web(game: &Game<'_>) -> bool {
    let Some(player) = game.player.as_ref() else {
        return false;
    };
    let x = player.position.x.floor() as i32;
    let y = player.position.y.floor() as i32;
    let z = player.position.z.floor() as i32;
    let web = |x: i32, y: i32, z: i32| {
        block_name_matches(game, x, y, z, |name| name == "minecraft:cobweb")
    };
    web(x, y, z) || web(x, y + 1, z)
}

fn ground_exempt(game: &Game<'_>) -> bool {
    collides_bottom(game, 0.66)
        && game
            .player
            .as_ref()
            .is_some_and(|player| player.velocity.y < 0.0)
}

fn round_thousandths(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// Whether the block at `x, y, z` has a collision shape.
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

fn block_name_matches(
    game: &Game<'_>,
    x: i32,
    y: i32,
    z: i32,
    predicate: impl FnOnce(&str) -> bool,
) -> bool {
    let state = game.world.block(x, y, z);
    let facts = game.res.facts();
    predicate(&facts.get(state).name)
}

/// Whether a solid block intersects the player's box moved by `dy`.
fn box_collides(game: &Game<'_>, dy: f64) -> bool {
    let Some(player) = game.player.as_ref() else {
        return false;
    };
    let bounds = player.bounds().move_by(Vec3::new(0.0, dy, 0.0));
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

fn collides_bottom(game: &Game<'_>, distance: f32) -> bool {
    box_collides(game, -f64::from(distance))
}
