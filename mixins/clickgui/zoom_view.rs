// The ClickGUI Zoom module's camera effect: while its bind key is held, ask
// for the module's factor through the same hook Horizon's own Zoom mod uses.
fn clickgui_zoom_view(
    game: &crate::game::Game<'_>,
    zoom: &mut Option<crate::game::native_hooks::NativeZoom>,
) {
    use crate::game::clickgui as m;

    if zoom.is_none()
        && game.native_on(m::ID)
        && let Some((factor, smooth, turn)) = m::zoom_view()
    {
        *zoom = Some(crate::game::native_hooks::NativeZoom {
            factor,
            smooth,
            turn,
        });
    }
}
