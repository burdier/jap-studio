//! Teclas y comandos de texto. Ratón y teclado comparten acciones de edición.
use super::{
    CenterView, DawApp, VimMode,
    cursor_fx::CursorFxMode,
    design::ThemePreset,
    pattern_editor::PatternAction,
    views::tracks::{TcpControlStyle, TcpLayoutPreset},
};
use gpui::{Context, KeyDownEvent, KeyUpEvent, Window};

impl DawApp {
    pub(super) fn handle_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Cualquier tecla ajena a dd cancela el operador, también dentro de overlays.
        let delete_pending = std::mem::take(&mut self.track_delete_pending);
        let key = event
            .keystroke
            .key_char
            .as_deref()
            .unwrap_or(&event.keystroke.key);
        self.last_key = if event.keystroke.modifiers.control {
            format!("C-{key}")
        } else {
            key.to_string()
        };
        if self.general_note.is_some() {
            self.track_delete_pending = false;
            return;
        }
        if self.action_form.is_some() {
            // Los controles Kit consumen edición/IME; ninguna tecla del formulario toca el DAW.
            return;
        }
        if self.fx.menu.is_some() {
            self.handle_fx_menu_key(event);
            return;
        }
        if self.track_menu.is_some() {
            self.handle_track_menu_key(event, window, cx);
            return;
        }
        if self.palette.is_open() {
            let text = !event.keystroke.modifiers.control && !event.keystroke.modifiers.alt;
            self.handle_palette_key(key, text, window, cx);
            return;
        }
        if self.settings.is_visible() {
            if self.audio_select_popup_focused(window, cx) {
                return;
            }
            self.handle_settings_key(event, cx);
            return;
        }
        if self.toolbar_icons.picker_open {
            self.handle_icon_picker_key(event, cx);
            return;
        }
        if self.clip_menu.is_some() {
            self.handle_clip_menu_key(event, window, cx);
            return;
        }
        if self.handle_performance_key(event) {
            return;
        }
        let modifiers = event.keystroke.modifiers;
        if self.vim_enabled
            && self.mode == VimMode::Normal
            && key == "e"
            && !modifiers.control
            && !modifiers.alt
            && !modifiers.platform
        {
            if !event.is_held {
                self.toggle_explorer();
            }
            return;
        }
        if self.handle_track_delete_key(event, delete_pending) {
            return;
        }
        // GPUI no genera KeyDown por cada carácter pegado: leer explícitamente el clipboard.
        if self.mode == VimMode::Command
            && event.keystroke.modifiers.control
            && key.eq_ignore_ascii_case("v")
        {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.command_buffer
                    .push_str(&text.replace(['\n', '\r'], " "));
                self.cmd_selected = None;
            }
            return;
        }
        if !self.vim_enabled {
            if matches!(
                key,
                "left"
                    | "right"
                    | "up"
                    | "down"
                    | "home"
                    | "insert"
                    | "enter"
                    | "escape"
                    | "tab"
                    | "+"
                    | "="
                    | "-"
                    | "["
                    | "]"
            ) {
                self.handle_normal_key(key);
            }
            return;
        }
        match self.mode {
            VimMode::Normal => self.handle_normal_key(key),
            VimMode::Insert => {
                if matches!(key, "escape" | "enter") {
                    self.set_mode(VimMode::Normal);
                }
            }
            VimMode::Command => {
                self.handle_command_key(key, event.keystroke.modifiers.shift, window, cx)
            }
        }
    }
    pub(super) fn handle_key_up(&mut self, event: &KeyUpEvent) {
        let key = event
            .keystroke
            .key_char
            .as_deref()
            .unwrap_or(&event.keystroke.key);
        if let Some(message) = self.controller.key_up(key) {
            self.apply_midi([message]);
        }
    }
    fn handle_normal_key(&mut self, key: &str) {
        match key {
            "enter" if self.center_view == CenterView::Session => {
                self.apply_pattern_action(PatternAction::Toggle {
                    lane: self.cursor_row,
                    step: self.cursor_col,
                })
            }
            "[" => self.change_gain(-0.1),
            "]" => self.change_gain(0.1),
            "a" | "insert" => self.open_palette(),
            "b" => self.toggle_bypass(),
            "escape" => {
                self.all_notes_off();
                self.status_message = "Normal".into();
            }
            "i" => self.set_mode(VimMode::Insert),
            ":" => self.set_mode(VimMode::Command),
            "tab" => self.set_center_view(match self.center_view {
                CenterView::Arrangement => CenterView::Session,
                CenterView::Session => CenterView::Mixer,
                CenterView::Mixer => CenterView::Arrangement,
            }),
            "h" | "left" if self.center_view == CenterView::Arrangement => {
                self.timeline.move_cursor(-1.0)
            }
            "l" | "right" if self.center_view == CenterView::Arrangement => {
                self.timeline.move_cursor(1.0)
            }
            "home" if self.center_view == CenterView::Arrangement => {
                if let Err(error) = self.seek_audio(0.) {
                    self.status_message = error;
                }
            }
            "+" | "=" if self.center_view == CenterView::Arrangement => {
                self.timeline.zoom_at_cursor(2.0)
            }
            "-" if self.center_view == CenterView::Arrangement => self.timeline.zoom_at_cursor(0.5),
            "h" | "left" => self.cursor_col = self.cursor_col.saturating_sub(1),
            "l" | "right" => {
                self.cursor_col = (self.cursor_col + 1).min(self.pattern_editor.step_count() - 1)
            }
            "k" | "up" if self.center_view == CenterView::Arrangement => {
                self.move_visible_track(false)
            }
            "j" | "down" if self.center_view == CenterView::Arrangement => {
                self.move_visible_track(true)
            }
            "k" | "up" => {
                self.cursor_row = self.cursor_row.saturating_sub(1);
                if self.center_view != CenterView::Session {
                    self.select_track(self.cursor_row, false, false);
                }
                self.reveal_track();
            }
            "j" | "down" => {
                let last = if self.center_view == CenterView::Session {
                    self.pattern_editor.lanes.len() - 1
                } else {
                    self.tracks.len().saturating_sub(1)
                };
                self.cursor_row = (self.cursor_row + 1).min(last);
                if self.center_view != CenterView::Session {
                    self.select_track(self.cursor_row, false, false);
                }
                self.reveal_track();
            }
            "1" => self.set_center_view(CenterView::Arrangement),
            "2" => self.set_center_view(CenterView::Session),
            "3" => self.set_center_view(CenterView::Mixer),
            "4" => self.open_selected_instrument(),
            "R" => {
                self.reload();
                self.status_message = "Design reloaded".into();
            }
            _ => {}
        }
    }
    fn handle_command_key(
        &mut self,
        key: &str,
        shift: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match key {
            "escape" => self.set_mode(VimMode::Normal),
            "up" => self.move_command_selection(true),
            "down" => self.move_command_selection(false),
            "tab" if shift => self.move_command_selection(true),
            "tab" => self.complete_command(),
            "enter" => {
                let cmd = self
                    .cmd_selected
                    .and_then(|index| {
                        self.cmd_completions()
                            .get(index)
                            .map(|(command, _)| (*command).to_string())
                    })
                    .unwrap_or_else(|| self.command_buffer.clone());
                if cmd.trim().is_empty() {
                    self.status_message = "Escribe un comando o elige una sugerencia".into();
                    return;
                }
                self.submit_command_line(&cmd, window, cx);
                self.set_mode(VimMode::Normal);
            }
            "backspace" => {
                self.command_buffer.pop();
                self.cmd_selected = None;
            }
            "space" => {
                self.command_buffer.push(' ');
                self.cmd_selected = None;
            }
            _ if key.chars().count() == 1 => {
                self.command_buffer.push_str(key);
                self.cmd_selected = None;
            }
            _ => {}
        }
    }
    pub(in crate::ui) fn submit_command_line(
        &mut self,
        cmd: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let command = cmd.trim().to_ascii_lowercase();
        if matches!(command.as_str(), "clip.rename" | "clip.gain" | "clip.color") {
            self.open_clip_catalog_page(&command, window.mouse_position(), window.viewport_size());
            cx.notify();
            return;
        }
        let action_command = match command.as_str() {
            "tempo" => "tempo.set",
            "meter" => "meter.set",
            other => other,
        };
        if let Some(entry) = elevate::catalog::entries().iter().find(|entry| {
                matches!(entry.action, elevate::catalog::CatalogAction::Invoke(value) if value == action_command)
            })
            && let Some(spec) = entry.action.parameters()
        {
            self.open_action_form(entry, spec, window, cx);
            return;
        }
        self.execute_command_with_context(cmd, window, cx);
    }
    pub(super) fn execute_command_with_context(
        &mut self,
        cmd: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(
            cmd.trim().to_ascii_lowercase().as_str(),
            "note" | "note.edit"
        ) {
            self.open_general_note(window, cx);
            return;
        }
        if cmd.trim() == "view.maximize" {
            self.toggle_workspace_focus(window, cx);
            return;
        }
        if cmd.trim() == "fx.components.reload" {
            self.reload_lua_components(cx);
            return;
        }
        if let Some(id) = cmd.trim().strip_prefix("fx.component ") {
            self.status_message = self
                .selected_fx_target()
                .ok_or_else(|| "Selecciona una pista".to_string())
                .and_then(|target| self.insert_lua_component(target, id.trim()))
                .map(|()| format!("Componente {id} insertado"))
                .unwrap_or_else(|error| error);
            return;
        }
        if !self.execute_audio_command(cmd.trim(), cx)
            && !self.execute_vst_script_command(cmd.trim(), cx)
            && !self.execute_vst_command(cmd.trim(), cx)
            && !self.execute_roll_command(cmd.trim(), window, cx)
            && !self.execute_tool_command(cmd.trim(), window, cx)
            && !self.execute_track_script_command(cmd.trim(), cx)
            && !self.execute_script_command(cmd.trim(), cx)
        {
            self.execute_command(cmd);
        }
    }
    pub(super) fn execute_command(&mut self, cmd: &str) {
        // El parser de export conserva mayúsculas y espacios entrecomillados en nombres/rutas.
        let cmd = cmd.trim();
        if matches!(cmd, "vim.toggle" | "vim toggle") {
            self.toggle_vim();
            return;
        }
        if self.audio_transport_command(cmd) {
            return;
        }
        if self.execute_roll_grid_command(cmd) {
            return;
        }
        if self.execute_timing_command(cmd) {
            return;
        }
        if self.handle_fx_command(cmd) {
            return;
        }
        if cmd
            .split_whitespace()
            .next()
            .is_some_and(|word| word.eq_ignore_ascii_case("file.import"))
        {
            match elevate::actions::tokenize(cmd) {
                Ok(parts) if parts.len() == 2 && !parts[1].trim().is_empty() => {
                    self.request_file_import(super::explorer_host::ImportSource::Path(
                        parts[1].clone().into(),
                    ));
                }
                _ => self.status_message = "Uso: file.import '/ruta/archivo.wav'".into(),
            }
            return;
        }
        if cmd
            .split_whitespace()
            .next()
            .is_some_and(|word| word.eq_ignore_ascii_case("export"))
        {
            self.start_export(cmd);
            return;
        }
        if let Some(raw) = cmd.strip_prefix("track.rename ") {
            match elevate::actions::tokenize(raw) {
                Ok(parts) if parts.len() == 1 && !parts[0].trim().is_empty() => {
                    if let Some(track) = self.tracks.get_mut(self.cursor_row) {
                        track.name = parts[0].trim().to_string().into();
                        self.track_revision = self.track_revision.wrapping_add(1);
                    }
                }
                _ => self.status_message = "Uso: track.rename 'Nombre de pista'".into(),
            }
            return;
        }
        match cmd.to_lowercase().as_str() {
            "explorer.toggle" | "explorer" => self.toggle_explorer(),
            "explorer.insert" => self.request_file_import(super::explorer_host::ImportSource::ExplorerSelection),
            "track.fold" => self.toggle_track_fold(self.cursor_row),
            "track.height.compact" => self.set_selected_track_height(Some(super::track_layout::COMPACT_TRACK_HEIGHT)),
            "track.height.default" => self.set_selected_track_height(None),
            "track.height.tall" => self.set_selected_track_height(Some(120.0)),
            other if other.starts_with("track.height ") => {
                match other[13..].trim().parse::<f32>() {
                    Ok(height) => self.set_selected_track_height(Some(height)),
                    Err(_) => self.status_message = "Uso: track.height 40 (32–240 px)".into(),
                }
            }
            "track.insert" => self.edit_track_structure(elevate::musical::track::TrackEdit::Insert),
            "track.delete" => self.edit_track_structure(elevate::musical::track::TrackEdit::Delete),
            "track.duplicate" => self.edit_track_structure(elevate::musical::track::TrackEdit::Duplicate),
            "track.group" => self.edit_track_structure(elevate::musical::track::TrackEdit::Group),
            "track.ungroup" => self.edit_track_structure(elevate::musical::track::TrackEdit::Ungroup),
            "hilo.layer" => self.set_hilo_mode(Some(elevate::musical::track::HiloMode::Layer)),
            "hilo.rr" | "hilo.round_robin" => self.set_hilo_mode(Some(elevate::musical::track::HiloMode::RoundRobin)),
            "hilo.off" => self.set_hilo_mode(None),
            "track.midi" => self.toggle_track_midi(self.cursor_row),
            "track.mute" => self.toggle_track_mute(self.cursor_row),
            "track.solo" => self.toggle_track_solo(self.cursor_row),
            "export.settings" => self.open_export_settings(),
            "export.cancel" => self.cancel_export(),
            "export.help" => self.status_message = "export -n Nombre.[wav,mp3,aiff,flac] -master|-individuals · -sr Hz -bits 16|24|32|32f -bitrate kbps|-vbr 0–9 -mono -tracks 1,3 -start beats -length beats -tail segundos -master-fx on|off -out carpeta · docs/EXPORT.md".into(),
            "utility" | "utility.open" => self.open_utility(),
            "utility.remove" => {
                self.apply_utility_action(elevate::devices::utility_slot::UtilityAction::Remove)
            }
            "utility.reset" => {
                self.apply_utility_action(elevate::devices::utility_slot::UtilityAction::Reset)
            }
            "utility.bypass" => self.toggle_bypass(),
            other if other.starts_with("utility.gain ") => {
                match other
                    .trim_start_matches("utility.gain ")
                    .trim()
                    .parse::<f32>()
                {
                    Ok(value) => self.apply_utility_action(
                        elevate::devices::utility_slot::UtilityAction::SetGain(value),
                    ),
                    Err(_) => self.status_message = "Uso: utility.gain 0.5 (0–2x)".into(),
                }
            }
            "sa" | "track.insert_sampler" => self.insert_sampler_track(),
            "sampler" => {
                if self.tracks.is_empty() { self.edit_track_structure(elevate::musical::track::TrackEdit::Insert); }
                if self.track_accepts_clips(self.cursor_row) { self.add_sampler(self.cursor_row); }
            }
            "add" | "browse" | "insert" => self.open_palette(),
            "play" => self.play_context(),
            "stop" => {
                self.stop_all();
                self.status_message = "Detenido".into();
            }
            "record" | "rec" => self.toggle_midi_record(),
            "capture" | "dump" => self.capture_midi_to_track(),
            "materialize" => self.materialize_midi_selection(),
            "arrangement" | "arr" => self.set_center_view(CenterView::Arrangement),
            "piano" | "instrument" | "inst" => self.open_selected_instrument(),
            "session" | "pat" | "pattern" => self.set_center_view(CenterView::Session),
            "mixer" | "mix" => self.set_center_view(CenterView::Mixer),
            "midi" | "kbd" => self.toggle_controller(),
            "detach" | "float" => self.window_request = Some(true),
            "dock" | "attach" => self.window_request = Some(false),
            "reload" => {
                self.reload();
                self.status_message = "Design reloaded".into();
            }
            "learn" => {
                self.toggle_midi_learn();
            }
            other if other.starts_with("clip.") => self.execute_clip_catalog_action(other),
            "midi-scan" => {
                self.midi_hw.reconnect();
                let ports = self.midi_hw.port_names();
                self.status_message = if ports.is_empty() {
                    "Sin controladores MIDI USB detectados".into()
                } else {
                    format!("MIDI Hardware conectado: {}", ports.join(", "))
                };
            }
            "theme" => {
                self.cycle_theme();
            }
            "icons" | "toolbar" => {
                self.toolbar_icons
                    .open_picker(super::icons::ToolbarScope::Transport);
            }
            other if other.starts_with("theme ") => {
                let arg = other.trim_start_matches("theme ").trim();
                self.set_theme(ThemePreset::from_token(arg));
            }
            other if other.starts_with("cursor ") => {
                let arg = other.trim_start_matches("cursor ").trim();
                self.set_cursor_fx(CursorFxMode::from_token(arg));
            }
            other if other.starts_with("tcp ") => {
                let arg = other.trim_start_matches("tcp ").trim();
                if matches!(arg, "knobs" | "perillas" | "bars" | "barras") {
                    self.set_tcp_control_style(TcpControlStyle::from_token(arg));
                } else if let Some(preset) = TcpLayoutPreset::from_token(arg) {
                    self.set_tcp_preset(preset);
                } else {
                    self.status_message = format!("? tcp {arg}");
                }
            }
            _ => self.status_message = format!("? {cmd}"),
        }
    }
}
