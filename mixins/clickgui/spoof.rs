// The movement-packet spoof chain, and the marker that says which packet it
// belongs to.
//
// NoFall, Criticals, Speed, Step and Fly rewrite the packet the position
// sender produced (ground flag, position offset), drop it, or — Criticals'
// force modes — make one when the sender had none. The rewrite used to sit on
// the `position_sender.update` call in `remote_send_state`, but that function
// now holds `use` items and closures, which horizon-mixins rejects for every
// site hook, and the sender itself lives in `horizon_client_lib` with a closure
// in its body, so a RETURN modifier is rejected there too.
//
// The one clean site left is `Game::send`, where the packet actually leaves.
// It carries every outgoing packet, including the extra ones the modes send
// themselves (NoFall's ground packets, Fly's bursts, Criticals' fake
// positions), so `arm` marks the tick's own movement packet first: it runs at
// the head of `remote_send_state` with the lowest priority, after the hooks
// that send those extras. `send` then spoofs the first movement packet it sees
// and disarms, so an extra packet never takes the tick's plan.
//
// A tick whose sender produced no packet (no move, no turn, no ground change)
// never reaches `send`, and Criticals' force modes still have to answer: `flush`
// runs the chain with `None` from `native_tick`, which is still before the
// tick's `ClientTickEnd`, so the chain runs exactly once per tick either way,
// as it did when it wrapped the sender's own result.
type Packet = horizon_protocol::packets::play::Serverbound;

thread_local! {
    static ARMED: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

fn is_movement(packet: &Packet) -> bool {
    matches!(
        packet,
        Packet::MovePlayerPos(_)
            | Packet::MovePlayerPosRot(_)
            | Packet::MovePlayerRot(_)
            | Packet::MovePlayerStatus(_)
    )
}

/// Marks the tick's movement packet as the next one to send. A rider's look
/// packet is pushed straight into the outgoing list, a camera following another
/// entity sends none, and a tick without a player or a remote returns before
/// the sender runs, so none of those are marked.
pub(crate) fn arm(game: &crate::game::Game<'_>) {
    if game.player.is_some() && game.remote.is_some() && game.riding.is_none() && game.controls_camera()
    {
        ARMED.set(true);
    }
}

/// The chain, in the order the modules ran before: NoFall, Criticals, Speed,
/// Step, Fly. Blink wraps the lot: it keeps the packet the others produced,
/// so what goes out later is their rewrite and not the original.
pub(crate) fn spoof(packet: Option<Packet>) -> Option<Packet> {
    crate::game::clickgui_blink::spoof(crate::game::remote::clickgui_fly::spoof(
        crate::game::remote::clickgui_step::spoof(crate::game::remote::clickgui_speed::spoof(
            crate::game::remote::clickgui_criticals::spoof(
                crate::game::remote::clickgui_nofall::spoof(packet),
            ),
        )),
    ))
}

fn deliver(game: &crate::game::Game<'_>, packet: Option<Packet>) {
    if let (Some(packet), Some(remote)) = (packet, &game.remote) {
        remote.session.send(packet);
    }
}

/// `Game::send`'s head: the marked movement packet goes out spoofed, or not at
/// all, and the send below is skipped. Every other packet is sent untouched.
pub(crate) fn send(
    game: &crate::game::Game<'_>,
    packet: &Packet,
) -> core::ops::ControlFlow<()> {
    if !ARMED.get() || !is_movement(packet) {
        return core::ops::ControlFlow::Continue(());
    }
    ARMED.set(false);
    deliver(game, spoof(Some(packet.clone())));
    core::ops::ControlFlow::Break(())
}

/// The tick's end for a mark `send` never claimed: the sender had no packet, so
/// the chain sees `None` and only Criticals' force modes answer with one.
pub(crate) fn flush(game: &crate::game::Game<'_>) {
    if !ARMED.replace(false) {
        return;
    }
    deliver(game, spoof(None));
}
