// NoRender's mobs: skip every mob's vanilla model, armour and held items
// when the setting is on. Name tags, shadows, flames and glowing outlines
// follow their own NoRender settings.
fn clickgui_norender_mob(game: &crate::game::Game<'_>, drawn: &mut bool) {
    use crate::game::clickgui as m;

    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    if m::norender().is_some_and(|config| config.mobs) {
        *drawn = true;
    }
}
