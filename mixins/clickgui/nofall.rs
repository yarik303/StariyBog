// NoFall: a port of LiquidBounce's ModuleNoFall and its modes.
//
// Copyright (c) 2015-2026 CCBlueX. LiquidBounce is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed in the
// hope that it will be useful, but WITHOUT ANY WARRANTY; without even the
// implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the LICENSE file or <https://www.gnu.org/licenses/>.
//
// The module works around the movement packet the remote player sends once a
// tick (`remote_send_state` -> `PositionSender::update`). `prepare` runs at the
// head of that function, after the tick's physics: it decides what the tick's
// own packet should say (ground flag or position offset), applies the mode's
// client-side effects (reset the fall distance, change the velocity, jump,
// place a block, mount a mob), and sends the extra packets some modes need
// (LiquidBounce's modes send those beside the untouched movement packet).
// `spoof` then rewrites the packet the sender produced, using the plan
// `prepare` left behind. The Cancel mode also answers the server's teleport in
// `teleport`.
//
// Blocks that count as "solid" here are the states the collision registry
// gives a non-empty shape (a state-dependent shape counts unless it is a
// liquid); the modes scan the column below the player instead of a full
// bounding-box collision query. MLG casts the click with `clip_ray` when it
// plans, and sends that exact hit through the remote client's own use code on
// the next tick (after the planned tick's movement packet, before the one
// that lands), rather than using LiquidBounce's placement planner and
// silent-hotbar manager. Mount and Rettungsplatform send the remote client's
// own use packets aimed for one click. Blink's packet queueing and
// HypixelPacket's timer change have no counterpart here, so Blink only spoofs
// the ground flag and HypixelPacket only sends its status packet.
//
// Besides LiquidBounce's mode list this port also carries the modes from its
// `implementations` folder and the other clients it was collected from
// (AlwaysSpoof, Damage, Vanilla, Packet1/2, MotionFlag, CubeCraft, Intave,
// Polar, Sentinel, LatestVulcan, the Matrix and AAC families, LAAC, the older
// Vulcan and packet variants, RoundNoFall, NoFallDamage, the Watchdog modes,
// HypixelTimer/HypixelFlag, Heypixel, ChunkLoad, Phase and the OldGrim
// bypasses (Grim72/Grim73, whose START_FALL_FLYING pair needs an elytra worn). Their client-side
// timer slowdowns (`Timer.timerSpeed`) have no effect here because the mod
// does not own the render clock, so the parts that rely on them only spoof
// packets or change the player's own state. The modes that need a queue of
// held packets or a packet-receive hook (AACv4, HypixelBlink, WatchdogBlink,
// GrimServer19) and the ViaVersion elytra and Clutch/MLG duplicates are not
// separate modes.
use crate::game::{Game, Hit, clickgui as m};
use horizon_client::{chunk::MIN_Y, first_person::Hand, mobs::Species};
use horizon_core::geometry::Vec3;
use horizon_protocol::packets::play::{self, Serverbound as S};
use std::cell::RefCell;
use std::ops::ControlFlow;

/// `Items.MACE`.
const MACE: &str = "minecraft:mace";
/// `NoFallBlocksMC.MIN_AGE`: ticks the mode waits before clipping.
const MIN_AGE: u64 = 20 * 5;

/// The ground bit of `ServerboundMovePlayerPacket`'s flags.
const ON_GROUND: u8 = 1;
/// The horizontal-collision bit of the same flags.
const HORIZONTAL_COLLISION: u8 = 1 << 1;
/// The jump key in the `PlayerInput` flags.
const INPUT_JUMP: u8 = 1 << 4;

/// The mace's smash fall threshold (`MaceItem.SMASH_ATTACK_FALL_THRESHOLD`).
const MAX_FALL_SPEED: f64 = 3.92;
/// The highest jump a plain jump gives (`Player.jumpFromGround`).
const JUMP_VELOCITY: f64 = 0.42;

/// OldGrimNoFall's `fallDistance > 3.0`.
const GRIM_FALL: f64 = 3.0;
/// OldGrimNoFall's 200 ms between START_FALL_FLYING pairs, in ticks.
const GRIM_SHORT: u8 = 4;
/// OldGrimNoFall's 300 ms spoof window after a packet or a setback, in ticks.
const GRIM_LONG: u8 = 6;
/// `ServerboundPlayerCommandPacket.Action.START_FALL_FLYING`.
const START_FALL_FLYING: i32 = 6;

/// LAAC checks `Entity.isOnLadder`: any block in `#minecraft:climbable`.
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
/// LAAC checks `Entity.isInWeb`.
const COBWEB: &[&str] = &["minecraft:cobweb"];

/// What this tick's movement packet should be. `prepare` fills it, `spoof`
/// applies it to the packet the sender produced.
#[derive(Clone, Copy, Default)]
struct Plan {
    /// The module ran this tick: `spoof` may rewrite the packet.
    active: bool,
    /// Drop the packet the sender produced.
    suppress: bool,
    /// Replace the packet's on-ground flag.
    ground: Option<bool>,
    /// Added to the packet's position (position-carrying packets only).
    offset: [f64; 3],
}

/// The client-side changes a mode makes to the player for this tick.
#[derive(Default)]
struct Effects {
    reset_fall: bool,
    /// Replace the fall distance instead of resetting it to zero.
    fall_set: Option<f64>,
    velocity_y: Option<f64>,
    /// Multiplies both horizontal speed components (0 stops them).
    velocity_xz: Option<f64>,
    sprint: Option<bool>,
    /// The client-side ground flag some modes clear or force.
    client_ground: Option<bool>,
    /// `Player.isSneaking`, which VulcanFast288 sets.
    sneaking: Option<bool>,
    position_add: [f64; 3],
}

/// A use or interaction the mode performs after the packet plan is computed.
enum Action {
    None,
    /// Select `slot` and use the held item on the block `hit` names.
    Use { slot: usize, hit: Hit },
    /// Select `slot` and use the held item where it points.
    Item { slot: usize },
    /// Aim at `at` and interact with the entity there.
    Interact { at: [f64; 3] },
}

/// The state the next tick needs, kept between movement packets.
#[derive(Default)]
struct State {
    plan: Plan,
    last_mode: Option<m::NoFallMode>,
    /// Ticks the module has run, for BlocksMC's AntiBot guard.
    age: u64,
    /// The ground flag and fall distance of the previous tick's packet.
    last_on_ground: bool,
    last_fall: f64,
    /// The hotbar slot to go back to after an MLG/Rettungsplatform use.
    restore_slot: Option<usize>,
    /// SpoofLanding: a landing was spoofed, so jump next tick on the ground.
    spoof_landing: bool,
    /// PacketJump's `falling` (the previous tick's fall was over the value).
    packet_jump_falling: bool,
    /// Spartan524Flag's `waitTicks(1)` between status packets.
    spartan_wait: bool,
    /// Hypixel's `doJump`.
    hypixel_jump: bool,
    /// ForceJump's `jumpTriggered`.
    force_jump: bool,
    /// BlocksMC's `shouldClip`.
    should_clip: bool,
    /// Blink's `blinkFall`.
    blink_fall: bool,
    /// Blink's `waitUntilGround`, inverted: the player has been on the ground
    /// since the mode was enabled.
    blink_ready: bool,
    /// Ticks before Mount tries the next mob.
    mount_cooldown: i32,
    /// Ticks before Rettungsplatform uses the item again.
    rettung_cooldown: i32,
    /// MLG's click, held until the movement packet of the tick that planned
    /// it has gone out: the server then checks the click's reach from the
    /// position the plan was made at, and the block is placed before the
    /// landing move arrives.
    pending_use: Option<(usize, Hit)>,
    /// MLG placed a block or is about to: no second plan until the fall ends.
    mlg_wait: bool,
    /// OldGrim: a spoofed packet is waiting for the server's setback.
    grim_sent: bool,
    /// OldGrim: packets left that report a ground flag after a setback
    /// (2: airborne, 1: on the ground).
    grim_counter: u8,
    /// OldGrim72: ticks since the last START_FALL_FLYING pair.
    grim_short: u8,
    /// OldGrim: ticks since the spoof window opened.
    grim_long: u8,
    /// AAC's `currentState`.
    aac_state: u8,
    /// Packet1/Packet2's `packetCount`.
    interval_count: u32,
    /// Matrix6.2.X's fall bookkeeping.
    matrix_can_spoof: bool,
    matrix_fall_ticks: i32,
    matrix_is_fall: bool,
    matrix_last_motion: f64,
    /// Matrix6.6.3's `matrixSend`.
    matrix_send: bool,
    /// AAC5.0.14's collision check, flag and timer.
    aac5_check: bool,
    aac5_do_flag: bool,
    aac5_timer: i32,
    /// LAAC's `jumped`.
    laac_jumped: bool,
    /// AAC5.0.4 and HypixelFlag's `isDamageFalling`.
    damage_falling: bool,
    /// Heypixel's and ChunkLoad's fake fall.
    heypixel_spoof: bool,
    chunk_unloaded: bool,
}

impl State {
    fn reset(&mut self) {
        *self = State::default();
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The player's values this tick, copied out so the mode code can borrow the
/// game mutably afterwards.
#[derive(Clone, Copy)]
struct Snapshot {
    position: [f64; 3],
    on_ground: bool,
    collision: bool,
    fall: f64,
    velocity_y: f64,
    may_fly: bool,
    flying: bool,
    /// LAAC's `isInWater`, `isOnLadder` and `isInWeb`.
    in_water: bool,
    climbing: bool,
    in_web: bool,
}

impl Snapshot {
    fn read(game: &Game<'_>) -> Option<Self> {
        let player = game.player.as_ref()?;
        let position = [player.position.x, player.position.y, player.position.z];
        let (x, y, z) = (
            position[0].floor() as i32,
            position[1].floor() as i32,
            position[2].floor() as i32,
        );
        Some(Self {
            position,
            on_ground: player.ground.on_ground,
            collision: player.collision.horizontal,
            fall: player.fall_distance,
            velocity_y: player.velocity.y,
            may_fly: player.may_fly,
            flying: player.flying,
            in_water: player.fluids.was_touching_water,
            climbing: block_named(game, x, y, z, CLIMBABLE)
                || block_named(game, x, y + 1, z, CLIMBABLE),
            in_web: block_named(game, x, y, z, COBWEB)
                || block_named(game, x, y + 1, z, COBWEB),
        })
    }
}

/// The module's conditions: enabled, playing, and none of the "Not" options
/// (LiquidBounce's `running`).
fn running_config(game: &Game<'_>) -> Option<m::NoFallConfig> {
    if !game.native_window() || !game.native_playing() || !game.native_on(m::ID) {
        return None;
    }
    if game.creative || game.spectator() {
        return None;
    }
    let config = m::nofall()?;
    let player = game.player.as_ref()?;
    if player.invulnerable || player.flying {
        return None;
    }
    if config.not_while_gliding && player.fall_flying {
        return None;
    }
    let main_hand = game.inventory.get(game.selected).and_then(|stack| stack.as_ref());
    if config.not_with_mace && main_hand.is_some_and(|stack| &*stack.item == MACE) {
        return None;
    }
    Some(config)
}

fn safe_fall_distance(game: &mut Game<'_>) -> f64 {
    game.player
        .as_mut()
        .and_then(|player| player.attribute("minecraft:safe_fall_distance").ok())
        .unwrap_or(3.0)
}

fn interaction_range(game: &mut Game<'_>) -> f64 {
    game.player
        .as_mut()
        .and_then(|player| player.attribute("minecraft:entity_interaction_range").ok())
        .unwrap_or(3.0)
}

/// Whether the integrated server runs this world.
fn local_server(game: &Game<'_>) -> bool {
    game.remote
        .as_ref()
        .is_none_or(|remote| remote.session.address == "local")
}

/// Switches to `slot` for this tick, telling the server now (the sender's own
/// SetCarriedItem comes later, after the use); the next tick switches back.
fn select(game: &mut Game<'_>, state: &mut State, slot: usize) {
    if game.selected != slot {
        state.restore_slot = Some(game.selected);
        game.selected = slot;
        game.send(S::SetCarriedItem(play::SetCarriedItem {
            slot: slot as i16,
        }));
    }
}

/// Called at the head of `remote_send_state`, before the packet is built.
pub(super) fn prepare(game: &mut Game<'_>, input: u8) {
    let mut state = STATE.with(|s| std::mem::take(&mut *s.borrow_mut()));
    // A use planned last tick goes out now: that tick's movement packet is
    // already on the wire, so the server checks the click from the position
    // it reports, and this tick's movement (the landing) follows the use.
    if let Some((slot, hit)) = state.pending_use.take() {
        use_pending(game, slot, hit);
    }
    // The sender reports this tick's slot at the end of the tick, after the
    // use that switched it, so the slot goes back here.
    if let Some(slot) = state.restore_slot.take() {
        game.selected = slot;
    }
    if state.mount_cooldown > 0 {
        state.mount_cooldown -= 1;
    }
    if state.rettung_cooldown > 0 {
        state.rettung_cooldown -= 1;
    }

    let Some(config) = running_config(game) else {
        state.reset();
        STATE.with(|s| *s.borrow_mut() = state);
        return;
    };
    if state.last_mode != Some(config.mode) {
        state.reset();
        state.last_mode = Some(config.mode);
    }
    state.age = state.age.saturating_add(1);
    let safe = safe_fall_distance(game);
    let Some(player) = Snapshot::read(game) else {
        STATE.with(|s| *s.borrow_mut() = state);
        return;
    };

    let mut plan = Plan {
        active: true,
        ..Plan::default()
    };
    let mut effects = Effects::default();
    let mut action = Action::None;
    let prev_on_ground = state.last_on_ground;
    let prev_fall = state.last_fall;
    let rotation = (game.yaw, game.pitch);

    use m::NoFallMode as Mode;
    match config.mode {
        Mode::SpoofGround => {
            let distance = m::NoFallConfig::smart_distance(
                safe,
                config.spoof_ground_distance,
                config.spoof_ground_value,
            );
            if player.fall >= distance {
                plan.ground = Some(true);
                if config.spoof_ground_reset {
                    effects.reset_fall = true;
                }
            }
        }
        Mode::SpoofLanding => {
            if state.spoof_landing && player.on_ground {
                effects.sprint = Some(false);
                effects.velocity_y = Some(JUMP_VELOCITY);
                state.spoof_landing = false;
            }
            if player.on_ground && !prev_on_ground && prev_fall >= safe {
                state.spoof_landing = true;
                plan.offset = config.offset;
                plan.ground = Some(false);
                effects.reset_fall = true;
            }
        }
        Mode::NoGround => plan.ground = Some(false),
        Mode::Packet => {
            let active = if config.packet_filter == 1 {
                true
            } else {
                let distance = m::NoFallConfig::smart_distance(
                    safe,
                    config.packet_distance,
                    config.packet_value,
                );
                player.fall - player.velocity_y > distance && game.ticks > 20
            };
            if active {
                let flags = ON_GROUND | u8::from(player.collision) * HORIZONTAL_COLLISION;
                game.send(move_packet(
                    config.packet_type,
                    player.position,
                    rotation,
                    flags,
                ));
                if config.packet_filter == 0 && config.packet_reset {
                    effects.reset_fall = true;
                }
            }
        }
        Mode::PacketJump => {
            let distance = m::NoFallConfig::smart_distance(
                safe,
                config.jump_distance,
                config.jump_value,
            );
            let falling_now = player.fall > distance;
            if config.jump_timing == 1 {
                // Falling: the extra packet carries the position plus 1e-9.
                if !player.on_ground && falling_now {
                    let position = [
                        player.position[0],
                        player.position[1] + 1.0e-9,
                        player.position[2],
                    ];
                    let flags = u8::from(player.collision) * HORIZONTAL_COLLISION;
                    game.send(move_packet(config.jump_packet_type, position, rotation, flags));
                    if config.jump_reset {
                        effects.reset_fall = true;
                    }
                }
            } else if player.on_ground && state.packet_jump_falling {
                // Landing: report the position before the move, off the ground.
                let position = [
                    game.previous_position.x,
                    game.previous_position.y + 1.0e-9,
                    game.previous_position.z,
                ];
                let flags = u8::from(player.collision) * HORIZONTAL_COLLISION;
                game.send(move_packet(config.jump_packet_type, position, rotation, flags));
            }
            state.packet_jump_falling = falling_now;
        }
        Mode::Mlg => {
            if player.on_ground || player.fall <= f64::from(config.mlg_min_fall) {
                state.mlg_wait = false;
            }
            // LiquidBounce's plan is made when the predicted collision is at
            // most one movement away, and the cell is above the block the
            // fall lands on. The click is the top face of that block, cast
            // from the eyes here; it goes out on the next tick, after the
            // movement this tick sends (so the server checks reach from this
            // position) and before the movement that lands the player.
            if !state.mlg_wait
                && player.fall > f64::from(config.mlg_min_fall)
                && let Some((target, ticks)) = mlg_collision(game)
                && ticks <= 1
                && let Some(slot) = mlg_slot(game)
                && let Some(eye) = game.eye_position()
            {
                let (dx, dy, dz) = (
                    target[0] - eye[0],
                    target[1] - eye[1],
                    target[2] - eye[2],
                );
                if dx * dx + dy * dy + dz * dz <= game.reach().powi(2)
                    && mlg_fits(game, slot, target)
                {
                    let (yaw, pitch) = look_at(eye, target);
                    let landing = [
                        target[0].floor() as i32,
                        target[1] as i32 - 1,
                        target[2].floor() as i32,
                    ];
                    if let Some(hit) =
                        game.clip_ray(eye, crate::game::view_vector(yaw, pitch), game.reach())
                        && hit.pos == landing
                        && hit.face == [0, 1, 0]
                    {
                        action = Action::Use { slot, hit };
                        state.mlg_wait = true;
                    }
                }
            }
        }
        Mode::Mount => {
            if player.fall > f64::from(config.mount_min_fall)
                && player.velocity_y < 0.0
                && !player.on_ground
                && game.riding.is_none()
                && state.mount_cooldown == 0
            {
                let range =
                    f64::from(config.mount_search_range).min(interaction_range(game));
                if let Some(at) = mount_target(game, range) {
                    action = Action::Interact { at };
                    state.mount_cooldown = config.mount_retry_delay;
                }
            }
        }
        Mode::Rettungsplatform => {
            if player.fall > 2.0
                && state.rettung_cooldown == 0
                && let Some(slot) = item_slot(game, &["minecraft:blaze_rod", "minecraft:magma_cream"])
            {
                let until = (player.position[1].floor() as i32 - 90).max(MIN_Y);
                // Only when the fall ends in the void.
                if !collides_below(game, player.position, until) {
                    action = Action::Item { slot };
                    state.rettung_cooldown = 20 * 5;
                }
            }
        }
        Mode::Spartan524Flag => {
            if player.fall > 2.0 {
                if state.spartan_wait {
                    state.spartan_wait = false;
                } else {
                    game.send(status_packet(true, player.collision));
                    state.spartan_wait = true;
                }
            } else {
                state.spartan_wait = false;
            }
        }
        Mode::Vulcan277 => {
            if player.fall > 7.0 {
                plan.ground = Some(true);
                effects.reset_fall = true;
                effects.velocity_y = Some(0.0);
            }
        }
        Mode::VulcanTp288 => {
            if (2.5..=50.0).contains(&player.fall)
                && collides_below(game, player.position, config.void_level)
            {
                plan.ground = Some(true);
                effects.velocity_y = Some(-99.887575);
            }
        }
        Mode::Verus => {
            if player.fall > 3.35 {
                plan.ground = Some(true);
                effects.reset_fall = true;
                effects.velocity_y = Some(0.0);
            }
        }
        Mode::ForceJump => {
            if player.fall > f64::from(config.force_fall_distance)
                && !state.force_jump
                && collides_bottom(game, config.block_distance)
            {
                effects.velocity_y = Some(f64::from(config.jump_height));
                state.force_jump = true;
            }
            if player.on_ground {
                state.force_jump = false;
            }
        }
        Mode::Cancel => {
            let distance = m::NoFallConfig::smart_distance(
                safe,
                config.cancel_distance,
                config.cancel_value,
            );
            if player.fall >= distance {
                plan.suppress = true;
                if config.cancel_reset {
                    effects.reset_fall = true;
                }
            }
        }
        Mode::Blink => {
            if game.creative || player.may_fly || player.flying {
                state.blink_fall = false;
            } else if player.on_ground {
                state.blink_fall = false;
                state.blink_ready = true;
            } else if !state.blink_ready || player.fall > f64::from(config.blink_maximum) {
                state.blink_fall = false;
            } else if player.fall > f64::from(config.blink_trigger) {
                state.blink_fall = true;
            }
            if state.blink_fall {
                plan.ground = Some(true);
            }
        }
        Mode::HypixelPacket => {
            let below = config.over_void || collides_below(game, player.position, MIN_Y);
            if player.fall - player.velocity_y >= 3.3 && below {
                game.send(status_packet(true, player.collision));
                effects.reset_fall = true;
            }
        }
        Mode::Hypixel => {
            if player.fall >= 3.3 {
                state.hypixel_jump = true;
            }
            if state.hypixel_jump && player.on_ground {
                plan.ground = Some(false);
                if input & INPUT_JUMP == 0 {
                    effects.position_add[1] += 0.09;
                }
                state.hypixel_jump = false;
            }
        }
        Mode::BlocksMc => {
            if state.age > MIN_AGE {
                if player.velocity_y < -0.7 {
                    state.should_clip = true;
                }
                if player.on_ground && state.should_clip {
                    plan.offset[1] -= 0.1;
                }
            }
        }
        Mode::Grim2371 => {
            if player.on_ground && !prev_on_ground && prev_fall >= 2.5 {
                plan.suppress = true;
                effects.velocity_y = Some(JUMP_VELOCITY);
                game.send(status_packet(true, player.collision));
            }
        }
        Mode::AlwaysSpoof => plan.ground = Some(true),
        Mode::Damage => {
            if player.fall > 3.5 {
                plan.ground = Some(true);
            }
        }
        Mode::Vanilla => {
            if player.fall > 2.5 {
                plan.ground = Some(true);
            }
        }
        Mode::Packet1 | Mode::Packet2 => {
            // IntervalGroundSpoofNoFallMode: spoof when the fall crosses
            // another multiple of the divisor (3 or 2).
            let divisor = if config.mode == Mode::Packet1 { 3 } else { 2 };
            let count = player.fall.max(0.0) as u32 / divisor;
            if count > state.interval_count {
                state.interval_count = count;
                plan.ground = Some(true);
            }
            if player.on_ground {
                state.interval_count = 0;
            }
        }
        Mode::MotionFlag => {
            if player.fall > 3.0 {
                effects.velocity_y = Some(f64::from(config.motion_flag_speed));
            }
        }
        Mode::CubeCraft => {
            if player.fall > 2.0 {
                effects.client_ground = Some(false);
                game.send(status_packet(true, player.collision));
            }
        }
        Mode::Intave => {
            if !player.on_ground && player.fall > 3.0 {
                plan.ground = Some(true);
            }
        }
        Mode::Polar => {
            if player.fall - player.velocity_y > 2.5 {
                effects.velocity_y = Some(0.0);
                effects.reset_fall = true;
                plan.ground = Some(true);
            }
        }
        Mode::Sentinel => {
            if player.fall - player.velocity_y > 3.0 {
                effects.velocity_y = Some(0.0);
                effects.reset_fall = true;
                effects.velocity_xz = Some(0.8);
                plan.ground = Some(true);
            }
        }
        Mode::LatestVulcan => {
            if player.fall > 2.9 {
                game.send(status_packet(true, player.collision));
                effects.velocity_y = Some(-0.11);
                effects.reset_fall = true;
            }
        }
        Mode::MatrixCollide => {
            if player.fall.trunc() - player.velocity_y > 3.0 {
                effects.velocity_y = Some(0.0);
                effects.reset_fall = true;
                effects.velocity_xz = Some(0.1);
                plan.ground = Some(true);
            }
        }
        Mode::MatrixNew => {
            if !player.on_ground && player.fall > 2.69 {
                plan.ground = Some(true);
                effects.reset_fall = true;
            }
            if player.velocity_y < -0.6 && box_collides(game, player.velocity_y) {
                plan.ground = Some(true);
                effects.client_ground = Some(true);
            }
        }
        Mode::MatrixNoFall => {
            if player.fall.round() - player.velocity_y > 3.0 {
                effects.velocity_y = Some(0.0);
                effects.reset_fall = true;
                effects.velocity_xz = Some(0.1);
                // The packet says airborne; the extra status packet that
                // follows says grounded.
                plan.ground = Some(false);
                game.send(status_packet(true, player.collision));
            }
        }
        Mode::Matrix62x => {
            if state.matrix_is_fall {
                effects.velocity_xz = Some(0.0);
                if player.on_ground {
                    state.matrix_is_fall = false;
                }
            }
            if player.fall - player.velocity_y > 3.0 {
                state.matrix_is_fall = true;
                if state.matrix_fall_ticks == 0 {
                    state.matrix_last_motion = player.velocity_y;
                }
                effects.velocity_y = Some(0.0);
                effects.velocity_xz = Some(0.0);
                effects.fall_set = Some(3.2);
                if (8..=9).contains(&state.matrix_fall_ticks) {
                    state.matrix_can_spoof = true;
                }
                state.matrix_fall_ticks += 1;
            }
            if state.matrix_fall_ticks > 12 && !player.on_ground {
                effects.velocity_y = Some(state.matrix_last_motion);
                effects.fall_set = Some(0.0);
                state.matrix_fall_ticks = 0;
                state.matrix_can_spoof = false;
            }
            if state.matrix_can_spoof {
                plan.ground = Some(true);
                state.matrix_can_spoof = false;
            }
        }
        Mode::Matrix62xPacket => {
            if !player.on_ground && player.fall - player.velocity_y > 3.0 {
                for _ in 0..5 {
                    game.send(status_packet(false, player.collision));
                }
                game.send(status_packet(true, player.collision));
                game.send(status_packet(false, player.collision));
                game.send(status_packet(false, player.collision));
                effects.reset_fall = true;
            }
        }
        Mode::Matrix663 => {
            let coll_y = mlg_collision(game).map_or(0.0, |(target, _)| target[1]);
            if player.fall - player.velocity_y > 3.0
                || ((coll_y - player.position[1]).abs() < 3.0
                    && player.fall - player.velocity_y > 2.0)
            {
                effects.reset_fall = true;
                state.matrix_send = true;
                if config.matrix_safe {
                    effects.velocity_xz = Some(0.5);
                }
            }
            if state.matrix_send {
                state.matrix_send = false;
                let coll_y = mlg_collision(game).map_or(0.0, |(target, _)| target[1]);
                if (coll_y - player.position[1]).abs() > 2.0 {
                    plan.suppress = true;
                    let [x, y, z] = player.position;
                    game.send(move_packet(1, [x, y, z], rotation, ON_GROUND));
                    game.send(move_packet(1, [x, y, z], rotation, 0));
                }
            }
        }
        Mode::Aac => {
            if player.fall > 2.0 {
                game.send(status_packet(true, player.collision));
                state.aac_state = 2;
            } else if state.aac_state == 2 && player.fall < 2.0 {
                effects.velocity_y = Some(0.1);
                state.aac_state = 3;
            } else {
                match state.aac_state {
                    3 => {
                        effects.velocity_y = Some(0.1);
                        state.aac_state = 4;
                    }
                    4 => {
                        effects.velocity_y = Some(0.1);
                        state.aac_state = 5;
                    }
                    5 => {
                        effects.velocity_y = Some(0.1);
                        state.aac_state = 1;
                    }
                    _ => {}
                }
            }
        }
        Mode::Aac3311 => {
            if player.fall > 2.0 {
                effects.velocity_xz = Some(0.0);
                let [x, y, z] = player.position;
                game.send(move_packet(
                    1,
                    [x, y - 1.0e-3, z],
                    rotation,
                    u8::from(player.on_ground),
                ));
                game.send(status_packet(true, player.collision));
            }
        }
        Mode::Aac3315 => {
            if player.fall > 2.0 && !local_server(game) {
                let [x, _y, z] = player.position;
                game.send(move_packet(1, [x, f64::NAN, z], rotation, 0));
                effects.fall_set = Some(-9999.0);
            }
        }
        Mode::Aac44xFlag => {
            // The original also damps the incoming velocity packet's motionY;
            // this module has no packet-receive hook, so only the ground flag
            // is spoofed.
            if player.fall > 1.6 {
                plan.ground = Some(true);
            }
        }
        Mode::Aac504 => {
            if player.fall > 3.0 {
                state.damage_falling = true;
            }
            if state.damage_falling && player.on_ground {
                state.damage_falling = false;
                plan.ground = Some(true);
                plan.offset[1] += 1.0;
                effects.client_ground = Some(false);
                let [x, y, z] = player.position;
                let y = y + 1.0;
                game.send(move_packet(1, [x, y - 1.0784, z], rotation, 0));
                game.send(move_packet(1, [x, y - 0.5, z], rotation, ON_GROUND));
            }
        }
        Mode::Aac5014 => {
            state.aac5_check = false;
            let x = player.position[0].floor() as i32;
            let z = player.position[2].floor() as i32;
            let mut offset = 0.0;
            while player.velocity_y - 1.5 < offset {
                let y = (player.position[1] + offset).floor() as i32;
                if solid_at(game, x, y, z) {
                    state.aac5_check = true;
                    break;
                }
                offset -= 0.5;
            }
            if player.on_ground {
                effects.fall_set = Some(-2.0);
                state.aac5_check = false;
            }
            if state.aac5_timer > 0 {
                state.aac5_timer -= 1;
            }
            if state.aac5_check && player.fall > 2.5 && !player.on_ground {
                state.aac5_do_flag = true;
                state.aac5_timer = 18;
            } else if state.aac5_timer < 2 {
                state.aac5_do_flag = false;
            }
            if state.aac5_do_flag {
                let y = player.position[1] + if player.on_ground { 0.5 } else { 0.42 };
                game.send(move_packet(
                    1,
                    [player.position[0], y, player.position[2]],
                    rotation,
                    ON_GROUND,
                ));
            }
        }
        Mode::Laac => {
            if player.on_ground {
                state.laac_jumped = false;
            }
            if player.velocity_y > 0.0 {
                state.laac_jumped = true;
            }
            if !state.laac_jumped && player.on_ground && !player.climbing && !player.in_water
                && !player.in_web
            {
                effects.velocity_y = Some(-6.0);
            }
            if !state.laac_jumped
                && !player.on_ground
                && !player.climbing
                && !player.in_water
                && !player.in_web
                && player.velocity_y < 0.0
            {
                effects.velocity_xz = Some(0.0);
            }
        }
        Mode::VulcanOld => {
            if player.fall > 3.0 {
                plan.ground = Some(true);
                effects.velocity_y = Some(-0.09800000190735147);
                effects.reset_fall = true;
            }
        }
        Mode::VulcanFast288 => {
            if (2.5..50.0).contains(&player.fall)
                && collides_below(game, player.position, MIN_Y)
            {
                plan.ground = Some(true);
                effects.velocity_xz = Some(0.0);
                effects.velocity_y = Some(-99.887575);
                effects.sneaking = Some(true);
            }
        }
        Mode::PacketLegacy => {
            if player.fall > 3.1 + player.velocity_y {
                game.send(status_packet(true, player.collision));
                effects.reset_fall = true;
            }
        }
        Mode::RoundNoFall => {
            let rounded = player.position[1].round();
            if player.fall > 3.0 && (rounded - player.position[1]).abs() < 0.005 {
                effects.position_add[1] = rounded - player.position[1];
                plan.ground = Some(true);
            }
        }
        Mode::NoFallDamage => {
            let distance = mlg_collision(game)
                .map_or(f64::INFINITY, |(target, _)| player.position[1] - target[1]);
            if player.fall > 0.0 && distance > 4.0 {
                effects.velocity_y = Some(0.0);
                effects.velocity_xz = Some(0.0);
            }
        }
        Mode::WatchdogPacket => {
            let ground_below = player.on_ground
                || collides_below(
                    game,
                    player.position,
                    player.position[1].floor() as i32 - 200,
                );
            if player.fall - player.velocity_y > 3.1 && ground_below {
                plan.offset[1] += 1.0e-13;
                game.send(status_packet(true, player.collision));
                effects.reset_fall = true;
            }
        }
        Mode::WatchdogPrediction => {
            let ground_below = player.on_ground
                || collides_below(
                    game,
                    player.position,
                    player.position[1].floor() as i32 - 200,
                );
            if player.fall > 3.0
                && ground_below
                && let Some((target, _)) = mlg_collision(game)
            {
                let [x, _y, z] = player.position;
                game.send(move_packet(1, [x, target[1], z], rotation, ON_GROUND));
                effects.reset_fall = true;
            }
        }
        Mode::HypixelTimer => {
            if player.fall - player.velocity_y >= 3.3
                && collides_below(game, player.position, MIN_Y)
            {
                plan.ground = Some(true);
                effects.reset_fall = true;
            }
        }
        Mode::HypixelFlag => {
            if player.fall > 3.0 {
                state.damage_falling = true;
            }
            if state.damage_falling && player.on_ground {
                state.damage_falling = false;
                plan.suppress = true;
                let [x, y, z] = player.position;
                game.send(move_packet(1, [x, y, z], rotation, 0));
                game.send(move_packet(1, [x, y, z - 23.0], rotation, ON_GROUND));
                game.send(move_packet(1, [x, y, z], rotation, 0));
            }
        }
        Mode::Heypixel => {
            if state.heypixel_spoof {
                effects.velocity_y = Some(0.0);
                plan.ground = Some(false);
                effects.position_add[1] -= 0.098;
            } else if player.velocity_y <= 0.0 && player.fall > 3.0 {
                let x = player.position[0].floor() as i32;
                let z = player.position[2].floor() as i32;
                let y = (player.position[1] + player.velocity_y).floor() as i32;
                if solid_at(game, x, y, z) {
                    effects.reset_fall = true;
                    state.heypixel_spoof = true;
                }
            }
        }
        Mode::ChunkLoad => {
            if state.chunk_unloaded {
                effects.velocity_y = Some(0.0);
                plan.ground = Some(false);
                effects.position_add[1] -= 0.098;
            } else if player.velocity_y <= 0.0 && player.fall > 3.0 {
                let x = player.position[0].floor() as i32;
                let z = player.position[2].floor() as i32;
                let y = (player.position[1] + player.velocity_y).floor() as i32;
                if solid_at(game, x, y, z) {
                    effects.reset_fall = true;
                    state.chunk_unloaded = true;
                }
            }
        }
        Mode::Phase => {
            if player.fall > 3.0 + f64::from(config.phase_offset)
                && let Some((target, ticks)) = mlg_collision(game)
                && ticks <= 5
                && target[1] - player.velocity_y / 20.0 < player.position[1]
            {
                let [x, _y, z] = player.position;
                game.send(move_packet(1, [x, target[1], z], rotation, ON_GROUND));
            }
        }
        // OldGrimNoFall (Grim 72): START_FALL_FLYING resets the fall the
        // server tracks, without the client starting to glide. Every packet
        // reports airborne until the window ends (the server's setback, or
        // GRIM_LONG ticks), and a setback makes the next packet report the
        // ground and the one after it airborne again.
        Mode::Grim72 | Mode::Grim73 => {
            state.grim_short = state.grim_short.saturating_add(1);
            state.grim_long = state.grim_long.saturating_add(1);
            let falling = player.fall > GRIM_FALL;
            if config.mode == Mode::Grim72 {
                if falling
                    && !player.on_ground
                    && state.grim_short >= GRIM_SHORT
                    && can_start_gliding(game)
                {
                    let id = game.remote.as_ref().map_or(0, |remote| remote.entity_id);
                    for _ in 0..2 {
                        game.send(S::PlayerCommand(play::PlayerCommand {
                            id,
                            action: START_FALL_FLYING,
                            data: 0,
                        }));
                    }
                    state.grim_short = 0;
                    state.grim_sent = true;
                    state.grim_long = 0;
                }
            } else if falling && player.on_ground && state.grim_counter == 0 {
                // The landing packet gets a status-only twin beside it, and
                // the landing itself is reported airborne below.
                game.send(status_packet(true, false));
                state.grim_sent = true;
                state.grim_long = 0;
            }
            if state.grim_long >= GRIM_LONG {
                state.grim_sent = false;
                state.grim_counter = 0;
            }
            if state.grim_sent {
                plan.ground = Some(false);
            }
            if state.grim_counter == 2 {
                plan.ground = Some(false);
            }
            if state.grim_counter == 1 {
                plan.ground = Some(true);
            }
            state.grim_counter = state.grim_counter.saturating_sub(1);
        }
    }

    state.last_on_ground = plan.ground.unwrap_or(player.on_ground);
    state.last_fall = if effects.reset_fall { 0.0 } else { player.fall };

    if let Some(player) = game.player.as_mut() {
        if effects.reset_fall {
            player.fall_distance = 0.0;
        }
        if let Some(fall) = effects.fall_set {
            player.fall_distance = fall;
        }
        if let Some(velocity_y) = effects.velocity_y {
            player.velocity.y = velocity_y;
        }
        if let Some(scale) = effects.velocity_xz {
            player.velocity.x *= scale;
            player.velocity.z *= scale;
        }
        if let Some(sprint) = effects.sprint {
            player.sprinting = sprint;
        }
        if let Some(ground) = effects.client_ground {
            player.ground.on_ground = ground;
        }
        if let Some(sneaking) = effects.sneaking {
            player.local_crouching = sneaking;
        }
        player.position.x += effects.position_add[0];
        player.position.y += effects.position_add[1];
        player.position.z += effects.position_add[2];
    }

    match action {
        Action::None => {}
        Action::Use { slot, hit } => {
            select(game, &mut state, slot);
            state.pending_use = Some((slot, hit));
        }
        Action::Item { slot } => {
            select(game, &mut state, slot);
            game.remote_use_item(Hand::Main);
        }
        Action::Interact { at } => {
            if let Some(eye) = game.eye_position() {
                let (yaw, pitch) = look_at(eye, at);
                let (old_yaw, old_pitch) = (game.yaw, game.pitch);
                game.yaw = yaw;
                game.pitch = pitch;
                game.remote_use_entity(Hand::Main);
                game.yaw = old_yaw;
                game.pitch = old_pitch;
            }
        }
    }

    state.plan = plan;
    STATE.with(|s| *s.borrow_mut() = state);
}

/// Sends MLG's planned click. The item was selected last tick and is still in
/// hand. The click is forced to the hit the plan computed, so it names the
/// block the plan aimed at rather than whatever the crosshair, from the
/// player's lower position, would pick now.
fn use_pending(game: &mut Game<'_>, slot: usize, hit: Hit) {
    if game.selected != slot {
        return;
    }
    let previous = game.forced_hit.replace(hit);
    game.remote_use_block(Hand::Main);
    game.forced_hit = previous;
}

/// Called with the packet `PositionSender::update` produced (if any).
pub(super) fn spoof(packet: Option<S>) -> Option<S> {
    let plan = STATE.with(|s| s.borrow().plan);
    if !plan.active {
        return packet;
    }
    if plan.suppress {
        return None;
    }
    Some(rewrite(packet?, plan.ground, plan.offset))
}

/// Called at the head of `remote_teleport`, before the teleport is applied.
pub(super) fn teleport(
    game: &mut Game<'_>,
    packet: &play::PlayerPosition,
) -> ControlFlow<horizon_client::Result<()>> {
    let Some(config) = running_config(game) else {
        return ControlFlow::Continue(());
    };
    match config.mode {
        m::NoFallMode::Cancel => {
            // In single player the integrated server applies the teleport and
            // refuses a second position packet for the same tick, so the
            // answer is only for real servers.
            let local = local_server(game);
            if local {
                return ControlFlow::Continue(());
            }
            use horizon_protocol::packets::play::relative as rel;
            let current = game.player.as_ref().map_or([0.0; 3], |player| {
                [player.position.x, player.position.y, player.position.z]
            });
            let pick = |bit: i32, base: f64, value: f64| {
                if packet.relatives & bit != 0 {
                    base + value
                } else {
                    value
                }
            };
            let x = pick(rel::X, current[0], packet.position[0]);
            let y = pick(rel::Y, current[1], packet.position[1]);
            let z = pick(rel::Z, current[2], packet.position[2]);
            let yaw = if packet.relatives & rel::Y_ROT != 0 {
                game.yaw + packet.yaw
            } else {
                packet.yaw
            };
            let pitch = (if packet.relatives & rel::X_ROT != 0 {
                game.pitch + packet.pitch
            } else {
                packet.pitch
            })
            .clamp(-90.0, 90.0);
            let collision = game
                .player
                .as_ref()
                .is_some_and(|player| player.collision.horizontal);
            game.send(S::MovePlayerPosRot(play::MovePlayerPosRot {
                x,
                y,
                z,
                yaw,
                pitch,
                flags: ON_GROUND | u8::from(collision) * HORIZONTAL_COLLISION,
            }));
            if config.cancel_setback {
                ControlFlow::Break(Ok(()))
            } else {
                ControlFlow::Continue(())
            }
        }
        m::NoFallMode::BlocksMc => {
            STATE.with(|s| s.borrow_mut().should_clip = false);
            ControlFlow::Continue(())
        }
        m::NoFallMode::Heypixel => {
            STATE.with(|s| s.borrow_mut().heypixel_spoof = false);
            ControlFlow::Continue(())
        }
        m::NoFallMode::ChunkLoad => {
            STATE.with(|s| s.borrow_mut().chunk_unloaded = false);
            ControlFlow::Continue(())
        }
        // OldGrim: the server's answer to the spoofed packets is a setback;
        // the next packets report airborne and then the ground, which resets
        // the fall the server tracks.
        m::NoFallMode::Grim72 | m::NoFallMode::Grim73 => {
            STATE.with(|s| {
                let mut state = s.borrow_mut();
                if state.grim_sent {
                    state.grim_counter = 2;
                    state.grim_sent = false;
                }
            });
            ControlFlow::Continue(())
        }
        _ => ControlFlow::Continue(()),
    }
}

/// The movement packet of the configured type, `FULL` being the rotation too.
fn move_packet(kind: u8, position: [f64; 3], rotation: (f32, f32), flags: u8) -> S {
    let [x, y, z] = position;
    if kind == 0 {
        S::MovePlayerPosRot(play::MovePlayerPosRot {
            x,
            y,
            z,
            yaw: rotation.0,
            pitch: rotation.1,
            flags,
        })
    } else {
        S::MovePlayerPos(play::MovePlayerPos { x, y, z, flags })
    }
}

/// The status-only packet a mode sends beside the movement packet.
fn status_packet(ground: bool, collision: bool) -> S {
    S::MovePlayerStatus(play::MovePlayerStatus {
        flags: u8::from(ground) | u8::from(collision) * HORIZONTAL_COLLISION,
    })
}

/// Rewrites the packet the sender produced for this tick.
fn rewrite(packet: S, ground: Option<bool>, offset: [f64; 3]) -> S {
    let flags = |flags: u8| match ground {
        Some(ground) => (flags & !ON_GROUND) | u8::from(ground),
        None => flags,
    };
    let moved = offset != [0.0; 3];
    match packet {
        S::MovePlayerPosRot(mut packet) => {
            if moved {
                packet.x += offset[0];
                packet.y += offset[1];
                packet.z += offset[2];
            }
            packet.flags = flags(packet.flags);
            S::MovePlayerPosRot(packet)
        }
        S::MovePlayerPos(mut packet) => {
            if moved {
                packet.x += offset[0];
                packet.y += offset[1];
                packet.z += offset[2];
            }
            packet.flags = flags(packet.flags);
            S::MovePlayerPos(packet)
        }
        S::MovePlayerRot(mut packet) => {
            packet.flags = flags(packet.flags);
            S::MovePlayerRot(packet)
        }
        S::MovePlayerStatus(mut packet) => {
            packet.flags = flags(packet.flags);
            S::MovePlayerStatus(packet)
        }
        other => other,
    }
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
        // A state-dependent shape (scaffolding, a liquid): liquids do not
        // collide, everything else does.
        Ok(None) => facts.get(state).fluid_level == 0,
        Err(_) => facts.get(state).full_collision,
    }
}

/// Whether a solid block sits below `feet` down to `until` (inclusive).
fn collides_below(game: &Game<'_>, feet: [f64; 3], until: i32) -> bool {
    let x = feet[0].floor() as i32;
    let z = feet[2].floor() as i32;
    let mut y = feet[1].floor() as i32 - 1;
    while y >= until {
        if solid_at(game, x, y, z) {
            return true;
        }
        y -= 1;
    }
    false
}

/// Whether the block at `x, y, z` is one of `names`.
fn block_named(game: &Game<'_>, x: i32, y: i32, z: i32, names: &[&str]) -> bool {
    let state = game.world.block(x, y, z);
    let facts = game.res.facts();
    names.iter().any(|name| &*facts.get(state).name == *name)
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

/// Whether a solid block intersects the player's box moved down by `distance`.
fn collides_bottom(game: &Game<'_>, distance: f32) -> bool {
    box_collides(game, -f64::from(distance))
}

/// MLG: the first solid block the fall lands on, and its top centre, with the
/// number of movement ticks until the feet reach it (0: the next movement
/// lands).
fn mlg_collision(game: &Game<'_>) -> Option<([f64; 3], u32)> {
    let player = game.player.as_ref()?;
    let x = player.position.x.floor() as i32;
    let z = player.position.z.floor() as i32;
    let mut block_y = player.position.y.floor() as i32;
    loop {
        if block_y < MIN_Y {
            return None;
        }
        if solid_at(game, x, block_y, z) {
            break;
        }
        block_y -= 1;
    }
    let top = f64::from(block_y) + 1.0;
    // Vanilla order: the movement uses this velocity, then gravity and drag
    // prepare the next tick's (`LivingEntity.travel`).
    let (mut y, mut velocity) = (player.position.y, player.velocity.y);
    for tick in 0..20 {
        y += velocity;
        if y <= top {
            return Some((
                [
                    f64::from(x) + 0.5,
                    top,
                    f64::from(z) + 0.5,
                ],
                tick,
            ));
        }
        velocity = (velocity - 0.08) * 0.98;
        if velocity < -MAX_FALL_SPEED {
            velocity = -MAX_FALL_SPEED;
        }
    }
    None
}

/// MLG: whether the placement clears the player's own box. Water, ladders
/// and vines have no cube collision, so they can go into the cell the player
/// is falling through; a full block fills the cell above the landing block,
/// so the feet (the bottom of the 1.8-tall box) must already be above it.
fn mlg_fits(game: &Game<'_>, slot: usize, target: [f64; 3]) -> bool {
    let Some(player) = game.player.as_ref() else {
        return false;
    };
    let item = game
        .inventory
        .get(slot)
        .and_then(|stack| stack.as_ref())
        .map(|stack| &*stack.item);
    let through = matches!(
        item,
        Some("minecraft:water_bucket" | "minecraft:ladder" | "minecraft:twisting_vines")
    );
    through || player.position.y - target[1] >= 1.0
}

/// MLG: the first hotbar slot holding a block that can save the fall.
fn mlg_slot(game: &Game<'_>) -> Option<usize> {
    const ITEMS: [&str; 8] = [
        "minecraft:scaffolding",
        "minecraft:cobweb",
        "minecraft:powder_snow_bucket",
        "minecraft:hay_block",
        "minecraft:slime_block",
        "minecraft:honey_block",
        "minecraft:ladder",
        "minecraft:twisting_vines",
    ];
    let water = !matches!(game.dim, crate::game::Dim::Nether);
    item_slot(game, &ITEMS)
        .or_else(|| water.then(|| item_slot(game, &["minecraft:water_bucket"])).flatten())
}

fn item_slot(game: &Game<'_>, items: &[&str]) -> Option<usize> {
    (0..9).find(|&slot| {
        game.inventory
            .get(slot)
            .and_then(|stack| stack.as_ref())
            .is_some_and(|stack| items.iter().any(|item| &*stack.item == *item))
    })
}

/// OldGrim72: the server only starts a glide with an elytra worn, and the
/// client must not already be gliding.
fn can_start_gliding(game: &Game<'_>) -> bool {
    let chest = game
        .armor[1]
        .as_ref()
        .is_some_and(|stack| &*stack.item == "minecraft:elytra");
    let gliding = game.player.as_ref().is_some_and(|player| player.fall_flying);
    chest && !gliding
}

/// Mount: the nearest rideable mob within `range` of the eyes.
fn mount_target(game: &Game<'_>, range: f64) -> Option<[f64; 3]> {
    let eye = game.eye_position()?;
    let limit = range * range;
    let mut best: Option<([f64; 3], f64)> = None;
    for mob in &game.mobs {
        if mob.removed || !mob.alive() || !rideable(mob.species()) {
            continue;
        }
        let center = mob.center();
        let (dx, dy, dz) = (center[0] - eye[0], center[1] - eye[1], center[2] - eye[2]);
        let distance = dx * dx + dy * dy + dz * dz;
        if distance <= limit && best.is_none_or(|(_, nearest)| distance < nearest) {
            best = Some((center, distance));
        }
    }
    best.map(|(center, _)| center)
}

fn rideable(species: Species) -> bool {
    matches!(
        species,
        Species::Horse
            | Species::Donkey
            | Species::Mule
            | Species::SkeletonHorse
            | Species::ZombieHorse
            | Species::Pig
            | Species::Strider
            | Species::Camel
            | Species::CamelHusk
            | Species::HappyGhast
    )
}

/// The yaw and pitch that look from `eye` at `target`, in vanilla's degrees.
fn look_at(eye: [f64; 3], target: [f64; 3]) -> (f32, f32) {
    let (dx, dy, dz) = (
        target[0] - eye[0],
        target[1] - eye[1],
        target[2] - eye[2],
    );
    let horizontal = (dx * dx + dz * dz).sqrt();
    let yaw = (-dx).atan2(dz).to_degrees() as f32;
    let pitch = (-dy).atan2(horizontal).to_degrees() as f32;
    (yaw, pitch)
}
