//! Autoguardado periódico en segundo plano y recuperación ante cierre inesperado.
use super::{Snapshot, effects};
use crate::ui::DawApp;
use gpui::{AppContext, Context, Div, MouseButton, div, prelude::*, px, rgb, rgba};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(25);

pub(in crate::ui) fn autosave_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("hay-daw")
        .join("autosave")
        .join("recovery.loro")
}

pub(in crate::ui) fn detect_recovery() -> Option<(PathBuf, String)> {
    let path = autosave_path();
    let meta = fs::metadata(&path).ok()?;
    if meta.len() == 0 {
        return None;
    }
    let modified = meta.modified().ok().unwrap_or(SystemTime::now());
    let secs = modified
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe {
        libc::localtime_r(&secs, &mut tm);
    }
    let stamp = format!(
        "{:02}/{:02} {:02}:{:02}",
        tm.tm_mday,
        tm.tm_mon + 1,
        tm.tm_hour,
        tm.tm_min
    );
    Some((path, stamp))
}

pub(in crate::ui) fn clear_autosave_file() {
    let path = autosave_path();
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(path.with_extension("loro.tmp"));
}

impl DawApp {
    pub(in crate::ui) fn project_busy_for_history(&self) -> bool {
        self.project.task.is_some()
    }

    pub(in crate::ui) fn tick_autosave(&mut self, cx: &mut Context<Self>) {
        if self.project_busy() || self.project.autosave_task.is_some() || self.is_gesture_active() {
            return;
        }
        // No sobrescribir una recuperación pendiente antes de que el usuario decida
        if self.project.recovery_available.is_some() && !self.project.recovery_dismissed {
            return;
        }
        // Solo autoguardar si hay contenido real en la sesión
        let has_content = !self.tracks.is_empty()
            || self
                .pattern_editor
                .session
                .patterns
                .iter()
                .any(|p| !p.notes.notes().is_empty());
        if !has_content {
            return;
        }
        if self
            .project
            .autosave_last
            .is_some_and(|t| t.elapsed() < AUTOSAVE_INTERVAL)
        {
            return;
        }
        let hash = self.current_project_hash();
        if hash == self.project.autosave_hash {
            self.project.autosave_last = Some(Instant::now());
            return;
        }
        let Ok(mut snapshot) = self.session_snapshot() else {
            self.project.autosave_last = Some(Instant::now());
            return;
        };
        // Para el autosave de emergencia, sanear solo instrumentos sin estado capturable
        // para que el resto del proyecto (pistas, VST3, Hay Filter, MIDI, clips, mezcla) se salve.
        for track in &mut snapshot.tracks {
            if let Some(id) = track.instrument {
                let supported = snapshot.pianos.iter().any(|p| p.id == id)
                    || snapshot.samplers.iter().any(|s| s.id == id)
                    || snapshot.vsts.iter().any(|v| v.id == id);
                if !supported {
                    track.instrument = None;
                }
            }
            if crate::audio::effects::needs_rack(&track.chain)
                && effects::validate(track.chain.nodes()).is_err()
            {
                track.chain = Default::default();
            }
        }
        self.project.autosave_last = Some(Instant::now());
        let base_doc = self.history_doc_bytes();
        let dest = autosave_path();
        let job = cx.background_spawn(async move {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let _ = snapshot.resolve_piano_sources();
            let doc = snapshot.document_from("autosave", base_doc.as_deref())?;
            doc.commit_change("Autoguardado", crate::ui::history::unix_now_secs());
            let tmp = dest.with_extension("loro.tmp");
            doc.save(&tmp).map_err(|e| e.to_string())?;
            fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        });
        self.project.autosave_task = Some(cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.project.autosave_task = None;
                if result.is_ok() {
                    this.project.autosave_hash = hash;
                    this.project.autosave_label = Some(crate::ui::history::local_hhmm());
                }
                cx.notify();
            });
        }));
    }

    pub(in crate::ui) fn force_autosave_now(&mut self, cx: &mut Context<Self>) {
        self.project.autosave_last = None;
        self.project.autosave_hash = 0;
        self.tick_autosave(cx);
        self.status_message = "Autoguardado solicitado…".into();
    }

    pub(in crate::ui) fn recover_autosave(&mut self, cx: &mut Context<Self>) {
        let Some((path, _)) = self.project.recovery_available.clone() else {
            return;
        };
        if self.project_busy() {
            return;
        }
        self.project.recovery_available = None;
        self.project.recovery_dismissed = true;
        self.status_message = "Recuperando sesión autoguardada…".into();
        let config = self.settings.audio.saved.clone();
        let registry = self.lua_components.registry.clone();
        let rate = config.sample_rate.unwrap_or(48_000);
        let target = path.clone();
        let job = cx.background_spawn(async move {
            let (snapshot, audio, document) = Snapshot::load(&target)?;
            let effects =
                effects::prepare(&snapshot.tracks, &[], &snapshot.master_chain, None, &config)?;
            let components = effects::prepare_components(
                &snapshot.tracks,
                &snapshot.master_chain,
                &registry,
                rate,
            )?;
            let samplers = snapshot.prepare_samplers(&[])?;
            let vsts = snapshot.prepare_vsts(&config)?;
            Ok::<_, String>((
                snapshot, audio, document, effects, components, samplers, vsts,
            ))
        });
        self.project.task = Some(cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.project.task = None;
                match result {
                    Ok((snapshot, audio, document, effects, components, samplers, vsts)) => {
                        match this.install_project(
                            snapshot, audio, path, document, effects, components, samplers, vsts,
                        ) {
                            Ok(()) => {
                                // Dejar sin ruta fija para que Ctrl+S pregunte dónde guardar.
                                // `install_project` ya cargó el historial Loro completo de `recovery.loro`.
                                this.project.path = None;
                                this.project.dirty = true;
                                this.status_message =
                                    "Sesión e historial Loro recuperados · usa Ctrl+S para guardar"
                                        .into();
                            }
                            Err(e) => {
                                this.status_message = format!("No se pudo recuperar: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        this.status_message = format!("No se pudo abrir el autoguardado: {e}");
                    }
                }
                cx.notify();
            });
        }));
    }

    pub(in crate::ui) fn discard_recovery(&mut self) {
        self.project.recovery_available = None;
        self.project.recovery_dismissed = true;
        clear_autosave_file();
        self.status_message = "Autoguardado descartado".into();
    }

    pub(in crate::ui) fn render_recovery_banner(&self, cx: &mut Context<Self>) -> Option<Div> {
        let (_, stamp) = self.project.recovery_available.as_ref()?;
        if self.project.recovery_dismissed {
            return None;
        }
        let ds = &self.ds;
        Some(
            div()
                .flex_shrink_0()
                .mb(px(4.))
                .px_3()
                .py(px(6.))
                .rounded(px(ds.radius_md))
                .bg(rgba((ds.warning << 8) | 0x22))
                .border_1()
                .border_color(rgba((ds.warning << 8) | 0x88))
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .text_color(rgb(ds.text))
                        .child(div().text_color(rgb(ds.warning)).child("⚡"))
                        .child(format!(
                            "Se encontró un autoguardado del {stamp} tras un cierre inesperado."
                        )),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("recovery-accept")
                                .px_2()
                                .py(px(3.))
                                .rounded(px(4.))
                                .bg(rgb(ds.warning))
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(ds.bg))
                                .cursor_pointer()
                                .child("Recuperar sesión")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.recover_autosave(cx);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                        )
                        .child(
                            div()
                                .id("recovery-discard")
                                .px_2()
                                .py(px(3.))
                                .rounded(px(4.))
                                .bg(rgba((ds.text << 8) | 0x14))
                                .hover(|b| b.bg(rgba((ds.danger << 8) | 0x35)))
                                .text_xs()
                                .text_color(rgb(ds.text))
                                .cursor_pointer()
                                .child("Descartar")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.discard_recovery();
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                        ),
                ),
        )
    }
}
