// NoRender's camera overlays: `build` makes the `misc/underwater` tint for
// `input.water` and the two `block/fire_1` quads for `input.on_fire`, and both
// are decided by those fields alone. Rebuilding the frame with the hidden ones
// cleared drops just those quads, leaving the in-block overlay, the spyglass,
// the vignette and the nausea swirl alone. The inner call returns here with
// nothing left to clear, so it runs the original body, once.
fn clickgui_norender_overlay(
    effects: &crate::screen_effects::ScreenEffects,
    input: &crate::screen_effects::ScreenInputs<'_>,
) -> core::ops::ControlFlow<Vec<ScreenQuad>> {
    let water = if crate::clickgui_norender::water_hidden() {
        None
    } else {
        input.water
    };
    let on_fire = input.on_fire && !crate::clickgui_norender::fire_hidden();
    if water == input.water && on_fire == input.on_fire {
        return core::ops::ControlFlow::Continue(());
    }
    let without = crate::screen_effects::ScreenInputs {
        sprites: input.sprites,
        flags: input.flags,
        aspect: input.aspect,
        partial: input.partial,
        block_overlay: input.block_overlay.clone(),
        water,
        on_fire,
        fire_lower: input.fire_lower,
        gui_overlays: input.gui_overlays,
    };
    core::ops::ControlFlow::Break(effects.build(&without))
}
