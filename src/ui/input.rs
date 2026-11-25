//! Teclas y comandos de texto. Ratón y teclado comparten acciones de edición.
use super::{
    CenterView, DawApp,
    cursor_fx::CursorFxMode,
    design::ThemePreset,
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
            return;
        }
        #[cfg(feature = "p2p")]
        if self.shared_invite_open() {
            return;
        }
        if self.action_form.is_some() {
            // Los controles Kit consumen edición/IME; ninguna tecla del formulario toca el DAW.
            return;
        }
        if event.keystroke.modifiers.control
            && !event.is_held
            && (key.eq_ignore_ascii_case("s") || key.eq_ignore_ascii_case("o"))
        {
            let command = if key.eq_ignore_ascii_case("o") {
                "project.open"
            } else if event.keystroke.modifiers.shift {
                "project.save_as"
            } else {
                "project.save"
            };
            self.project_command(command, window, cx);
            cx.stop_propagation();
            return;
        }
        if (event.keystroke.modifiers.control || event.keystroke.modifiers.platform)
            && !event.keystroke.modifiers.alt
            && !event.is_held
            && !self.command_open
            && !self.palette.is_open()
            && !self.settings.is_visible()
        {
            if key.eq_ignore_ascii_case("p") && !event.keystroke.modifiers.shift {
                self.open_command_bar();
                cx.stop_propagation();
                return;
            }
            if key.eq_ignore_ascii_case("e") && !event.keystroke.modifiers.shift {
                self.toggle_explorer();
                cx.stop_propagation();
                return;
            }
            if key.eq_ignore_ascii_case("z") {
                if event.keystroke.modifiers.shift {
                    self.redo_project();
                } else {
                    self.undo_project();
                }
                cx.stop_propagation();
                return;
            }
            if key.eq_ignore_ascii_case("y") && !event.keystroke.modifiers.shift {
                self.redo_project();
                cx.stop_propagation();
                return;
            }
            if key.eq_ignore_ascii_case("h") && !event.keystroke.modifiers.shift {
                self.history.open = !self.history.open;
                cx.stop_propagation();
                return;
            }
            if key.eq_ignore_ascii_case("k") && !event.keystroke.modifiers.shift {
                self.toggle_metronome();
                cx.stop_propagation();
                return;
            }
            if key.eq_ignore_ascii_case("a") && event.keystroke.modifiers.shift {
                self.toggle_automation_mode();
                cx.stop_propagation();
                return;
            }
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
        if self.command_open {
            // GPUI no genera KeyDown por cada carácter pegado: leer explícitamente el clipboard.
            if event.keystroke.modifiers.control && key.eq_ignore_ascii_case("v") {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.command_buffer
                        .push_str(&text.replace(['\n', '\r'], " "));
                    self.cmd_selected = None;
                }
                return;
            }
            self.handle_command_key(key, event.keystroke.modifiers.shift, window, cx);
            return;
        }
        if self.handle_performance_key(event) {
            return;
        }
        if self.handle_arrangement_edit_key(event, window, cx) {
            return;
        }
        if self.handle_track_delete_key(event) {
            return;
        }
        let modifiers = event.keystroke.modifiers;
        if !modifiers.control && !modifiers.alt && !modifiers.platform {
            self.handle_workspace_key(key);
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
    fn handle_workspace_key(&mut self, key: &str) {
        let drums_active = self.drums_panel_active();
        if drums_active && self.handle_drum_key(key) {
            return;
        }
        let arrangement = self.arrangement_visible() && !drums_active;
        match key {
            ":" => self.open_command_bar(),
            "[" => self.change_gain(-0.1),
            "]" => self.change_gain(0.1),
            "insert" => self.open_palette(),
            "escape" => {
                self.all_notes_off();
                self.status_message = "Listo".into();
            }
            "f1" => self.set_center_view(CenterView::Arrangement),
            "f2" => self.set_center_view(CenterView::Session),
            "f3" => self.set_center_view(CenterView::Mixer),
            "left" if arrangement => self.timeline.move_cursor(-1.0),
            "right" if arrangement => self.timeline.move_cursor(1.0),
            "home" if arrangement => {
                if let Err(error) = self.seek_audio(0.) {
                    self.status_message = error;
                }
            }
            "+" | "=" if arrangement => self.timeline.zoom_at_cursor(2.0),
            "-" if arrangement => self.timeline.zoom_at_cursor(0.5),
            "up" if arrangement => self.move_visible_track(false),
            "down" if arrangement => self.move_visible_track(true),
            "up" if self.center_view == CenterView::Mixer => {
                self.select_track(self.cursor_row.saturating_sub(1), false, false);
            }
            "down" if self.center_view == CenterView::Mixer => {
                let last = self.tracks.len().saturating_sub(1);
                self.select_track((self.cursor_row + 1).min(last), false, false);
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
            "escape" => self.close_command_bar(),
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
                self.close_command_bar();
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
        if self.hydrogen_command(cmd, cx) || self.project_command(cmd, window, cx) {
            return;
        }
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
        if cmd.trim().eq_ignore_ascii_case("autosave") {
            self.force_autosave_now(cx);
            return;
        }
        if matches!(
            cmd.trim().to_ascii_lowercase().as_str(),
            "consolidate" | "consolidar"
        ) {
            self.consolidate_selection(cx);
            return;
        }
        if matches!(
            cmd.trim().to_ascii_lowercase().as_str(),
            "export.midi" | "midi.export"
        ) {
            self.export_arrangement_midi_dialog(cx);
            return;
        }
        if cmd.trim().eq_ignore_ascii_case("kit.save") {
            self.save_drum_kit_dialog(cx);
            return;
        }
        if cmd.trim().eq_ignore_ascii_case("kit.load") {
            self.load_drum_kit_dialog(cx);
            return;
        }
        if matches!(
            cmd.trim().to_ascii_lowercase().as_str(),
            "audio.to_midi" | "audio2midi"
        ) {
            self.convert_audio_clip_to_midi(cx);
            return;
        }
        if cmd.trim() == "fx.components.reload" {
            self.reload_lua_components(cx);
            return;
        }
        if cmd
            .split_whitespace()
            .next()
            .is_some_and(|word| word.eq_ignore_ascii_case("audio.collect"))
        {
            match elevate::actions::tokenize(cmd) {
                Ok(parts) if parts.len() == 2 && !parts[1].trim().is_empty() => {
                    self.start_audio_collect(parts[1].clone().into(), cx);
                }
                _ => self.status_message = "Uso: audio.collect '/ruta/carpeta-nueva'".into(),
            }
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
            self.status_message =
                "Modo Vim deshabilitado · usa Ctrl+P o : para comandos y F1–F3 para vistas".into();
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
        if cmd
            .split_whitespace()
            .next()
            .is_some_and(|word| word.eq_ignore_ascii_case("hilo.rule"))
        {
            match elevate::actions::tokenize(cmd) {
                Ok(parts) if parts.len() == 2 => self.set_hilo_rule(&parts[1]),
                _ => self.status_message = "Uso: hilo.rule '1,3,2' o hilo.rule auto".into(),
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
            "track.insert_return" | "return" => self.insert_return_track(),
            "track.delete" => self.edit_track_structure(elevate::musical::track::TrackEdit::Delete),
            "track.duplicate" => self.edit_track_structure(elevate::musical::track::TrackEdit::Duplicate),
            "track.group" => self.edit_track_structure(elevate::musical::track::TrackEdit::Group),
            "tejer.selection" => self.tejer_selected_tracks(),
            "track.ungroup" => self.edit_track_structure(elevate::musical::track::TrackEdit::Ungroup),
            "hilo.layer" => self.set_hilo_mode(Some(elevate::musical::track::HiloMode::Layer)),
            "hilo.rr" | "hilo.round_robin" => self.set_hilo_mode(Some(elevate::musical::track::HiloMode::RoundRobin)),
            "hilo.off" => self.set_hilo_mode(None),
            "track.midi" => self.toggle_track_midi(self.cursor_row),
            "track.mute" => self.toggle_track_mute(self.cursor_row),
            "track.solo" => self.toggle_track_solo(self.cursor_row),
            "export.settings" => self.open_export_settings(),
            "export.cancel" => self.cancel_export(),
            "export.help" => self.status_message = "export -n Nombre.[wav,mp3,aiff,flac] -master|-individuals · -sr Hz -bits 16|24|32|32f -bitrate kbps|-vbr 0–9 -mono -lufs -14|off -tp -1|off -tracks 1,3 -start beats -length beats -tail segundos -master-fx on|off -out carpeta · docs/EXPORT.md".into(),
            "utility" | "utility.open" => self.open_utility(),
            "master.fx" | "mfx" => self.show_master_fx(),
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
            "stop" => self.stop_transport(),
            "metro" | "metronome" | "click" => self.toggle_metronome(),
            "auto" | "automation" => self.toggle_automation_mode(),
            "auto.vol" | "auto.volume" => {
                self.set_automation_param(elevate::musical::track_automation::AutomationParam::Volume);
            }
            "auto.pan" => {
                self.set_automation_param(elevate::musical::track_automation::AutomationParam::Pan);
            }
            "auto.next" => self.cycle_automation_param(),
            "auto.vst" => self.automate_last_touched_vst(),
            "auto.clear" => self.clear_selected_track_automation(),
            cmd if cmd.starts_with("drum.swing ") => {
                if let Ok(val) = cmd.trim_start_matches("drum.swing ").trim().parse::<i16>() {
                    let current = i16::from(self.pattern_editor.session.active_pattern().swing);
                    let target = val.clamp(0, i16::from(elevate::musical::drums::DrumPattern::MAX_SWING));
                    self.apply_pattern_action(super::pattern_editor::PatternAction::Swing(
                        target - current,
                    ));
                } else {
                    self.status_message = "Uso: :drum.swing <0..75>".into();
                }
            }
            "countin" | "count_in" => self.cycle_count_in(),
            other if other.starts_with("countin ") => {
                match other.trim_start_matches("countin ").trim().parse::<u32>() {
                    Ok(bars) => self.set_count_in_bars(bars),
                    Err(_) => self.status_message = "Uso: countin 0 | 1 | 2".into(),
                }
            }
            other if other.starts_with("metro.vol ") => {
                match other.trim_start_matches("metro.vol ").trim().parse::<f32>() {
                    Ok(pct) => self.set_metronome_gain(pct / 100.0),
                    Err(_) => self.status_message = "Uso: metro.vol 75 (0–150%)".into(),
                }
            }
            "record" | "rec" => self.toggle_midi_record(),
            "rec.audio" | "mic" => self.toggle_audio_record(None),
            "rec.stereo" => self.toggle_audio_record_stereo(),
            "capture" | "dump" => self.capture_midi_to_track(),
            "materialize" => self.materialize_midi_selection(),
            "bounce" => self.bounce_midi_to_audio(),
            "undo" => self.undo_project(),
            "redo" => self.redo_project(),
            "history" | "historial" => self.history.open = !self.history.open,
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
            "learn.vst" | "midi.learn.vst" => {
                self.arm_last_touched_vst_learn();
            }
            "learn.list" | "midi.map" => {
                self.open_midi_map_settings();
            }
            "freeze" => {
                self.toggle_freeze_track(self.cursor_row);
            }
            "unfreeze" => {
                if let Some(track) = self.tracks.get(self.cursor_row) {
                    let id = track.id;
                    self.unfreeze_track(id);
                }
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
