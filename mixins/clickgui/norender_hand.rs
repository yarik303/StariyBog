// NoRender's first-person hand: skip the hand and its item when the setting
// is on.
fn clickgui_norender_hand(game: &crate::game::Game<'_>, canceled: &mut bool) {
    use crate::game::clickgui as m;

    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    if m::norender().is_some_and(|config| config.hand) {
        *canceled = true;
    }
}
