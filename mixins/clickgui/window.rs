// The window's event dispatcher. While the GUI is open this consumes
// keyboard, mouse and wheel input before the game sees it, so the player does
// not walk, attack or re-grab the pointer behind the window. While it is
// closed, module binds and the ClickGUI key toggle their modules. Lifecycle
// events (redraw, resize, focus, close) still go through.
//
// The target is `App::window_event`, the small dispatcher that forwards every
// window event to the loading screen or the game. It has no macros, so the
// bundle also applies on horizon-mixins versions that cannot preserve a body
// with macros; intercepting here is equivalent to intercepting the game's own
// handler, which it calls next.
fn clickgui_window_event(
    app: &mut crate::game::App,
    event: &winit::event::WindowEvent,
) -> std::ops::ControlFlow<()> {
    use crate::game::{App, clickgui as m, clickgui_ui as ui};
    use std::ops::ControlFlow;
    use winit::{
        event::{ElementState, MouseButton, WindowEvent},
        keyboard::PhysicalKey,
    };

    let App::Ready(game) = app else {
        // The loading screen took over (a pack reload): drop the window.
        if ui::is_open() {
            ui::hide();
        }
        return ControlFlow::Continue(());
    };
    // The app now holds the splitscreen host, whose `host` is P1's game.
    let game = &mut game.host;

    if !game.native_on(m::ID) {
        if ui::is_open() {
            ui::close(game);
        }
        return ControlFlow::Continue(());
    }

    ui::ensure_loaded(&*game);

    if !ui::is_open() {
        // Closed: the ClickGUI key opens the window, and module binds toggle
        // their modules. Both are consumed before the game sees them.
        match event {
            WindowEvent::KeyboardInput { event: key, .. } => {
                if let PhysicalKey::Code(code) = key.physical_key
                    && let Some(name) = horizon_client::keybinds::key_name(code)
                {
                    let mut consumed = false;
                    if m::zoom_bound_key(name) {
                        if key.state == ElementState::Pressed {
                            if !key.repeat {
                                if game.native_playing() {
                                    if !m::is_enabled(m::ZOOM) {
                                        ui::set_enabled(game, m::ZOOM, true);
                                    }
                                    m::zoom_key(true, true);
                                    consumed = true;
                                }
                            } else if m::zoom_held() {
                                consumed = true;
                            }
                        } else {
                            let was_held = m::zoom_held();
                            m::zoom_key(false, game.native_playing());
                            consumed = was_held;
                        }
                    }
                    if key.state == ElementState::Pressed && !key.repeat {
                        let chosen = game.native_mod_key(m::ID, m::KEY, m::DEFAULT_KEY);
                        if game.native_playing() && name == chosen {
                            ui::open(game);
                            return ControlFlow::Break(());
                        }
                        if ui::toggle_bound(game, name) {
                            consumed = true;
                        }
                    } else if m::module_bound_to(name).is_some() {
                        // Swallow the release (or repeat) of a key bound to a
                        // module, whether or not Zoom also holds it.
                        consumed = true;
                    }
                    if consumed {
                        return ControlFlow::Break(());
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let name = horizon_client::keybinds::mouse_name(*button);
                let mut consumed = false;
                if m::zoom_bound_key(&name) {
                    if *state == ElementState::Pressed {
                        if game.native_playing() {
                            if !m::is_enabled(m::ZOOM) {
                                ui::set_enabled(game, m::ZOOM, true);
                            }
                            m::zoom_key(true, true);
                            consumed = true;
                        }
                    } else {
                        let was_held = m::zoom_held();
                        m::zoom_key(false, game.native_playing());
                        consumed = was_held;
                    }
                }
                if *state == ElementState::Pressed {
                    if ui::toggle_bound(game, &name) {
                        consumed = true;
                    }
                } else if m::module_bound_to(&name).is_some() {
                    consumed = true;
                }
                if consumed {
                    return ControlFlow::Break(());
                }
            }
            WindowEvent::Focused(false) => {
                // A lost focus never sends the key's release; let go of a
                // held zoom here so it cannot outlive the world's input.
                m::zoom_release();
            }
            _ => {}
        }
        return ControlFlow::Continue(());
    }

    // A text setting is being edited: keys type into it (Enter or Escape
    // finish, Backspace deletes), and a click elsewhere ends it and still
    // reaches the widgets.
    if ui::is_editing() {
        match event {
            WindowEvent::KeyboardInput { event: key, .. } => {
                if key.state == ElementState::Pressed {
                    match key.physical_key {
                        PhysicalKey::Code(
                            winit::keyboard::KeyCode::Escape
                            | winit::keyboard::KeyCode::Enter
                            | winit::keyboard::KeyCode::NumpadEnter,
                        ) => ui::stop_editing(),
                        PhysicalKey::Code(winit::keyboard::KeyCode::Backspace) => {
                            ui::backspace_text(game)
                        }
                        _ => {
                            if let Some(text) = key.text.as_deref() {
                                ui::type_text(game, text);
                            }
                        }
                    }
                }
                return ControlFlow::Break(());
            }
            WindowEvent::MouseInput { state, .. } => {
                // A click elsewhere ends editing; the draw starts it again
                // when the click lands on the field. Releases leave it alone.
                if *state == ElementState::Pressed {
                    ui::stop_editing();
                }
            }
            WindowEvent::MouseWheel { .. } => {
                ui::stop_editing();
            }
            WindowEvent::Focused(false) => {
                ui::hide();
                return ControlFlow::Continue(());
            }
            _ => return ControlFlow::Continue(()),
        }
    }

    // A module's bind row is waiting for a key: take the next one (Escape
    // cancels) before the window's own shortcuts see it.
    if ui::is_listening() {
        match event {
            WindowEvent::KeyboardInput { event: key, .. } => {
                if key.state == ElementState::Pressed && !key.repeat {
                    match key.physical_key {
                        PhysicalKey::Code(winit::keyboard::KeyCode::Escape) => ui::cancel_bind(),
                        PhysicalKey::Code(code) => {
                            match horizon_client::keybinds::key_name(code)
                                .and_then(|name| ui::capture_bind(game, name))
                            {
                                Some(ui::Captured::Bound(module, key_name)) => {
                                    game.native_chat(&format!("{module} bound to {key_name}"));
                                }
                                Some(ui::Captured::Reserved) => {
                                    game.native_chat("That key opens StariyBog; pick another");
                                }
                                None => {}
                            }
                        }
                        PhysicalKey::Unidentified(_) => {}
                    }
                }
                return ControlFlow::Break(());
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *state == ElementState::Pressed {
                    let name = horizon_client::keybinds::mouse_name(*button);
                    match ui::capture_bind(game, &name) {
                        Some(ui::Captured::Bound(module, key_name)) => {
                            game.native_chat(&format!("{module} bound to {key_name}"));
                        }
                        Some(ui::Captured::Reserved) => {
                            game.native_chat("That key opens StariyBog; pick another");
                        }
                        None => {}
                    }
                }
                return ControlFlow::Break(());
            }
            WindowEvent::MouseWheel { .. } => return ControlFlow::Break(()),
            WindowEvent::Focused(false) => {
                ui::hide();
                return ControlFlow::Continue(());
            }
            _ => return ControlFlow::Continue(()),
        }
    }

    match event {
        WindowEvent::KeyboardInput { event: key, .. } => {
            if key.state == ElementState::Pressed && !key.repeat {
                let close = match key.physical_key {
                    PhysicalKey::Code(code) => {
                        let chosen = game.native_mod_key(m::ID, m::KEY, m::DEFAULT_KEY);
                        code == winit::keyboard::KeyCode::Escape
                            || horizon_client::keybinds::key_name(code) == Some(chosen.as_str())
                    }
                    PhysicalKey::Unidentified(_) => false,
                };
                if close {
                    ui::close(game);
                }
            }
            ControlFlow::Break(())
        }
        WindowEvent::MouseInput { state, button, .. } => {
            if *button == MouseButton::Left {
                ui::mouse_button(*button, *state == ElementState::Pressed);
            }
            ControlFlow::Break(())
        }
        WindowEvent::MouseWheel { delta, .. } => {
            ui::wheel(*delta);
            ControlFlow::Break(())
        }
        WindowEvent::Focused(false) => {
            // Let the game handle losing focus (it releases the pointer and may
            // pause); the GUI goes away without grabbing the pointer back.
            ui::hide();
            ControlFlow::Continue(())
        }
        _ => ControlFlow::Continue(()),
    }
}
