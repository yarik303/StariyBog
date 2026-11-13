// /clickgui [open|close|reset], /fullbright [on|off], /bind <module> <key> and
// /config [save|load|list|path] [name].
//
// The body lives here rather than in the `native_command` hook point: those
// helpers are inlined into `native_hooks`, and that file has to stay free of
// macros (Horizon's `hook_points_stay_hookable` asserts it, because a macro in
// a hook point stops any mod injecting there). `format!` is a macro, so the
// formatting belongs in a module of its own.
pub(super) fn handle(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    use crate::game::{clickgui as m, clickgui_ui as ui};

    /// A safe file stem for `/config save|load`; an omitted name is `default`.
    fn config_name(arg: &str) -> Option<String> {
        if arg.trim().is_empty() {
            Some("default".to_owned())
        } else {
            m::valid_config_name(arg)
        }
    }

    if !game.native_on(m::ID) {
        return;
    }

    if let Some(args) = command.strip_prefix("clickgui") {
        if !(args.is_empty() || args.starts_with(' ')) {
            return;
        }
        *ran = true;
        match args.trim() {
            "" | "toggle" => {
                if ui::is_open() {
                    ui::close(game);
                    game.native_chat("StariyBog closed");
                } else if ui::open(game) {
                    game.native_chat("StariyBog opened");
                } else {
                    game.native_chat("Join a world to open StariyBog");
                }
            }
            "open" => {
                if ui::open(game) {
                    game.native_chat("StariyBog opened");
                } else {
                    game.native_chat("Join a world to open StariyBog");
                }
            }
            "close" => {
                ui::close(game);
                game.native_chat("StariyBog closed");
            }
            "reset" => {
                ui::reset_position();
                game.native_chat("StariyBog moved back to the centre");
            }
            _ => game.native_chat("Usage: /clickgui [open|close|reset]"),
        }
        return;
    }

    if let Some(args) = command.strip_prefix("fullbright") {
        if !(args.is_empty() || args.starts_with(' ')) {
            return;
        }
        *ran = true;
        let on = match args.trim() {
            "on" => {
                ui::set_enabled(game, m::FULLBRIGHT, true);
                true
            }
            "off" => {
                ui::set_enabled(game, m::FULLBRIGHT, false);
                false
            }
            "" | "toggle" => ui::toggle_module(game, m::FULLBRIGHT),
            _ => {
                game.native_chat("Usage: /fullbright [on|off]");
                return;
            }
        };
        game.native_chat(&format!("FullBright {}", if on { "on" } else { "off" }));
        return;
    }

    if let Some(args) = command.strip_prefix("config") {
        if !(args.is_empty() || args.starts_with(' ')) {
            return;
        }
        *ran = true;
        let mut parts = args.trim().split_whitespace();
        let sub = parts.next().unwrap_or("");
        let arg = parts.next().unwrap_or("");
        match sub {
            "" => game.native_chat("Usage: /config [save|load|list|path] [name]"),
            "path" => match m::config_directory() {
                Ok(dir) => game.native_chat(&format!(
                    "Configs save in {} (one .cfg file per config)",
                    dir.display()
                )),
                Err(error) => game.native_chat(&format!("Config folder unavailable: {error}")),
            },
            "save" => {
                let Some(name) = config_name(arg) else {
                    game.native_chat("Config names use letters, digits, - and _ (max 32)");
                    return;
                };
                ui::ensure_loaded(&*game);
                let document = m::config_document();
                let path = match m::config_directory() {
                    Ok(dir) => dir.join(format!("{name}.cfg")),
                    Err(error) => {
                        game.native_chat(&format!("Config folder unavailable: {error}"));
                        return;
                    }
                };
                match std::fs::write(&path, document) {
                    Ok(()) => {
                        game.native_chat(&format!("Saved config {name} to {}", path.display()))
                    }
                    Err(error) => game.native_chat(&format!(
                        "Could not save config {name} at {}: {error}",
                        path.display()
                    )),
                }
            }
            "load" => {
                let Some(name) = config_name(arg) else {
                    game.native_chat("Config names use letters, digits, - and _ (max 32)");
                    return;
                };
                ui::ensure_loaded(&*game);
                let path = match m::config_directory() {
                    Ok(dir) => dir.join(format!("{name}.cfg")),
                    Err(error) => {
                        game.native_chat(&format!("Config folder unavailable: {error}"));
                        return;
                    }
                };
                match std::fs::read_to_string(&path) {
                    Ok(text) => {
                        let entries = m::parse_config(&text);
                        let applied = m::apply_config(&entries);
                        for (key, value) in &entries {
                            game.native_set_setting(m::ID, key, value);
                        }
                        game.native_chat(&format!(
                            "Loaded config {name}: {applied} keys from {}",
                            path.display()
                        ));
                    }
                    Err(error) => game
                        .native_chat(&format!("No config {name} at {}: {error}", path.display())),
                }
            }
            "list" => match (m::config_directory(), m::config_file_names()) {
                (Ok(dir), Ok(names)) => {
                    if names.is_empty() {
                        game.native_chat(&format!(
                            "No configs saved yet; /config save <name> writes to {}",
                            dir.display()
                        ));
                    } else {
                        game.native_chat(&format!(
                            "Configs ({}): {} (in {})",
                            names.len(),
                            names.join(", "),
                            dir.display()
                        ));
                    }
                }
                (_, Err(error)) | (Err(error), _) => {
                    game.native_chat(&format!("Config folder unavailable: {error}"))
                }
            },
            _ => game.native_chat("Usage: /config [save|load|list|path] [name]"),
        }
        return;
    }

    if let Some(args) = command.strip_prefix("bind") {
        if !(args.is_empty() || args.starts_with(' ')) {
            return;
        }
        *ran = true;
        let mut parts = args.trim().splitn(2, char::is_whitespace);
        let module_arg = parts.next().unwrap_or("").trim();
        let key_arg = parts.next().unwrap_or("").trim();
        if module_arg.is_empty() {
            game.native_chat("Usage: /bind <module> <key>  (or /bind <module> clear)");
            return;
        }
        let Some(id) = m::module_id(module_arg) else {
            game.native_chat(&format!("No module named {module_arg}"));
            return;
        };
        let name = m::module_name(id).unwrap_or(id);
        if key_arg.is_empty()
            || key_arg.eq_ignore_ascii_case("clear")
            || key_arg.eq_ignore_ascii_case("none")
        {
            ui::set_bind(game, id, None);
            game.native_chat(&format!("{name} is unbound"));
            return;
        }
        match ui::parse_key(key_arg) {
            Some(key) => {
                if key == game.native_mod_key(m::ID, m::KEY, m::DEFAULT_KEY) {
                    game.native_chat("That key opens StariyBog; pick another");
                    return;
                }
                let shown = horizon_client::keybinds::display_name(&key).plain();
                ui::set_bind(game, id, Some(key));
                game.native_chat(&format!("{name} bound to {shown}"));
            }
            None => game.native_chat(&format!(
                "Unknown key '{key_arg}' (try g, space, right shift, mouse.right)"
            )),
        }
    }
}
