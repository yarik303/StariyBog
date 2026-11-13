// Reach: widens the crosshair's entity reach.
//
// The reference is Alan's `Reach` (its GHOST category): it raises the range of
// the entity raycast the crosshair uses, so a click can pick a target further
// out than vanilla's three blocks. Horizon computes that range once a tick from
// the `minecraft:entity_interaction_range` attribute (`Game::entity_reach`), and
// `Game::mob_hit` clips the entity ray to it, so a transient attribute modifier
// is the whole mechanism — the same trick Step uses for `step_height`.
//
// This is a ghost feature, and the limit is the server's. The client sends the
// attack either way; only the server decides whether the hit counts, from its
// own copy of the attribute (`ServerGamePacketListenerImpl.handleAttack`'s
// `Player.isWithinAttackRange`). So the setting buys reach only as far as the
// server tolerates it — on a strict server, nothing.
//
// The reference randomises its reach between two bounds rather than holding one
// value, so the pick never sits at a constant maximum; the sliders keep that.
//
// Not ported: the reference's `Buffer Abuse` (it cancels attacks to build a
// buffer, which is a bypass rather than reach), and its `LegitReach` companion.
// That one is a backtrack: it holds the player's own packets while a target is
// out of range, which belongs with Blink's hold and the movement chain's
// ordering, not in a combat module.
use crate::game::{Game, clickgui as m};
use std::cell::Cell;

const ATTRIBUTE: &str = "minecraft:entity_interaction_range";
const MODIFIER: &str = "clickgui:reach";
/// The attribute's own base: a player starts at three blocks and creative adds
/// two, which is exactly what `Game::interaction_range` falls back to.
const SURVIVAL: f64 = 3.0;
const CREATIVE: f64 = 5.0;

thread_local! {
    /// The jitter's own roll. It deliberately does not come from
    /// `world.random`: that stream is the level's, the client's own simulation
    /// draws from it every tick (sound pitch, projectile spread, mob AI), and
    /// taking a value would shift all of that on the ticks this module runs.
    static ROLL: Cell<u32> = const { Cell::new(0x9E37_79B9) };
}

pub(crate) fn tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let Some(config) = m::reach() else {
        apply(game, 0.0);
        return;
    };
    if game.player.is_none() || !game.native_playing() {
        apply(game, 0.0);
        return;
    }
    let reach = f64::from(config.pick(roll()));
    apply(game, reach);
}

/// xorshift32, mapped to `0..1`.
fn roll() -> f32 {
    ROLL.with(|state| {
        let mut x = state.get();
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        state.set(x);
        // The top 24 bits, over 2^24: the value lands in `0..1`.
        (x >> 8) as f32 / 16_777_216.0
    })
}

fn apply(game: &mut Game<'_>, reach: f64) {
    let (amount, present) = modifier(reach, game.creative);
    if let Some(player) = game.player.as_mut() {
        let _ = player.set_modifier(ATTRIBUTE, MODIFIER, amount, "add_value", present);
    }
}

/// The modifier's amount and whether to apply it at all. Split out so the
/// arithmetic can be pinned by a test: a reach at or below the attribute's own
/// base adds nothing rather than shrinking it.
fn modifier(reach: f64, creative: bool) -> (f64, bool) {
    let base = if creative { CREATIVE } else { SURVIVAL };
    let amount = reach - base;
    (amount.max(0.0), amount > 0.001)
}

#[cfg(test)]
mod tests {
    use super::{modifier, roll};

    #[test]
    fn a_longer_reach_adds_the_difference() {
        assert_eq!(modifier(4.0, false), (1.0, true));
        assert_eq!(modifier(6.0, false), (3.0, true));
    }

    #[test]
    fn vanilla_reach_adds_nothing() {
        assert_eq!(modifier(3.0, false), (0.0, false));
        assert_eq!(modifier(2.0, false), (0.0, false));
    }

    #[test]
    fn creative_already_reaches_five_blocks() {
        assert_eq!(modifier(4.0, true), (0.0, false));
        assert_eq!(modifier(5.0, true), (0.0, false));
        assert_eq!(modifier(6.0, true), (1.0, true));
    }

    #[test]
    fn the_roll_stays_inside_one_and_moves() {
        let first = roll();
        let mut moved = false;
        for _ in 0..32 {
            let next = roll();
            assert!((0.0..1.0).contains(&next), "{next}");
            moved |= next != first;
        }
        assert!(moved, "the roll never changed");
    }
}
