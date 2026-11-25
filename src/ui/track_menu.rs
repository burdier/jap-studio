//! Menú de pistas; utiliza el catálogo y los mismos controladores que los comandos.
use super::DawApp;
use elevate::musical::track::{HiloMode, TrackKind};
use gpui::{Context, KeyDownEvent, Pixels, Point, Size, Window, point, px};

pub(super) const ITEMS: &[(&str, &str)] = &[
    ("Track FX…", "action.fx.show"),
    ("Insertar pista", "action.track.insert"),
    ("Insertar sampler", "action.track.insert_sampler"),
    ("Insertar bus Return (FX)", "action.track.insert_return"),
    ("Renombrar…", "action.track.rename"),
    ("Duplicar selección", "action.track.duplicate"),
    ("Agrupar selección", "action.track.group"),
    ("Desagrupar", "action.track.ungroup"),
    ("Silenciar", "action.track.mute"),
    ("Solo", "action.track.solo"),
    ("Eliminar selección y sus clips", "action.track.delete"),
];

pub(super) const BLANK_ITEMS: &[(&str, &str)] = &[
    ("+ Agregar pista de audio", "action.track.insert_end"),
    ("+ Agregar pista Hay Sampler", "action.track.insert_sampler"),
    ("+ Agregar bus Return (FX)", "action.track.insert_return"),
    (
        "Abrir catálogo de instrumentos / VST [+]",
        "action.palette.open",
    ),
];

pub(super) struct TrackMenu {
    pub position: Point<Pixels>,
    pub selected: usize,
    pub max_height: Pixels,
    pub blank_space: bool,
}

impl DawApp {
    pub(in crate::ui) fn insert_audio_track_at_end(&mut self) {
        if !self.tracks.is_empty() {
            let last = self.tracks.len() - 1;
            self.select_track(last, false, false);
        }
        self.edit_track_structure(elevate::musical::track::TrackEdit::Insert);
        self.reveal_track();
    }

    pub(in crate::ui) fn insert_return_track(&mut self) {
        self.edit_track_structure(elevate::musical::track::TrackEdit::InsertReturn);
        self.reveal_track();
    }

    pub(in crate::ui) fn track_menu_items(&self) -> Vec<(&'static str, &'static str)> {
        if self.track_menu.as_ref().is_some_and(|m| m.blank_space) {
            return BLANK_ITEMS.to_vec();
        }
        let mut items = ITEMS.to_vec();
        let is_group = self
            .tracks
            .get(self.cursor_row)
            .is_some_and(|t| t.kind == TrackKind::Group);
        let is_return = self
            .tracks
            .get(self.cursor_row)
            .is_some_and(|t| t.is_return());
        if !is_group {
            items.insert(
                7,
                ("Tejer pistas seleccionadas", "action.track.tejer.selection"),
            );
        }
        let height_at = if is_group { 7 } else { 8 };
        items.splice(
            height_at..height_at,
            [
                ("Altura · compacta", "action.track.height.compact"),
                ("Altura · predeterminada", "action.track.height.default"),
                ("Altura · amplia", "action.track.height.tall"),
                ("Altura personalizada…", "action.track.height"),
            ],
        );
        if !is_group && !is_return {
            let freeze_label = self
                .tracks
                .get(self.cursor_row)
                .is_some_and(|t| self.is_track_frozen(t.id))
                .then_some("❄ Descongelar pista (Unfreeze)")
                .unwrap_or("❄ Congelar pista (Freeze)");
            items.insert(1, (freeze_label, "action.track.freeze"));
        }
        if is_group {
            items.splice(
                7..7,
                [
                    ("Hilo · capas", "action.track.hilo.layer"),
                    ("Tejer · automático", "action.track.hilo.round_robin"),
                    ("Tejer · secuencia de ramas…", "action.track.hilo.rule"),
                    ("Hilo · desactivar", "action.track.hilo.off"),
                ],
            );
            items.insert(
                10,
                (
                    if self.track_collapsed(self.cursor_row) {
                        "Desplegar grupo"
                    } else {
                        "Plegar grupo"
                    },
                    "action.track.fold",
                ),
            );
        }
        items
    }

    pub(in crate::ui) fn open_track_menu(
        &mut self,
        track: usize,
        position: Point<Pixels>,
        viewport: Size<Pixels>,
    ) {
        if !self
            .tracks
            .get(track)
            .is_some_and(|t| self.selected_tracks.contains(&t.id))
        {
            self.select_track(track, false, false);
        }
        self.cursor_row = track;
        self.clip_menu = None;
        self.track_color_menu = None;
        let height = px((self.track_menu_items().len() as f32 * 27.0 + 100.0)
            .min(f32::from(viewport.height) - 16.0)
            .max(100.0));
        self.track_menu = Some(TrackMenu {
            max_height: height,
            position: point(
                position.x.min((viewport.width - px(278.0)).max(px(0.0))),
                position.y.min((viewport.height - height).max(px(0.0))),
            ),
            selected: 0,
            blank_space: false,
        });
    }

    pub(in crate::ui) fn open_blank_track_menu(
        &mut self,
        position: Point<Pixels>,
        viewport: Size<Pixels>,
    ) {
        self.clip_menu = None;
        self.track_color_menu = None;
        let height = px((BLANK_ITEMS.len() as f32 * 27.0 + 44.0)
            .min(f32::from(viewport.height) - 16.0)
            .max(96.0));
        self.track_menu = Some(TrackMenu {
            max_height: height,
            position: point(
                position.x.min((viewport.width - px(278.0)).max(px(0.0))),
                position.y.min((viewport.height - height).max(px(0.0))),
            ),
            selected: 0,
            blank_space: true,
        });
    }

    pub(in crate::ui) fn run_track_menu(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = self.track_menu_items();
        self.track_menu = None;
        if let Some((_, command)) = items.get(index) {
            if *command == "action.track.insert_end" {
                self.insert_audio_track_at_end();
                cx.notify();
                return;
            }
            if *command == "action.track.insert_return" {
                self.insert_return_track();
                cx.notify();
                return;
            }
            if *command == "action.track.freeze" {
                self.toggle_freeze_track(self.cursor_row);
                cx.notify();
                return;
            }
            if *command == "action.palette.open" {
                self.open_palette();
                cx.notify();
                return;
            }
            if *command == "action.track.tejer.selection" {
                self.tejer_selected_tracks();
                cx.notify();
                return;
            }
            let mode = match *command {
                "action.track.hilo.layer" => Some(Some(HiloMode::Layer)),
                "action.track.hilo.round_robin" => Some(Some(HiloMode::RoundRobin)),
                "action.track.hilo.off" => Some(None),
                _ => None,
            };
            if let Some(mode) = mode {
                self.set_hilo_mode(mode);
                cx.notify();
                return;
            }
        }
        if let Some(entry) = items
            .get(index)
            .and_then(|(_, id)| elevate::catalog::find(id))
        {
            self.activate_catalog_entry(entry, window, cx);
        }
    }

    pub(in crate::ui) fn handle_track_menu_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = self.track_menu_items().len();
        let Some(menu) = self.track_menu.as_mut() else {
            return;
        };
        match event.keystroke.key.as_str() {
            "escape" => self.track_menu = None,
            "up" => menu.selected = (menu.selected + count - 1) % count,
            "down" => menu.selected = (menu.selected + 1) % count,
            "enter" => {
                let selected = menu.selected;
                self.run_track_menu(selected, window, cx);
            }
            _ => {}
        }
    }
}
