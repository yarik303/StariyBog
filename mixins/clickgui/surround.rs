// Surround: walls the player's feet in with blocks.
//
// The four references are two different things. `SurroundPlanner` is crystal
// damage comparison (friend protection, anti-suicide, target priority) and
// belongs to an AutoCrystal, not here. `Surround` and `FeetPlaceModule` are the
// placement engines; `SurroundBlockInfo` is the useful piece from them: the set
// of cells to fill depends on the SHAPE of the hole you stand in (1x1, 2x1 or
// 2x2), so the perimeter is not always four blocks.
//
// This port uses `runtime.rs::scan_holes` and `scan_quads` — the same sealed-
// hole detector HoleESP and HoleFill use — to find which cells the player
// occupies, then takes the outer ring of that shape. Four placements for a
// single cell, six for a double, eight for a quad.
//
// Placing is one `UseItemOn` per cell. HoleFill can always click the floor,
// because a sealed hole has a solid floor by definition; a surround cell on
// open ground often does NOT. So each cell gets a support search over its six
// neighbours, and the click names the first solid face whose outward normal
// points back into the cell. The server places at `pos + face`, i.e. the cell.
//
// This is a packet-only placement: the client does not predict it, so a
// `pending` list remembers each cell until the world reports a block there (or
// 20 ticks pass), otherwise the next cycle would place the same block again.
// The hotbar switch is Scaffold's `set_slot` pattern, which keeps
// `Game::selected`, `SetCarriedItem` and `Remote::carried` in step.
//
// Not ported: the references' async/crystal-spawn reactions (they need a
// ClientboundAddEntity hook and a placement path that runs outside the tick),
// sequential placement on block-update events, firework/multi-task support,
// block-ignore policies, smart-face and instant modes, and centre force-packets.
use core::cell::RefCell;

use horizon_client::items::ItemStack;
use horizon_protocol::packets::play::{self, Serverbound as S};

use crate::game::{Game, clickgui as m};

/// A survival player's block interaction reach; the server checks the eye's
/// distance to the block we click.
const REACH: f64 = 4.5;
const MAIN: i32 = 0;
/// No client-side prediction to acknowledge for a packet-only placement.
const UNPREDICTED: i32 = 0;
const PENDING_TICKS: u64 = 20;

const BEDROCK: &str = "minecraft:bedrock";
const OBSIDIAN: &str = "minecraft:obsidian";
const CRYING_OBSIDIAN: &str = "minecraft:crying_obsidian";
const ENDER_CHEST: &str = "minecraft:ender_chest";

#[derive(Default)]
struct State {
    /// Ticks left before the next placement cycle.
    wait: u32,
    /// Cells a placement was sent for, and the tick it was sent.
    pending: Vec<([i32; 3], u64)>,
    /// Whether this activation has placed anything, for `Auto disable`.
    placed: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The click that fills `cell`: a solid neighbour and the face whose outward
/// normal points back into the cell.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Placement {
    pos: [i32; 3],
    face: i32,
    cursor: [f32; 3],
}

pub(crate) fn tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let config = if game.native_on(m::ID) {
        m::surround()
    } else {
        None
    };
    if let Some(config) = config
        && ready(game, &config)
    {
        STATE.with(|state| run(game, &config, &mut state.borrow_mut()));
    } else {
        STATE.with(|state| *state.borrow_mut() = State::default());
    }
}

fn ready(game: &Game<'_>, config: &m::SurroundConfig) -> bool {
    game.player.is_some()
        && game.remote.is_some()
        && game.carried.is_none()
        && !game.spectator()
        && game.native_playing()
        && (!config.on_ground
            || game
                .player
                .as_ref()
                .is_some_and(|player| player.ground.on_ground))
}

fn run(game: &mut Game<'_>, config: &m::SurroundConfig, state: &mut State) {
    if state.wait > 0 {
        state.wait -= 1;
        return;
    }
    // The references clamp the player into the middle 0.3..0.7 of their cell
    // before building, so the surround is not lopsided. A small enough move
    // that the tick's own interpolation draws it as a slide, not a snap.
    if config.center
        && let Some(player) = game.player.as_mut()
    {
        let (cx, cz) = (
            player.position.x.floor() + 0.5,
            player.position.z.floor() + 0.5,
        );
        player.position.x = player.position.x.clamp(cx - 0.2, cx + 0.2);
        player.position.z = player.position.z.clamp(cz - 0.2, cz + 0.2);
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
    let own = game.remote.as_ref().map_or(-1, |r| r.entity_id);

    // All the world reads stay in one scope, leaving only the placements for
    // `&mut Game` below.
    let (targets, remaining) = {
        let facts = game.res.facts();
        let state_of = |cell: [i32; 3]| game.world.block(cell[0], cell[1], cell[2]);
        let open = |cell: [i32; 3]| {
            let state = state_of(cell);
            facts.is_air(state) || facts.get(state).replaceable
        };
        let solid = |cell: [i32; 3]| {
            let state = state_of(cell);
            !facts.is_air(state) && !facts.get(state).replaceable && facts.get(state).full_collision
        };
        let hole_material = |x, y, z| material(game, x, y, z);

        state.pending.retain(|(cell, tick)| {
            now.saturating_sub(*tick) < PENDING_TICKS && open(*cell)
        });

        let cells = hole_cells(feet, &hole_material);
        let mut candidates = perimeter(&cells);
        if config.flatten {
            candidates.push([feet[0], feet[1] - 1, feet[2]]);
        }
        if config.cover {
            // A player is two blocks tall; the block above the head is +2.
            candidates.push([feet[0], feet[1] + 2, feet[2]]);
        }

        // Only cells we could actually place into count as work left. A cell
        // with no solid support (Cover on open ground, for instance) would
        // otherwise keep `remaining` above zero forever and stop `Auto
        // disable` from ever firing.
        let mut remaining = 0usize;
        let mut targets: Vec<([i32; 3], Placement)> = Vec::new();
        for cell in candidates {
            if !open(cell) {
                continue;
            }
            let Some(click) = placement_for(cell, &solid) else {
                continue;
            };
            if distance(eye, centre(click.pos)) > REACH {
                continue;
            }
            remaining += 1;
            if state.pending.iter().any(|(c, _)| *c == cell) || occupied(game, own, cell) {
                continue;
            }
            targets.push((cell, click));
        }
        targets.sort_by(|a, b| {
            distance(eye, centre(a.0)).total_cmp(&distance(eye, centre(b.0)))
        });
        targets.truncate(config.per_tick as usize);
        (targets, remaining)
    };

    // Only turn off after this activation actually built something: enabling
    // the module while already surrounded also finds no open cells.
    if remaining == 0 {
        if config.auto_disable && state.placed {
            disable();
        }
        state.wait = config.delay;
        return;
    }
    if targets.is_empty() {
        state.wait = config.delay;
        return;
    }
    let Some(slot) = block_slot(game, config.block) else {
        state.wait = config.delay;
        return;
    };
    place(game, state, config, &targets, slot);
    state.placed = true;
    state.wait = config.delay;
}

/// Which cells of a sealed hole the player occupies. Falls back to the single
/// feet cell on open ground, which is where Surround is most often used.
fn hole_cells(
    feet: [i32; 3],
    material: &impl Fn(i32, i32, i32) -> m::HoleMaterial,
) -> Vec<[i32; 3]> {
    let mut scan = m::HoleEspConfig::default();
    scan.radius = 3;
    scan.vertical = 1;
    scan.doubles = true;
    scan.ignore_own = false;
    let mut holes = m::scan_holes(feet, &scan, material);
    holes.extend(m::scan_quads(feet, 3, 1, false, material));
    for hole in &holes {
        if hole.y != feet[1] {
            continue;
        }
        let mut cells = Vec::new();
        for dx in 0..hole.width {
            for dz in 0..hole.depth {
                cells.push([hole.x + dx, hole.y, hole.z + dz]);
            }
        }
        if cells.contains(&feet) {
            return cells;
        }
    }
    vec![feet]
}

/// The outer ring of a hole's cells: four for a 1x1, six for a 2x1, eight for
/// a 2x2. Split out so the shape arithmetic can be pinned by a test.
fn perimeter(cells: &[[i32; 3]]) -> Vec<[i32; 3]> {
    let mut out = Vec::new();
    for &cell in cells {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let next = [cell[0] + dx, cell[1], cell[2] + dz];
            if !cells.contains(&next) && !out.contains(&next) {
                out.push(next);
            }
        }
    }
    out
}

/// The first solid face that would place a block into `cell`. The floor comes
/// first (the common case), then the four sides, then the ceiling. `face` is
/// the direction from `pos` back to `cell`, and `cursor` is that face's middle.
fn placement_for(cell: [i32; 3], solid: &impl Fn([i32; 3]) -> bool) -> Option<Placement> {
    const FACES: [([i32; 3], i32, [f32; 3]); 6] = [
        ([0, -1, 0], 1, [0.5, 1.0, 0.5]),  // floor, UP
        ([-1, 0, 0], 5, [1.0, 0.5, 0.5]),  // west neighbour, EAST face
        ([1, 0, 0], 4, [0.0, 0.5, 0.5]),   // east neighbour, WEST face
        ([0, 0, -1], 3, [0.5, 0.5, 1.0]),  // north neighbour, SOUTH face
        ([0, 0, 1], 2, [0.5, 0.5, 0.0]),   // south neighbour, NORTH face
        ([0, 1, 0], 0, [0.5, 0.0, 0.5]),   // ceiling, DOWN
    ];
    for (offset, face, cursor) in FACES {
        let pos = [
            cell[0] + offset[0],
            cell[1] + offset[1],
            cell[2] + offset[2],
        ];
        if solid(pos) {
            return Some(Placement { pos, face, cursor });
        }
    }
    None
}

/// The same classification HoleESP's scan uses.
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
        f64::from(cell[1]) + 0.5,
        f64::from(cell[2]) + 0.5,
    ]
}

/// Never place into a cell another entity is standing in.
fn occupied(game: &Game<'_>, own: i32, cell: [i32; 3]) -> bool {
    game.mobs.iter().any(|mob| {
        mob.id as i32 != own
            && mob.death_time == 0
            && (f64::from(cell[0]) - 0.3..=f64::from(cell[0]) + 1.3).contains(&mob.position.x)
            && (f64::from(cell[2]) - 0.3..=f64::from(cell[2]) + 1.3).contains(&mob.position.z)
            && (f64::from(cell[1]) - 1.0..=f64::from(cell[1]) + 2.0).contains(&mob.position.y)
    })
}

fn block_allowed(name: &str, choice: m::SurroundBlock) -> bool {
    match choice {
        m::SurroundBlock::Obsidian => name == OBSIDIAN || name == CRYING_OBSIDIAN,
        m::SurroundBlock::EnderChest => name == ENDER_CHEST,
        m::SurroundBlock::Any => true,
    }
}

fn usable(game: &Game<'_>, stack: &ItemStack, choice: m::SurroundBlock) -> bool {
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
fn block_slot(game: &Game<'_>, choice: m::SurroundBlock) -> Option<usize> {
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
    config: &m::SurroundConfig,
    targets: &[([i32; 3], Placement)],
    slot: usize,
) {
    let previous = game.selected;
    set_slot(game, slot);
    for &(cell, click) in targets {
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
            .find(|module| module.id == m::SURROUND)
        {
            module.enabled = false;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{Placement, block_allowed, perimeter, placement_for};
    use crate::game::clickgui::SurroundBlock;

    #[test]
    fn a_single_cell_has_four_neighbours() {
        let ring = perimeter(&[[0, 64, 0]]);
        assert_eq!(ring.len(), 4);
        for p in [[1, 64, 0], [-1, 64, 0], [0, 64, 1], [0, 64, -1]] {
            assert!(ring.contains(&p));
        }
    }

    #[test]
    fn a_double_has_six_and_a_quad_has_eight() {
        let double = [[0, 64, 0], [1, 64, 0]];
        let ring = perimeter(&double);
        assert_eq!(ring.len(), 6, "{ring:?}");
        assert!(!ring.contains(&double[0]) && !ring.contains(&double[1]));

        let quad = [
            [0, 64, 0],
            [1, 64, 0],
            [0, 64, 1],
            [1, 64, 1],
        ];
        assert_eq!(perimeter(&quad).len(), 8);
    }

    #[test]
    fn a_floor_is_the_preferred_support() {
        let solid = |p: [i32; 3]| p == [5, 63, 5];
        assert_eq!(
            placement_for([5, 64, 5], &solid),
            Some(Placement {
                pos: [5, 63, 5],
                face: 1,
                cursor: [0.5, 1.0, 0.5],
            })
        );
    }

    #[test]
    fn without_a_floor_a_side_wall_supports_the_placement() {
        // Only the west neighbour is solid: click its EAST face.
        let solid = |p: [i32; 3]| p == [4, 64, 5];
        assert_eq!(
            placement_for([5, 64, 5], &solid),
            Some(Placement {
                pos: [4, 64, 5],
                face: 5,
                cursor: [1.0, 0.5, 0.5],
            })
        );
        // `pos + face` must land back in the cell for every one of the six.
        let faces: [([i32; 3], i32); 6] = [
            ([0, -1, 0], 1),
            ([-1, 0, 0], 5),
            ([1, 0, 0], 4),
            ([0, 0, -1], 3),
            ([0, 0, 1], 2),
            ([0, 1, 0], 0),
        ];
        for (offset, face) in faces {
            let neighbour = [5 + offset[0], 64 + offset[1], 5 + offset[2]];
            let click = placement_for([5, 64, 5], &|p| p == neighbour).unwrap();
            assert_eq!(click.pos, neighbour);
            assert_eq!(click.face, face);
        }
    }

    #[test]
    fn a_floating_cell_has_no_support() {
        assert_eq!(placement_for([5, 64, 5], &|_| false), None);
    }

    #[test]
    fn only_the_configured_blocks_are_usable() {
        assert!(block_allowed("minecraft:obsidian", SurroundBlock::Obsidian));
        assert!(block_allowed(
            "minecraft:crying_obsidian",
            SurroundBlock::Obsidian
        ));
        assert!(!block_allowed(
            "minecraft:ender_chest",
            SurroundBlock::Obsidian
        ));
        assert!(block_allowed(
            "minecraft:ender_chest",
            SurroundBlock::EnderChest
        ));
        assert!(block_allowed("minecraft:cobweb", SurroundBlock::Any));
    }
}
