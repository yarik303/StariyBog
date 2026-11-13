// Called once per tick in every game. Only the window's game does anything:
// in single player the server thread has its own copy of the state.
fn clickgui_tick(game: &mut crate::game::Game<'_>) {
    use crate::game::{
        clickgui_autoarmor as autoarmor, clickgui_autotool as autotool,
        clickgui_autoweapon as autoweapon, clickgui_cheststealer as cheststealer,
        clickgui_fullbright as fullbright, clickgui_offhand as offhand,
        clickgui_timer_module as timer_module, clickgui_ui as ui,
        remote::{
            clickgui_aimassist as aimassist, clickgui_autoclicker as autoclicker,
            clickgui_autoxp as autoxp, clickgui_fastbreak as fastbreak,
            clickgui_crystalaura as crystalaura, clickgui_holefill as holefill,
            clickgui_reach as reach, clickgui_surround as surround,
        },
    };

    if !game.native_window() {
        return;
    }
    ui::ensure_loaded(&*game);
    // The screen changed under the GUI (pause, death, a chat screen): close it
    // so the pointer is not left released with widgets you cannot reach.
    if ui::is_open() && !game.native_playing() {
        ui::close(game);
    }
    fullbright::tick(game);
    timer_module::tick(game);
    // Before the hotbar-switching modules, so `Weapon only` reads the item the
    // player is actually holding rather than a tool AutoTool just picked.
    autoclicker::tick(game);
    autotool::tick(game);
    autoweapon::tick(game);
    cheststealer::tick(game);
    fastbreak::tick(game);
    // Last, so the off hand sees the slot AutoWeapon and AutoTool settled on.
    offhand::tick(game);
    // After OffHand: both click the same menu, and its swap changes which
    // slots hold what.
    // After OffHand and AutoArmor: all three can click the player's own
    // inventory menu, and AutoXP's restock swap has to see what they settled.
    autoarmor::tick(game);
    autoxp::tick(game);
    // Last: it sends its own placement packets and must see the hotbar slot
    // the other modules settled on.
    holefill::tick(game);
    // Surround after HoleFill: both switch the hotbar slot and restore it in
    // the same tick, so they cannot see each other's temporary selection.
    surround::tick(game);
    crystalaura::tick(game);
    reach::tick(game);
    // Last: it writes `Game::yaw` / `Game::pitch`, so every other module has
    // already settled its rotation for the tick.
    aimassist::tick(game);
    game.clickgui_ping_tick();
}
