// The ClickGUI's immediate-mode layer and its input glue.
//
// The widget API follows imgui-rs's shape (`button`, `checkbox`, `slider`,
// `same_line`-style explicit rectangles) because a native mod cannot add Cargo
// dependencies or borrow the game's wgpu context, so imgui-rs itself cannot be
// linked into Horizon. Everything here draws through the HUD batch that the
// game hands `native_hud`: filled rectangles and font text, which is exactly
// what the click GUI needs.
use crate::game::{Game, clickgui as m};
use horizon_client::hud::HudBatch;
use winit::event::{MouseButton, MouseScrollDelta};

// Window geometry, in GUI pixels. The width and height are per-window (see
// `GuiState::size`); these are the fixed parts, the corner radius and the
// resize grip.
const GRIP: f32 = 18.0;
const GRIP_INSET: f32 = 4.0;
const RADIUS: f32 = 6.0;
const TITLE_H: f32 = 15.0;
const TABS_H: f32 = 16.0;
const PAD: f32 = 5.0;
const LIST_W: f32 = 160.0;
const ROW_H: f32 = 14.0;
const SETTING_H: f32 = 20.0;

// Colours: 0xAARRGGBB for fills, plain RGB for text.
const DIM_SCREEN: u32 = 0x5810_1018;
const BG: u32 = 0xF810_1018;
const PANEL: u32 = 0xFF18_1822;
const HEADER: u32 = 0xFF22_2232;
const HOVER: u32 = 0xFF28_2838;
const TRACK: u32 = 0xFF30_3040;
const BORDER: u32 = 0xFF3C_3C50;
const ACCENT: u32 = 0xFF9B_7BFF;
const GREEN: u32 = 0xFF3F_D37B;
const RED: u32 = 0xFFE0_5A5A;
const TEXT: u32 = 0xED_ED_F2;
const MUTED: u32 = 0x9A_9A_A8;
const ON_ACCENT: u32 = 0x10_10_18;
/// The bottom-right grip is subtle until the pointer is over it.
const GRIP_IDLE: u32 = 0x559B_7BFF;
const GRIP_HOVER: u32 = 0x999B_7BFF;

/// The module's saved settings first, then the value, one file per key:
/// `enabled.<module>`, `bind.<module>` and `<module>.<setting>` in the mod's
/// data folder, plus `window.size`.
pub(super) fn ensure_loaded(game: &Game<'_>) {
    if m::with(|c| c.loaded) {
        return;
    }
    m::with(|c| {
        for module in &mut c.modules {
            let key = format!("enabled.{}", module.id);
            if let Some(value) = game.native_setting(m::ID, &key) {
                module.enabled = matches!(value.as_str(), "true" | "1" | "on");
            }
            let key = format!("bind.{}", module.id);
            if let Some(value) = game.native_setting(m::ID, &key) {
                let value = value.trim();
                module.bind = if value.is_empty() {
                    None
                } else {
                    Some(value.to_owned())
                };
            }
            for setting in &mut module.settings {
                let key = format!("{}.{}", module.id, setting.id);
                if let Some(value) = game.native_setting(m::ID, &key) {
                    setting.set_text(&value);
                }
            }
        }
        if let Some(value) = game.native_setting(m::ID, "window.size")
            && let Some(size) = parse_size(&value)
        {
            c.gui.size = size;
        }
        c.gui.size = c.gui.clamped_size();
        c.loaded = true;
    });
}

fn parse_size(text: &str) -> Option<(f32, f32)> {
    let (w, h) = text.split_once('x')?;
    let w = w.trim().parse::<f32>().ok()?;
    let h = h.trim().parse::<f32>().ok()?;
    (w.is_finite() && h.is_finite()).then_some((w, h))
}

/// Opens the window over the world. Returns false outside a world.
pub(super) fn open(game: &mut Game<'_>) -> bool {
    if !game.native_on(m::ID) || !game.native_playing() {
        return false;
    }
    ensure_loaded(&*game);
    let already = m::with(|c| {
        let open = c.gui.open;
        c.gui.open = true;
        c.gui.drag = None;
        c.gui.listening = None;
        c.gui.editing = None;
        c.gui.configs_dirty = true;
        c.input.reset();
        c.zoom.held = None;
        open
    });
    if already {
        return true;
    }
    // Release the pointer so the system cursor can move over the GUI, and drop
    // the keys held when it opened: the game must not keep walking or mining.
    game.capture(false);
    game.keys.clear();
    game.physical_keys.clear();
    game.left = false;
    game.right = false;
    game.stop_mining();
    game.mouse.set_ignore_first_move();
    // F1 hides the HUD, and the GUI is drawn in the HUD layer.
    game.ui.hide_gui = false;
    true
}

/// Closes the window and takes the pointer back while playing.
pub(super) fn close(game: &mut Game<'_>) {
    let was_open = m::with(|c| {
        let open = c.gui.open;
        c.gui.open = false;
        c.gui.drag = None;
        c.gui.listening = None;
        c.gui.editing = None;
        c.input.reset();
        c.zoom.held = None;
        open
    });
    if !was_open {
        return;
    }
    game.left = false;
    game.right = false;
    game.stop_mining();
    if game.native_playing() && game.native_player().is_some() {
        game.capture(true);
    }
}

/// Closes without taking the pointer back (the window lost focus; the game's
/// own focus handler releases it).
pub(super) fn hide() {
    m::with(|c| {
        c.gui.open = false;
        c.gui.drag = None;
        c.gui.listening = None;
        c.gui.editing = None;
        c.input.reset();
        c.zoom.held = None;
    });
}

pub(super) fn is_open() -> bool {
    m::is_open()
}

/// Puts the window back in the centre the next time it draws.
pub(super) fn reset_position() {
    m::with(|c| {
        c.gui.pos = None;
        c.gui.list_scroll = 0;
        c.gui.settings_scroll = 0;
    });
}

pub(super) fn set_enabled(game: &mut Game<'_>, id: &str, on: bool) {
    let changed = m::with(|c| {
        c.modules
            .iter_mut()
            .find(|module| module.id == id)
            .is_some_and(|module| {
                let changed = module.enabled != on;
                module.enabled = on;
                changed
            })
    });
    if changed {
        game.native_set_setting(
            m::ID,
            &format!("enabled.{id}"),
            if on { "true" } else { "false" },
        );
    }
    if id == m::ZOOM && !on {
        m::zoom_release();
    }
}

pub(super) fn toggle_module(game: &mut Game<'_>, id: &str) -> bool {
    let now = !m::is_enabled(id);
    set_enabled(game, id, now);
    now
}

/// Binds a module to a vanilla key name, or clears it with None, and keeps it.
/// A key can be bound to several modules at once; binding it here does not
/// take it from the others.
pub(super) fn set_bind(game: &Game<'_>, id: &str, bind: Option<String>) {
    let stored = bind.clone();
    m::with(|c| {
        if let Some(module) = c.modules.iter_mut().find(|module| module.id == id) {
            module.bind = stored;
        }
    });
    game.native_set_setting(m::ID, &format!("bind.{id}"), bind.as_deref().unwrap_or(""));
}

/// Turns `.bind`'s text into a vanilla key name, or None when unknown.
pub(super) fn parse_key(text: &str) -> Option<String> {
    let lower = text.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return None;
    }
    let key = normalize_key(&lower);
    (horizon_client::keybinds::key_code(&key).is_some()
        || horizon_client::keybinds::mouse_button(&key).is_some())
    .then_some(key)
}

fn normalize_key(lower: &str) -> String {
    // `key.keyboard.g` and `key.mouse.right` are accepted as-is.
    if lower.starts_with("key.") {
        return lower.to_owned();
    }
    // `mouse.right`, `mouse 4`, `rmb`.
    if let Some(rest) = lower
        .strip_prefix("mouse.")
        .or_else(|| lower.strip_prefix("mouse "))
    {
        return format!("key.mouse.{}", rest.trim().replace(' ', "."));
    }
    let dotted = lower.replace([' ', '_'], ".");
    let alias = match dotted.as_str() {
        "ctrl" | "control" => "left.control",
        "shift" => "left.shift",
        "alt" => "left.alt",
        "esc" => "escape",
        "return" => "enter",
        "del" => "delete",
        "ins" => "insert",
        "pgup" => "page.up",
        "pgdn" => "page.down",
        "lmb" => "mouse.left",
        "rmb" => "mouse.right",
        "mmb" => "mouse.middle",
        other => other,
    };
    if let Some(rest) = alias.strip_prefix("mouse.") {
        return format!("key.mouse.{rest}");
    }
    format!("key.keyboard.{alias}")
}

/// What pressing a key while a bind row listens did.
pub(super) enum Captured {
    /// The module was bound; its name and the key's display name.
    Bound(&'static str, String),
    /// The key opens the ClickGUI; it was not taken and listening continues.
    Reserved,
}

/// While a bind row is listening, stores the key pressed next.
pub(super) fn capture_bind(game: &mut Game<'_>, key: &str) -> Option<Captured> {
    let id = m::with(|c| c.gui.listening)?;
    if key == game.native_mod_key(m::ID, m::KEY, m::DEFAULT_KEY) {
        return Some(Captured::Reserved);
    }
    set_bind(game, id, Some(key.to_owned()));
    m::with(|c| c.gui.listening = None);
    let name = m::module_name(id)?;
    Some(Captured::Bound(
        name,
        horizon_client::keybinds::display_name(key).plain(),
    ))
}

pub(super) fn cancel_bind() {
    m::with(|c| c.gui.listening = None);
}

pub(super) fn is_listening() -> bool {
    m::with(|c| c.gui.listening.is_some())
}

/// Toggles every module bound to `key` (except Zoom, whose key is held by the
/// window hook). Returns whether a module was toggled.
pub(super) fn toggle_bound(game: &mut Game<'_>, key: &str) -> bool {
    let ids = m::modules_bound_to(key);
    if ids.is_empty() || !game.native_playing() {
        return false;
    }
    let mut toggled = false;
    for id in ids {
        if id == m::ZOOM {
            continue;
        }
        let on = toggle_module(game, id);
        if let Some(name) = m::module_name(id) {
            game.native_chat(&format!("{} {}", name, if on { "on" } else { "off" }));
        }
        toggled = true;
    }
    toggled
}

/// A mouse button from the window event; only the left button drives widgets.
pub(super) fn mouse_button(button: MouseButton, pressed: bool) {
    if button != MouseButton::Left {
        return;
    }
    m::with(|c| {
        if pressed {
            c.input.down = true;
            c.input.pressed = true;
            // A click ends text editing; the draw starts it again when the
            // click lands on the field.
            c.gui.editing = None;
        } else {
            c.input.down = false;
        }
    });
}

/// Whether a text field is taking keys.
pub(super) fn is_editing() -> bool {
    m::with(|c| c.gui.editing.is_some())
}

pub(super) fn stop_editing() {
    m::with(|c| c.gui.editing = None);
}

/// The most a text setting holds; block lists stay one line.
const TEXT_LIMIT: usize = 512;
/// A config file stem's longest name (see `m::valid_config_name`).
const CONFIG_NAME_LIMIT: usize = 32;

/// Types `text` into the text field being edited, keeping printable ASCII.
pub(super) fn type_text(game: &Game<'_>, text: &str) {
    let typed: String = text
        .chars()
        .filter(|c| c.is_ascii_graphic() || *c == ' ')
        .collect();
    if typed.is_empty() {
        return;
    }
    m::with(|c| match c.gui.editing {
        Some(m::Editing::ConfigName) => {
            for ch in typed.chars() {
                if c.gui.config_name.len() + ch.len_utf8() > CONFIG_NAME_LIMIT {
                    break;
                }
                c.gui.config_name.push(ch);
            }
        }
        Some(m::Editing::Setting {
            module: module_id,
            setting: setting_id,
        }) => {
            let Some(module) = c.modules.iter_mut().find(|m| m.id == module_id) else {
                return;
            };
            let Some(setting) = module.setting_mut(setting_id) else {
                return;
            };
            let m::Value::Text(value) = &mut setting.value else {
                return;
            };
            for ch in typed.chars() {
                if value.len() + ch.len_utf8() > TEXT_LIMIT {
                    break;
                }
                value.push(ch);
            }
            save_setting(game, module_id, setting);
        }
        None => {}
    });
}

/// Removes the last character of the text field being edited.
pub(super) fn backspace_text(game: &Game<'_>) {
    m::with(|c| match c.gui.editing {
        Some(m::Editing::ConfigName) => {
            c.gui.config_name.pop();
        }
        Some(m::Editing::Setting {
            module: module_id,
            setting: setting_id,
        }) => {
            let Some(module) = c.modules.iter_mut().find(|m| m.id == module_id) else {
                return;
            };
            let Some(setting) = module.setting_mut(setting_id) else {
                return;
            };
            let m::Value::Text(value) = &mut setting.value else {
                return;
            };
            value.pop();
            save_setting(game, module_id, setting);
        }
        None => {}
    });
}

/// Wheel steps while the GUI is open. Positive is up, as winit reports it.
pub(super) fn wheel(delta: MouseScrollDelta) {
    let steps = match delta {
        MouseScrollDelta::LineDelta(_, y) => y,
        MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 50.0,
    };
    m::with(|c| c.input.scroll += steps);
}

/// Draws the window when it is open. Returns whether the close button was hit.
pub(super) fn draw(game: &mut Game<'_>, batch: &mut HudBatch) {
    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    ensure_loaded(&*game);
    let mut close_requested = false;
    m::with(|c| {
        if c.gui.open {
            close_requested = draw_client(&*game, batch, c);
        }
    });
    if close_requested {
        close(game);
    }
    run_config_action(game);
}

/// Runs a Config tab button's work. It happens after the draw's registry
/// borrow ends because the config helpers (`m::config_document`,
/// `m::apply_config`) borrow the same registry.
fn run_config_action(game: &Game<'_>) {
    let Some(action) = m::with(|c| c.gui.config_pending.take()) else {
        return;
    };
    match action {
        m::ConfigAction::Make(name) => {
            let result = match config_path(&name) {
                Ok(path) if path.exists() => {
                    Err(format!("{name} already exists; pick another name or Save"))
                }
                Ok(path) => std::fs::write(&path, m::config_document())
                    .map(|()| format!("Made config {name}"))
                    .map_err(|error| format!("Could not make {name}: {error}")),
                Err(error) => Err(error),
            };
            config_result(result, Some(&name));
        }
        m::ConfigAction::Save(name) => {
            let result = match config_path(&name) {
                Ok(path) => std::fs::write(&path, m::config_document())
                    .map(|()| format!("Saved config {name}"))
                    .map_err(|error| format!("Could not save {name}: {error}")),
                Err(error) => Err(error),
            };
            config_result(result, Some(&name));
        }
        m::ConfigAction::Load(name) => {
            let result = match config_path(&name) {
                Ok(path) => match std::fs::read_to_string(&path) {
                    Ok(text) => {
                        let entries = m::parse_config(&text);
                        let applied = m::apply_config(&entries);
                        for (key, value) in &entries {
                            game.native_set_setting(m::ID, key, value);
                        }
                        Ok(format!("Loaded {name}: {applied} keys"))
                    }
                    Err(error) => Err(format!("Could not load {name}: {error}")),
                },
                Err(error) => Err(error),
            };
            config_result(result, Some(&name));
        }
        m::ConfigAction::OpenFolder => {
            let result = match m::config_directory() {
                Ok(dir) => {
                    crate::game::packs::open_folder(&dir);
                    Ok(format!("Opened {}", dir.display()))
                }
                Err(error) => Err(format!("Config folder unavailable: {error}")),
            };
            config_result(result, None);
        }
    }
}

/// `config/stariy-bog/<name>.cfg`, or why the folder is unavailable.
fn config_path(name: &str) -> Result<std::path::PathBuf, String> {
    m::config_directory().map(|dir| dir.join(format!("{name}.cfg")))
}

/// Shows a Config tab action's result and refreshes the list and name field
/// after a successful write or load.
fn config_result(result: Result<String, String>, name: Option<&str>) {
    m::with(|c| match result {
        Ok(message) => {
            if let Some(name) = name {
                c.gui.config_selected = Some(name.to_owned());
                c.gui.config_name = name.to_owned();
                c.gui.configs_dirty = true;
            }
            c.gui.config_message = message;
            c.gui.config_error = false;
        }
        Err(message) => {
            c.gui.config_message = message;
            c.gui.config_error = true;
        }
    });
}

fn draw_client(game: &Game<'_>, batch: &mut HudBatch, c: &mut m::Client) -> bool {
    let (sw, sh) = (batch.width, batch.height);
    let (mut w, mut h) = c.gui.clamped_size();
    // Never draw past the screen, and keep the resize grip reachable.
    w = w.min(sw).max(m::MIN_W.min(sw));
    h = h.min(sh).max(m::MIN_H.min(sh));
    c.gui.size = (w, h);
    let mut pos = c
        .gui
        .pos
        .unwrap_or_else(|| ((sw - w) * 0.5, (sh - h) * 0.5));
    pos.0 = pos.0.clamp(0.0, (sw - w).max(0.0));
    pos.1 = pos.1.clamp(0.0, (sh - h).max(0.0));
    c.gui.pos = Some(pos);

    // Copy the input edges: the widgets read them, then the draw clears them.
    let input = c.input;
    let modules = &mut c.modules;
    let gui = &mut c.gui;
    let mut ui = Ui {
        game,
        batch,
        input,
        gui,
        mouse: game.cursor,
        close: false,
    };

    ui.window_drag();
    let pos = ui.gui.pos.unwrap_or(pos);
    let (w, h) = ui.gui.size;

    // A dim over the world (and the HUD), then the window. The outer rounded
    // rectangle is the border; the inner one is the body.
    ui.fill(0.0, 0.0, sw, sh, DIM_SCREEN);
    ui.rounded_rect(pos.0, pos.1, w, h, RADIUS, BORDER);
    ui.rounded_rect(
        pos.0 + 1.0,
        pos.1 + 1.0,
        w - 2.0,
        h - 2.0,
        (RADIUS - 1.0).max(1.0),
        BG,
    );
    ui.rounded_top(
        pos.0 + 1.0,
        pos.1 + 1.0,
        w - 2.0,
        TITLE_H - 1.0,
        (RADIUS - 1.0).max(1.0),
        HEADER,
    );
    ui.text("StariyBog", pos.0 + PAD + 1.0, pos.1 + 4.0, 0x9B7BFF, true);
    let version = format!("v{}", m::VERSION);
    ui.text(
        &version,
        pos.0 + w - PAD - ui.tw(&version) - 13.0,
        pos.1 + 4.0,
        MUTED,
        false,
    );
    if ui.button(pos.0 + w - 13.0, pos.1 + 3.0, 9.0, 9.0, "x", false) {
        ui.close = true;
    }

    // Category tabs.
    let tab_y = pos.1 + TITLE_H;
    let tab_w = (w - PAD * 2.0) / m::Category::ALL.len() as f32;
    for (i, category) in m::Category::ALL.iter().enumerate() {
        let x = pos.0 + PAD + i as f32 * tab_w;
        if ui.button(
            x,
            tab_y,
            tab_w - 1.0,
            TABS_H,
            category.label(),
            i == ui.gui.category,
        ) {
            ui.gui.category = i;
            ui.gui.list_scroll = 0;
            ui.gui.settings_scroll = 0;
            ui.gui.listening = None;
            ui.gui.editing = None;
            ui.gui.configs_dirty = true;
            let first = modules
                .iter()
                .find(|module| module.category.index() == i)
                .map(|module| module.id);
            ui.gui.selected = first;
        }
    }

    let content_y = pos.1 + TITLE_H + TABS_H + PAD;
    let content_h = h - TITLE_H - TABS_H - PAD * 2.0;
    let list_x = pos.0 + PAD;
    let pane_x = list_x + LIST_W + PAD;
    let pane_w = w - (pane_x - pos.0) - PAD;

    if m::Category::ALL.get(ui.gui.category) == Some(&m::Category::Config) {
        draw_configs(&mut ui, list_x, pane_x, pane_w, content_y, content_h);
    } else {
        // Module list.
        ui.fill(list_x, content_y, LIST_W, content_h, PANEL);
        let visible: Vec<usize> = modules
            .iter()
            .enumerate()
            .filter(|(_, module)| module.category.index() == ui.gui.category)
            .map(|(i, _)| i)
            .collect();
        let visible_rows = (content_h / ROW_H).floor() as usize;
        let max_scroll = visible.len().saturating_sub(visible_rows);
        if ui.input.scroll != 0.0 && ui.hovered(list_x, content_y, LIST_W, content_h) {
            ui.gui.list_scroll = (ui.gui.list_scroll as f32 - ui.input.scroll)
                .clamp(0.0, max_scroll as f32) as usize;
        }
        let start = ui.gui.list_scroll.min(max_scroll);
        for (slot, &index) in visible.iter().enumerate().skip(start).take(visible_rows) {
            let y = content_y + (slot - start) as f32 * ROW_H;
            let module = &mut modules[index];
            let hovered = ui.hovered(list_x, y, LIST_W, ROW_H);
            if hovered {
                ui.fill(list_x, y, LIST_W, ROW_H, HOVER);
            }
            if ui.gui.selected == Some(module.id) {
                ui.fill(list_x, y, 2.0, ROW_H, ACCENT);
            }
            let mut enabled = module.enabled;
            if ui.checkbox(list_x + 5.0, y + 2.5, 9.0, &mut enabled) {
                module.enabled = enabled;
                game.native_set_setting(
                    m::ID,
                    &format!("enabled.{}", module.id),
                    if enabled { "true" } else { "false" },
                );
            }
            let color = if module.enabled { TEXT } else { MUTED };
            ui.text(module.name, list_x + 18.0, y + 4.0, color, false);
            if ui.input.pressed && hovered {
                if ui.gui.selected != Some(module.id) {
                    ui.gui.listening = None;
                    ui.gui.editing = None;
                }
                ui.gui.selected = Some(module.id);
            }
        }
        if visible.is_empty() {
            ui.text(
                "No modules yet",
                list_x + 5.0,
                content_y + 5.0,
                MUTED,
                false,
            );
        }

        // Settings pane for the selected module.
        ui.fill(pane_x, content_y, pane_w, content_h, PANEL);
        let selected = ui
            .gui
            .selected
            .and_then(|id| modules.iter().position(|module| module.id == id));
        if let Some(index) = selected {
            let module = &mut modules[index];
            draw_settings(&mut ui, game, module, pane_x, pane_w, content_y, content_h);
        } else {
            ui.text(
                "Select a module",
                pane_x + 6.0,
                content_y + 5.0,
                MUTED,
                false,
            );
        }
    }

    // The resize grip: a small translucent blue triangle inside the rounded
    // bottom-right corner, right-angled at the corner and growing left/up.
    let grip_x = pos.0 + w - GRIP - GRIP_INSET;
    let grip_y = pos.1 + h - GRIP - GRIP_INSET;
    let grip_color = if ui.hovered(grip_x, grip_y, GRIP, GRIP) {
        GRIP_HOVER
    } else {
        GRIP_IDLE
    };
    for i in 0..GRIP as i32 {
        let t = (i as f32 + 0.5) / GRIP;
        let width = GRIP * t;
        ui.fill(
            grip_x + GRIP - width,
            grip_y + i as f32,
            width,
            1.0,
            grip_color,
        );
    }

    let close = ui.close;
    c.input.pressed = false;
    c.input.scroll = 0.0;
    close
}

/// The Config tab: the saved configs on the left and the name field and
/// buttons on the right. File work is queued in `config_pending` and run after
/// the draw, when the registry borrow has ended.
fn draw_configs(
    ui: &mut Ui<'_, '_>,
    list_x: f32,
    pane_x: f32,
    pane_w: f32,
    content_y: f32,
    content_h: f32,
) {
    if ui.gui.configs_dirty {
        match m::config_file_names() {
            Ok(names) => {
                ui.gui.configs = names;
                ui.gui.configs_dirty = false;
                if let Some(selected) = &ui.gui.config_selected
                    && !ui.gui.configs.contains(selected)
                {
                    ui.gui.config_selected = None;
                }
            }
            Err(error) => {
                ui.gui.configs.clear();
                ui.gui.configs_dirty = false;
                ui.gui.config_message = format!("Config folder unavailable: {error}");
                ui.gui.config_error = true;
            }
        }
    }

    // Saved configs.
    ui.fill(list_x, content_y, LIST_W, content_h, PANEL);
    let configs = ui.gui.configs.clone();
    let visible_rows = (content_h / ROW_H).floor() as usize;
    let max_scroll = configs.len().saturating_sub(visible_rows);
    if ui.input.scroll != 0.0 && ui.hovered(list_x, content_y, LIST_W, content_h) {
        ui.gui.list_scroll =
            (ui.gui.list_scroll as f32 - ui.input.scroll).clamp(0.0, max_scroll as f32) as usize;
    }
    let start = ui.gui.list_scroll.min(max_scroll);
    for (slot, name) in configs.iter().enumerate().skip(start).take(visible_rows) {
        let y = content_y + (slot - start) as f32 * ROW_H;
        let hovered = ui.hovered(list_x, y, LIST_W, ROW_H);
        if hovered {
            ui.fill(list_x, y, LIST_W, ROW_H, HOVER);
        }
        if ui.gui.config_selected.as_deref() == Some(name.as_str()) {
            ui.fill(list_x, y, 2.0, ROW_H, ACCENT);
        }
        ui.text(
            &fit_tail(name, LIST_W - 12.0),
            list_x + 5.0,
            y + 4.0,
            if ui.gui.config_selected.as_deref() == Some(name.as_str()) {
                TEXT
            } else {
                MUTED
            },
            false,
        );
        if ui.input.pressed && hovered {
            ui.gui.config_selected = Some(name.clone());
            ui.gui.config_name = name.clone();
            ui.gui.editing = None;
        }
    }
    if configs.is_empty() {
        ui.text(
            "No configs yet",
            list_x + 5.0,
            content_y + 5.0,
            MUTED,
            false,
        );
    }

    // Name and actions.
    ui.fill(pane_x, content_y, pane_w, content_h, PANEL);
    let x = pane_x + 7.0;
    let right = pane_x + pane_w - 7.0;
    let mut y = content_y + 5.0;
    ui.text("Configs", x, y, 0x9B7BFF, true);
    y += 14.0;
    ui.text("Name", x, y + 4.0, TEXT, false);
    let field_x = x + (ui.tw("Name") + 10.0).max(42.0);
    let field_w = (right - field_x).max(40.0);
    let name = ui.gui.config_name.clone();
    let editing = ui.gui.editing == Some(m::Editing::ConfigName);
    if ui.text_field(field_x, y + 1.0, field_w, 14.0, &name, editing) {
        ui.gui.editing = Some(m::Editing::ConfigName);
    }
    y += SETTING_H + 2.0;

    let gap = 4.0;
    let half = ((pane_w - 14.0 - gap) * 0.5).max(40.0);
    if ui.button(x, y, half, 15.0, "Make", false) {
        match m::valid_config_name(&ui.gui.config_name) {
            Some(name) => ui.gui.config_pending = Some(m::ConfigAction::Make(name)),
            None => set_config_error(ui, "Use letters, digits, - and _ (max 32)"),
        }
    }
    if ui.button(x + half + gap, y, half, 15.0, "Save", false) {
        match config_target(ui.gui) {
            Some(name) => ui.gui.config_pending = Some(m::ConfigAction::Save(name)),
            None => set_config_error(ui, "Type a name for the config"),
        }
    }
    y += 19.0;
    if ui.button(x, y, half, 15.0, "Load", false) {
        match config_target(ui.gui) {
            Some(name) => ui.gui.config_pending = Some(m::ConfigAction::Load(name)),
            None => set_config_error(ui, "Pick or type a config name"),
        }
    }
    if ui.button(x + half + gap, y, half, 15.0, "Open Folder", false) {
        ui.gui.config_pending = Some(m::ConfigAction::OpenFolder);
    }
    y += 22.0;

    let selected = ui.gui.config_selected.clone();
    let line = selected.map_or_else(
        || "No config selected".to_owned(),
        |name| format!("Selected: {name}"),
    );
    ui.text(&line, x, y, MUTED, false);
    y += 13.0;
    let message = ui.gui.config_message.clone();
    let color = if ui.gui.config_error { RED } else { GREEN };
    for line in wrap(&message, pane_w - 14.0) {
        ui.text(&line, x, y, color, false);
        y += 12.0;
    }
}

/// The name a Save or Load button uses: the field, or the picked config when
/// the field was cleared.
fn config_target(gui: &m::GuiState) -> Option<String> {
    m::valid_config_name(&gui.config_name).or_else(|| gui.config_selected.clone())
}

fn set_config_error(ui: &mut Ui<'_, '_>, message: &str) {
    ui.gui.config_message.clear();
    ui.gui.config_message.push_str(message);
    ui.gui.config_error = true;
}

fn draw_settings(
    ui: &mut Ui<'_, '_>,
    game: &Game<'_>,
    module: &mut m::Module,
    pane_x: f32,
    pane_w: f32,
    content_y: f32,
    content_h: f32,
) {
    let x = pane_x + 7.0;
    let mut y = content_y + 5.0;
    ui.text(module.name, x, y, 0x9B7BFF, true);
    let state = if module.enabled { "ON" } else { "OFF" };
    let state_color = if module.enabled { GREEN } else { MUTED };
    ui.text(
        state,
        pane_x + pane_w - 7.0 - ui.tw(state),
        y,
        state_color,
        false,
    );
    y += 13.0;
    for line in wrap(module.description, pane_w - 14.0) {
        ui.text(&line, x, y, MUTED, false);
        y += 12.0;
    }
    y += 5.0;

    // Bind row: click the key box, then press a key (or Escape to cancel).
    let bind_label = "Bind";
    ui.text(bind_label, x, y + 4.0, TEXT, false);
    let bx = x + (ui.tw(bind_label) + 10.0).max(60.0);
    let bxx = pane_x + pane_w - 7.0;
    let has_bind = module.bind.is_some();
    let clear_w = if has_bind { 15.0 } else { 0.0 };
    let bw = (bxx - clear_w - bx).max(40.0);
    let listening = ui.gui.listening == Some(module.id);
    let bind_text = if listening {
        "Press a key...".to_owned()
    } else {
        module
            .bind
            .as_deref()
            .map(|key| horizon_client::keybinds::display_name(key).plain())
            .unwrap_or_else(|| "None".to_owned())
    };
    if ui.button(bx, y + 2.0, bw, 12.0, &bind_text, listening) {
        ui.gui.listening = Some(module.id);
    }
    if has_bind && ui.button(bxx - 13.0, y + 2.0, 13.0, 12.0, "x", false) {
        module.bind = None;
        game.native_set_setting(m::ID, &format!("bind.{}", module.id), "");
    }
    y += SETTING_H;

    // Only the selected mode's settings are listed; the rest keep their
    // values for when that mode is picked again.
    let visible: Vec<usize> = (0..module.settings.len())
        .filter(|&i| module.setting_visible(&module.settings[i]))
        .collect();
    let visible_rows = ((content_y + content_h - y) / SETTING_H).floor().max(0.0) as usize;
    let max_scroll = visible.len().saturating_sub(visible_rows);
    if ui.input.scroll != 0.0 && ui.hovered(pane_x, content_y, pane_w, content_h) {
        ui.gui.settings_scroll = (ui.gui.settings_scroll as f32 - ui.input.scroll)
            .clamp(0.0, max_scroll as f32) as usize;
    }
    let start = ui.gui.settings_scroll.min(max_scroll);

    for (slot, &index) in visible.iter().enumerate().skip(start).take(visible_rows) {
        let row = y + (slot - start) as f32 * SETTING_H;
        let setting = &mut module.settings[index];
        match setting.kind {
            m::SettingKind::Bool => {
                let mut value = setting.as_bool();
                if ui.checkbox(x, row + 3.0, 9.0, &mut value) {
                    setting.value = m::Value::Bool(value);
                    save_setting(game, module.id, setting);
                }
                ui.text(setting.label, x + 15.0, row + 4.0, TEXT, false);
            }
            m::SettingKind::Int { min, max, step } => {
                slider_row(
                    ui,
                    game,
                    module.id,
                    setting,
                    x,
                    pane_x,
                    pane_w,
                    row,
                    min as f32,
                    max as f32,
                    step.max(1) as f32,
                    true,
                );
            }
            m::SettingKind::Float { min, max, step } => {
                slider_row(
                    ui, game, module.id, setting, x, pane_x, pane_w, row, min, max, step, false,
                );
            }
            m::SettingKind::Choice(options) => {
                ui.text(setting.label, x, row + 4.0, TEXT, false);
                let arrow_x = pane_x + pane_w - 7.0 - 15.0;
                // Long labels push the arrows right instead of running into them.
                let left_x = x + (ui.tw(setting.label) + 10.0).max(60.0);
                let value_x = left_x + 18.0;
                let mut choice = setting.as_choice();
                let mut changed = false;
                if ui.button(left_x, row + 2.0, 13.0, 12.0, "<", false) {
                    choice = (choice + options.len() - 1) % options.len().max(1);
                    changed = true;
                }
                if ui.button(arrow_x, row + 2.0, 13.0, 12.0, ">", false) {
                    choice = (choice + 1) % options.len().max(1);
                    changed = true;
                }
                ui.text(setting.choice_label(), value_x, row + 4.0, 0x9B7BFF, false);
                if changed {
                    setting.value = m::Value::Choice(choice);
                    save_setting(game, module.id, setting);
                }
            }
            m::SettingKind::Text => {
                ui.text(setting.label, x, row + 4.0, TEXT, false);
                let left_x = x + (ui.tw(setting.label) + 10.0).max(60.0);
                let field_w = (pane_x + pane_w - 7.0 - left_x).max(30.0);
                let editing = ui.gui.editing
                    == Some(m::Editing::Setting {
                        module: module.id,
                        setting: setting.id,
                    });
                if ui.text_field(left_x, row + 2.0, field_w, 14.0, setting.as_text(), editing) {
                    ui.gui.editing = Some(m::Editing::Setting {
                        module: module.id,
                        setting: setting.id,
                    });
                }
            }
        }
    }
}

fn slider_row(
    ui: &mut Ui<'_, '_>,
    game: &Game<'_>,
    module: &str,
    setting: &mut m::Setting,
    x: f32,
    pane_x: f32,
    pane_w: f32,
    row: f32,
    min: f32,
    max: f32,
    step: f32,
    integer: bool,
) {
    ui.text(setting.label, x, row + 4.0, TEXT, false);
    let value_text = if integer {
        format!("{}", setting.as_int())
    } else {
        format!("{:.2}", setting.as_float())
    };
    let value_x = pane_x + pane_w - 7.0 - ui.tw(&value_text);
    ui.text(&value_text, value_x, row + 4.0, MUTED, false);
    // Keep the track clear of the label, however long it is.
    let track_x = x + (ui.tw(setting.label) + 10.0).max(72.0);
    let track_w = (value_x - track_x - 6.0).max(20.0);
    let id = m::widget_id(module, setting.id);
    if let Some(value) = ui.slider(
        id,
        track_x,
        row + 7.0,
        track_w,
        setting.as_float(),
        min,
        max,
        step,
    ) {
        setting.set_number(value);
        save_setting(game, module, setting);
    }
}

fn save_setting(game: &Game<'_>, module: &str, setting: &m::Setting) {
    game.native_set_setting(m::ID, &format!("{module}.{}", setting.id), &setting.text());
}

/// The immediate-mode UI: one pass, explicit rectangles, edges consumed by the
/// draw. This mirrors the calls imgui-rs exposes (`button`, `checkbox`,
/// `slider`) without its context/renderer plumbing.
struct Ui<'a, 'g> {
    game: &'a Game<'g>,
    batch: &'a mut HudBatch,
    input: m::Input,
    gui: &'a mut m::GuiState,
    mouse: (f32, f32),
    close: bool,
}

impl Ui<'_, '_> {
    fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, argb: u32) {
        self.game.native_fill(self.batch, [x, y, w, h], argb);
    }

    fn border(&mut self, x: f32, y: f32, w: f32, h: f32, argb: u32) {
        self.fill(x, y, w, 1.0, argb);
        self.fill(x, y + h - 1.0, w, 1.0, argb);
        self.fill(x, y, 1.0, h, argb);
        self.fill(x + w - 1.0, y, 1.0, h, argb);
    }

    /// A filled rectangle with all four corners rounded, drawn as one-pixel
    /// rows so the curve stays inside the edges. `r` is clamped to half the
    /// smaller side.
    fn rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, argb: u32) {
        let r = r.max(0.0).min(w * 0.5).min(h * 0.5);
        if r < 1.0 {
            self.fill(x, y, w, h, argb);
            return;
        }
        let steps = r.ceil() as i32;
        for i in 0..steps {
            let row = i as f32;
            let dy = r - (row + 0.5);
            let inset = r - (r * r - dy * dy).max(0.0).sqrt();
            let row_w = w - inset * 2.0;
            self.fill(x + inset, y + row, row_w, 1.0, argb);
            self.fill(x + inset, y + h - row - 1.0, row_w, 1.0, argb);
        }
        let middle = h - r * 2.0;
        if middle > 0.0 {
            self.fill(x, y + r, w, middle, argb);
        }
    }

    /// Rounded at the top, square at the bottom (the title bar).
    fn rounded_top(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, argb: u32) {
        let r = r.max(0.0).min(w * 0.5).min(h);
        if r < 1.0 {
            self.fill(x, y, w, h, argb);
            return;
        }
        let steps = r.ceil() as i32;
        for i in 0..steps {
            let row = i as f32;
            let dy = r - (row + 0.5);
            let inset = r - (r * r - dy * dy).max(0.0).sqrt();
            self.fill(x + inset, y + row, w - inset * 2.0, 1.0, argb);
        }
        let middle = h - r;
        if middle > 0.0 {
            self.fill(x, y + r, w, middle, argb);
        }
    }

    fn text(&mut self, text: &str, x: f32, y: f32, rgb: u32, shadow: bool) {
        crate::game::clickgui_font::draw(self.game, self.batch, text, [x, y], rgb, shadow);
    }

    fn tw(&self, text: &str) -> f32 {
        crate::game::clickgui_font::width(text)
    }

    fn hovered(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        let (mx, my) = self.mouse;
        mx >= x && mx < x + w && my >= y && my < y + h
    }

    fn button(&mut self, x: f32, y: f32, w: f32, h: f32, label: &str, active: bool) -> bool {
        let hovered = self.hovered(x, y, w, h);
        let fill = if active {
            ACCENT
        } else if hovered {
            HOVER
        } else {
            PANEL
        };
        self.fill(x, y, w, h, fill);
        let color = if active {
            ON_ACCENT
        } else if hovered {
            TEXT
        } else {
            MUTED
        };
        let tx = x + ((w - self.tw(label)) * 0.5).max(0.0);
        let ty = y + ((h - 8.0) * 0.5).max(0.0);
        self.text(label, tx, ty, color, false);
        self.input.pressed && hovered
    }

    /// A one-line text box. Returns whether it was clicked; the caret shows
    /// while `editing`, and long text scrolls to its tail.
    fn text_field(&mut self, x: f32, y: f32, w: f32, h: f32, text: &str, editing: bool) -> bool {
        let hovered = self.hovered(x, y, w, h);
        let fill = if editing {
            TRACK
        } else if hovered {
            HOVER
        } else {
            PANEL
        };
        self.fill(x, y, w, h, fill);
        self.border(x, y, w, h, if editing { ACCENT } else { BORDER });
        let ty = y + ((h - 8.0) * 0.5).max(0.0);
        let (shown, color) = if text.is_empty() {
            ("empty".to_owned(), MUTED)
        } else {
            (fit_tail(text, w - 7.0), TEXT)
        };
        self.text(&shown, x + 3.0, ty, color, false);
        if editing {
            let caret = if text.is_empty() {
                x + 3.0
            } else {
                x + 3.0 + self.tw(&shown) + 1.0
            };
            self.fill(caret.min(x + w - 3.0), ty - 1.0, 1.0, 10.0, TEXT);
        }
        self.input.pressed && hovered
    }

    fn checkbox(&mut self, x: f32, y: f32, size: f32, value: &mut bool) -> bool {
        let hovered = self.hovered(x - 2.0, y - 2.0, size + 4.0, size + 4.0);
        let mut clicked = false;
        if self.input.pressed && hovered {
            *value = !*value;
            clicked = true;
        }
        self.fill(x, y, size, size, if *value { ACCENT } else { TRACK });
        self.border(x, y, size, size, if *value { ACCENT } else { BORDER });
        if *value {
            // A tiny check mark: two 2px steps.
            self.fill(x + 2.0, y + 4.0, 2.0, 2.0, ON_ACCENT);
            self.fill(x + 4.0, y + 2.0, 2.0, 4.0, ON_ACCENT);
        }
        clicked
    }

    fn slider(
        &mut self,
        id: u64,
        x: f32,
        y: f32,
        w: f32,
        value: f32,
        min: f32,
        max: f32,
        step: f32,
    ) -> Option<f32> {
        let hovered = self.hovered(x, y - 3.0, w, 9.0);
        let mut out = None;
        let mut active = self.gui.drag == Some(m::Drag::Slider(id));
        if self.input.pressed && hovered {
            self.gui.drag = Some(m::Drag::Slider(id));
            active = true;
            out = Some(self.slider_value(x, w, min, max, step));
        }
        if !self.input.down && active {
            self.gui.drag = None;
            active = false;
        }
        if active && self.input.down {
            out = Some(self.slider_value(x, w, min, max, step));
        }

        let t = ((value - min) / (max - min).max(f32::EPSILON)).clamp(0.0, 1.0);
        self.fill(x, y, w, 3.0, TRACK);
        self.fill(x, y, w * t, 3.0, ACCENT);
        self.fill(x + (w * t - 1.5).max(0.0), y - 2.0, 3.0, 7.0, TEXT);
        out
    }

    fn slider_value(&self, x: f32, w: f32, min: f32, max: f32, step: f32) -> f32 {
        let t = ((self.mouse.0 - x) / w.max(1.0)).clamp(0.0, 1.0);
        let raw = min + t * (max - min);
        let step = if step > 0.0 { step } else { 0.01 };
        ((raw / step).round() * step).clamp(min, max)
    }

    fn window_drag(&mut self) {
        let (sw, sh) = (self.batch.width, self.batch.height);
        let (w, h) = self.gui.clamped_size();
        let pos = self.gui.pos.unwrap_or_default();
        let on_grip = self.hovered(
            pos.0 + w - GRIP - GRIP_INSET,
            pos.1 + h - GRIP - GRIP_INSET,
            GRIP,
            GRIP,
        );
        let on_title = self.hovered(pos.0, pos.1, w, TITLE_H);
        let on_close = self.hovered(pos.0 + w - 13.0, pos.1 + 3.0, 9.0, 9.0);
        if self.input.pressed {
            if on_grip {
                self.gui.drag = Some(m::Drag::Resize {
                    offset: (pos.0 + w - self.mouse.0, pos.1 + h - self.mouse.1),
                });
            } else if on_title && !on_close {
                self.gui.drag = Some(m::Drag::Window {
                    grab: (self.mouse.0 - pos.0, self.mouse.1 - pos.1),
                });
            }
        }
        match self.gui.drag {
            Some(m::Drag::Window { grab }) => {
                if self.input.down {
                    self.gui.pos = Some((self.mouse.0 - grab.0, self.mouse.1 - grab.1));
                } else {
                    self.gui.drag = None;
                }
            }
            Some(m::Drag::Resize { offset }) => {
                if self.input.down {
                    // Keep the window on screen; the left and top edges stay put.
                    let min_w = m::MIN_W.min(sw);
                    let min_h = m::MIN_H.min(sh);
                    let max_w = (sw - pos.0).max(min_w);
                    let max_h = (sh - pos.1).max(min_h);
                    let w = (self.mouse.0 + offset.0 - pos.0).clamp(min_w, max_w);
                    let h = (self.mouse.1 + offset.1 - pos.1).clamp(min_h, max_h);
                    self.gui.size = (w, h);
                } else {
                    self.gui.drag = None;
                    self.gui.size = self.gui.clamped_size();
                    let (w, h) = self.gui.size;
                    self.game
                        .native_set_setting(m::ID, "window.size", &format!("{w:.0}x{h:.0}"));
                }
            }
            _ => {}
        }
    }
}

/// The tail of `text` that fits `width`, after `..` when it had to be cut.
/// The HUD batch does not clip, so a text box trims instead.
fn fit_tail(text: &str, width: f32) -> String {
    if crate::game::clickgui_font::width(text) <= width {
        return text.to_owned();
    }
    let mut tail = String::new();
    for ch in text.chars().rev() {
        let mut candidate = String::new();
        candidate.push(ch);
        candidate.push_str(&tail);
        if crate::game::clickgui_font::width(&format!("..{candidate}")) > width {
            break;
        }
        tail = candidate;
    }
    format!("..{tail}")
}

/// Greedy word wrap with the ClickGUI font's width, for the description.
fn wrap(text: &str, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && crate::game::clickgui_font::width(&candidate) > width {
            lines.push(line);
            line = word.to_owned();
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
