// CrystalAura: place and break end crystals where the blast hurts the target
// more than it hurts the player.
//
// The references are Zio's `AutoCrystalModule`/`AutoCrystalTargeting` and
// catlean's `AutoCrystal`. Both are the same loop — pick a target, find the
// best blast, place a crystal on the base, break any crystal already in range —
// wrapped in a large amount of anti-cheat shaping (async scanning, id
// prediction, face-place, damage sync, extrapolation, swap modes).
//
// The damage model is NOT reimplemented here. `Game::seen_percent` is
// `ServerExplosion.getSeenPercent`, and `game/explosion.rs` applies it as
//     diameter = radius * 2, dist = distance / diameter, pow = (1 - dist) * exposure
//     amount   = (pow^2 + pow) / 2 * 7 * diameter + 1
// so this uses exactly the numbers the client itself would apply. Exposure is
// what makes crystal damage sane: a crystal beside a player standing on the
// ground is mostly occluded, and the ray sampling accounts for that.
//
// Placement follows the client's own `Game::place_end_crystal`: the base must be
// obsidian or bedrock, the block above must be air, and no entity may occupy the
// two-block room. `UseItemOn` names the base's top face, which the server places
// at `base + up` — the crystal's own position.
//
// Not ported, and deliberately: the async scan threads (a single-threaded scan
// capped at `SCAN_CAP` candidates is enough at these ranges), id prediction and
// packet-order attacks, face-place, damage sync, base-place (mining obsidian
// first), extrapolated target bounds, anti-quiver/portal/echest-farm, the swap
// modes beyond a silent restore, multipoint visibility sampling, and the whole
// render group. `Instant` here is one tick of latency, not a packet hook: the
// crystal list is refreshed when the spawn arrives, so a new id is attacked on
// the first tick it is visible. There is no silent rotation in Horizon, so
// `Rotate` moves the real camera and defaults off.
use core::cell::RefCell;

use horizon_client::items::ItemStack;
use horizon_protocol::packets::play::{self, Serverbound as S};

use crate::game::{Game, clickgui as m};

/// `EndCrystal`'s explosion power: radius 6, so the damage formula's diameter
/// is 12.
const CRYSTAL_RADIUS: f64 = 6.0;
/// The most candidate bases whose blast is actually scored in one cycle. The
/// cheap filters run first, so this only bites on a large obsidian floor.
const SCAN_CAP: usize = 160;
const MAIN: i32 = 0;
const UNPREDICTED: i32 = 0;
/// `Direction.UP`.
const UP: i32 = 1;
/// Ticks before the same crystal may be attacked again. The references keep a
/// retry timer for this; without it a crystal that survives the first hit (out
/// of the server's reach, or invulnerable to this client's view) would take an
/// attack packet every tick.
const ATTACK_RETRY_TICKS: u64 = 4;
/// Ticks a base stays claimed after a placement, until the crystal it should
/// hold appears. Without this the room looks empty for the latency window and
/// the same base is placed on again every tick.
const PENDING_TICKS: u64 = 20;
/// Ticks between `Debug` chat lines.
const REPORT_TICKS: u64 = 20;
const OBSIDIAN: &str = "minecraft:obsidian";
const BEDROCK: &str = "minecraft:bedrock";
const END_CRYSTAL: &str = "minecraft:end_crystal";
const PLAYER: &str = "minecraft:player";
const MANNEQUIN: &str = "minecraft:mannequin";

#[derive(Default)]
struct State {
    place_wait: u32,
    break_wait: u32,
    /// Crystal ids seen last tick, for `Instant`.
    known: Vec<u32>,
    /// The crystal last attacked and the tick it happened, for the retry guard.
    last_attack: Option<(i32, u64)>,
    /// Bases a placement was sent for, and the tick it was sent.
    pending: Vec<([i32; 3], u64)>,
    /// The tick `Debug` last printed, so it speaks once a second.
    last_report: u64,
}

/// What the base scan saw, for `Debug`. Kept even when nothing is chosen, so
/// the chat line can say whether the gates or the geometry were the problem.
#[derive(Default, Clone, Copy)]
struct Report {
    scanned: usize,
    best_target: f32,
    best_self: f32,
    /// Where the best-scoring base was, so the chat line shows whether the
    /// platform cells were found at all.
    best_base: Option<[i32; 3]>,
    /// Why the best-scoring base did not become a placement.
    rejected: Rejected,
    /// Cells the scan cube dropped before scoring, by reason.
    not_base: usize,
    no_air: usize,
    room: usize,
    far: usize,
    claimed: usize,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Rejected {
    #[default]
    None,
    /// Below `Min damage`.
    Weak,
    /// Over `Max self damage` and not an acceptable trade.
    SelfDamage,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// One scored blast: the base a crystal would stand on, and what it would do.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Blast {
    base: [i32; 3],
    to_target: f32,
    to_self: f32,
}

pub(crate) fn tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let Some(config) = m::crystal_aura() else {
        STATE.with(|state| *state.borrow_mut() = State::default());
        return;
    };
    if !ready(game) {
        STATE.with(|state| *state.borrow_mut() = State::default());
        return;
    }
    let mut state = STATE.with(|state| std::mem::take(&mut *state.borrow_mut()));
    run(game, &config, &mut state);
    STATE.with(|slot| *slot.borrow_mut() = state);
}

fn ready(game: &Game<'_>) -> bool {
    game.player.is_some()
        && game.remote.is_some()
        && game.carried.is_none()
        && !game.spectator()
        && game.native_playing()
}

fn run(game: &mut Game<'_>, config: &m::CrystalAuraConfig, state: &mut State) {
    if state.place_wait > 0 {
        state.place_wait -= 1;
    }
    if state.break_wait > 0 {
        state.break_wait -= 1;
    }
    let debug = config.debug && game.ticks.saturating_sub(state.last_report) >= REPORT_TICKS;
    if debug {
        state.last_report = game.ticks;
    }
    let candidates = targets(game, config);
    if candidates.is_empty() {
        if debug {
            let line = format!("CrystalAura: no target ({} mobs)", game.mobs.len());
            game.system_message(horizon_client::text::Text::literal(line));
        }
        state.known.clear();
        return;
    };
    let Some(eye) = game.eye_position() else {
        return;
    };

    // Breaking first: a crystal already standing next to the target is damage
    // available now, and killing it frees the base for the placement below.
    let fresh = state.known.clone();
    if config.breaking && (state.break_wait == 0 || config.instant) {
        for &index in &candidates {
            let Some(blast) = best_crystal(game, config, index, eye) else {
                continue;
            };
            let new_id = !fresh.contains(&(blast.0 as u32));
            let retrying = state.last_attack.is_some_and(|(id, tick)| {
                id == blast.0 && game.ticks.saturating_sub(tick) < ATTACK_RETRY_TICKS
            });
            if !retrying && (state.break_wait == 0 || (config.instant && new_id)) {
                attack(game, config, blast.0, eye);
                state.last_attack = Some((blast.0, game.ticks));
                state.break_wait = config.break_delay;
            }
            break;
        }
    }

    // A claimed base is released once its crystal exists or a second passes.
    state.pending.retain(|(base, tick)| {
        game.ticks.saturating_sub(*tick) < PENDING_TICKS
            && !room_occupied(game, [base[0], base[1] + 1, base[2]])
    });

    // Every target in range, nearest first. One unplaceable target must not
    // stop the module from trying the next: a player standing in a hole can
    // leave no free base around them while the one beside them has several.
    let mut scanned = 0usize;
    let mut best_target = 0.0f32;
    let mut best_self = 0.0f32;
    let mut best_at: Option<[i32; 3]> = None;
    let mut rejected = Rejected::None;
    let mut drops = Report::default();
    let mut placed = false;
    // Only report when a scan actually ran. With a non-zero `Place delay` most
    // ticks skip it, and printing then would claim `scanned 0` on a tick that
    // never looked — sending the reader after a geometry bug that is not there.
    let mut scanned_now = false;
    if config.place && state.place_wait == 0 {
        scanned_now = true;
        for &index in &candidates {
            let (chosen, report) = best_base(game, config, index, eye, &state.pending);
            scanned += report.scanned;
            drops.not_base += report.not_base;
            drops.no_air += report.no_air;
            drops.room += report.room;
            drops.far += report.far;
            drops.claimed += report.claimed;
            if report.best_target > best_target {
                best_target = report.best_target;
                best_self = report.best_self;
                best_at = report.best_base;
                rejected = report.rejected;
            }
            if let Some(blast) = chosen
                && place(game, config, &blast, eye)
            {
                state.pending.push((blast.base, game.ticks));
                state.place_wait = config.place_delay;
                placed = true;
                break;
            }
        }
    }
    if debug && scanned_now && !placed {
        let why = match rejected {
            Rejected::None => "none",
            Rejected::Weak => "weak",
            Rejected::SelfDamage => "self",
        };
        let at = match best_at {
            Some([x, y, z]) => format!("@{x},{y},{z}"),
            None => "@-".to_owned(),
        };
        let line = format!(
            "CrystalAura: {} tgt scanned {scanned} best {best_target:.1}/{best_self:.1} {at} why {why} \
             drop nb{} na{} rm{} far{} cl{} gates {:.0}/{:.0} hp {:.0}",
            candidates.len(),
            drops.not_base,
            drops.no_air,
            drops.room,
            drops.far,
            drops.claimed,
            config.min_damage,
            config.max_self_damage,
            my_health(game),
        );
        game.system_message(horizon_client::text::Text::literal(line));
    }

    state.known.clear();
    state.known.extend(game.crystals.iter().map(|crystal| crystal.id));
}

/// Every living player in range, skipping friends, nearest first.
fn targets(game: &Game<'_>, config: &m::CrystalAuraConfig) -> Vec<usize> {
    let own = game.remote.as_ref().map_or(-1, |r| r.entity_id);
    let friends = m::friend_names();
    let friend_ids: Vec<u32> = game
        .remote_player_names()
        .iter()
        .filter(|(_, name)| m::is_friend(&friends, name))
        .map(|(id, _)| *id)
        .collect();
    let reach = f64::from(config.target_range);
    let reach_sq = reach * reach;
    let mut found: Vec<(usize, f64)> = Vec::new();
    for (index, mob) in game.mobs.iter().enumerate() {
        if mob.id as i32 == own
            || mob.death_time > 0
            || friend_ids.contains(&mob.id)
            || !(&*mob.kind.id == PLAYER || &*mob.kind.id == MANNEQUIN)
        {
            continue;
        }
        let distance = squared_to_player(game, mob);
        if distance > reach_sq {
            continue;
        }
        found.push((index, distance));
    }
    found.sort_by(|a, b| a.1.total_cmp(&b.1));
    found.into_iter().map(|(index, _)| index).collect()
}

/// Distance from the player, squared.
fn squared_to_player(game: &Game<'_>, mob: &horizon_client::mobs::Mob) -> f64 {
    let Some(player) = game.player.as_ref() else {
        return f64::INFINITY;
    };
    let d = [
        mob.position.x - player.position.x,
        mob.position.y - player.position.y,
        mob.position.z - player.position.z,
    ];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

/// The crystal whose blast hurts the target most, within both damage gates and
/// `break_range` of the eye.
fn best_crystal(
    game: &Game<'_>,
    config: &m::CrystalAuraConfig,
    index: usize,
    eye: [f64; 3],
) -> Option<(i32, f32)> {
    let target = game.mobs.get(index)?;
    let mine = self_feet(game)?;
    let health = my_health(game);
    let mut best: Option<(i32, f32)> = None;
    for crystal in &game.crystals {
        if crystal.id == 0 || crystal.invulnerable {
            continue;
        }
        let center = [crystal.position.x, crystal.position.y, crystal.position.z];
        if distance(eye, center) > f64::from(config.break_range) {
            continue;
        }
        let to_target = blast_damage(game, center, target);
        if to_target < config.min_damage {
            continue;
        }
        let to_self = blast_damage_player(game, center, mine);
        if !self_allowed(config, to_target, to_self, health) {
            continue;
        }
        if best.is_none_or(|(_, damage)| to_target > damage) {
            best = Some((crystal.id as i32, to_target));
        }
    }
    best
}

/// The base whose crystal would hurt the target most, within both damage gates
/// and `place_range` of the eye.
fn best_base(
    game: &Game<'_>,
    config: &m::CrystalAuraConfig,
    index: usize,
    eye: [f64; 3],
    claimed: &[([i32; 3], u64)],
) -> (Option<Blast>, Report) {
    let mut report = Report::default();
    let (Some(target), Some(mine)) = (game.mobs.get(index), self_feet(game)) else {
        return (None, report);
    };
    let health = my_health(game);
    let radius = config.place_range.ceil() as i32;
    // Around the eye, not the target: a base further than `place_range` from
    // the eye can never be placed on, so a cube centred on a distant target
    // would spend its whole budget scoring cells that cannot be used.
    let origin = [
        eye[0].floor() as i32,
        eye[1].floor() as i32,
        eye[2].floor() as i32,
    ];
    // Collect the usable bases first, then score the ones nearest the target.
    // Scoring in cube order let `SCAN_CAP` cut an arbitrary x-slab off the end,
    // so which bases were even considered depended on where the player stood.
    // The name check in `crystal_fits` short-circuits on air, which is most of
    // the cube, so the collection is one block read per cell.
    let mut bases: Vec<[i32; 3]> = Vec::new();
    for dx in -radius..=radius {
        for dy in -radius..=radius {
            for dz in -radius..=radius {
                let base = [origin[0] + dx, origin[1] + dy, origin[2] + dz];
                if claimed.iter().any(|(b, _)| *b == base) {
                    report.claimed += 1;
                    continue;
                }
                match crystal_fits(game, base) {
                    Fits::Ok => {}
                    Fits::NotBase => {
                        report.not_base += 1;
                        continue;
                    }
                    Fits::NoAir => {
                        report.no_air += 1;
                        continue;
                    }
                    Fits::Room => {
                        report.room += 1;
                        continue;
                    }
                }
                let center = [
                    f64::from(base[0]) + 0.5,
                    f64::from(base[1]) + 1.0,
                    f64::from(base[2]) + 0.5,
                ];
                if distance(eye, center) > f64::from(config.place_range) {
                    report.far += 1;
                    continue;
                }
                bases.push(base);
            }
        }
    }
    let target_at = [target.position.x, target.position.y, target.position.z];
    bases.sort_by(|a, b| {
        let ca = [
            f64::from(a[0]) + 0.5,
            f64::from(a[1]) + 1.0,
            f64::from(a[2]) + 0.5,
        ];
        let cb = [
            f64::from(b[0]) + 0.5,
            f64::from(b[1]) + 1.0,
            f64::from(b[2]) + 0.5,
        ];
        distance(ca, target_at).total_cmp(&distance(cb, target_at))
    });

    let mut best: Option<Blast> = None;
    for base in bases.into_iter().take(SCAN_CAP) {
        let center = [
            f64::from(base[0]) + 0.5,
            f64::from(base[1]) + 1.0,
            f64::from(base[2]) + 0.5,
        ];
        report.scanned += 1;
        let to_target = blast_damage(game, center, target);
        if to_target > report.best_target {
            report.best_target = to_target;
            report.best_self = blast_damage_player(game, center, mine);
            report.best_base = Some(base);
        }
        if to_target < config.min_damage {
            report.rejected = Rejected::Weak;
            continue;
        }
        let to_self = blast_damage_player(game, center, mine);
        if !self_allowed(config, to_target, to_self, health) {
            report.rejected = Rejected::SelfDamage;
            continue;
        }
        if best.is_none_or(|b| to_target > b.to_target) {
            best = Some(Blast {
                base,
                to_target,
                to_self,
            });
        }
    }
    (best, report)
}

/// Why a cell is not a usable base. Reported by `Debug` so one chat line says
/// which of the client's three rules the scan is failing on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fits {
    Ok,
    /// Not obsidian or bedrock.
    NotBase,
    /// The block above is not air.
    NoAir,
    /// An entity stands in the two-block room.
    Room,
}

/// The client's own `place_end_crystal` rule: obsidian or bedrock, air above,
/// and no entity in the two-block room.
fn crystal_fits(game: &Game<'_>, base: [i32; 3]) -> Fits {
    let facts = game.res.facts();
    let below = game.world.block(base[0], base[1], base[2]);
    // Borrowed, not cloned: this runs for every cell of the scan cube.
    let name = &*facts.get(below).name;
    if !(name == OBSIDIAN || name == BEDROCK) {
        return Fits::NotBase;
    }
    let above = [base[0], base[1] + 1, base[2]];
    if !facts.is_air(game.world.block(above[0], above[1], above[2])) {
        return Fits::NoAir;
    }
    if room_occupied(game, above) {
        return Fits::Room;
    }
    Fits::Ok
}

/// Any mob or crystal overlapping `[x, y, z] .. [x+1, y+2, z+1]`.
fn room_occupied(game: &Game<'_>, above: [i32; 3]) -> bool {
    let (x, y, z) = (
        f64::from(above[0]),
        f64::from(above[1]),
        f64::from(above[2]),
    );
    let overlaps = |px: f64, py: f64, pz: f64, w: f64, h: f64| {
        px + w > x && px - w < x + 1.0 && py + h > y && py < y + 2.0 && pz + w > z && pz - w < z + 1.0
    };
    if let Some(player) = game.player.as_ref()
        && overlaps(
            player.position.x,
            player.position.y,
            player.position.z,
            f64::from(player.dimensions.width) / 2.0,
            f64::from(player.dimensions.height),
        )
    {
        return true;
    }
    for mob in &game.mobs {
        if mob.death_time > 0 {
            continue;
        }
        if overlaps(
            mob.position.x,
            mob.position.y,
            mob.position.z,
            f64::from(mob.kind.width) / 2.0,
            f64::from(mob.kind.height),
        ) {
            return true;
        }
    }
    game.crystals.iter().any(|crystal| {
        overlaps(crystal.position.x, crystal.position.y, crystal.position.z, 1.0, 2.0)
    })
}

/// Explosion damage to a mob, using the client's own formula.
fn blast_damage(game: &Game<'_>, center: [f64; 3], target: &horizon_client::mobs::Mob) -> f32 {
    let feet = [target.position.x, target.position.y, target.position.z];
    let half = f64::from(target.kind.width) / 2.0;
    let height = f64::from(target.kind.height);
    blast(game, center, feet, half, height)
}

/// The player's health plus absorption, for the "do not kill yourself" guard.
fn my_health(game: &Game<'_>) -> f32 {
    game.player
        .as_ref()
        .map_or(0.0, |player| player.health.health() + player.health.absorption())
}

/// Whether a blast may be taken. The cap alone is not enough: on flat ground
/// the target and the player stand the same distance from the crystal, so they
/// take the same damage and an absolute cap rejects every placement there.
/// A trade is allowed when it hurts the target at least as much and would not
/// kill the player.
fn self_allowed(
    config: &m::CrystalAuraConfig,
    to_target: f32,
    to_self: f32,
    health: f32,
) -> bool {
    if to_self <= config.max_self_damage {
        return true;
    }
    config.allow_trades && to_target >= to_self && to_self < health
}

/// The player's feet, as a plain array so no borrow of `Game` is held.
fn self_feet(game: &Game<'_>) -> Option<[f64; 3]> {
    let player = game.player.as_ref()?;
    Some([player.position.x, player.position.y, player.position.z])
}

fn blast_damage_player(game: &Game<'_>, center: [f64; 3], feet: [f64; 3]) -> f32 {
    let Some(player) = game.player.as_ref() else {
        return 0.0;
    };
    blast(
        game,
        center,
        feet,
        f64::from(player.dimensions.width) / 2.0,
        f64::from(player.dimensions.height),
    )
}

/// `Explosion.explode`'s per-entity damage, with `seen_percent` for exposure.
/// Split out so the polynomial can be pinned by a test.
fn blast(game: &Game<'_>, center: [f64; 3], feet: [f64; 3], half: f64, height: f64) -> f32 {
    let diameter = CRYSTAL_RADIUS * 2.0;
    let dist = distance(feet, center) / diameter;
    if dist > 1.0 {
        return 0.0;
    }
    let exposure = f64::from(game.seen_percent(
        center,
        [feet[0] - half, feet[1], feet[2] - half],
        [feet[0] + half, feet[1] + height, feet[2] + half],
    ));
    blast_amount(dist, exposure)
}

/// The polynomial on its own, so a test can check it without a world.
fn blast_amount(dist: f64, exposure: f64) -> f32 {
    let diameter = CRYSTAL_RADIUS * 2.0;
    let pow = (1.0 - dist) * exposure;
    ((pow * pow + pow) / 2.0 * 7.0 * diameter + 1.0) as f32
}

fn attack(game: &mut Game<'_>, config: &m::CrystalAuraConfig, id: i32, eye: [f64; 3]) {
    if config.rotate
        && let Some(crystal) = game.crystals.iter().find(|c| c.id as i32 == id)
    {
        turn(game, eye, [crystal.position.x, crystal.position.y, crystal.position.z]);
    }
    game.send(S::Attack(play::Attack { id }));
}

fn place(
    game: &mut Game<'_>,
    config: &m::CrystalAuraConfig,
    blast: &Blast,
    eye: [f64; 3],
) -> bool {
    let Some(slot) = crystal_slot(game) else {
        return false;
    };
    let previous = game.selected;
    set_slot(game, slot);
    if config.rotate {
        turn(
            game,
            eye,
            [
                f64::from(blast.base[0]) + 0.5,
                f64::from(blast.base[1]) + 1.0,
                f64::from(blast.base[2]) + 0.5,
            ],
        );
    }
    game.send(S::UseItemOn(play::UseItemOn {
        hand: MAIN,
        pos: blast.base,
        face: UP,
        cursor: [0.5, 1.0, 0.5],
        inside: false,
        world_border: false,
        sequence: UNPREDICTED,
    }));
    if config.silent {
        set_slot(game, previous);
    }
    true
}

/// The hotbar slot with the most end crystals.
fn crystal_slot(game: &Game<'_>) -> Option<usize> {
    let mut best: Option<(usize, u32)> = None;
    for slot in 0..9 {
        let Some(stack) = game.inventory.get(slot).and_then(Option::as_ref) else {
            continue;
        };
        if !usable(stack) {
            continue;
        }
        if best.is_none_or(|(_, count)| stack.count > count) {
            best = Some((slot, stack.count));
        }
    }
    best.map(|(slot, _)| slot)
}

fn usable(stack: &ItemStack) -> bool {
    stack.count > 0 && &*stack.item == END_CRYSTAL
}

/// `MultiPlayerGameMode.ensureHasSentCarriedItem` reads this cache, so all
/// three have to move together.
fn set_slot(game: &mut Game<'_>, slot: usize) {
    if game.selected == slot {
        return;
    }
    game.selected = slot;
    game.send(S::SetCarriedItem(play::SetCarriedItem {
        slot: slot as i16,
    }));
    if let Some(remote) = game.remote.as_mut() {
        remote.carried.set(Some(slot));
    }
}

/// Yaw and pitch toward a point, in Horizon's convention.
fn look(from: [f64; 3], to: [f64; 3]) -> (f32, f32) {
    let dx = to[0] - from[0];
    let dy = to[1] - from[1];
    let dz = to[2] - from[2];
    (
        (-dx).atan2(dz).to_degrees() as f32,
        (-dy).atan2(dx.hypot(dz)).to_degrees() as f32,
    )
}

fn turn(game: &mut Game<'_>, eye: [f64; 3], at: [f64; 3]) {
    let (yaw, pitch) = look(eye, at);
    game.yaw = yaw;
    game.pitch = pitch.clamp(-90.0, 90.0);
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::{blast_amount, look, self_allowed};
    use crate::game::clickgui::CrystalAuraConfig;

    #[test]
    fn the_damage_polynomial_matches_the_client() {
        // Full exposure at the centre: pow = 1, so (1 + 1) / 2 * 7 * 12 + 1.
        assert!((blast_amount(0.0, 1.0) - 85.0).abs() < 1e-3);
        // Three blocks out, half exposed.
        let expected = {
            let pow = (1.0 - 3.0 / 12.0) * 0.5;
            ((pow * pow + pow) / 2.0 * 7.0 * 12.0 + 1.0) as f32
        };
        assert!((blast_amount(3.0 / 12.0, 0.5) - expected).abs() < 1e-3);
    }

    #[test]
    fn no_exposure_means_no_damage_from_the_blast() {
        // Fully occluded still leaves the flat +1 the formula adds.
        assert!((blast_amount(0.5, 0.0) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn a_trade_is_taken_only_when_it_is_not_lethal() {
        // Pinned to a cap of 10 so the rule is tested, not the defaults.
        let config = CrystalAuraConfig {
            max_self_damage: 10.0,
            allow_trades: true,
            ..CrystalAuraConfig::default()
        };
        // Inside the cap, always fine.
        assert!(self_allowed(&config, 20.0, 8.0, 20.0));
        // Over the cap, hurting the target at least as much, survivable.
        assert!(self_allowed(&config, 18.0, 18.0, 20.0));
        // Over the cap and lethal to the player: never.
        assert!(!self_allowed(&config, 21.0, 21.0, 20.0));
        // Over the cap and hurting the player more than the target: no.
        assert!(!self_allowed(&config, 10.0, 14.0, 20.0));
        // With trades off, the cap is the cap.
        let strict = CrystalAuraConfig {
            max_self_damage: 10.0,
            allow_trades: false,
            ..CrystalAuraConfig::default()
        };
        assert!(!self_allowed(&strict, 18.0, 18.0, 20.0));
    }

    #[test]
    fn looking_at_a_point_uses_the_game_convention() {
        let (yaw, pitch) = look([0.0, 1.0, 0.0], [0.0, 1.0, 3.0]);
        assert!(yaw.abs() < 1e-4 && pitch.abs() < 1e-4);
        let (yaw, _) = look([0.0, 0.0, 0.0], [3.0, 0.0, 0.0]);
        assert!((yaw + 90.0).abs() < 1e-4);
    }
}
