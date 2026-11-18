//! La misma lista alimenta ratón, teclado y estados deshabilitados.
use super::{ClipAction as A, MenuItem, MenuPage};
use crate::ui::{DawApp, timeline::DragKind};

impl MenuItem {
    fn new(label: impl Into<String>, action: A) -> Self {
        Self {
            label: label.into(),
            action,
            enabled: true,
            checked: false,
            separator: false,
        }
    }
    fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }
    fn separated(mut self) -> Self {
        self.separator = true;
        self
    }
}

impl DawApp {
    pub(in crate::ui) fn clip_menu_items(&self) -> Vec<MenuItem> {
        let Some(menu) = &self.clip_menu else {
            return Vec::new();
        };
        let Some(kind) = menu.clip_kind else {
            return vec![
                MenuItem::new("Insertar clip MIDI", A::InsertMidi),
                MenuItem::new(
                    format!("Insertar captura MIDI ({})", self.midi_capture.note_count()),
                    A::InsertCapture,
                )
                .enabled(self.midi_capture.has_notes()),
            ];
        };
        let Some(props) = self.active_clip_props(kind) else {
            return Vec::new();
        };
        let back = MenuItem::new("‹ Volver", A::Page(MenuPage::Main));
        match menu.page {
            MenuPage::Route => {
                let selected = self
                    .timeline
                    .midi_clip
                    .as_ref()
                    .and_then(|clip| clip.hilo_target);
                let mut items = vec![
                    back,
                    MenuItem::new("Salida del carril", A::RouteMidi(None))
                        .checked(selected.is_none()),
                ];
                for (index, group) in self
                    .tracks
                    .iter()
                    .enumerate()
                    .filter(|(_, track)| track.hilo.is_some())
                {
                    items.push(
                        MenuItem::new(
                            format!("Hilo {} · {}", index + 1, group.name),
                            A::RouteMidi(Some(group.id)),
                        )
                        .checked(selected == Some(group.id)),
                    );
                }
                items
            }
            MenuPage::Rename => vec![
                MenuItem::new("Guardar nombre · Enter", A::Rename)
                    .enabled(!menu.rename.trim().is_empty()),
                MenuItem::new("Cancelar · Esc", A::Page(MenuPage::Main)),
            ],
            MenuPage::Gain => vec![
                back,
                MenuItem::new("Reducir 3 dB", A::Gain(Some(-3.0))).enabled(props.gain_db > -24.0),
                MenuItem::new("Restablecer a 0 dB", A::Gain(None)),
                MenuItem::new("Aumentar 3 dB", A::Gain(Some(3.0))).enabled(props.gain_db < 12.0),
            ],
            MenuPage::Main => {
                let mut items = Vec::new();
                if kind == DragKind::Midi {
                    items.push(MenuItem::new("Editar en Piano Roll", A::EditMidi));
                    let destination = self
                        .timeline
                        .midi_clip
                        .as_ref()
                        .and_then(|clip| clip.hilo_target)
                        .and_then(|id| self.tracks.iter().find(|track| track.id == id));
                    items.push(MenuItem::new(
                        format!(
                            "Salida MIDI · {}  ›",
                            destination.map_or("Carril", |track| track.name.as_ref())
                        ),
                        A::Page(MenuPage::Route),
                    ));
                    items.push(
                        MenuItem::new(
                            format!("Materialize · {} MIDI", self.timeline.selected_midi.len()),
                            A::Materialize,
                        )
                        .enabled(!self.timeline.selected_midi.is_empty()),
                    );
                }
                items.extend([
                    MenuItem::new("Renombrar…", A::Page(MenuPage::Rename)),
                    MenuItem::new("Duplicar a continuación", A::Duplicate),
                    MenuItem::new("Dividir en el cursor", A::Split)
                        .enabled(!props.locked && self.timeline.can_split_clip(kind, menu.beat)),
                    MenuItem::new("Restaurar recorte", A::RestoreTrim).enabled(
                        !props.locked
                            && self
                                .timeline
                                .clip_extent(kind)
                                .is_some_and(|(_, _, _, trimmed)| trimmed),
                    ),
                    MenuItem::new("Silenciar", A::Mute)
                        .checked(props.muted)
                        .separated(),
                    MenuItem::new("Bloquear posición y tamaño", A::Lock).checked(props.locked),
                    MenuItem::new(
                        format!("Ganancia · {:+.1} dB  ›", props.gain_db),
                        A::Page(MenuPage::Gain),
                    ),
                    MenuItem::new("Mover a pista anterior", A::Move(-1))
                        .enabled(!props.locked && menu.track > 0)
                        .separated(),
                    MenuItem::new("Mover a pista siguiente", A::Move(1))
                        .enabled(!props.locked && menu.track + 1 < self.tracks.len()),
                ]);
                items.push(MenuItem::new("Eliminar clip", A::Delete).separated());
                items.push(
                    MenuItem::new("Color de la pista", A::Color(None))
                        .checked(props.color_index.is_none())
                        .separated(),
                );
                for index in 0..self.ds.clip_palette().len() {
                    items.push(
                        MenuItem::new("", A::Color(Some(index as u8)))
                            .checked(props.color_index == Some(index as u8)),
                    );
                }
                items
            }
        }
    }
}
