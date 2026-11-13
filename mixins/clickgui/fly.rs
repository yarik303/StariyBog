// Fly: a port of Alan's Flight modes, the FDPClient/LiquidBounce fly modes,
// and the Sigma, Flux, Celestial, Tenacity, Siekera, Prisma and White Floor
// flight implementations.
//
// The references act from the tick head (their tick/update/pre-motion events),
// so `tick` runs at the head of `tick_remote_level`. Setting `player.velocity`
// there is what their motion assignment is: the physics moves by exactly that
// velocity this tick, then gravity and drag rewrite the stored velocity for the
// next one, and the mode overwrites it again. The modes that set their own
// horizontal velocity gate the vanilla travel input off through
// `clickgui_terrain::block_fly_input`, so the input acceleration does not add
// on top of the reference's `strafe`. `confirm` runs at the head of
// `remote_send_state`, after the physics and before the movement packet: it
// sends the modes' extra position/status packets (the enable bursts, periodic
// vertical round-trips, the four-packet self-damage boost and the elytra
// command). `spoof` rewrites the position sender's packet: ground flags, the
// OldNCP height jitter, BufferAbuse's packet drop. `snapshot` remembers the
// post-physics velocity so the next tick sees a server SetEntityMotion, and
// `teleport` observes server setbacks for Matrix and VerusDamage.
//
// Horizon has no collision-shape injection, packet queue or inventory layer,
// so the reference modes built on those (Collide, FakeGround, VulcanGhost,
// Blink, Fireball, Enderpearl, Boat, ElytraGlide, the position clips) are left
// out rather than approximated; the mode list documents them. The abilities-
// based Creative modes are left out too: setting `player.flying` sends
// `PlayerAbilities`, which the server rejects while flying.
use crate::game::clickgui as m;
use crate::game::clickgui::{FlyConfig, FlyMode as Mode};
use crate::game::{Game, Screen};
use horizon_client::clock::clickgui_timer as timer;
use horizon_client::mobs::Species;
use horizon_client::motion::clickgui_terrain as terrain;
use horizon_core::geometry::Vec3;
use horizon_protocol::packets::play::{self, Serverbound as S};
use std::cell::{Cell, RefCell};
use winit::keyboard::KeyCode;

/// The ground bit of `ServerboundMovePlayerPacket`'s flags.
const ON_GROUND: u8 = 1;
/// `ServerboundPlayerCommandPacket` START_FALL_FLYING.
const START_FALL_FLYING: i32 = 6;
/// The FDPClient enable burst: 65 rising/falling position pairs and a ground
/// packet.
const BURST_PAIRS: i32 = 65;
const BURST_STEP: f64 = 0.049;
/// BoostHypixel's fall-damage burst.
const BOOST_HYPIXEL_START: f64 = 3.0125;
const BOOST_HYPIXEL_STEP: f64 = 0.062_498_642_1;
const BOOST_HYPIXEL_TAIL: f64 = 0.000_001_357_9;
/// The four-packet self-damage boost (Sentinel20thApr/26thDec,
/// VerusB3896Damage).
const BOOST_DAMAGE: f64 = 3.25;
/// Vanilla's one-tick dip, every 40 ticks, so the "moving while flying" check
/// sees a fall.
const VANILLA_DIP: f64 = -0.04;
/// Siekera's anti-kick: the interval and the distance it dips by.
const ANTI_KICK_INTERVAL: i32 = 30;
const ANTI_KICK_DISTANCE: f64 = 0.07;
/// BufferAbuse's cancel distance: 10 - speed - 0.15 blocks from the last
/// reported position.
const BUFFER_REACH: f64 = 9.85;
/// MatrixHighJump's self-damage fall. Horizon applies gravity before the move,
/// so the four vanilla jumps only accumulate about 3.3 blocks of fall distance.
/// This packet fall crosses the 3.0-block safe distance and the 4.0 needed for
/// the first damage point in one go.
const HIGHJUMP_SELF_FALL: f64 = 4.5;
/// Ticks to wait for the hurt after the self-damage fall before starting the
/// boost anyway (creative, fall damage off, Feather Falling).
const HIGHJUMP_SELF_TIMEOUT: i32 = 20;

/// The counters and flags the modes keep between ticks and packets.
struct State {
    mode: Option<Mode>,
    /// Ticks since the mode was enabled.
    ticks: i32,
    /// Ticks since leaving the ground.
    air_ticks: i32,
    /// The server's last setback, for the cancel-distance modes.
    server_pos: Option<[f64; 3]>,
    /// An explosion was received since the last tick.
    explosion: bool,
    /// The last position a movement packet reported (BufferAbuse).
    last_sent: Option<[f64; 3]>,
    /// The current tick's position, for the packet rewrite.
    position: [f64; 3],
    /// GrimOld: a server velocity unlocked the one-tick flight.
    should_fly: bool,
    /// GrimOld: the fall was reported and the player is held in place.
    frozen: bool,
    /// NCPBurst and HypixelBoost: the enable burst ran.
    burst_done: bool,
    /// NCP: the take-off started or already ended, and its decaying speed.
    ncp_started: bool,
    ncp_done: bool,
    ncp_speed: f32,
    /// NCP Clip: the three clip packets were sent.
    ncp_clip_sent: bool,
    /// Hypixel: the explosion curve's stage and tick.
    hypixel_seen: bool,
    hypixel_stage: i32,
    hypixel_ticks: i32,
    /// HypixelBoost: the speed ramp's state and distance.
    boost_state: i32,
    boost_speed: f64,
    /// Sentinel: the boost was sent.
    sentinel_boosted: bool,
    /// Damage: ticks left in the hurt window.
    damage_left: i32,
    /// VerusDamage: the self-damage boost state and lifetime.
    damage_boosted: bool,
    damage_flying: bool,
    damage_ticks: i32,
    /// Matrix: the server set the player back since enabling.
    teleported: bool,
    matrix_ticks: i32,
    /// MatrixHighJump: the hurt boost and the motion a setback restores.
    highjump_boosting: bool,
    highjump_saved: Option<[f64; 3]>,
    /// MatrixHighJump: the packet fall's stage (0 waiting for the ground, 1
    /// the rise was sent, 2 the landing was sent, 3 done) and the ticks spent
    /// waiting for the hurt afterwards.
    highjump_self_stage: i32,
    highjump_self_wait: i32,
    highjump_self_at: [f64; 3],
    /// Explosion: the decaying strafe.
    explosion_strafe: f32,
    /// The packet rewrite the tick asked for.
    spoof_ground: bool,
    spoof_y: f64,
    drop: bool,
    /// The packets `confirm` sends before the movement packet.
    extra: Vec<S>,
}

impl State {
    fn new(mode: Option<Mode>) -> Self {
        Self {
            mode,
            ticks: 0,
            air_ticks: 0,
            server_pos: None,
            explosion: false,
            last_sent: None,
            position: [0.0; 3],
            should_fly: false,
            frozen: false,
            burst_done: false,
            ncp_started: false,
            ncp_done: false,
            ncp_speed: 0.0,
            ncp_clip_sent: false,
            hypixel_seen: false,
            hypixel_stage: 0,
            hypixel_ticks: 0,
            boost_state: 0,
            boost_speed: 0.0,
            sentinel_boosted: false,
            damage_left: 0,
            damage_boosted: false,
            damage_flying: false,
            damage_ticks: 0,
            teleported: false,
            matrix_ticks: 0,
            highjump_boosting: false,
            highjump_saved: None,
            highjump_self_stage: 0,
            highjump_self_wait: 0,
            highjump_self_at: [0.0; 3],
            explosion_strafe: 0.0,
            spoof_ground: false,
            spoof_y: 0.0,
            drop: false,
            extra: Vec::new(),
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
    hurt_time: i32,
    hurt: bool,
    fall_distance: f64,
    position: [f64; 3],
    velocity: [f64; 3],
    hspeed: f32,
    air_ticks: i32,
    jump_held: bool,
    shift_held: bool,
    riding: bool,
    explo: bool,
    knock: bool,
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
fn running_config(game: &Game<'_>) -> Option<FlyConfig> {
    if !game.native_window() || !game.native_playing() || !game.native_on(m::ID) {
        return None;
    }
    if game.spectator() || game.screen != Screen::Playing {
        return None;
    }
    game.player.as_ref()?;
    m::fly()
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
    let velocity = [player.velocity.x, player.velocity.y, player.velocity.z];
    let on_ground = player.ground.on_ground;
    if on_ground {
        state.air_ticks = 0;
    } else {
        state.air_ticks += 1;
    }
    let hspeed = (velocity[0] * velocity[0] + velocity[2] * velocity[2]).sqrt() as f32;
    let explosion = std::mem::take(&mut state.explosion);
    Ctx {
        dir,
        moving: dir[0] != 0.0 || dir[1] != 0.0,
        on_ground,
        yaw: game.yaw,
        hurt_time: player.health.hurt.hurt_time,
        hurt: player.health.hurt.hurt_time > 0,
        fall_distance: player.fall_distance,
        position,
        velocity,
        hspeed,
        air_ticks: state.air_ticks,
        jump_held: held(KeyCode::Space),
        shift_held: held(KeyCode::ShiftLeft),
        riding: game.riding.is_some(),
        explo: explosion,
        knock: LAST
            .with(|last| last.get())
            .is_some_and(|last| differs(velocity, last)),
    }
}

fn differs(a: [f64; 3], b: [f64; 3]) -> bool {
    (a[0] - b[0]).abs() > 1.0e-7 || (a[1] - b[1]).abs() > 1.0e-7 || (a[2] - b[2]).abs() > 1.0e-7
}

/// The tick hook, before the player's input and physics move.
pub(super) fn tick(game: &mut Game<'_>) {
    terrain::begin_fly_tick();
    timer::begin_fly_tick();
    let mut state = take_state();
    let Some(config) = running_config(game) else {
        if state.mode.is_some() {
            stop(game);
        }
        put_state(State::default());
        return;
    };
    if state.mode != Some(config.mode) {
        if state.mode.is_some() {
            stop(game);
        }
        state = State::new(Some(config.mode));
        if let Some(player) = game.player.as_ref() {
            state.server_pos = Some([player.position.x, player.position.y, player.position.z]);
        }
    }
    state.ticks += 1;
    let ctx = context(game, &mut state);
    state.position = ctx.position;
    if state.last_sent.is_none() {
        state.last_sent = Some(ctx.position);
    }
    state.spoof_ground = false;
    state.spoof_y = 0.0;
    state.drop = false;
    state.extra.clear();
    if !ctx.riding {
        match config.mode {
            Mode::Vanilla => vanilla(game, &config, &mut state, &ctx),
            Mode::AirWalk => air_walk(game, &config, &mut state, &ctx),
            Mode::Grim => grim(game, &config, &mut state, &ctx),
            Mode::GrimOld => grim_old(game, &config, &mut state, &ctx),
            Mode::GrimElytra => grim_elytra(game, &mut state, &ctx),
            Mode::GrimFloat => grim_float(game, &config, &mut state, &ctx),
            Mode::GrimEntity => grim_entity(game, &config, &mut state, &ctx),
            Mode::Ncp => ncp(game, &config, &mut state, &ctx),
            Mode::OldNcp => old_ncp(game, &config, &mut state, &ctx),
            Mode::NcpBurst => ncp_burst(game, &config, &mut state, &ctx),
            Mode::Hypixel => hypixel(game, &config, &mut state, &ctx),
            Mode::HypixelBoost => hypixel_boost(game, &config, &mut state, &ctx),
            Mode::Vulcan => vulcan(game, &config, &mut state, &ctx),
            Mode::Verus => verus(game, &config, &mut state, &ctx),
            Mode::VerusDamage => verus_damage(game, &config, &mut state, &ctx),
            Mode::Spartan => spartan(game, &config, &mut state, &ctx),
            Mode::AacGlide => aac_glide(game, &config, &mut state, &ctx),
            Mode::Matrix => matrix(game, &config, &mut state, &ctx),
            Mode::Damage => damage(game, &config, &mut state, &ctx),
            Mode::Sentinel => sentinel(game, &config, &mut state, &ctx),
            Mode::Jetpack => jetpack(game, &config, &ctx),
            Mode::Explosion => explosion(game, &config, &mut state, &ctx),
            Mode::BufferAbuse => buffer_abuse(game, &config, &mut state, &ctx),
            Mode::MatrixHighJump => matrix_high_jump(game, &config, &mut state, &ctx),
        }
    }
    put_state(state);
}

/// Zeroes the horizontal motion the modes leave behind when they stop.
fn stop(game: &mut Game<'_>) {
    if let Some(player) = game.player.as_mut() {
        player.velocity.x = 0.0;
        player.velocity.z = 0.0;
    }
}

/// The tick hook's packet stage: the mode's extras go out before the movement
/// packet the position sender builds.
pub(super) fn confirm(game: &mut Game<'_>) {
    let Some(config) = m::fly() else {
        return;
    };
    let mut state = take_state();
    if state.mode != Some(config.mode) {
        put_state(state);
        return;
    }
    for packet in state.extra.drain(..) {
        game.send(packet);
    }
    if let Some(player) = game.player.as_ref() {
        state.position = [player.position.x, player.position.y, player.position.z];
    }
    put_state(state);
}

/// The snapshot hook, at the head of `remote_send_state`.
pub(super) fn snapshot(game: &Game<'_>) {
    let velocity = game
        .player
        .as_ref()
        .map(|player| [player.velocity.x, player.velocity.y, player.velocity.z]);
    LAST.with(|last| last.set(velocity));
}

/// The explosion hook, at the head of `remote_explode`.
pub(crate) fn explode(game: &mut Game<'_>, _packet: &play::Explode) {
    if game.player.is_none() {
        return;
    }
    let mut state = take_state();
    state.explosion = true;
    put_state(state);
}

/// The teleport hook: record the server's setback. The reference Matrix and
/// NCP Clip modes also cancel the packet; Horizon's teleport handler owns the
/// accept handshake, so the modes only observe it and boost afterwards.
pub(super) fn teleport(game: &mut Game<'_>, packet: &play::PlayerPosition) {
    let Some(config) = m::fly() else {
        return;
    };
    let mut state = take_state();
    if state.mode != Some(config.mode) {
        put_state(state);
        return;
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
    state.server_pos = Some([
        pick(rel::X, current[0], packet.position[0]),
        pick(rel::Y, current[1], packet.position[1]),
        pick(rel::Z, current[2], packet.position[2]),
    ]);
    if config.mode == Mode::Matrix {
        state.teleported = true;
        state.matrix_ticks = 0;
    }
    if config.mode == Mode::MatrixHighJump && state.highjump_boosting {
        state.highjump_saved = game
            .player
            .as_ref()
            .map(|player| [player.velocity.x, player.velocity.y, player.velocity.z]);
        state.highjump_boosting = false;
        // A setback is the end of the boost, and nothing else can start
        // another one: the damage sequence only runs below stage 3, and the
        // only other way in is a fresh hurt inside the hurt window, which a
        // player who is not falling never takes. Left at 3 here, the mode is
        // inert from the first setback until it is toggled — which is what
        // "MatrixHighJump does nothing" looks like on a server that sets the
        // player back. Rewind so the next landing runs the sequence again.
        state.highjump_self_stage = 0;
        state.highjump_self_wait = 0;
    }
    put_state(state);
}

/// The module's movement-packet rewrite, after NoFall, Criticals, Speed and
/// Step have had their say.
pub(super) fn spoof(packet: Option<S>) -> Option<S> {
    let Some(config) = m::fly() else {
        return packet;
    };
    let mut state = take_state();
    if state.mode != Some(config.mode) {
        put_state(state);
        return packet;
    }
    let Some(packet) = packet else {
        put_state(state);
        return None;
    };
    if config.mode == Mode::BufferAbuse
        && !config.buffer_send_flying
        && matches!(&packet, S::MovePlayerStatus(_))
    {
        put_state(state);
        return None;
    }
    if config.mode == Mode::BufferAbuse {
        let reach = BUFFER_REACH - f64::from(config.buffer_speed);
        let close = state
            .last_sent
            .is_none_or(|last| distance3(state.position, last) <= reach.max(0.0));
        if close {
            put_state(state);
            return None;
        }
        remember_sent(&mut state, &packet);
    }
    let packet = if state.spoof_ground {
        ground(packet)
    } else {
        packet
    };
    let packet = if state.spoof_y != 0.0 {
        offset_y(packet, state.spoof_y)
    } else {
        packet
    };
    let drop = state.drop;
    put_state(state);
    if drop { None } else { Some(packet) }
}

fn remember_sent(state: &mut State, packet: &S) {
    match packet {
        S::MovePlayerPos(p) => state.last_sent = Some([p.x, p.y, p.z]),
        S::MovePlayerPosRot(p) => state.last_sent = Some([p.x, p.y, p.z]),
        _ => {}
    }
}

fn distance3(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn ground(packet: S) -> S {
    match packet {
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
    }
}

fn offset_y(packet: S, offset: f64) -> S {
    match packet {
        S::MovePlayerPos(mut p) => {
            p.y += offset;
            S::MovePlayerPos(p)
        }
        S::MovePlayerPosRot(mut p) => {
            p.y += offset;
            S::MovePlayerPosRot(p)
        }
        other => other,
    }
}

fn pos(x: f64, y: f64, z: f64, flags: u8) -> S {
    S::MovePlayerPos(play::MovePlayerPos { x, y, z, flags })
}

fn status(flags: u8) -> S {
    S::MovePlayerStatus(play::MovePlayerStatus { flags })
}

fn set_velocity(game: &mut Game<'_>, x: f64, y: f64, z: f64) {
    if let Some(player) = game.player.as_mut() {
        player.velocity.x = x;
        player.velocity.y = y;
        player.velocity.z = z;
    }
}

fn set_y(game: &mut Game<'_>, y: f32) {
    if let Some(player) = game.player.as_mut() {
        player.velocity.y = f64::from(y);
    }
}

fn add_y(game: &mut Game<'_>, delta: f64) {
    if let Some(player) = game.player.as_mut() {
        player.velocity.y += delta;
    }
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

/// Replaces the horizontal velocity with `speed` along the input direction.
fn strafe_speed(game: &mut Game<'_>, ctx: &Ctx, speed: f32) {
    let Some(player) = game.player.as_mut() else {
        return;
    };
    let current = [player.velocity.x as f32, player.velocity.z as f32];
    let [x, z] = m::with_strafe(current, ctx.dir, ctx.yaw, Some(speed), 1.0);
    player.velocity.x = f64::from(x);
    player.velocity.z = f64::from(z);
}

/// The jump the references perform from the ground.
fn launch(game: &mut Game<'_>, power: f32) {
    let yaw = game.yaw;
    if !(power > 1.0e-5) {
        return;
    }
    if let Some(player) = game.player.as_mut() {
        player.velocity.y = f64::from(power).max(player.velocity.y);
        if player.sprinting {
            let rad = f64::from(yaw.to_radians());
            player.velocity.x -= rad.sin() * 0.2;
            player.velocity.z += rad.cos() * 0.2;
        }
        player.no_jump_delay = 2;
    }
}

fn key_vertical(ctx: &Ctx, speed: f32) -> f32 {
    if ctx.jump_held && !ctx.shift_held {
        speed
    } else if ctx.shift_held && !ctx.jump_held {
        -speed
    } else {
        0.0
    }
}

fn vanilla(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    let vertical = if ctx.jump_held ^ ctx.shift_held {
        key_vertical(ctx, config.vanilla_vertical)
    } else {
        config.vanilla_glide
    };
    set_y(game, vertical);
    strafe_speed(game, ctx, config.vanilla_speed);
    if config.vanilla_bypass && state.ticks % 40 == 0 {
        set_y(game, VANILLA_DIP as f32);
    }
    if config.anti_kick && !ctx.shift_held {
        if state.ticks % ANTI_KICK_INTERVAL == 0 {
            set_y(game, -(ANTI_KICK_DISTANCE as f32));
        } else if state.ticks % ANTI_KICK_INTERVAL == 1 {
            set_y(game, ANTI_KICK_DISTANCE as f32);
        }
    }
}

fn air_walk(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    state.spoof_ground = true;
    timer::request_fly(config.airwalk_timer);
    set_y(game, key_vertical(ctx, config.airwalk_vertical));
    strafe_speed(game, ctx, config.airwalk_speed);
    if state.ticks % i32::try_from(config.airwalk_ticks).unwrap_or(1).max(1) == 0 {
        state.extra.push(status(ON_GROUND));
    }
}

fn grim(game: &mut Game<'_>, config: &FlyConfig, _state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    let speed = if ctx.hurt {
        config.grim_hurt_speed
    } else {
        config.grim_speed
    };
    let vertical = if ctx.jump_held ^ ctx.shift_held {
        key_vertical(ctx, config.grim_vertical)
    } else {
        -config.grim_sink
    };
    set_y(game, vertical);
    strafe_speed(game, ctx, speed);
}

fn grim_old(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    timer::request_fly(config.grim_old_timer);
    if ctx.knock {
        state.should_fly = true;
        state.frozen = false;
    }
    if state.should_fly {
        state.should_fly = false;
        set_y(game, 0.0);
        strafe_speed(game, ctx, config.grim_old_speed);
        return;
    }
    if !state.frozen && ctx.fall_distance > 0.0 {
        state.extra.push(status(ON_GROUND));
        if let Some(player) = game.player.as_mut() {
            player.fall_distance = 0.0;
        }
        state.frozen = true;
    }
    if state.frozen {
        set_velocity(game, 0.0, 0.0, 0.0);
    }
}

fn grim_elytra(game: &mut Game<'_>, state: &mut State, ctx: &Ctx) {
    if ctx.on_ground {
        return;
    }
    let id = game_entity_id(game);
    state.extra.push(S::PlayerCommand(play::PlayerCommand {
        id,
        action: START_FALL_FLYING,
        data: 0,
    }));
}

fn grim_float(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    let vertical = if ctx.jump_held || ctx.shift_held {
        key_vertical(ctx, config.grim_float_vertical)
    } else {
        (state.ticks as f32 * 0.1).sin() * 0.01
    };
    set_y(game, vertical);
    strafe_speed(game, ctx, config.grim_float_speed);
    if state.ticks % 10 == 0 {
        state.extra.push(pos(
            ctx.position[0],
            ctx.position[1] - 0.1,
            ctx.position[2],
            ON_GROUND,
        ));
    }
}

fn grim_entity(game: &mut Game<'_>, config: &FlyConfig, _state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    if !exempt_nearby(game, config.grim_entity_distance) {
        set_velocity(game, 0.0, 0.0, 0.0);
        return;
    }
    let rad = ctx.yaw.to_radians();
    let (sin, cos) = (rad.sin(), rad.cos());
    let (strafe, forward) = (ctx.dir[0], ctx.dir[1]);
    let mut x = (strafe * cos - forward * sin) * config.grim_entity_speed;
    let mut y = key_vertical(ctx, config.grim_entity_vertical);
    let mut z = (forward * cos + strafe * sin) * config.grim_entity_speed;
    let length = (x * x + y * y + z * z).sqrt();
    if length > 1.1 {
        let scale = 1.1 / length;
        x *= scale;
        y *= scale;
        z *= scale;
    }
    set_velocity(game, f64::from(x), f64::from(y), f64::from(z));
}

fn exempt_nearby(game: &Game<'_>, distance: f32) -> bool {
    let Some(player) = game.player.as_ref() else {
        return false;
    };
    let reach = f64::from(distance);
    let bounds = player.bounds().inflate(Vec3::new(reach, reach, reach));
    let mob = game.mobs.iter().any(|mob| {
        !mob.removed
            && matches!(mob.species(), Species::Shulker | Species::HappyGhast)
            && bounds.intersects(mob.bounds())
    });
    mob || game
        .boats
        .iter()
        .any(|boat| bounds.intersects(boat.bounds()))
}

fn ncp(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    if state.ncp_done {
        return;
    }
    if config.ncp_clip {
        timer::request_fly(config.ncp_timer);
        if !state.ncp_started {
            let above = box_collides(game, 1.0);
            if above && !state.ncp_clip_sent {
                state
                    .extra
                    .push(pos(ctx.position[0], ctx.position[1], ctx.position[2], 0));
                state.extra.push(pos(
                    ctx.position[0],
                    ctx.position[1] - 0.1,
                    ctx.position[2],
                    0,
                ));
                state
                    .extra
                    .push(pos(ctx.position[0], ctx.position[1], ctx.position[2], 0));
                state.ncp_clip_sent = true;
            } else if !above && state.ncp_clip_sent {
                state.ncp_started = true;
                state.ncp_speed = config.ncp_speed + 1.0;
                launch(game, 0.42);
                strafe_speed(game, ctx, state.ncp_speed);
            } else {
                set_velocity(game, 0.0, 0.0, 0.0);
            }
            return;
        }
        state.ncp_speed = (state.ncp_speed * 0.935).max(config.ncp_speed - 0.4);
        add_y(game, 0.025);
        strafe_speed(game, ctx, state.ncp_speed);
        return;
    }
    if !state.ncp_started {
        if !box_collides(game, 1.0) {
            state.ncp_started = true;
            state.ncp_speed = config.ncp_speed;
            launch(game, 0.42);
            strafe_speed(game, ctx, state.ncp_speed);
        } else {
            set_velocity(game, 0.0, 0.0, 0.0);
        }
        return;
    }
    state.ncp_speed *= 0.935;
    add_y(game, 0.025);
    strafe_speed(game, ctx, state.ncp_speed);
    if ctx.velocity[1] < -0.5 {
        state.ncp_done = true;
        stop(game);
    }
}

fn old_ncp(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    set_y(game, 0.0);
    strafe_speed(game, ctx, config.old_ncp_speed);
    state.spoof_y = if state.ticks % 2 == 0 { 2.0e-5 } else { 0.0 };
}

fn ncp_burst(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    state.spoof_ground = true;
    if ctx.on_ground && !state.burst_done {
        for _ in 0..BURST_PAIRS {
            state.extra.push(pos(
                ctx.position[0],
                ctx.position[1] + BURST_STEP,
                ctx.position[2],
                0,
            ));
            state
                .extra
                .push(pos(ctx.position[0], ctx.position[1], ctx.position[2], 0));
        }
        state.extra.push(pos(
            ctx.position[0],
            ctx.position[1] + 0.1,
            ctx.position[2],
            ON_GROUND,
        ));
        multiply_xz(game, 0.1);
        state.burst_done = true;
    }
    set_y(game, -config.ncp_burst_vertical);
    strafe_speed(game, ctx, config.ncp_burst_speed);
}

fn hypixel(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    timer::request_fly(config.hypixel_timer);
    if !state.hypixel_seen && ctx.explo {
        state.hypixel_seen = true;
        state.hypixel_stage = 0;
        state.hypixel_ticks = 0;
    }
    if !state.hypixel_seen {
        set_velocity(game, 0.0, 0.0, 0.0);
        return;
    }
    if config.hypixel_flat {
        state.hypixel_ticks += 1;
        if state.hypixel_ticks > 30 {
            state.hypixel_seen = false;
            return;
        }
        set_y(game, 0.0314 + (state.ticks % 7) as f32 / 1000.0);
        strafe_speed(game, ctx, ctx.hspeed);
        return;
    }
    match state.hypixel_stage {
        0 => {
            set_y(game, config.hypixel_vertical);
            state.hypixel_stage = 1;
        }
        1 => {
            strafe_speed(game, ctx, config.hypixel_speed);
            set_y(game, 1.0);
            state.hypixel_stage = 2;
        }
        2 => {
            multiply_xyz(game, 1.05, 1.0, 1.05);
            state.hypixel_stage = 3;
            state.hypixel_ticks = 0;
        }
        3 => {
            strafe_speed(game, ctx, ctx.hspeed);
            state.hypixel_ticks += 1;
            if state.hypixel_ticks >= 19 {
                add_y(game, 0.42);
                state.hypixel_stage = 4;
            }
        }
        _ => {
            strafe_speed(game, ctx, ctx.hspeed);
            if ctx.on_ground {
                state.hypixel_seen = false;
            }
        }
    }
}

fn hypixel_boost(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    timer::request_fly(config.hypixel_boost_timer);
    if state.ticks % 2 == 0 {
        state.spoof_y = 1.0e-5;
    }
    if !state.burst_done && ctx.on_ground {
        for _ in 0..10 {
            state.extra.push(pos(
                ctx.position[0],
                ctx.position[1],
                ctx.position[2],
                ON_GROUND,
            ));
        }
        let mut fall = BOOST_HYPIXEL_START;
        while fall > 0.0 {
            let y = ctx.position[1];
            let x = ctx.position[0];
            let z = ctx.position[2];
            state
                .extra
                .push(pos(x, y + fall + BOOST_HYPIXEL_STEP, z, 0));
            state.extra.push(pos(x, y + fall + 0.0625, z, 0));
            state
                .extra
                .push(pos(x, y + fall + BOOST_HYPIXEL_STEP, z, 0));
            state
                .extra
                .push(pos(x, y + fall + BOOST_HYPIXEL_TAIL, z, 0));
            fall -= BOOST_HYPIXEL_STEP;
        }
        state.extra.push(pos(
            ctx.position[0],
            ctx.position[1],
            ctx.position[2],
            ON_GROUND,
        ));
        launch(game, 0.42);
        state.burst_done = true;
        state.boost_state = 1;
    }
    set_y(game, 0.0);
    let amp = game
        .potions
        .effects
        .amplifier("speed")
        .map_or(0, |level| level + 1);
    let base = f64::from(config.hypixel_boost_speed) * (1.0 + 0.2 * f64::from(amp));
    state.boost_speed = match state.boost_state {
        1 => {
            state.boost_state = 2;
            base * 2.034
        }
        2 => {
            state.boost_state = 3;
            state.boost_speed * 2.16
        }
        _ => {
            let factor = if state.ticks % 2 == 0 { 0.0103 } else { 0.0123 };
            (state.boost_speed - factor * (state.boost_speed - base)).min(0.3)
        }
    };
    strafe_speed(game, ctx, state.boost_speed as f32);
}

fn vulcan(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    if ctx.fall_distance > 0.0 {
        let down = if state.ticks % 2 == 0 {
            config.vulcan_down
        } else {
            config.vulcan_up
        };
        set_y(game, -down);
    }
    move_at(game, ctx, config.vulcan_speed);
}

fn verus(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    state.spoof_ground = true;
    if ctx.on_ground {
        if state.ticks % 14 == 0 {
            set_y(game, 0.42);
            strafe_speed(game, ctx, 0.69);
        } else {
            set_y(game, 0.0);
            strafe_speed(game, ctx, config.verus_ground_speed);
        }
    } else {
        set_y(game, 0.0);
        strafe_speed(game, ctx, config.verus_air_speed);
    }
}

fn verus_damage(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    if !state.damage_boosted {
        damage_boost(&mut state.extra, ctx.position);
        state.damage_boosted = true;
        state.damage_flying = true;
        state.damage_ticks = 0;
        if state.server_pos.is_none() {
            state.server_pos = Some(ctx.position);
        }
    }
    if !state.damage_flying {
        return;
    }
    set_y(game, key_vertical(ctx, config.verus_damage_vertical));
    strafe_speed(game, ctx, config.verus_damage_speed);
    let close = state.server_pos.is_some_and(|server| {
        distance3(ctx.position, server) <= f64::from(config.verus_damage_cancel)
    });
    if close {
        state.drop = true;
    } else {
        state.damage_ticks += 1;
        if state.damage_ticks >= i32::try_from(config.verus_damage_ticks).unwrap_or(20) {
            state.damage_flying = false;
            stop(game);
        }
    }
}

fn spartan(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    set_y(game, 0.0);
    move_at(game, ctx, config.spartan_speed);
    let interval = i32::try_from(config.spartan_interval).unwrap_or(12).max(1);
    if state.ticks % interval == 0 {
        let (x, y, z) = (ctx.position[0], ctx.position[1], ctx.position[2]);
        state
            .extra
            .push(pos(x, y + f64::from(config.spartan_offset), z, ON_GROUND));
        state
            .extra
            .push(pos(x, y - f64::from(config.spartan_offset), z, ON_GROUND));
    }
}

fn aac_glide(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    if ctx.on_ground {
        timer::request_fly(1.0);
        return;
    }
    let ticks = i32::try_from(config.aac_glide_ticks).unwrap_or(12).max(1);
    if ctx.air_ticks >= ticks {
        set_y(game, config.aac_glide_lift);
        state.air_ticks = 0;
        timer::request_fly(config.aac_glide_timer);
    } else {
        timer::request_fly(1.0);
    }
}

fn matrix(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    if ctx.air_ticks > 1 {
        add_y(game, 0.003_48);
    }
    if ctx.on_ground {
        launch(game, 0.42);
    }
    if state.teleported {
        state.matrix_ticks += 1;
        if state.matrix_ticks == 20 {
            set_y(game, config.matrix_vertical);
            strafe_speed(game, ctx, config.matrix_speed);
        }
    } else if ctx.fall_distance > 0.1 {
        set_y(game, config.matrix_vertical);
        strafe_speed(game, ctx, 1.97);
    }
}

/// The Self Damage start. Jumping here cannot hurt a modern player: Horizon
/// applies gravity and drag before moving, which makes a tick-head 0.42 jump
/// only ~0.83 blocks high, and four of them stay under the safe distance. The
/// server is instead given a rise packet and, on the next tick, a landing
/// packet at the real feet position. That is one server-side fall the client
/// cannot reset. Returns whether this tick sent a position packet, in which
/// case the normal movement packet must be dropped (one per tick on a LAN).
fn highjump_self_damage(state: &mut State, ctx: &Ctx) -> bool {
    match state.highjump_self_stage {
        0 => {
            if !ctx.on_ground {
                return false;
            }
            state.highjump_self_at = ctx.position;
            let [x, y, z] = state.highjump_self_at;
            state.extra.push(pos(x, y + HIGHJUMP_SELF_FALL, z, 0));
            state.drop = true;
            state.highjump_self_stage = 1;
            true
        }
        1 => {
            let [x, y, z] = state.highjump_self_at;
            state.extra.push(pos(x, y, z, ON_GROUND));
            state.drop = true;
            state.highjump_self_stage = 2;
            state.highjump_self_wait = 0;
            true
        }
        _ => false,
    }
}

fn matrix_high_jump(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    if let Some(saved) = state.highjump_saved.take() {
        set_velocity(game, saved[0], saved[1], saved[2]);
    }
    if state.highjump_boosting {
        set_y(game, config.highjump_height);
    }
    if config.highjump_self_damage
        && state.highjump_self_stage < 3
        && !state.highjump_boosting
        && !ctx.hurt
        && highjump_self_damage(state, ctx)
    {
        return;
    }
    let window = (1..=8).contains(&ctx.hurt_time);
    if window {
        // The fall hurt the player, which is what the mode waits for.
        state.highjump_self_stage = 3;
        if ctx.on_ground {
            launch(game, 0.42);
        } else if ctx.velocity[1] < 0.2 {
            state.highjump_boosting = true;
        }
    } else if state.highjump_self_stage == 2 {
        state.highjump_self_wait += 1;
        if state.highjump_self_wait >= HIGHJUMP_SELF_TIMEOUT {
            // No hurt arrived: creative, fall damage off or Feather Falling.
            // Start the boost anyway rather than leaving the mode inert.
            state.highjump_boosting = true;
            state.highjump_self_stage = 3;
            set_y(game, config.highjump_height);
        }
    } else if state.highjump_self_stage == 3 && !state.highjump_boosting {
        // The hurt landed while the player was still on the ground and the
        // launch never lifted them — a low ceiling, or a hurt window that
        // opened with a single tick left — so the window closed on stage 3
        // with no boost. Nothing re-arms from there, so rewind and run the
        // sequence again. Cannot fire while boosting.
        state.highjump_self_stage = 0;
        state.highjump_self_wait = 0;
    }
}

fn damage(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    if state.damage_left <= 0 {
        let trigger = if config.damage_trigger == 1 {
            ctx.knock
        } else {
            ctx.hurt_time == 9
        };
        if trigger {
            state.damage_left = i32::try_from(config.damage_ticks).unwrap_or(14);
        }
    }
    if state.damage_left <= 0 {
        return;
    }
    state.damage_left -= 1;
    timer::request_fly(config.damage_timer);
    let vertical = if ctx.jump_held ^ ctx.shift_held {
        key_vertical(ctx, config.damage_vertical)
    } else {
        let y = ctx.velocity[1] as f32;
        if y > 0.0 { y + 0.1 } else { y * 0.8 }
    };
    set_y(game, vertical);
    strafe_speed(game, ctx, config.damage_speed);
}

fn sentinel(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    timer::request_fly(config.sentinel_timer);
    if state.ticks == i32::try_from(config.sentinel_ticks).unwrap_or(20) + 1 {
        stop(game);
        return;
    }
    if !state.sentinel_boosted {
        damage_boost(&mut state.extra, ctx.position);
        state.sentinel_boosted = true;
    } else if config.sentinel_reboost > 0 {
        let interval = i32::try_from(config.sentinel_reboost).unwrap_or(0).max(1);
        if state.ticks % interval == 0 {
            damage_boost(&mut state.extra, ctx.position);
        }
    }
    set_y(game, key_vertical(ctx, config.sentinel_vertical));
    strafe_speed(game, ctx, config.sentinel_speed);
}

fn damage_boost(extra: &mut Vec<S>, position: [f64; 3]) {
    let (x, y, z) = (position[0], position[1], position[2]);
    extra.push(pos(x, y, z, 0));
    extra.push(pos(x, y + BOOST_DAMAGE, z, 0));
    extra.push(pos(x, y, z, 0));
    extra.push(pos(x, y, z, ON_GROUND));
}

fn jetpack(game: &mut Game<'_>, config: &FlyConfig, ctx: &Ctx) {
    if ctx.jump_held {
        add_y(game, f64::from(config.jetpack_impulse));
        multiply_xz(game, config.jetpack_speed);
    }
}

fn explosion(game: &mut Game<'_>, config: &FlyConfig, state: &mut State, ctx: &Ctx) {
    if ctx.explo {
        if let Some(player) = game.player.as_mut() {
            player.velocity.x = 0.0;
            player.velocity.z = 0.0;
            player.velocity.y *= f64::from(config.explosion_vertical);
        }
        state.explosion_strafe = config.explosion_speed;
    }
    if ctx.on_ground {
        state.explosion_strafe = 0.0;
        return;
    }
    if state.explosion_strafe > 0.0 {
        terrain::block_fly_input();
        strafe_speed(game, ctx, state.explosion_strafe);
        state.explosion_strafe = (state.explosion_strafe - config.explosion_decrease).max(0.0);
    }
}

fn buffer_abuse(game: &mut Game<'_>, config: &FlyConfig, _state: &mut State, ctx: &Ctx) {
    terrain::block_fly_input();
    let vertical = if ctx.jump_held && !ctx.shift_held {
        config.buffer_speed
    } else if ctx.shift_held && !ctx.jump_held {
        -config.buffer_speed
    } else {
        0.0
    };
    set_y(game, vertical);
    strafe_speed(game, ctx, config.buffer_speed);
}

/// Strafe at `speed` when the player is moving; otherwise leave the reference's
/// horizontal motion alone.
fn move_at(game: &mut Game<'_>, ctx: &Ctx, speed: f32) {
    if ctx.moving {
        terrain::block_fly_input();
        strafe_speed(game, ctx, speed);
    }
}

fn game_entity_id(game: &Game<'_>) -> i32 {
    game.remote.as_ref().map_or(0, |remote| remote.entity_id)
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
