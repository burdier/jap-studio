//! Drums Latex: Canción arriba, conmutación de pasos y Piano Roll Light abajo.
mod inspector;
mod roll;
mod song;
mod steps;

use crate::ui::{
    DawApp,
    components::{
        button::{flat_control, toggle_badge},
        meter::track_peak_meter,
    },
    pattern_editor::PatternAction,
    workspace_focus::Panel,
};
use elevate::musical::drums::DrumPlaybackSource;
use gpui::{Context, Div, MouseButton, SharedString, Stateful, div, prelude::*, px, rgb};

impl DawApp {
    pub(in crate::ui) fn render_pattern(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let editor = &self.pattern_editor;
        let selected = editor.roll.piece.and_then(|id| {
            editor
                .session
                .kit
                .pieces
                .iter()
                .find(|piece| piece.id == id)
        });
        let instrument = selected.or_else(|| editor.session.kit.pieces.get(editor.cursor_lane));
        div()
            .id("drums-latex")
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .bg(rgb(self.ds.panel_dark))
            .capture_any_mouse_down(cx.listener(|this, _, window, cx| {
                if this.workspace_focus.active != Panel::Center {
                    this.workspace_focus.active = Panel::Center;
                    this.sync_midi_focus();
                }
                window.focus(&this.focus_handle, cx);
            }))
            .child(self.render_drum_song(cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .min_w_0()
                    .bg(rgb(self.ds.panel_dark))
                    .child(
                        toggle_badge(&self.ds, "Mixer", editor.mixer_open, self.ds.cyan)
                            .id("drums-mixer-toggle")
                            .h(px(22.0))
                            .mx_1()
                            .px_2()
                            .tooltip(|window, cx| {
                                gpui::component::tooltip::Tooltip::new("Mixer del kit")
                                    .build(window, cx)
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.pattern_editor.mixer_open =
                                        !this.pattern_editor.mixer_open;
                                    this.mixer_drag = None;
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(self.render_pattern_controls(cx).flex_1()),
            )
            .when_some(
                instrument.filter(|_| editor.inspector_open && !editor.mixer_open),
                |panel, piece| panel.child(self.render_drum_inspector(piece, cx)),
            )
            .child(if editor.mixer_open {
                self.render_drum_mixer(cx)
            } else if let Some(piece) = selected {
                self.render_drum_roll(piece, cx)
            } else {
                self.render_drum_steps(cx)
            })
    }

    fn render_pattern_controls(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let ds = &self.ds;
        let session = &self.pattern_editor.session;
        let peak = self
            .player
            .as_ref()
            .map_or(0.0, |player| player.drum_bus_peak());
        div()
            .id("drum-pattern-controls")
            .flex()
            .flex_shrink_0()
            .min_w_0()
            .items_center()
            .gap_1()
            .p_1()
            .bg(rgb(ds.panel_dark))
            .overflow_x_scroll()
            .font_family("monospace")
            .children(
                [
                    ("Patrón", DrumPlaybackSource::Pattern),
                    ("Canción", DrumPlaybackSource::Song),
                    ("Arrangement", DrumPlaybackSource::Arrangement),
                ]
                .into_iter()
                .map(|(label, source)| {
                    self.drum_action_chip(
                        label,
                        source == session.source,
                        PatternAction::UseSource(source),
                        cx,
                    )
                }),
            )
            .child(
                div()
                    .w(px(1.0))
                    .h(px(15.0))
                    .flex_shrink_0()
                    .bg(rgb(ds.border)),
            )
            .child(self.drum_action_chip("+ Compás", false, PatternAction::ExtendBar, cx))
            .child(self.drum_action_chip("Duplicar", false, PatternAction::Duplicate, cx))
            .child(
                flat_control(ds, false, ds.accent)
                    .id("send-drums-to-arrangement")
                    .h(px(22.0))
                    .px_2()
                    .text_xs()
                    .text_color(rgb(ds.text))
                    .child("→ Arrangement")
                    .tooltip(|window, cx| {
                        gpui::component::tooltip::Tooltip::new(
                            "Enviar las piezas al cursor del Arrangement",
                        )
                        .build(window, cx)
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.send_drums_to_arrangement();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .relative()
                    .id("drums-bus-peak")
                    .h(px(22.0))
                    .w(px(13.0))
                    .flex_shrink_0()
                    .tooltip(|window, cx| {
                        gpui::component::tooltip::Tooltip::new("Pico del bus Drums")
                            .build(window, cx)
                    })
                    .child(track_peak_meter(ds, peak)),
            )
            .child(
                toggle_badge(ds, "🥁", self.pattern_editor.inspector_open, ds.accent)
                    .id("drum-inspector-toggle")
                    .h(px(22.0))
                    .px_2()
                    .tooltip(|window, cx| {
                        gpui::component::tooltip::Tooltip::new("Ajustes de la pieza")
                            .build(window, cx)
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.pattern_editor.inspector_open =
                                !this.pattern_editor.inspector_open;
                            cx.notify();
                        }),
                    ),
            )
    }

    fn drum_action_chip(
        &self,
        label: impl Into<SharedString>,
        active: bool,
        action: PatternAction,
        cx: &mut Context<Self>,
    ) -> Div {
        flat_control(&self.ds, active, self.ds.accent)
            .h(px(22.0))
            .px_2()
            .text_xs()
            .text_color(rgb(if active { self.ds.accent } else { self.ds.text }))
            .child(label.into())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.apply_pattern_action(action.clone());
                    cx.notify();
                }),
            )
    }
}
