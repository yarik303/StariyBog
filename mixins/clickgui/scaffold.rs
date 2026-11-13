// Scaffold: a port of the placement engines from Flux's Scaffold submodules
// (Normal, Legit, New, AAC, Hypixel), Alan's Scaffold, Augustus's ScaffoldWalk,
// Novoline's Scaffold, Sigma's Scaffold, Tenacity's Scaffold, Yuri's Scaffold,
// and LiquidBounce's Expand, GodBridge and Breezily techniques.
//
// Aoba's scaffold (Flux Normal with radius/jitter settings) and LiquidBounce's
// plain Normal technique were the same placement as Normal, so they are not
// separate modes. The sprint and tower folders are compiled into their
// settings after dropping the duplicates (BypassSprint behaved like
// DisabledSprint; NCP/Matrix/Verus/Timer towers were Vanilla with clock tweaks
// Horizon does not expose; AirJump was AutoJump; the Kotlin towers were
// Watchdog).
//
// The tick runs at the head of `remote_send_state`, after physics: it picks
// the block under the next step, aims at a face of a solid neighbour, swaps
// the held block in, sends the use and puts the camera rotation back. The
// rotation packet is pushed through the position sender, so the server sees
// the placement look and then the player's own look, and the tick's movement
// packet still carries the restored rotation.
use crate::game::clickgui as m;
use crate::game::clickgui::{
    ScaffoldConfig, ScaffoldDownwards as Downwards, ScaffoldMode as Mode, ScaffoldSprint as Sprint,
    ScaffoldTower as Tower,
};
use crate::game::{Game, Screen};
use horizon_client::first_person::Hand;
use horizon_client::items::ItemStack;
use horizon_protocol::packets::play::{self, Serverbound as S};
use std::cell::{Cell, RefCell};
use winit::keyboard::KeyCode;

/// The reach the block pick uses for a survival player.
const REACH: f64 = 4.5;

/// A `remote_send_state` tick's transient state.
#[derive(Default)]
struct State {
    last_place_ms: i64,
    /// The floor of the last grounded position, for Keep Y.
    floor_y: Option<i32>,
    /// Ticks the sneak key has been held, for Verus downwards.
    sneak_ticks: i32,
    /// Whether this module inserted ShiftLeft into the game's key set.
    sneaked: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    /// Eagle's sneak countdown, in ticks. Its own cell, because the sneak is
    /// set from the movement-input hook and not from the placement tick.
    static EAGLE: Cell<i32> = const { Cell::new(0) };
}

fn take_state() -> State {
    STATE.with(|state| std::mem::take(&mut *state.borrow_mut()))
}

fn put_state(state: State) {
    STATE.with(|slot| *slot.borrow_mut() = state);
}

/// The block a mode places against: the air position, its solid neighbour and
/// the neighbour face's outward normal.
struct Target {
    neighbour: [i32; 3],
    normal: [f64; 3],
}

/// The behaviour that differs between the original clients' modes.
struct Traits {
    /// How far ahead of the player the mode predicts the next block, along
    /// the movement input.
    expand: f64,
    /// A fixed pitch for modes that stare down instead of at the face.
    pitch: Option<f32>,
    /// Aim at the face point closest to the eyes (Augustus/Sigma's nearest
    /// rotation) instead of its centre.
    nearest: bool,
    /// Whether the mode swaps the held slot (Legit and AAC leave it alone).
    can_switch: bool,
    /// The implementation's own placement delay floor, in milliseconds.
    min_delay: i32,
    /// Whether the mode keeps the last grounded block height while airborne.
    keep_y: bool,
}

fn traits(mode: Mode) -> Traits {
    match mode {
        Mode::Normal => Traits {
            expand: 0.4,
            pitch: None,
            nearest: false,
            can_switch: true,
            min_delay: 0,
            keep_y: false,
        },
        Mode::Legit => Traits {
            expand: 0.0,
            pitch: Some(90.0),
            nearest: false,
            can_switch: false,
            min_delay: 300,
            keep_y: false,
        },
        Mode::New => Traits {
            expand: 0.4,
            pitch: None,
            nearest: false,
            can_switch: true,
            min_delay: 300,
            keep_y: false,
        },
        Mode::Aac => Traits {
            expand: 0.0,
            pitch: Some(82.0),
            nearest: false,
            can_switch: false,
            min_delay: 0,
            keep_y: false,
        },
        Mode::Hypixel => Traits {
            expand: 1.0,
            pitch: None,
            nearest: false,
            can_switch: true,
            min_delay: 0,
            keep_y: true,
        },
        Mode::Alan => Traits {
            expand: 0.4,
            pitch: None,
            nearest: false,
            can_switch: true,
            min_delay: 80,
            keep_y: false,
        },
        Mode::NewScaffold => Traits {
            expand: 0.4,
            pitch: None,
            nearest: true,
            can_switch: true,
            min_delay: 0,
            keep_y: false,
        },
        Mode::Novoline => Traits {
            expand: 0.45,
            pitch: None,
            nearest: false,
            can_switch: true,
            min_delay: 250,
            keep_y: true,
        },
        Mode::Sigma => Traits {
            expand: 0.4,
            pitch: None,
            nearest: true,
            can_switch: true,
            min_delay: 0,
            keep_y: true,
        },
        Mode::Tenacity => Traits {
            expand: 0.4,
            pitch: None,
            nearest: false,
            can_switch: true,
            min_delay: 0,
            keep_y: true,
        },
        Mode::Yuri => Traits {
            expand: 0.4,
            pitch: None,
            nearest: false,
            can_switch: true,
            min_delay: 0,
            keep_y: false,
        },
        Mode::Expand => Traits {
            expand: 1.0,
            pitch: None,
            nearest: false,
            can_switch: true,
            min_delay: 0,
            keep_y: false,
        },
        Mode::GodBridge => Traits {
            expand: 0.0,
            pitch: Some(80.0),
            nearest: false,
            can_switch: true,
            min_delay: 0,
            keep_y: false,
        },
        Mode::Breezily => Traits {
            expand: 0.0,
            pitch: None,
            nearest: false,
            can_switch: true,
            min_delay: 0,
            keep_y: false,
        },
        Mode::Off => Traits {
            expand: 0.0,
            pitch: None,
            nearest: false,
            can_switch: true,
            min_delay: 0,
            keep_y: false,
        },
        Mode::Eagle => Traits {
            // Never reached: `tick` returns before a target is picked.
            expand: 0.0,
            pitch: None,
            nearest: false,
            can_switch: false,
            min_delay: 0,
            keep_y: false,
        },
    }
}

/// The module's conditions: enabled, playing, in the world with no screen up.
fn running_config(game: &Game<'_>) -> Option<ScaffoldConfig> {
    if !game.native_window() || !game.native_playing() || !game.native_on(m::ID) {
        return None;
    }
    if game.spectator() || game.screen != Screen::Playing {
        return None;
    }
    game.player.as_ref()?;
    let config = m::scaffold()?;
    (config.mode != Mode::Off).then_some(config)
}

pub(super) fn tick(game: &mut Game<'_>) {
    let mut state = take_state();
    let Some(config) = running_config(game) else {
        release_keys(game, &mut state);
        put_state(state);
        return;
    };

    // Eagle places nothing: it holds sneak through the movement input, so the
    // placement tick only has to make sure no key was left behind.
    if config.mode == Mode::Eagle {
        release_keys(game, &mut state);
        put_state(state);
        return;
    }

    if let Some(player) = game.player.as_ref()
        && player.ground.on_ground
    {
        state.floor_y = Some(player.position.y.floor() as i32);
    }

    hold_keys(game, &mut state, &config);
    count_sneak(game, &mut state, &config);
    apply_sprint(game, &config);
    auto_jump(game, &config);

    if let Some(target) = find_target(game, &config, &state) {
        try_place(game, &config, &mut state, &target);
    }

    put_state(state);
}

/// Sneak while the module asks for it, and Breezily's alternate-tick sneak.
fn hold_keys(game: &mut Game<'_>, state: &mut State, config: &ScaffoldConfig) {
    let physical = game.physical_keys.contains(&KeyCode::ShiftLeft);
    let breezily = config.mode == Mode::Breezily && game.ticks % 2 == 0;
    if config.sneak || breezily {
        game.keys.insert(KeyCode::ShiftLeft);
        if !physical {
            state.sneaked = true;
        }
    } else if state.sneaked && !physical {
        game.keys.remove(&KeyCode::ShiftLeft);
        state.sneaked = false;
    }
}

/// Drops the sneak this module added when it lets go or turns off.
fn release_keys(game: &mut Game<'_>, state: &mut State) {
    if state.sneaked {
        if !game.physical_keys.contains(&KeyCode::ShiftLeft) {
            game.keys.remove(&KeyCode::ShiftLeft);
        }
        state.sneaked = false;
    }
}

/// `native_movement_input`: Alan's LegitScaffold, which the original calls
/// "Eagle". It holds sneak at the edge so the player stops instead of walking
/// off, and places nothing at all.
///
/// The reference sets `MoveInputEvent`'s sneak from its own `sneakTicks`
/// countdown, refreshed every tick it sees air below the feet. This runs in the
/// same place — after the input is read and before it is applied — so the sneak
/// counts for this tick, and the server's own `ServerboundPlayerInputPacket`
/// carries it too. It deliberately does not insert the key into `game.keys`:
/// that set is consumed by the next tick's physics, which is a tick too late
/// for an edge.
///
/// The reference reads the block straight down, which is solid while the
/// player still stands on the block they are about to leave. This also reads
/// the block under the next tick's step — that is the edge itself, and it is
/// what makes the mode useful rather than a report that you are already
/// falling.
pub(crate) fn movement_input(game: &Game<'_>, sneak: &mut bool) {
    let Some(config) = running_config(game) else {
        EAGLE.with(|ticks| ticks.set(0));
        return;
    };
    if config.mode != Mode::Eagle {
        EAGLE.with(|ticks| ticks.set(0));
        return;
    }
    if over_air(game) {
        EAGLE.with(|ticks| ticks.set(config.eagle_length));
    }
    let owed = EAGLE.with(Cell::get);
    if owed <= 0 || !eagle_gates(game, &config) {
        EAGLE.with(|ticks| ticks.set(0));
        return;
    }
    EAGLE.with(|ticks| ticks.set(owed - 1));
    *sneak = true;
}

/// Eagle's gates: the conditions the reference checks before it will sneak.
fn eagle_gates(game: &Game<'_>, config: &ScaffoldConfig) -> bool {
    let Some(player) = game.player.as_ref() else {
        return false;
    };
    if config.eagle_ground && !player.ground.on_ground {
        return false;
    }
    // The reference calls this "backwards only": it checks that the forward
    // key is up, not that the player is moving the other way.
    if config.eagle_backwards && game.physical_keys.contains(&KeyCode::KeyW) {
        return false;
    }
    if config.eagle_blocks && !game.held().is_some_and(|stack| is_scaffold(game, stack)) {
        return false;
    }
    true
}

/// Whether there is nothing to stand on under the feet, or under the step the
/// player's own velocity is about to take.
fn over_air(game: &Game<'_>) -> bool {
    let Some(player) = game.player.as_ref() else {
        return false;
    };
    let air = |x: i32, y: i32, z: i32| game.res.facts().is_air(game.world.block(x, y, z));
    edge(
        [player.position.x, player.position.y, player.position.z],
        [player.velocity.x, player.velocity.z],
        &air,
    )
}

/// The edge test itself, split out so the geometry can be pinned by a test:
/// the block under the feet, or the one under the next tick's step.
fn edge(at: [f64; 3], vel: [f64; 2], air: &impl Fn(i32, i32, i32) -> bool) -> bool {
    let below = (at[1] - 1.0).floor() as i32;
    let cell = |x: f64, z: f64| air(x.floor() as i32, below, z.floor() as i32);
    cell(at[0], at[2]) || cell(at[0] + vel[0], at[2] + vel[1])
}

/// Verus downwards counts sneak ticks and only drops the target once the
/// player is falling and standing on a whole block.
fn count_sneak(game: &Game<'_>, state: &mut State, config: &ScaffoldConfig) {
    let sneaking = game.physical_keys.contains(&KeyCode::ShiftLeft);
    if sneaking && config.downwards != Downwards::Off {
        state.sneak_ticks = state.sneak_ticks.saturating_add(1);
    } else {
        state.sneak_ticks = 0;
    }
}

fn downwards_active(game: &Game<'_>, state: &State, config: &ScaffoldConfig) -> bool {
    if !game.physical_keys.contains(&KeyCode::ShiftLeft) {
        return false;
    }
    match config.downwards {
        Downwards::Off => false,
        Downwards::Normal => true,
        Downwards::Verus => {
            let Some(player) = game.player.as_ref() else {
                return false;
            };
            player.velocity.y <= 0.0
                && player.position.y.fract() == 0.0
                && state.sneak_ticks >= 15
        }
    }
}

fn apply_sprint(game: &mut Game<'_>, config: &ScaffoldConfig) {
    let Some(player) = game.player.as_mut() else {
        return;
    };
    match config.sprint {
        Sprint::Off => {}
        Sprint::Disabled => player.sprinting = false,
        Sprint::Legit => {
            if player.ground.on_ground {
                player.sprinting = false;
            }
        }
        Sprint::Matrix => {
            if player.sprinting {
                player.velocity.x *= 0.95;
                player.velocity.z *= 0.95;
            }
        }
        Sprint::Verus => {
            player.sprinting = false;
            if player.ground.on_ground {
                player.velocity.y = 0.0;
            }
        }
        Sprint::Vulcan => {
            player.sprinting = false;
            if player.ground.on_ground {
                player.velocity.y = 0.012_500_047_683_714;
            }
        }
        Sprint::Watchdog => player.sprinting = true,
    }
}

/// Jumps when the next step's block is missing, like the originals' edge
/// detectors.
fn auto_jump(game: &mut Game<'_>, config: &ScaffoldConfig) {
    if !config.auto_jump {
        return;
    }
    let (dx, dz) = input_dir(game);
    if dx == 0.0 && dz == 0.0 {
        return;
    }
    let Some(player) = game.player.as_ref() else {
        return;
    };
    if !player.ground.on_ground {
        return;
    }
    let x = (player.position.x + dx * 0.5).floor() as i32;
    let z = (player.position.z + dz * 0.5).floor() as i32;
    let y = player.position.y.floor() as i32 - 1;
    if is_open(game, x, y, z) {
        if let Some(player) = game.player.as_mut() {
            player.velocity.y = 0.42;
            player.ground.on_ground = false;
            player.fall_distance = 0.0;
        }
    }
}

/// The direction of the movement input, unit length, from the bound keys and
/// the camera yaw (momentum when the player has no input).
fn input_dir(game: &Game<'_>) -> (f64, f64) {
    let keys = &game.keys;
    let forward =
        i32::from(keys.contains(&KeyCode::KeyW)) - i32::from(keys.contains(&KeyCode::KeyS));
    let strafe = i32::from(keys.contains(&KeyCode::KeyA)) - i32::from(keys.contains(&KeyCode::KeyD));
    if forward == 0 && strafe == 0 {
        if let Some(player) = game.player.as_ref() {
            let (x, z) = (player.velocity.x, player.velocity.z);
            let length = x.hypot(z);
            if length > 0.05 {
                return (x / length, z / length);
            }
        }
        return (0.0, 0.0);
    }
    let yaw = f64::from(game.yaw).to_radians();
    let (sin, cos) = yaw.sin_cos();
    let (forward, strafe) = (f64::from(forward), f64::from(strafe));
    let x = forward * -sin + strafe * cos;
    let z = forward * cos + strafe * sin;
    let length = x.hypot(z).max(1.0e-6);
    (x / length, z / length)
}

fn is_open(game: &Game<'_>, x: i32, y: i32, z: i32) -> bool {
    let state = game.world.block(x, y, z);
    let facts = game.res.facts();
    facts.is_air(state) || facts.get(state).replaceable
}

fn is_solid(game: &Game<'_>, x: i32, y: i32, z: i32) -> bool {
    let state = game.world.block(x, y, z);
    let facts = game.res.facts();
    !facts.is_air(state) && !facts.get(state).replaceable
}

/// The candidate positions and the face normals pointing from the neighbour
/// toward the target, in the Java clients' scan order.
const NEIGHBOURS: [([i32; 3], [f64; 3]); 6] = [
    ([0, -1, 0], [0.0, 1.0, 0.0]),
    ([-1, 0, 0], [1.0, 0.0, 0.0]),
    ([1, 0, 0], [-1.0, 0.0, 0.0]),
    ([0, 0, -1], [0.0, 0.0, 1.0]),
    ([0, 0, 1], [0.0, 0.0, -1.0]),
    ([0, 1, 0], [0.0, -1.0, 0.0]),
];

fn find_target(game: &Game<'_>, config: &ScaffoldConfig, state: &State) -> Option<Target> {
    let player = game.player.as_ref()?;
    let traits = traits(config.mode);
    let floor = player.position.y.floor() as i32;
    let base = if (config.keep_y || traits.keep_y) && !player.ground.on_ground {
        state.floor_y.unwrap_or(floor)
    } else {
        floor
    } - 1;
    let drop = if downwards_active(game, state, config) { -1 } else { 0 };
    let expand = if config.mode == Mode::Expand {
        f64::from(config.extend)
    } else {
        traits.expand
    };
    let (dx, dz) = input_dir(game);

    if expand > 0.0 && (dx != 0.0 || dz != 0.0) {
        let x = (player.position.x + dx * expand).floor() as i32;
        let z = (player.position.z + dz * expand).floor() as i32;
        let y = base + drop;
        if is_open(game, x, y, z)
            && let Some(target) = target_at(game, [x, y, z])
        {
            return Some(target);
        }
    }

    let x = player.position.x.floor() as i32;
    let z = player.position.z.floor() as i32;
    let y = base + drop;
    if is_open(game, x, y, z) {
        target_at(game, [x, y, z])
    } else {
        None
    }
}

fn target_at(game: &Game<'_>, position: [i32; 3]) -> Option<Target> {
    for (offset, normal) in NEIGHBOURS {
        let neighbour = [
            position[0] + offset[0],
            position[1] + offset[1],
            position[2] + offset[2],
        ];
        if !is_solid(game, neighbour[0], neighbour[1], neighbour[2]) {
            continue;
        }
        if is_open(game, neighbour[0], neighbour[1], neighbour[2]) {
            continue;
        }
        return Some(Target { neighbour, normal });
    }
    None
}

fn try_place(
    game: &mut Game<'_>,
    config: &ScaffoldConfig,
    state: &mut State,
    target: &Target,
) {
    let traits = traits(config.mode);
    let now = game.debug_millis();
    let delay = i64::from(config.place_delay.max(traits.min_delay));
    if now - state.last_place_ms < delay {
        return;
    }
    let Some(slot) = block_slot(game) else {
        return;
    };
    let held_block = game.held().is_some_and(|stack| is_scaffold(game, stack));
    if !traits.can_switch && !held_block {
        return;
    }
    let Some((yaw, pitch)) = aim(game, target, &traits) else {
        return;
    };

    let old_rotation = (game.yaw, game.pitch);
    if config.rotations {
        game.yaw = yaw;
        game.pitch = pitch;
        sync_sender(game, (yaw, pitch));
    }

    let old_slot = if traits.can_switch && game.selected != slot {
        let old = game.selected;
        set_slot(game, slot);
        (config.silent).then_some(old)
    } else {
        None
    };

    state.last_place_ms = now;
    // The use path always swings the arm locally; the Swing setting decides
    // whether that sticks.
    let swing_before = (game.swing, game.swings_started);
    game.remote_use_block(Hand::Main);
    if !config.swing {
        game.swing = swing_before.0;
        game.swings_started = swing_before.1;
    }

    if let Some(old) = old_slot {
        set_slot(game, old);
    }
    if config.rotations {
        game.yaw = old_rotation.0;
        game.pitch = old_rotation.1;
        sync_sender(game, old_rotation);
    }

    tower(game, config);
}

/// The first hotbar slot holding a block, preferring the biggest stack.
fn block_slot(game: &Game<'_>) -> Option<usize> {
    let mut best: Option<(usize, u32)> = None;
    for slot in 0..9 {
        let Some(stack) = game.inventory.get(slot).and_then(Option::as_ref) else {
            continue;
        };
        if !is_scaffold(game, stack) {
            continue;
        }
        if best.is_none_or(|(_, count)| stack.count > count) {
            best = Some((slot, stack.count));
        }
    }
    best.map(|(slot, _)| slot)
}

fn is_scaffold(game: &Game<'_>, stack: &ItemStack) -> bool {
    if stack.count == 0 {
        return false;
    }
    let Some(info) = game.items().get(&stack.item) else {
        return false;
    };
    let Some(state) = info.block else {
        return false;
    };
    !blacklisted(&game.res.facts().get(state).name)
}

/// The blocks the originals refuse to place: containers and interactables
/// that open a screen, and shapes that do not give a usable face.
fn blacklisted(name: &str) -> bool {
    const BAD: &[&str] = &[
        "minecraft:air",
        "minecraft:cave_air",
        "minecraft:void_air",
        "minecraft:chest",
        "minecraft:trapped_chest",
        "minecraft:ender_chest",
        "minecraft:crafting_table",
        "minecraft:furnace",
        "minecraft:anvil",
        "minecraft:enchanting_table",
        "minecraft:sand",
        "minecraft:gravel",
        "minecraft:soul_sand",
        "minecraft:water",
        "minecraft:lava",
        "minecraft:tnt",
        "minecraft:ice",
        "minecraft:packed_ice",
        "minecraft:blue_ice",
        "minecraft:snow",
        "minecraft:snow_layer",
        "minecraft:torch",
        "minecraft:redstone_torch",
        "minecraft:ladder",
        "minecraft:vine",
        "minecraft:slime_block",
        "minecraft:honey_block",
        "minecraft:cobweb",
        "minecraft:daylight_detector",
        "minecraft:noteblock",
        "minecraft:jukebox",
        "minecraft:dispenser",
        "minecraft:dropper",
        "minecraft:hopper",
        "minecraft:piston",
        "minecraft:sticky_piston",
        "minecraft:observer",
        "minecraft:lever",
        "minecraft:rail",
        "minecraft:spawner",
    ];
    BAD.contains(&name)
        || name.ends_with("_pane")
        || name.ends_with("_fence")
        || name.ends_with("_fence_gate")
        || name.ends_with("_door")
        || name.ends_with("_trapdoor")
        || name.ends_with("_button")
        || name.ends_with("_wall")
        || name.ends_with("_banner")
        || name.ends_with("_sign")
        || name.ends_with("_sapling")
        || name.ends_with("_flower")
        || name.ends_with("_carpet")
        || name.ends_with("_head")
        || name.ends_with("_skull")
        || name.contains("pressure_plate")
}

/// Yaw and pitch from the eyes to the neighbour's face.
fn aim(game: &Game<'_>, target: &Target, traits: &Traits) -> Option<(f32, f32)> {
    let eye = game.eye_position()?;
    let neighbour = target.neighbour;
    let normal = target.normal;
    let mut hit = [
        f64::from(neighbour[0]) + 0.5 + normal[0] * 0.5,
        f64::from(neighbour[1]) + 0.5 + normal[1] * 0.5,
        f64::from(neighbour[2]) + 0.5 + normal[2] * 0.5,
    ];
    if traits.nearest {
        for axis in 0..3 {
            if normal[axis] == 0.0 {
                hit[axis] = eye[axis].clamp(
                    f64::from(neighbour[axis]),
                    f64::from(neighbour[axis]) + 1.0,
                );
            }
        }
    }
    let delta = [
        hit[0] - eye[0],
        hit[1] - eye[1],
        hit[2] - eye[2],
    ];
    if delta[0].hypot(delta[1]).hypot(delta[2]) > REACH + 0.5 {
        return None;
    }
    let yaw = (-delta[0]).atan2(delta[2]).to_degrees() as f32;
    let mut pitch = (-delta[1]).atan2(delta[0].hypot(delta[2])).to_degrees() as f32;
    if let Some(forced) = traits.pitch
        && normal[1] > 0.0
    {
        pitch = forced;
    }
    Some((yaw, pitch.clamp(-90.0, 90.0)))
}

/// Pushes a rotation through the position sender, so the server gets it now
/// and the sender's baseline matches when the camera rotation is restored.
fn sync_sender(game: &mut Game<'_>, rotation: (f32, f32)) {
    let Some(player) = game.player.as_ref() else {
        return;
    };
    let position = [player.position.x, player.position.y, player.position.z];
    let on_ground = player.ground.on_ground;
    let collision = player.collision.horizontal;
    let packet = game
        .remote
        .as_mut()
        .and_then(|remote| remote.position_sender.update(position, rotation, on_ground, collision));
    if let Some(packet) = packet {
        game.send(packet);
    }
}

fn set_slot(game: &mut Game<'_>, slot: usize) {
    game.selected = slot;
    game.send(S::SetCarriedItem(play::SetCarriedItem {
        slot: slot as i16,
    }));
    if let Some(remote) = game.remote.as_mut() {
        remote.carried.set(Some(slot));
    }
}

/// The tower modes' upward motion once a block is placed.
fn tower(game: &mut Game<'_>, config: &ScaffoldConfig) {
    if config.tower == Tower::Off || !game.keys.contains(&KeyCode::Space) {
        return;
    }
    let (dx, dz) = input_dir(game);
    if (dx != 0.0 || dz != 0.0) && !config.tower_move {
        return;
    }
    let Some(player) = game.player.as_mut() else {
        return;
    };
    let velocity = match config.tower {
        Tower::Off => return,
        Tower::Vanilla | Tower::Legit | Tower::Watchdog => 0.42,
        Tower::Vulcan => 0.30,
        Tower::Snap => {
            player.position.y = player.position.y.round();
            0.42
        }
    };
    player.velocity.y = velocity;
    player.ground.on_ground = false;
}

#[cfg(test)]
mod tests {
    use super::edge;

    /// A world where only the listed cells are solid.
    fn world(solid: Vec<(i32, i32, i32)>) -> impl Fn(i32, i32, i32) -> bool {
        move |x, y, z| !solid.contains(&(x, y, z))
    }

    #[test]
    fn standing_on_a_block_is_not_an_edge() {
        let air = world(vec![(0, 63, 0), (0, 63, 1)]);
        assert!(!edge([0.5, 64.0, 0.5], [0.0, 0.0], &air));
    }

    #[test]
    fn the_next_step_past_the_block_is_an_edge() {
        // Standing near the far edge of a two-block platform, walking off it.
        let air = world(vec![(0, 63, 0), (0, 63, 1)]);
        assert!(edge([0.5, 64.0, 1.9], [0.0, 0.3], &air));
        // The same step the other way stays on the platform.
        assert!(!edge([0.5, 64.0, 1.9], [0.0, -0.3], &air));
    }

    #[test]
    fn already_over_air_is_an_edge_whatever_the_velocity() {
        let air = world(vec![(0, 63, 0)]);
        assert!(edge([4.5, 64.0, 4.5], [0.0, 0.0], &air));
        assert!(edge([4.5, 64.0, 4.5], [0.3, 0.0], &air));
    }
}
