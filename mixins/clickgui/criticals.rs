// Criticals: a port of LiquidBounce's ModuleCriticals and its modes, plus the
// FDPClient, Alan, Mospixel, NCP, Summer, Tenacity, Flux, Sigma, Deadcode,
// Celestial and Novoline implementations.
//
// Copyright (c) 2015-2026 CCBlueX. LiquidBounce is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed in
// the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the
// implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the LICENSE file or <https://www.gnu.org/licenses/>.
//
// Two hooks make the port work. `attack` runs at the head of
// `remote_start_attack`, before the attack packet goes out: the modes that
// send fake positions (the Packet family, Mospixel, Summer, Tenacity, Flux,
// Sigma and the Celestial packets) do it there, exactly where the original
// clients insert their C03 packets before C02. `prepare` runs at the head of
// `remote_send_state`, before the tick's movement packet is built: the modes
// that rewrite the pre-motion packet (NoGround, Vulcan, Edit, Watchdog,
// Tenacity Edit, Sigma Hover, Novoline, Celestial's Sunrise/OldMatrix/Matrix6.3.0)
// leave a plan there, and `spoof` applies it to the packet the sender produced
// (or synthesizes one when the sender had nothing to send).
//
// The differences from the originals: the render clock is not ours, so Timer
// cannot slow the world down; the packet queue behind Blink does not exist, so
// it only holds the movement packet back; `onCriticalHit`'s local sound and
// particle call is not exposed, so the Visuals group spawns the crit and magic
// particles itself; and KillAura/AutoClicker/Flight module checks are
// approximated with the local player's state and the nearest entity.
use crate::game::clickgui as m;
use crate::game::clickgui::{CriticalsConfig, CriticalsMode as Mode};
use crate::game::Game;
use horizon_client::particles::{ParticleOptions, ParticleType};
use horizon_protocol::packets::play::{self, Serverbound as S};
use std::cell::RefCell;

/// The ground bit of `ServerboundMovePlayerPacket`'s flags.
const ON_GROUND: u8 = 1;
/// The horizontal-collision bit of the same flags.
const HORIZONTAL_COLLISION: u8 = 1 << 1;
/// `ServerboundPlayerCommandPacket.Action.STOP_SPRINTING`.
const STOP_SPRINTING: i32 = 2;
/// `ServerboundPlayerCommandPacket.Action.START_FALL_FLYING`.
const START_FALL_FLYING: i32 = 6;
/// Celestial's Matrix 6.3.0 reset threshold.
const MATRIX630_FALL: f64 = 3.0;

/// `Entity.isOnLadder`: any block in `#minecraft:climbable`.
const CLIMBABLE: &[&str] = &[
    "minecraft:ladder",
    "minecraft:vine",
    "minecraft:scaffolding",
    "minecraft:twisting_vines",
    "minecraft:twisting_vines_plant",
    "minecraft:weeping_vines",
    "minecraft:weeping_vines_plant",
    "minecraft:cave_vines",
    "minecraft:cave_vines_plant",
];
/// `Entity.isInWeb`.
const COBWEB: &[&str] = &["minecraft:cobweb"];
/// `Entity.isInLiquid`'s lava half (`was_touching_water` covers water).
const LAVA: &[&str] = &["minecraft:lava", "minecraft:flowing_lava"];

/// What this tick's movement packet should be. `prepare` fills it, `spoof`
/// applies it to the packet the sender produced.
#[derive(Clone, Copy, Default)]
struct Plan {
    /// The module ran this tick: `spoof` may rewrite the packet.
    active: bool,
    /// Drop the packet the sender produced (Blink).
    suppress: bool,
    /// Replace the packet's on-ground flag.
    ground: Option<bool>,
    /// Added to the packet's position (position-carrying packets only).
    offset: [f64; 3],
    /// Build a packet when the sender produced none (a mode that has to reach
    /// the server every tick).
    force: bool,
    /// The player's position, rotation and flags when `prepare` ran, for a
    /// synthesized packet.
    position: [f64; 3],
    rotation: (f32, f32),
    flags: u8,
}

/// The counters and timers the modes keep between attacks and ticks.
#[derive(Default)]
struct State {
    plan: Plan,
    /// The mode the state belongs to (a change resets the counters).
    mode: Option<Mode>,
    was_active: bool,
    /// Celestial Matrix 6.3.0's Reduce damage on disable.
    last_reduce: bool,
    /// Ticks on the ground, and in the air (`airTicks` for Vulcan).
    ground_ticks: i32,
    air_ticks: i32,
    /// NCP's pre-motion counter.
    ncp_ticks: i32,
    /// Edit's `attacked` and `ticks`.
    edit_attacked: bool,
    edit_stage: i32,
    /// Watchdog's `pendingCrit` and `critTicks`.
    watchdog_pending: bool,
    watchdog_stage: i32,
    /// Tenacity Watchdog Edit's alternating `stage`.
    tenacity_stage: bool,
    /// Sigma Hover's `y` and `stage`.
    hover_y: f64,
    hover_stage: i32,
    /// Blink's next window, in milliseconds.
    blink_until: i64,
    /// The timers of the modes that wait between hits.
    last_attack_ms: i64,
    last_summer_ms: i64,
    last_celestial_ncp_ms: i64,
    last_old_matrix_ms: i64,
    last_flux_ms: i64,
    last_sigma_ms: i64,
    last_edit_ms: i64,
    last_alan_ms: i64,
    last_watchdog_ms: i64,
    last_deadcode_ms: i64,
    rng: u64,
}

impl State {
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

    fn range(&mut self, min: f64, max: f64) -> f64 {
        min + self.random() * (max - min)
    }
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

/// The player's values this tick, copied out so the mode code can borrow the
/// game mutably afterwards.
#[derive(Clone, Copy)]
struct Snapshot {
    position: [f64; 3],
    rotation: (f32, f32),
    on_ground: bool,
    collision: bool,
    fall: f64,
    sprinting: bool,
    jumping: bool,
    in_water: bool,
    hurt_time: i32,
    strength: f32,
    ticker: i32,
    ticks: u64,
    now: i64,
}

impl Snapshot {
    fn read(game: &Game<'_>) -> Option<Self> {
        let player = game.player.as_ref()?;
        Some(Self {
            position: [
                player.position.x,
                player.position.y,
                player.position.z,
            ],
            rotation: (game.yaw, game.pitch),
            on_ground: player.ground.on_ground,
            collision: player.collision.horizontal,
            fall: player.fall_distance,
            sprinting: player.sprinting,
            jumping: player.jumping,
            in_water: player.fluids.was_touching_water,
            hurt_time: player.health.hurt.hurt_time,
            strength: game
                .hand_strength
                .scale(0.5, game.main_hand_attack_speed()),
            ticker: game.hand_strength.ticker,
            ticks: game.ticks,
            now: game.debug_millis(),
        })
    }
}

/// A living entity the crosshair is on.
#[derive(Clone, Copy)]
struct Target {
    hurt: i32,
    /// `hurtResistantTime` (the 20-tick invulnerability).
    resist: i32,
    position: [f64; 3],
}

/// The module's conditions: enabled, playing, and not in a spectator's view.
fn running_config(game: &Game<'_>) -> Option<CriticalsConfig> {
    if !game.native_window() || !game.native_playing() || !game.native_on(m::ID) {
        return None;
    }
    if game.spectator() {
        return None;
    }
    game.player.as_ref()?;
    m::criticals()
}

fn block_named(game: &Game<'_>, x: i32, y: i32, z: i32, names: &[&str]) -> bool {
    let state = game.world.block(x, y, z);
    let facts = game.res.facts();
    names.iter().any(|name| &*facts.get(state).name == *name)
}

fn block_named_xz(game: &Game<'_>, x: i32, y: i32, z: i32, names: &[&str]) -> bool {
    block_named(game, x, y, z, names) || block_named(game, x, y + 1, z, names)
}

/// The conditions that stop a hit from being critical in every client: water
/// and lava, ladders, cobwebs, riding, a busy hand, blindness, levitation,
/// slow falling, gliding and creative flight.
fn blocks_crit(game: &Game<'_>) -> bool {
    let Some(player) = game.player.as_ref() else {
        return true;
    };
    if player.spectator || player.flying || player.fall_flying {
        return true;
    }
    if player.fluids.was_touching_water || game.riding.is_some() || game.using.is_some() {
        return true;
    }
    if player.status.levitation.is_some() || player.status.slow_falling {
        return true;
    }
    if game.potions.effects.get("blindness").is_some() {
        return true;
    }
    let (x, y, z) = (
        player.position.x.floor() as i32,
        player.position.y.floor() as i32,
        player.position.z.floor() as i32,
    );
    block_named_xz(game, x, y, z, CLIMBABLE)
        || block_named_xz(game, x, y, z, COBWEB)
        || block_named_xz(game, x, y, z, LAVA)
}

/// The living entity under the crosshair.
fn target(game: &Game<'_>) -> Option<Target> {
    let (index, ..) = game.mob_hit()?;
    let mob = game.mobs.get(index)?;
    if !mob.alive() || Some(mob.id as i32) == game.remote.as_ref().map(|r| r.entity_id) {
        return None;
    }
    Some(Target {
        hurt: mob.hurt_time,
        resist: mob.invulnerable,
        position: [
            mob.position.x,
            mob.position.y + 0.9,
            mob.position.z,
        ],
    })
}

fn distance_sq(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

fn enemy_in_range(game: &Game<'_>, range: f32) -> bool {
    let Some(player) = game.player.as_ref() else {
        return false;
    };
    let me = game.remote.as_ref().map_or(-1, |r| r.entity_id);
    let at = [player.position.x, player.position.y, player.position.z];
    let limit = f64::from(range) * f64::from(range);
    game.mobs
        .iter()
        .any(|m| m.alive() && m.id as i32 != me && distance_sq([m.position.x, m.position.y, m.position.z], at) <= limit)
}

fn nearest_enemy(game: &Game<'_>, within: f32) -> bool {
    enemy_in_range(game, within)
}

/// A packet that reports a position (and the local rotation when `full`).
fn move_packet(kind: u8, position: [f64; 3], rotation: (f32, f32), flags: u8) -> S {
    if kind == 0 {
        S::MovePlayerPosRot(play::MovePlayerPosRot {
            x: position[0],
            y: position[1],
            z: position[2],
            yaw: rotation.0,
            pitch: rotation.1,
            flags,
        })
    } else {
        S::MovePlayerPos(play::MovePlayerPos {
            x: position[0],
            y: position[1],
            z: position[2],
            flags,
        })
    }
}

fn send_offset(
    game: &mut Game<'_>,
    kind: u8,
    position: [f64; 3],
    rotation: (f32, f32),
    dy: f64,
    ground: bool,
) {
    let flags = if ground { ON_GROUND } else { 0 };
    game.send(move_packet(
        kind,
        [position[0], position[1] + dy, position[2]],
        rotation,
        flags,
    ));
}

fn set_jump(game: &mut Game<'_>, height: f64, fall: Option<f64>) {
    if let Some(player) = game.player.as_mut() {
        player.velocity.y = height;
        player.ground.on_ground = false;
        if let Some(fall) = fall {
            player.fall_distance = fall;
        }
    }
}

fn stop_sprint(game: &mut Game<'_>) {
    if let Some(player) = game.player.as_mut() {
        player.sprinting = false;
    }
    let id = game.remote.as_ref().map_or(0, |r| r.entity_id);
    if let Some(remote) = game.remote.as_mut() {
        // The stop is on its way; the sender must not send a second one.
        remote.was_sprinting = false;
    }
    game.send(S::PlayerCommand(play::PlayerCommand {
        id,
        action: STOP_SPRINTING,
        data: 0,
    }));
}

/// `MoveUtils.isOnGround(0.001)`: the player stands on a block, not merely
/// within a small fall of one.
fn near_ground(game: &Game<'_>) -> bool {
    game.player.as_ref().is_some_and(|p| p.ground.on_ground)
}

fn air_above_two(game: &Game<'_>, position: [f64; 3]) -> bool {
    let x = position[0].floor() as i32;
    let y = (position[1] + 2.0).floor() as i32;
    let z = position[2].floor() as i32;
    game.res.facts().is_air(game.world.block(x, y, z))
}

/// The reference's "would a hit right now be critical": off the ground and
/// falling.
fn would_crit(game: &Game<'_>) -> bool {
    game.player
        .as_ref()
        .is_some_and(|p| !p.ground.on_ground && p.fall_distance > 0.0 && !p.sprinting)
}

fn spawn_criticals(game: &mut Game<'_>, config: &CriticalsConfig, at: [f64; 3], state: &mut State) {
    for _ in 0..config.visual_critical {
        let p = [
            at[0] + state.range(-0.2, 0.2),
            at[1] + state.range(-0.2, 0.2),
            at[2] + state.range(-0.2, 0.2),
        ];
        let v = [
            state.range(-0.1, 0.1),
            state.range(0.1, 0.3),
            state.range(-0.1, 0.1),
        ];
        game.particles
            .add(&ParticleOptions::simple(ParticleType::Crit), p, v);
    }
    for _ in 0..config.visual_magic {
        let p = [
            at[0] + state.range(-0.2, 0.2),
            at[1] + state.range(-0.2, 0.2),
            at[2] + state.range(-0.2, 0.2),
        ];
        game.particles
            .add(&ParticleOptions::simple(ParticleType::EnchantedHit), p, [0.0; 3]);
    }
}

/// The attack click, at the head of `remote_start_attack`: the modes' fake
/// positions go out before the attack packet.
pub(super) fn attack(game: &mut Game<'_>) {
    let Some(config) = running_config(game) else {
        return;
    };
    if config.mode == Mode::Off {
        return;
    }
    let Some(snapshot) = Snapshot::read(game) else {
        return;
    };
    if game.clicks.miss_time > 0 || blocks_crit(game) {
        return;
    }
    let Some(hit) = target(game) else {
        return;
    };
    if config.only_sprinting && !snapshot.sprinting {
        return;
    }
    if let Some(mut state) = Some(take_state()) {
        if config.sprint_stop == 3 && snapshot.sprinting {
            stop_sprint(game);
        }
        // Sprinting blocks a server-side critical hit in the versions this
        // port targets; the packet modes cannot land one while it is on.
        if snapshot.sprinting && config.sprint_stop != 3 {
            put_state(state);
            return;
        }

        let packet_kind = config.packet_type;
        let mut visual = false;
        let position = snapshot.position;
        let rotation = snapshot.rotation;
        let on_ground = snapshot.on_ground;
        let now = snapshot.now;
        let ticks = snapshot.ticks;

        // The FDP-style modes wait out the delay and skip a target whose hurt
        // animation has not run past the configured hurt time.
        if matches!(
            config.mode,
            Mode::Packet
                | Mode::NoGround
                | Mode::Jump
                | Mode::Blink
                | Mode::Timer
                | Mode::NcpPacket
                | Mode::BlocksMc
                | Mode::BlocksMc2
                | Mode::Hop
                | Mode::MiniJump
                | Mode::TpHop
                | Mode::LowJump
                | Mode::CustomMotion
                | Mode::Visual
        ) && (hit.hurt > config.hurt_time
            || now - state.last_attack_ms < i64::from(config.delay))
        {
            put_state(state);
            return;
        }

        use Mode::*;
        match config.mode {
            Packet => {
                match config.packet_mode {
                    0 => {
                        send_offset(game, packet_kind, position, rotation, 0.2, false);
                        send_offset(game, packet_kind, position, rotation, 0.01, false);
                        visual = true;
                    }
                    1 => {
                        send_offset(game, packet_kind, position, rotation, 0.11, false);
                        send_offset(
                            game,
                            packet_kind,
                            position,
                            rotation,
                            0.1100013579,
                            false,
                        );
                        send_offset(
                            game,
                            packet_kind,
                            position,
                            rotation,
                            0.0000013579,
                            false,
                        );
                        visual = true;
                    }
                    2 => {
                        send_offset(game, packet_kind, position, rotation, 0.0625, false);
                        send_offset(
                            game,
                            packet_kind,
                            position,
                            rotation,
                            0.0625013579,
                            false,
                        );
                        send_offset(
                            game,
                            packet_kind,
                            position,
                            rotation,
                            0.0000013579,
                            false,
                        );
                        visual = true;
                    }
                    3 => {
                        send_offset(game, packet_kind, position, rotation, 1.0e-9, false);
                        send_offset(game, packet_kind, position, rotation, 0.0, false);
                        visual = true;
                    }
                    4 => {
                        send_offset(game, packet_kind, position, rotation, -1.0e-9, false);
                        visual = true;
                    }
                    5 => {
                        if !on_ground {
                            send_offset(game, packet_kind, position, rotation, -0.000001, false);
                            visual = true;
                        }
                    }
                    6 => {
                        if ticks % 4 == 0 {
                            send_offset(game, packet_kind, position, rotation, 0.0011, true);
                            send_offset(game, packet_kind, position, rotation, 0.0, false);
                            visual = true;
                        }
                    }
                    _ => {}
                }
            }
            Off
            | NoGround
            | Blink
            | Timer
            | Vulcan
            | TenacityWatchdog
            | SigmaHover
            | DeadcodeFall
            | DeadcodeMatrixDisabler
            | CelestialSunriseGround
            | CelestialSunriseAir
            | CelestialSunriseDrop
            | CelestialOldSunrise
            | CelestialOldMatrix
            | CelestialMatrix630
            | Novoline => {}
            Edit => {
                if on_ground && now - state.last_edit_ms >= i64::from(config.edit_delay) {
                    state.edit_attacked = true;
                    state.last_edit_ms = now;
                }
            }
            Watchdog => {
                if on_ground
                    && now - state.last_watchdog_ms >= i64::from(config.watchdog_delay)
                {
                    state.watchdog_pending = true;
                    state.last_watchdog_ms = now;
                }
            }
            Jump => {
                if on_ground {
                    set_jump(game, f64::from(config.jump_height), None);
                }
            }
            NcpPacket => {
                send_offset(game, 1, position, rotation, 0.11, false);
                send_offset(game, 1, position, rotation, 0.1100013579, false);
                send_offset(game, 1, position, rotation, 0.0000013579, false);
                visual = true;
            }
            BlocksMc => {
                send_offset(game, 1, position, rotation, 0.001091981, true);
                send_offset(game, 1, position, rotation, 0.0, false);
            }
            BlocksMc2 => {
                if ticks % 4 == 0 {
                    send_offset(game, 1, position, rotation, 0.0011, true);
                    send_offset(game, 1, position, rotation, 0.0, false);
                }
            }
            Hop => {
                if let Some(player) = game.player.as_mut() {
                    player.velocity.y = 0.1;
                    player.fall_distance = 0.1;
                    player.ground.on_ground = false;
                }
            }
            MiniJump => {
                send_offset(game, 1, position, rotation, 0.0625, false);
                send_offset(game, 1, position, rotation, 0.0, true);
                if let Some(player) = game.player.as_mut() {
                    player.velocity.y = 0.06;
                    player.fall_distance = 0.06;
                    player.ground.on_ground = false;
                }
                visual = true;
            }
            TpHop => {
                send_offset(game, 1, position, rotation, 0.02, false);
                send_offset(game, 1, position, rotation, 0.01, false);
                if let Some(player) = game.player.as_mut() {
                    player.position.y += 0.01;
                }
            }
            LowJump => set_jump(game, 0.3425, None),
            CustomMotion => set_jump(game, f64::from(config.custom_motion_y), None),
            Visual => visual = true,
            PacketAttack => {
                if snapshot.ticker > 0
                    && now - state.last_alan_ms >= i64::from(config.alan_delay)
                {
                    send_offset(game, 1, position, rotation, 0.42, false);
                    send_offset(game, 1, position, rotation, 0.0, false);
                    state.last_alan_ms = now;
                    visual = true;
                }
            }
            Verus => {
                if snapshot.ticker > 2
                    && snapshot.hurt_time != 0
                    && now - state.last_alan_ms >= i64::from(config.alan_delay)
                {
                    send_offset(game, 1, position, rotation, 0.0625, false);
                    send_offset(game, 1, position, rotation, 0.0, false);
                    state.last_alan_ms = now;
                    visual = true;
                }
            }
            Mospixel => {
                let base = 0.04 + state.range(0.0, 0.04);
                let mid = base * 0.3 + state.range(0.0, 0.005);
                let tiny = 0.0005 + state.range(0.0, 0.001);
                send_offset(game, 1, position, rotation, base, false);
                send_offset(game, 1, position, rotation, mid, false);
                send_offset(game, 1, position, rotation, tiny, false);
                send_offset(game, 1, position, rotation, 0.0, false);
            }
            Ncp => {
                state.ncp_ticks += 1;
                if state.ncp_ticks > config.ncp_ticks {
                    if on_ground {
                        send_offset(game, 1, position, rotation, 0.00001, false);
                        send_offset(game, 1, position, rotation, 0.000008, false);
                        send_offset(game, 1, position, rotation, 0.0000002, false);
                    }
                    state.ncp_ticks = 0;
                    visual = true;
                }
            }
            Summer => {
                if state.ground_ticks > config.summer_ground_ticks
                    && now - state.last_summer_ms >= i64::from(config.summer_delay)
                {
                    send_offset(game, 1, position, rotation, 0.056, false);
                    send_offset(game, 1, position, rotation, 0.016, false);
                    send_offset(game, 1, position, rotation, 0.003, false);
                    state.last_summer_ms = now;
                }
            }
            TenacityPacket => {
                if on_ground && hit.resist > config.tenacity_delay {
                    send_offset(game, 1, position, rotation, 0.006253453, false);
                    send_offset(game, 1, position, rotation, 0.002253453, false);
                    send_offset(game, 1, position, rotation, 0.001253453, false);
                }
            }
            TenacityDev => {
                if on_ground && hit.resist > config.tenacity_delay {
                    send_offset(game, 1, position, rotation, 0.06253453, false);
                    send_offset(game, 1, position, rotation, 0.02253453, false);
                    send_offset(game, 1, position, rotation, 0.001253453, false);
                    send_offset(game, 1, position, rotation, 0.0001135346, false);
                }
            }
            TenacityVerus => {
                if on_ground {
                    let offset = match hit.resist {
                        17 | 19 => 0.001 + state.range(0.0, 0.0001),
                        18 | 20 => 0.03 + state.range(0.0, 0.0001),
                        _ => 0.0,
                    };
                    if offset > 0.0 {
                        send_offset(game, 1, position, rotation, offset, false);
                    }
                }
            }
            FluxPacket => {
                if on_ground
                    && hit.resist <= config.flux_hurt_time
                    && now - state.last_flux_ms >= i64::from(config.flux_delay)
                {
                    send_offset(game, 1, position, rotation, 0.06, false);
                    send_offset(game, 1, position, rotation, 0.0, false);
                    send_offset(game, 1, position, rotation, 0.03, false);
                    send_offset(game, 1, position, rotation, 0.0, false);
                    state.last_flux_ms = now;
                }
            }
            FluxJump => {
                if on_ground && hit.resist <= config.flux_hurt_time {
                    set_jump(game, 0.42, None);
                }
            }
            FluxHypixel => {
                if on_ground
                    && hit.resist <= config.flux_hurt_time
                    && now - state.last_flux_ms >= i64::from(config.flux_delay)
                {
                    let fraction = position[1] - position[1].floor();
                    if fraction == 0.0 || !config.flux_post_motion {
                        if !config.flux_post_motion {
                            send_offset(game, 1, position, rotation, 0.0, false);
                        }
                        let y1 = [0.104080378093037, 0.105454222033912, 0.102888018147468, 0.099634532004642];
                        let first = 0.075
                            + state.range(0.0, 0.008)
                                * if state.random() < 0.5 { 0.96 } else { 0.97 }
                            + (ticks as f64 % 0.0215) * 0.92;
                        let second = if state.random() < 0.5 {
                            0.010634691223
                        } else {
                            0.013999777
                        } * if state.random() < 0.5 { 0.95 } else { 0.96 }
                            * y1[(state.random() * y1.len() as f64) as usize % y1.len()]
                            * 9.5;
                        for offset in [first, second] {
                            let jitter = 1.0 + state.range(-0.005, 0.005);
                            send_offset(game, 1, position, rotation, offset * jitter, false);
                        }
                        state.last_flux_ms = now;
                    }
                }
            }
            SigmaPacket | SigmaHPacket => {
                if on_ground
                    && hit.resist <= config.sigma_hurt_time
                    && (now - state.last_sigma_ms >= 200 || hit.resist > 0)
                    && state.ground_ticks > 1
                {
                    send_offset(game, 0, position, rotation, 0.0626, false);
                    if config.mode == SigmaHPacket {
                        send_offset(game, 0, position, rotation, 0.0626 + 1.0e-11, false);
                    }
                    send_offset(game, 0, position, rotation, 0.0, false);
                    state.last_sigma_ms = now;
                }
            }
            SigmaJump => {
                if on_ground && !snapshot.jumping && hit.resist <= config.sigma_hurt_time {
                    set_jump(game, 0.42, None);
                }
            }
            SigmaMinis => {
                if on_ground && !snapshot.jumping && hit.resist <= config.sigma_hurt_time {
                    game.send(move_packet(0, position, rotation, ON_GROUND));
                }
            }
            DeadcodePacket | DeadcodeMatrix => {
                if now - state.last_deadcode_ms >= i64::from(config.deadcode_delay) {
                    let allowed = config.mode == DeadcodeMatrix || hit.hurt < 7;
                    if allowed {
                        send_offset(game, 1, position, rotation, 0.0625, true);
                        send_offset(game, 1, position, rotation, 0.0, false);
                        state.last_deadcode_ms = now;
                        visual = true;
                    }
                }
            }
            DeadcodeJump => {
                if now - state.last_deadcode_ms >= i64::from(config.deadcode_delay)
                    && hit.hurt < 7
                {
                    set_jump(game, 0.42, None);
                    state.last_deadcode_ms = now;
                }
            }
            CelestialPacket => {
                if config.celestial_mini_jump {
                    send_offset(game, 1, position, rotation, 0.04, false);
                }
                send_offset(game, 1, position, rotation, 0.11, false);
                send_offset(game, 1, position, rotation, 0.1100013579, false);
                send_offset(game, 1, position, rotation, 0.0000013579, false);
                visual = true;
            }
            CelestialNcp => {
                if on_ground
                    && now - state.last_celestial_ncp_ms >= i64::from(config.celestial_ncp_delay)
                {
                    if config.celestial_mini_jump {
                        send_offset(game, 1, position, rotation, 0.04, false);
                    }
                    send_offset(game, 1, position, rotation, 0.4100000000000002, false);
                    send_offset(game, 1, position, rotation, 0.01099999999999924, false);
                    state.last_celestial_ncp_ms = now;
                    visual = true;
                }
            }
            CelestialNcpNew => {
                if on_ground && !snapshot.in_water && air_above_two(game, position) {
                    send_offset(game, 1, position, rotation, 0.4100000000000002, false);
                    send_offset(game, 1, position, rotation, 0.01099999999999924, false);
                    visual = true;
                }
            }
        }

        state.last_attack_ms = now;
        if config.visuals && (config.visual_fake || visual) {
            spawn_criticals(game, &config, hit.position, &mut state);
        }
        put_state(state);
    }
}

/// The pre-motion side of the modes, at the head of `remote_send_state`.
pub(super) fn prepare(game: &mut Game<'_>) {
    let mut state = take_state();
    state.plan = Plan::default();

    // Celestial's Matrix 6.3.0 clears the fall it built when it is turned off.
    if state.was_active && state.mode == Some(Mode::CelestialMatrix630) && state.last_reduce {
        if running_config(game).is_none() {
            let fall = game.player.as_ref().map_or(0.0, |p| p.fall_distance);
            let position = game
                .player
                .as_ref()
                .map_or([0.0; 3], |p| [p.position.x, p.position.y, p.position.z]);
            if fall > MATRIX630_FALL {
                game.send(move_packet(0, position, (game.yaw, game.pitch), ON_GROUND));
                if let Some(player) = game.player.as_mut() {
                    player.fall_distance = 0.0;
                }
            }
            state = State::default();
            put_state(state);
            return;
        }
    }

    let Some(config) = running_config(game) else {
        state = State::default();
        put_state(state);
        return;
    };
    if state.mode != Some(config.mode) {
        let rng = state.rng;
        state = State::default();
        state.rng = rng;
        state.mode = Some(config.mode);
    }
    state.was_active = true;
    state.last_reduce = config.celestial_reduce_damage;

    let Some(snapshot) = Snapshot::read(game) else {
        put_state(state);
        return;
    };
    let position = snapshot.position;
    let on_ground = snapshot.on_ground;
    let now = snapshot.now;
    let ticks = snapshot.ticks;

    if on_ground {
        state.ground_ticks = state.ground_ticks.saturating_add(1);
        state.air_ticks = 0;
    } else {
        state.ground_ticks = 0;
        state.air_ticks = state.air_ticks.saturating_add(1);
    }
    state.ncp_ticks = state.ncp_ticks.saturating_add(1);

    // Stop sprinting before the sender compares the flag: the stop goes out
    // with this tick's packets, and the attack that follows next tick is one
    // the server sees as airborne.
    if matches!(config.sprint_stop, 1 | 2)
        && snapshot.sprinting
        && enemy_in_range(game, config.sprint_range)
        && !blocks_crit(game)
    {
        if let Some(player) = game.player.as_mut() {
            player.sprinting = false;
        }
    }

    let mut plan = Plan {
        position,
        rotation: snapshot.rotation,
        flags: u8::from(snapshot.on_ground)
            | u8::from(snapshot.collision) * HORIZONTAL_COLLISION,
        ..Plan::default()
    };

    use Mode::*;
    match config.mode {
        NoGround => {
            plan.active = true;
            plan.ground = Some(false);
        }
        Vulcan => {
            if snapshot.fall < 1.8 && state.air_ticks <= 18 {
                plan.active = true;
                plan.ground = Some(false);
            }
        }
        Edit => {
            if on_ground && state.edit_attacked {
                state.edit_stage += 1;
                state.edit_stage = state.edit_stage.min(3);
                match state.edit_stage {
                    1 => plan.offset[1] = 5.0e-4,
                    2 => {
                        plan.offset[1] = 1.0e-4;
                        state.edit_attacked = false;
                    }
                    _ => {}
                }
                plan.active = true;
                plan.ground = Some(false);
                plan.force = true;
            } else {
                state.edit_attacked = false;
                state.edit_stage = 0;
            }
        }
        Watchdog => {
            if on_ground && state.watchdog_pending {
                state.watchdog_stage += 1;
                match state.watchdog_stage {
                    1 => plan.offset[1] = 0.001,
                    2 => state.watchdog_pending = false,
                    _ => {}
                }
                plan.active = true;
                plan.ground = Some(false);
                plan.force = true;
            } else {
                state.watchdog_pending = false;
                state.watchdog_stage = 0;
            }
        }
        TenacityWatchdog if config.tenacity_watchdog_mode == 1 => {
            if on_ground && enemy_in_range(game, 4.0) {
                state.tenacity_stage = !state.tenacity_stage;
                plan.offset[1] = if state.tenacity_stage { 0.015 } else { 0.01 }
                    - state.range(0.0, 0.0001);
                if state.ground_ticks > 2 {
                    plan.active = true;
                    plan.ground = Some(false);
                    plan.force = true;
                }
            }
        }
        SigmaHover => {
            if near_ground(game) {
                plan.active = true;
                plan.force = true;
                plan.ground = Some(false);
                match state.hover_stage {
                    0 => {
                        state.hover_y = position[1] + 1.0e-8;
                        plan.ground = Some(true);
                    }
                    1 => state.hover_y -= 5.0e-15,
                    _ => state.hover_y -= 4.0e-15,
                }
                if state.hover_y <= position[1] {
                    state.hover_stage = 0;
                    state.hover_y = position[1];
                    plan.ground = Some(true);
                }
                plan.offset[1] = state.hover_y - position[1];
                state.hover_stage += 1;
            } else {
                state.hover_stage = 0;
                state.hover_y = position[1];
            }
        }
        CelestialSunriseGround | CelestialMatrix630 => {
            plan.active = true;
            plan.ground = Some(false);
            plan.force = true;
        }
        CelestialOldMatrix => {
            if now - state.last_old_matrix_ms >= 300 {
                state.last_old_matrix_ms = now;
                let id = game.remote.as_ref().map_or(0, |r| r.entity_id);
                game.send(S::PlayerCommand(play::PlayerCommand {
                    id,
                    action: START_FALL_FLYING,
                    data: 0,
                }));
            }
            if config.celestial_ground_false {
                plan.active = true;
                plan.ground = Some(false);
            }
            if !snapshot.jumping && on_ground {
                plan.active = true;
                plan.ground = Some(false);
                plan.offset[1] = if ticks % 2 != 1 { 0.08 } else { 0.05 };
                plan.force = true;
            }
        }
        CelestialOldSunrise => {
            if !snapshot.jumping && on_ground {
                plan.active = true;
                plan.ground = Some(false);
                plan.offset[1] = if ticks % 2 != 1 { 0.14 } else { 0.09 };
                plan.force = true;
            }
        }
        CelestialSunriseDrop => {
            if !snapshot.jumping && on_ground {
                plan.active = true;
                plan.ground = Some(false);
                plan.offset[1] = if ticks % 2 == 0 { 0.3 } else { 0.23 };
                plan.force = true;
            }
        }
        Novoline => {
            // Rewrites the movement packet the sender produces: on Hypixel the
            // lift depends on the target's hurt-resistant time, elsewhere it is
            // a random lift skipped every fifth tick.
            if let Some(hit) = target(game) {
                if on_ground
                    && !snapshot.in_water
                    && !snapshot.jumping
                    && !blocks_crit(game)
                {
                    let random = f64::from(config.novoline_random);
                    let y = if config.novoline_hypixel {
                        match hit.resist {
                            17 => Some(0.00163166800276),
                            18 => Some(0.11921599284565),
                            19 => Some(0.15919999545217),
                            20 => Some(0.11999999731779),
                            _ => None,
                        }
                        .map(|base| base + state.range(0.0, random))
                    } else if ticks % 5 != 0 {
                        Some(state.range(0.0, random))
                    } else {
                        None
                    };
                    if let Some(y) = y.filter(|y| *y > 0.0) {
                        plan.active = true;
                        plan.ground = Some(false);
                        plan.offset[1] = y;
                    }
                }
            }
        }
        DeadcodeMatrixDisabler => {
            if on_ground {
                if let Some(player) = game.player.as_mut() {
                    player.velocity.y = 0.16;
                    player.ground.on_ground = false;
                }
            }
        }
        DeadcodeFall => {
            if config.deadcode_auto_jump
                && on_ground
                && nearest_enemy(game, config.deadcode_auto_jump_distance)
            {
                let fall = f64::from(config.deadcode_fall_distance);
                set_jump(game, 0.42, Some(fall));
            }
        }
        Blink => {
            if enemy_in_range(game, config.blink_range) && !would_crit(game) {
                if now < state.blink_until {
                    plan.active = true;
                    plan.suppress = true;
                } else {
                    let low = i64::from(config.blink_delay_min.min(config.blink_delay_max));
                    let high = i64::from(config.blink_delay_max.max(config.blink_delay_min));
                    let delay = state.range(low as f64, high as f64) as i64;
                    state.blink_until = now + delay;
                }
            } else {
                state.blink_until = 0;
            }
        }
        Jump => {
            if on_ground
                && (!config.jump_optimize || snapshot.strength > 0.9)
                && enemy_in_range(game, config.jump_range)
            {
                set_jump(game, f64::from(config.jump_height), None);
            }
        }
        _ => {}
    }

    state.plan = plan;
    put_state(state);
}

/// Applies the plan to the packet the position sender produced, or builds one
/// when the mode has to reach the server every tick.
pub(super) fn spoof(packet: Option<S>) -> Option<S> {
    let plan = STATE.with(|s| s.borrow().plan);
    if !plan.active {
        return packet;
    }
    if plan.suppress {
        return None;
    }
    let Some(packet) = packet else {
        if !plan.force {
            return None;
        }
        let position = [
            plan.position[0] + plan.offset[0],
            plan.position[1] + plan.offset[1],
            plan.position[2] + plan.offset[2],
        ];
        let mut flags = plan.flags & !ON_GROUND;
        if plan.ground.unwrap_or(plan.flags & ON_GROUND != 0) {
            flags |= ON_GROUND;
        }
        return Some(move_packet(0, position, plan.rotation, flags));
    };
    Some(rewrite(packet, plan.ground, plan.offset))
}

fn rewrite(packet: S, ground: Option<bool>, offset: [f64; 3]) -> S {
    let flags = |current: u8| match ground {
        Some(on_ground) => (current & !ON_GROUND) | u8::from(on_ground),
        None => current,
    };
    match packet {
        S::MovePlayerPosRot(mut p) => {
            p.x += offset[0];
            p.y += offset[1];
            p.z += offset[2];
            p.flags = flags(p.flags);
            S::MovePlayerPosRot(p)
        }
        S::MovePlayerPos(mut p) => {
            p.x += offset[0];
            p.y += offset[1];
            p.z += offset[2];
            p.flags = flags(p.flags);
            S::MovePlayerPos(p)
        }
        S::MovePlayerRot(mut p) => {
            p.flags = flags(p.flags);
            S::MovePlayerRot(p)
        }
        S::MovePlayerStatus(mut p) => {
            p.flags = flags(p.flags);
            S::MovePlayerStatus(p)
        }
        other => other,
    }
}
