// A key or mouse button went down or up. AutoWeapon's OnAttack switches the
// held weapon on the attack click itself, before the game turns it into a
// tick's attack; that is the only point before the click in a server world.
fn clickgui_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use crate::game::clickgui as m;

    if !game.native_on(m::ID) {
        return;
    }
    crate::game::clickgui_autoweapon::on_key(game, key, pressed);
}
