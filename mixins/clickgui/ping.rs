// The Watermark's ping: a real round trip to the server, the same probe the
// F3 network chart uses (`PingRequest`/`PongResponse`). `handle_packet` logs
// each reply's delay into `debug_charts.ping`; the Watermark asks for a sample
// every second while playing and shows the newest one. The player-list latency
// is only a fallback until the first reply arrives (some servers leave it 0).
use super::Game;
use crate::game::clickgui as m;
use std::cell::Cell;

/// The wait between probes, in milliseconds.
const INTERVAL: i64 = 1000;

thread_local! {
    static LAST: Cell<i64> = const { Cell::new(i64::MIN / 2) };
}

impl Game<'_> {
    /// The newest measured round trip in milliseconds; the local player's
    /// player-list latency until one exists; 0 in a local world without either.
    pub(crate) fn clickgui_ping(&self) -> i32 {
        if let Some(ms) = self.debug_charts.ping.clickgui_latest() {
            return i32::try_from(ms).unwrap_or(i32::MAX).max(0);
        }
        let Some(remote) = self.remote.as_ref() else {
            return 0;
        };
        remote
            .tab_players
            .get(&remote.profile)
            .map_or(0, |player| player.latency)
    }

    /// Sends a ping probe at most once a second while playing.
    pub(crate) fn clickgui_ping_tick(&self) {
        if !self.native_on(m::ID) || self.remote.is_none() || !self.native_playing() {
            return;
        }
        let now = self.debug_millis();
        let due = LAST.with(|last| {
            if now - last.get() < INTERVAL {
                return false;
            }
            last.set(now);
            true
        });
        if due {
            use horizon_protocol::packets::{play::Serverbound, status::Ping};
            self.send(Serverbound::PingRequest(Ping { time: now }));
        }
    }
}
