"""StariyBog in a running modded game: its row in Options > Mods, the settings page it puts there,
and FullBright brightening the world it claims to brighten.

The ClickGUI's own menu is a screen the mod draws itself, so `ui` cannot list its widgets and a
proof cannot click them by id: what this drives is the vanilla screens the mod hangs off — its row
in Options > Mods, and the settings page behind that row's Settings button — plus the one module
whose effect a screenshot can see. The menu itself is left to a person, or to a later proof that
clicks coordinates.

The row's suffix is not asserted: `Zoom 2.0.0: ON` is what an *installed* mod's row reads, and a
mod built into the binary may label itself differently, so this only requires a row beginning with
the mod's title and records the whole label. Everything else is asserted.

Not run yet: the harness needs prepared Minecraft 26.3 resources (README, "Proving it in a running
game"), and none are on this machine. The page's buttons come from `settings_impl.rs`, the toggle
from `click_impl.rs`, and the brightness threshold is a guess at a midnight screenshot.
"""
import json
import time
from game import Game, difference

# `list_impl.rs` titles the row `StariyBog <version>`.
TITLE = "StariyBog 0.1.0"
# `settings_impl.rs` pushes the reset first, then FullBright as it currently stands (off to start).
RESET = "Reset StariyBog position"
FB_OFF, FB_ON = "FullBright: OFF", "FullBright: ON"


def run(ctl, resources, out):
    with Game(ctl, resources, out) as g:
        results = {}
        g.run("options", "mods")
        row = next((label for label in g.labels() if label and label.startswith(TITLE)), None)
        assert row is not None, g.labels()
        results["row"] = row
        g.run("click", g.ids()["Settings"])
        page = g.labels()
        assert RESET in page, page
        assert FB_OFF in page, page
        g.shot("1-settings-page")
        # The FullBright button toggles the module it names, and says so both ways.
        g.run("click", g.ids()[FB_OFF])
        assert FB_ON in g.labels(), g.labels()
        g.run("click", g.ids()[FB_ON])
        assert FB_OFF in g.labels(), g.labels()
        # Midnight, then the same view with the module on: the lightmap brightens.
        g.play()
        g.run("command", "time set midnight")
        time.sleep(2)
        dark = g.shot("2-midnight-off")
        g.run("options", "mods")
        g.run("click", g.ids()["Settings"])
        g.run("click", g.ids()[FB_OFF])
        assert FB_ON in g.labels(), g.labels()
        g.play()
        time.sleep(3)
        lit = g.shot("3-midnight-on")
        results["diff_fullbright"] = round(difference(dark, lit), 1)
        assert results["diff_fullbright"] > 5, results
        # Leave the game as it was found.
        g.run("options", "mods")
        g.run("click", g.ids()["Settings"])
        g.run("click", g.ids()[FB_ON])
        assert FB_OFF in g.labels(), g.labels()
        print(json.dumps({"proof": "clickgui", "ok": True, **results}, indent=2))
