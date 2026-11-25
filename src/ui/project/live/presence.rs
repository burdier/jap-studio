//! El foco viaja por la sesión; los créditos confirmados se guardan con la revisión Loro.
use super::{DawApp, Packet, Presence, Snapshot};
use crate::collaboration::live::display_text;
use crate::ui::{
    CenterView,
    components::modal::{ModalClose, modal_backdrop},
    workspace_focus::Panel,
};
use elevate::project::ProjectDoc;
use gpui::{
    AppContext, Context, Div, Entity, Window,
    component::{
        Sizable,
        button::{Button, ButtonVariants},
        input::{Input, InputState},
    },
    div,
    prelude::*,
    px, rgb,
};
use std::rc::Rc;

pub(in crate::ui::project) struct IdentityDialog {
    field: Entity<InputState>,
    saved_history: Vec<String>,
}

fn machine_alias() -> String {
    ["USER", "LOGNAME"]
        .iter()
        .filter_map(|key| std::env::var(key).ok())
        .map(|s| display_text(&s, 36))
        .find(|s| !s.is_empty())
        .unwrap_or_else(|| "Músico".into())
}

impl DawApp {
    pub(in crate::ui) fn peer_note_presence(
        &self,
        clip: &elevate::musical::clip::MidiClip,
    ) -> Option<(String, crate::collaboration::live::NotePresence)> {
        let peer = self.project.shared.as_ref()?.peer.as_ref()?;
        let note = peer.note?;
        if peer.clip != Some(clip.id.value())
            || note.start < 0
            || note.duration <= 0
            || note.pitch > 127
            || note.start.checked_add(note.duration)? > clip.pattern.length().0
            || !clip.notes().iter().any(|n| n.id.0 == note.id)
        {
            return None;
        }
        Some((peer.alias.clone(), note))
    }

    pub(in crate::ui) fn mark_drum_step(
        &mut self,
        step: crate::collaboration::live::DrumStepPresence,
    ) {
        if let Some(shared) = self
            .project
            .shared
            .as_mut()
            .filter(|s| s.connected && s.initialized)
        {
            shared.last_drum_step = Some((std::time::Instant::now(), step));
        }
    }

    pub(in crate::ui) fn peer_drum_step(
        &self,
        piece: elevate::musical::model::InstrumentId,
    ) -> Option<(String, crate::collaboration::live::DrumStepPresence)> {
        let peer = self.project.shared.as_ref()?.peer.as_ref()?;
        let step = peer.drum_step?;
        let editor = &self.pattern_editor;
        (step.pattern == editor.session.active.0
            && step.piece == piece.0
            && step.pitch <= 127
            && (0..editor.pattern().length().0).contains(&step.tick))
        .then(|| (peer.alias.clone(), step))
    }

    pub(in crate::ui) fn peer_band_cut(
        &self,
        track: &elevate::musical::track::Track,
    ) -> Option<(elevate::musical::fx::FxId, u16)> {
        use elevate::musical::fx::{FxId, FxKind};
        let cut = self.project.shared.as_ref()?.peer.as_ref()?.band_cut?;
        if cut.track != track.id.0 {
            return None;
        }
        let id = FxId(cut.crossover);
        match &track.chain.find(id)?.kind {
            FxKind::Crossover { hz, .. }
                if *hz == cut.from_hz && track.chain.accepts_cut_preview(id, cut.hz) =>
            {
                Some((id, cut.hz))
            }
            _ => None,
        }
    }

    pub(super) fn publish_presence(&mut self) {
        if !self
            .project
            .shared
            .as_ref()
            .is_some_and(|s| s.initialized && s.connected)
        {
            return;
        }
        let alias = self.project.alias.get_or_insert_with(machine_alias).clone();
        let (clip, activity) =
            if self.piano_roll.open && self.workspace_focus.active == Panel::PianoRoll {
                self.timeline
                    .midi_clip
                    .as_ref()
                    .map(|c| (Some(c.id.value()), format!("Piano roll · {}", c.name)))
                    .unwrap_or((None, "Piano roll".into()))
            } else if self.center_view == CenterView::Arrangement
                && let Some(c) = self.shown_sample()
            {
                (Some(c.id.value()), format!("Editor de audio · {}", c.name))
            } else if self.track_control_drag.is_some() || self.track_reorder_drag.is_some() {
                let track = self
                    .track_control_drag
                    .as_ref()
                    .and_then(|d| self.tracks.get(d.track))
                    .or_else(|| {
                        self.track_reorder_drag
                            .as_ref()
                            .and_then(|d| self.tracks.iter().find(|t| t.id == d.source))
                    });
                (
                    None,
                    track.map_or_else(
                        || "Pistas".into(),
                        |t| format!("Editando pista · {}", t.name),
                    ),
                )
            } else {
                match self.center_view {
                    CenterView::Arrangement => self
                        .timeline
                        .active_clip_presence()
                        .map(|(id, name, verb)| (Some(id), format!("{verb} · {name}")))
                        .unwrap_or((None, String::new())),
                    CenterView::Mixer => (None, "Mixer".into()),
                    CenterView::Session => (None, "Secuenciador".into()),
                }
            };
        let note = if self.piano_roll.open
            && self.timeline.midi_clip.as_ref().map(|c| c.id.value()) == clip
        {
            self.dragged_note_presence()
        } else {
            None
        };
        // La rueda y el clic son instantáneos. Conserva la marca un momento tras la última edición.
        let drum_step =
            if self.center_view == CenterView::Session && !self.pattern_editor.mixer_open {
                self.pattern_editor.dragged_drum_step().or_else(|| {
                    let (when, step) = self.project.shared.as_ref()?.last_drum_step?;
                    (when.elapsed() < std::time::Duration::from_millis(1800)
                        && step.pattern == self.pattern_editor.session.active.0)
                        .then_some(step)
                })
            } else {
                None
            };
        let band_cut = self.fx.tejer.and_then(|target| {
            let (id, hz) = self.fx.cut_draft?;
            let track = self.tracks.iter().find(|t| t.id == target.track)?;
            let elevate::musical::fx::FxKind::Crossover { hz: from_hz, .. } =
                &track.chain.find(id)?.kind
            else {
                return None;
            };
            Some(crate::collaboration::live::BandCutPresence {
                track: track.id.0,
                crossover: id.0,
                from_hz: *from_hz,
                hz,
            })
        });
        // Envía el último valor aunque el mouse se suelte entre dos pulsos de presencia.
        // La marca se limpia en el pulso siguiente a publicar el cambio duradero.
        let band_cut = band_cut.or_else(|| {
            use elevate::musical::fx::{FxId, FxKind};
            let shared = self.project.shared.as_ref()?;
            let mut cut = shared.sent_presence.as_ref()?.band_cut?;
            let hz_in = |tracks: &[elevate::musical::track::Track]| {
                let track = tracks.iter().find(|t| t.id.0 == cut.track)?;
                match track.chain.find(FxId(cut.crossover))?.kind {
                    FxKind::Crossover { hz, .. } => Some(hz),
                    _ => None,
                }
            };
            let final_hz = hz_in(&self.tracks)?;
            if final_hz == cut.from_hz || final_hz == hz_in(&shared.last.tracks)? {
                return None;
            }
            cut.hz = final_hz;
            Some(cut)
        });
        let activity = band_cut.map_or(activity, |cut| format!("Tejer EQ · {} Hz", cut.hz));
        let presence = Presence {
            band_cut,
            drum_step,
            note,
            alias,
            clip,
            activity: display_text(&activity, 96),
        };
        if let Some(shared) = &mut self.project.shared
            && shared.sent_presence.as_ref() != Some(&presence)
            && shared.room.send(Packet::Presence(presence.clone())).is_ok()
        {
            shared.sent_presence = Some(presence);
        }
    }

    pub(super) fn credit_shared_edits(
        &self,
        doc: &ProjectDoc,
        old: &Snapshot,
        new: &Snapshot,
    ) -> Result<(), String> {
        let alias = self.project.alias.clone().unwrap_or_else(machine_alias);
        let write = |action: &str, name: &str, id: u64| {
            doc.append_edit_credit(&format!(
                "{alias} {action} «{}» · #{id}",
                display_text(name, 72)
            ))
        };
        clip_changes(
            &old.audio,
            &new.audio,
            |c| c.id.value(),
            |c| &c.name,
            |a, b| a.start_beat != b.start_beat || a.track != b.track,
            &write,
        )?;
        clip_changes(
            &old.midi,
            &new.midi,
            |c| c.id.value(),
            |c| &c.name,
            |a, b| a.start_beat != b.start_beat || a.track != b.track,
            &write,
        )?;
        if old.drums != new.drums {
            doc.append_edit_credit(&format!("{alias} editó Drums Latex"))?;
        }
        if old.tempo != new.tempo || old.meter != new.meter {
            doc.append_edit_credit(&format!("{alias} cambió el tempo o el compás"))?;
        }
        if old.navigation != new.navigation {
            doc.append_edit_credit(&format!("{alias} editó los marcadores o el loop"))?;
        }
        if old.pianos != new.pianos {
            doc.append_edit_credit(&format!("{alias} editó la instrumentación"))?;
        }
        for (i, track) in new.tracks.iter().enumerate() {
            let before = old.tracks.iter().position(|t| t.id == track.id);
            let action = match before {
                None => "añadió la pista",
                Some(j)
                    if old.tracks[j] != *track
                        || old.levels.get(j) != new.levels.get(i)
                        || old.pans.get(j) != new.pans.get(i) =>
                {
                    "editó la pista"
                }
                Some(j) if j != i && old.tracks.len() == new.tracks.len() => "reordenó la pista",
                _ => continue,
            };
            write(action, track.name.as_ref(), u64::from(track.id.0))?;
        }
        for track in &old.tracks {
            if !new.tracks.iter().any(|t| t.id == track.id) {
                write("borró la pista", track.name.as_ref(), u64::from(track.id.0))?;
            }
        }
        Ok(())
    }

    pub(in crate::ui::project) fn open_identity_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let alias = self.project.alias.get_or_insert_with(machine_alias).clone();
        let field = cx.new(|cx| InputState::new(window, cx).default_value(alias));
        let saved_history = if self.project.shared.is_none() {
            self.project
                .document
                .as_deref()
                .and_then(|b| ProjectDoc::import(b).ok())
                .map(|d| d.recent_edit_credits(8))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        self.project.identity_dialog = Some(IdentityDialog {
            field,
            saved_history,
        });
    }

    pub(super) fn render_identity_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        let dialog = self.project.identity_dialog.as_ref()?;
        let history = self
            .project
            .shared
            .as_ref()
            .map(|s| s.document.recent_edit_credits(8))
            .unwrap_or_else(|| dialog.saved_history.clone());
        let close: ModalClose =
            Rc::new(cx.listener(|this, _, window, cx| this.close_shared_invite(window, cx)));
        let card = div()
            .w(px(520.))
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .bg(rgb(self.ds.panel_dark))
            .rounded(px(self.ds.radius_lg))
            .child(div().text_lg().child("Nombre e historial de edición"))
            .child("Tu nombre en esta sesión")
            .child(Input::new(&dialog.field).w_full())
            .child(
                div()
                    .mt_2()
                    .text_color(rgb(self.ds.muted))
                    .child("Ediciones recientes"),
            )
            .when(history.is_empty(), |d| {
                d.child("Aún no hay ediciones compartidas.")
            })
            .children(history.into_iter().map(|s| div().text_sm().child(s)))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("identity-close")
                            .small()
                            .label("Cerrar")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_shared_invite(window, cx)
                            })),
                    )
                    .child(
                        Button::new("identity-apply")
                            .small()
                            .primary()
                            .label("Aplicar")
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(dialog) = &this.project.identity_dialog {
                                    let name =
                                        display_text(dialog.field.read(cx).value().as_ref(), 36);
                                    this.project.alias = Some(if name.is_empty() {
                                        machine_alias()
                                    } else {
                                        name
                                    });
                                    this.publish_presence();
                                }
                                this.close_shared_invite(window, cx);
                            })),
                    ),
            );
        Some(modal_backdrop(0x05070bd9, close, card))
    }
}

// Snapshot mantiene los clips ordenados por ID estable, incluso tras moverlos de pista.
fn clip_changes<T: PartialEq>(
    old: &[T],
    new: &[T],
    id: impl Fn(&T) -> u64,
    name: impl Fn(&T) -> &str,
    moved: impl Fn(&T, &T) -> bool,
    write: impl Fn(&str, &str, u64) -> Result<(), String>,
) -> Result<(), String> {
    for c in new {
        let action = match old.binary_search_by_key(&id(c), &id) {
            Err(_) => "añadió",
            Ok(i) if moved(&old[i], c) => "movió",
            Ok(i) if old[i] != *c => "editó",
            _ => continue,
        };
        write(action, name(c), id(c))?;
    }
    for c in old {
        if new.binary_search_by_key(&id(c), &id).is_err() {
            write("borró", name(c), id(c))?;
        }
    }
    Ok(())
}
