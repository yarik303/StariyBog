// HoleFill: puts a block into the sealed holes near the player so nobody can
// use them for cover.
//
// The references differ in how they *choose* holes — earthhack's hole manager,
// ThunderHack's HoleUtility, Zio's plain/smart policies, LiquidBounce's
// BlockPlacer, Shoreline's proximity check — but they all come down to the
// same three steps: find a 1x1 / 1x2 / 2x2 cell sealed by bedrock or obsidian,
// decide whether it is worth filling, and right-click its floor. This module
// is those three steps with the reachable filters from all eight references.
//
// The hole scan is HoleESP's own `scan_holes` (1x1 and 1x2) plus a 2x2 scan
// added to `runtime.rs`. Both read only `game.world.block` and the block facts,
// so no prepared resources or engine state are needed.
//
// Placement is one `UseItemOn` per cell, aimed at the *floor* with the UP face:
// the server puts the new block at `floor + UP`, which is exactly the hole's
// feet cell. That is a packet-only placement — the client does not predict it,
// so a `pending` list remembers each cell until the world reports a block
// there (or 20 ticks pass), otherwise the next cycle would place it again.
//
// Hotbar switching follows Scaffold's `set_slot`: set `Game::selected`, send
// `SetCarriedItem` and tell `Remote::carried`. Keeping those in sync matters —
// `Game::send` runs `ensureHasSentCarriedItem` before every `UseItemOn`, and a
// stale `carried` would insert the player's own slot between the switch and
// the placement. `Silent` puts the old slot back in the same tick.
//
// Not ported: the references' offhand-mode manager, their block-place
// animations, per-block-update sequential placement, jump disable, and the
// client-side rotation request. This module never turns the player's view;
// the placement packet names the block and face directly.
use core::cell::RefCell;

use horizon_client::items::ItemStack;
use horizon_protocol::packets::play::{self, Serverbound as S};

use crate::game::{Game, clickgui as m};

/// A survival player's block interaction reach. The server checks the eye's
/// distance to the block being clicked, so a hole at the edge of the scan can
/// be out of reach even though the scan found it.
const REACH: f64 = 4.5;
/// `Direction.UP`: the floor's top face. The placed block lands one cell up.
const UP: i32 = 1;
const MAIN: i32 = 0;
/// No client-side prediction to acknowledge for a packet-only placement.
const UNPREDICTED: i32 = 0;
/// How long a sent placement waits for the server's block update.
const PENDING_TICKS: u64 = 20;
/// Within 60 degrees of the direction to the hole.
const TOWARD: f64 = 0.5;

const PLAYER: &str = "minecraft:player";
const BEDROCK: &str = "minecraft:bedrock";
const OBSIDIAN: &str = "minecraft:obsidian";
const CRYING_OBSIDIAN: &str = "minecraft:crying_obsidian";
const ENDER_CHEST: &str = "minecraft:ender_chest";

#[derive(Default)]
struct State {
    /// Ticks left before the next placement cycle.
    wait: u32,
    /// Cells a placement was sent for, and the tick it was sent. Dropped as
    /// soon as the world shows a block there, or after `PENDING_TICKS`.
    pending: Vec<([i32; 3], u64)>,
    /// Whether this activation has filled anything yet. `Auto disable` needs
    /// it: an empty scan in open ground is not "all holes are filled".
    placed: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// A nearby player and the horizontal velocity its last tick showed.
#[derive(Clone, Copy, Debug)]
struct Enemy {
    at: [f64; 3],
    vel: [f64; 2],
}

/// The click that fills `cell`: its floor, from above. Split out from the game
/// so the geometry can be pinned by a test.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Placement {
    pos: [i32; 3],
    face: i32,
    cursor: [f32; 3],
}

fn placement(cell: [i32; 3]) -> Placement {
    Placement {
        pos: [cell[0], cell[1] - 1, cell[2]],
        face: UP,
        cursor: [0.5, 1.0, 0.5],
    }
}

pub(crate) fn tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let config = if game.native_on(m::ID) {
        m::holefill()
    } else {
        None
    };
    if let Some(config) = config
        && ready(game)
    {
        STATE.with(|state| run(game, &config, &mut state.borrow_mut()));
    } else {
        STATE.with(|state| *state.borrow_mut() = State::default());
    }
}

fn ready(game: &Game<'_>) -> bool {
    game.player.is_some()
        && game.remote.is_some()
        && game.carried.is_none()
        && !game.spectator()
        && game.native_playing()
}

/// What a block is to the hole scan: the same classification HoleESP uses.
fn material(game: &Game<'_>, x: i32, y: i32, z: i32) -> m::HoleMaterial {
    let state = game.world.block(x, y, z);
    let facts = game.res.facts();
    if facts.is_air(state) {
        return m::HoleMaterial::Air;
    }
    let block = facts.get(state);
    if block.name.as_ref() == BEDROCK {
        m::HoleMaterial::Bedrock
    } else if block.explosion_resistance >= 600.0 {
        m::HoleMaterial::Obsidian
    } else if block.full_collision {
        m::HoleMaterial::Solid
    } else {
        m::HoleMaterial::Other
    }
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn centre(cell: [i32; 3]) -> [f64; 3] {
    [
        f64::from(cell[0]) + 0.5,
        f64::from(cell[1]),
        f64::from(cell[2]) + 0.5,
    ]
}

fn run(game: &mut Game<'_>, config: &m::HoleFillConfig, state: &mut State) {
    if state.wait > 0 {
        state.wait -= 1;
        return;
    }
    let Some(player) = game.player.as_ref() else {
        return;
    };
    let at = [player.position.x, player.position.y, player.position.z];
    let feet = [
        at[0].floor() as i32,
        at[1].floor() as i32,
        at[2].floor() as i32,
    ];
    let Some(eye) = game.eye_position() else {
        return;
    };
    let now = game.ticks;
    // Every other player, unfiltered: `occupied` has to see someone standing
    // in a cell even when they are outside `Target range`, or a small target
    // range would let the module trap them.
    let all = enemies(game);
    let nearby: Vec<Enemy> = all
        .iter()
        .copied()
        .filter(|enemy| distance(at, enemy.at) <= f64::from(config.target_range))
        .collect();

    // The scan borrows `game` immutably, so all of it stays in one scope and
    // only the resulting cells leave it; the placement needs `&mut Game`.
    let (cells, empty) = {
        let block = |x, y, z| material(game, x, y, z);
        state.pending.retain(|(cell, tick)| {
            now.saturating_sub(*tick) < PENDING_TICKS
                && block(cell[0], cell[1], cell[2]) == m::HoleMaterial::Air
        });

        let mut scan = m::HoleEspConfig::default();
        scan.radius = config.range;
        scan.vertical = config.vertical;
        scan.doubles = config.doubles;
        scan.ignore_own = !config.self_fill;
        let mut holes = m::scan_holes(feet, &scan, &block);
        if config.quads {
            holes.extend(m::scan_quads(
                feet,
                config.range,
                config.vertical,
                scan.ignore_own,
                &block,
            ));
        }

        let guard_self = config.min_self > 0.0 && !own_hole(&block, feet);
        let mut cells: Vec<[i32; 3]> = Vec::new();
        for hole in &holes {
            let middle = [
                f64::from(hole.x) + f64::from(hole.width) * 0.5,
                f64::from(hole.y),
                f64::from(hole.z) + f64::from(hole.depth) * 0.5,
            ];
            if guard_self {
                let (dx, dz) = (middle[0] - at[0], middle[2] - at[2]);
                if (dx * dx + dz * dz).sqrt() < f64::from(config.min_self) {
                    continue;
                }
            }
            if config.smart && !moving_toward(&nearby, middle) {
                continue;
            }
            for dx in 0..hole.width {
                for dz in 0..hole.depth {
                    let cell = [hole.x + dx, hole.y, hole.z + dz];
                    if state.pending.iter().any(|(c, _)| *c == cell)
                        || occupied(&all, cell)
                        || distance(eye, centre(cell)) > REACH
                    {
                        continue;
                    }
                    cells.push(cell);
                }
            }
        }
        cells.sort_by(|a, b| distance(eye, centre(*a)).total_cmp(&distance(eye, centre(*b))));
        cells.truncate(config.per_tick as usize);
        (cells, holes.is_empty())
    };

    // Nothing sealed is left in range. Only turn off if this activation has
    // actually filled something: enabling in open ground also finds no holes,
    // and turning itself off there would read as the module being broken.
    if empty {
        if config.auto_disable && state.placed {
            disable();
        }
        state.wait = config.delay;
        return;
    }
    if config.require_target && nearby.is_empty() {
        state.wait = config.delay;
        return;
    }
    if cells.is_empty() {
        state.wait = config.delay;
        return;
    }
    let Some(slot) = block_slot(game, config.block) else {
        state.wait = config.delay;
        return;
    };
    place(game, state, config, &cells, slot);
    state.placed = true;
    state.wait = config.delay;
}

/// Every other player the client can see, with the velocity its last tick
/// showed. Not range-filtered: the occupancy check needs all of them.
fn enemies(game: &Game<'_>) -> Vec<Enemy> {
    let own = game.remote.as_ref().map_or(-1, |r| r.entity_id);
    game.mobs
        .iter()
        .filter(|mob| mob.id as i32 != own && &*mob.kind.id == PLAYER && mob.death_time == 0)
        .map(|mob| Enemy {
            at: [mob.position.x, mob.position.y, mob.position.z],
            vel: [
                mob.position.x - mob.previous.x,
                mob.position.z - mob.previous.z,
            ],
        })
        .collect()
}

/// Whether any nearby player is walking toward the hole. A stationary player
/// counts as not moving toward anything, which is what the smart policies
/// want: fill the hole they are heading for, not the one they stand in.
fn moving_toward(enemies: &[Enemy], middle: [f64; 3]) -> bool {
    enemies.iter().any(|enemy| {
        let speed = (enemy.vel[0] * enemy.vel[0] + enemy.vel[1] * enemy.vel[1]).sqrt();
        if speed < 0.001 {
            return false;
        }
        let (dx, dz) = (middle[0] - enemy.at[0], middle[2] - enemy.at[2]);
        let length = (dx * dx + dz * dz).sqrt().max(0.001);
        (enemy.vel[0] * dx + enemy.vel[1] * dz) / (speed * length) > TOWARD
    })
}

/// Never place a block into a cell a player is standing in. The references
/// skip those holes too — a block in an occupied cell is refused, and on a
/// server that accepts it, it traps someone in place.
fn occupied(enemies: &[Enemy], cell: [i32; 3]) -> bool {
    enemies.iter().any(|enemy| {
        (f64::from(cell[0]) - 0.3..=f64::from(cell[0]) + 1.3).contains(&enemy.at[0])
            && (f64::from(cell[2]) - 0.3..=f64::from(cell[2]) + 1.3).contains(&enemy.at[2])
            && (f64::from(cell[1]) - 1.0..=f64::from(cell[1]) + 2.0).contains(&enemy.at[1])
    })
}

/// Whether the player's own feet cell is a sealed hole. `Min self` stops
/// protecting holes once the player is already in one.
fn own_hole(block: &impl Fn(i32, i32, i32) -> m::HoleMaterial, feet: [i32; 3]) -> bool {
    let sealed = |x, y, z| {
        matches!(
            block(x, y, z),
            m::HoleMaterial::Bedrock | m::HoleMaterial::Obsidian
        )
    };
    block(feet[0], feet[1], feet[2]) == m::HoleMaterial::Air
        && block(feet[0], feet[1] + 1, feet[2]) == m::HoleMaterial::Air
        && sealed(feet[0], feet[1] - 1, feet[2])
        && [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .all(|(dx, dz)| sealed(feet[0] + dx, feet[1], feet[2] + dz))
}

/// Whether a block item's placed state is one of the configured blocks.
fn block_allowed(name: &str, choice: m::HoleFillBlock) -> bool {
    match choice {
        m::HoleFillBlock::Obsidian => name == OBSIDIAN || name == CRYING_OBSIDIAN,
        m::HoleFillBlock::EnderChest => name == ENDER_CHEST,
        m::HoleFillBlock::Any => true,
    }
}

fn usable(game: &Game<'_>, stack: &ItemStack, choice: m::HoleFillBlock) -> bool {
    if stack.count == 0 {
        return false;
    }
    let Some(info) = game.items().get(&stack.item) else {
        return false;
    };
    let Some(state) = info.block else {
        return false;
    };
    block_allowed(game.res.facts().get(state).name.as_ref(), choice)
}

/// The hotbar slot with the biggest usable stack.
fn block_slot(game: &Game<'_>, choice: m::HoleFillBlock) -> Option<usize> {
    let mut best: Option<(usize, u32)> = None;
    for slot in 0..9 {
        let Some(stack) = game.inventory.get(slot).and_then(Option::as_ref) else {
            continue;
        };
        if !usable(game, stack, choice) {
            continue;
        }
        if best.is_none_or(|(_, count)| stack.count > count) {
            best = Some((slot, stack.count));
        }
    }
    best.map(|(slot, _)| slot)
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

fn place(
    game: &mut Game<'_>,
    state: &mut State,
    config: &m::HoleFillConfig,
    cells: &[[i32; 3]],
    slot: usize,
) {
    let previous = game.selected;
    set_slot(game, slot);
    for &cell in cells {
        let click = placement(cell);
        game.send(S::UseItemOn(play::UseItemOn {
            hand: MAIN,
            pos: click.pos,
            face: click.face,
            cursor: click.cursor,
            inside: false,
            world_border: false,
            sequence: UNPREDICTED,
        }));
        state.pending.push((cell, game.ticks));
    }
    if config.silent {
        set_slot(game, previous);
    }
}

fn disable() {
    m::with(|client| {
        if let Some(module) = client
            .modules
            .iter_mut()
            .find(|module| module.id == m::HOLEFILL)
        {
            module.enabled = false;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{
        Enemy, Placement, block_allowed, moving_toward, occupied, placement,
    };
    use crate::game::clickgui::HoleFillBlock;

    fn enemy(x: f64, y: f64, z: f64, vx: f64, vz: f64) -> Enemy {
        Enemy {
            at: [x, y, z],
            vel: [vx, vz],
        }
    }

    #[test]
    fn a_placement_clicks_the_floor_from_above() {
        assert_eq!(
            placement([3, 64, -2]),
            Placement {
                pos: [3, 63, -2],
                face: 1,
                cursor: [0.5, 1.0, 0.5],
            }
        );
        // `floor + UP` is the hole cell again: that is where the block lands.
        let click = placement([10, 70, 10]);
        assert_eq!(
            [click.pos[0], click.pos[1] + 1, click.pos[2]],
            [10, 70, 10]
        );
    }

    #[test]
    fn only_the_configured_blocks_are_usable() {
        assert!(block_allowed("minecraft:obsidian", HoleFillBlock::Obsidian));
        assert!(block_allowed(
            "minecraft:crying_obsidian",
            HoleFillBlock::Obsidian
        ));
        assert!(!block_allowed(
            "minecraft:ender_chest",
            HoleFillBlock::Obsidian
        ));
        assert!(block_allowed(
            "minecraft:ender_chest",
            HoleFillBlock::EnderChest
        ));
        assert!(block_allowed("minecraft:cobweb", HoleFillBlock::Any));
        assert!(!block_allowed("minecraft:cobweb", HoleFillBlock::Obsidian));
    }

    #[test]
    fn smart_only_fills_a_hole_someone_walks_toward() {
        let toward = [enemy(0.0, 64.0, 0.0, 0.0, 0.3)];
        assert!(moving_toward(&toward, [0.0, 64.0, 3.0]));
        assert!(!moving_toward(&toward, [0.0, 64.0, -3.0]));
        // Walking across it is not walking toward it.
        assert!(!moving_toward(
            &[enemy(0.0, 64.0, 0.0, 0.3, 0.0)],
            [0.0, 64.0, 3.0]
        ));
        assert!(!moving_toward(
            &[enemy(0.0, 64.0, 0.0, 0.0, 0.0)],
            [0.0, 64.0, 3.0]
        ));
        assert!(!moving_toward(&[], [0.0, 64.0, 3.0]));
    }

    #[test]
    fn an_occupied_cell_is_never_filled() {
        let cell = [5, 64, 5];
        assert!(occupied(&[enemy(5.5, 64.0, 5.5, 0.0, 0.0)], cell));
        assert!(!occupied(&[enemy(9.5, 64.0, 9.5, 0.0, 0.0)], cell));
        // A player far above or below is not in the cell.
        assert!(!occupied(&[enemy(5.5, 80.0, 5.5, 0.0, 0.0)], cell));
        assert!(!occupied(&[], cell));
    }
}
