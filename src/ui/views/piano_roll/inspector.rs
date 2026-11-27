//! Inspector del piano roll con la jerarquía de superficies de Track FX.
use crate::ui::{DawApp, DesignSystem, components::button::badge_tag, piano_roll::RollAction};
use gpui::{Context, Div, IntoElement, MouseButton, div, prelude::*, px, rgb};

impl DawApp {
    pub(super) fn render_roll_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let clip = self.timeline.midi_clip.as_ref().expect("clip visible");
        let clip_color = ds.clip_color(clip.track, clip.props);
        let width = match self.piano_roll.inspector_tab {
            4 => 344.0,
            5 => 260.0,
            _ => 224.0,
        };
        div()
            .w(px(width))
            .flex_shrink_0()
            .h_full()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(rgb(ds.panel_dark))
            .border_r_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .min_h(px(34.0))
                    .flex_shrink_0()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .py(px(5.0))
                    .bg(rgb(ds.panel))
                    .border_b_1()
                    .border_color(rgb(ds.border))
                    .children(
                        ["Clip", "Notas", "Escala", "Vista", "Acordes", "MPE"]
                            .into_iter()
                            .enumerate()
                            .map(|(index, label)| {
                                self.roll_button(
                                    label,
                                    self.piano_roll.inspector_tab == index as u8,
                                    RollAction::InspectorTab(index as u8),
                                    cx,
                                )
                                .px(px(6.0))
                                .h(px(21.0))
                            }),
                    ),
            )
            .child(
                div()
                    .id("roll-inspector-scroll")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .min_w_0()
                    .overflow_y_scroll()
                    .p_2()
                    .child(div().w_full().min_w_0().when(
                        self.piano_roll.inspector_tab == 0,
                        |container| {
                            container.child(
                                section(ds, "PROPIEDADES DEL CLIP")
                                    .child(
                                        div()
                                            .h(px(5.0))
                                            .mb(px(4.0))
                                            .rounded(px(2.0))
                                            .bg(rgb(clip_color)),
                                    )
                                    .child(property_action(
                                        ds,
                                        "Nombre",
                                        self.roll_button(
                                            clip.name.clone(),
                                            false,
                                            RollAction::Form("midi.clip.rename"),
                                            cx,
                                        ),
                                    ))
                                    .child(property_action(
                                        ds,
                                        "Color",
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .size(px(21.0))
                                                    .flex_shrink_0()
                                                    .rounded(px(3.0))
                                                    .border_1()
                                                    .border_color(rgb(ds.frame_border()))
                                                    .bg(rgb(clip_color)),
                                            )
                                            .child(self.roll_button(
                                                "Cambiar",
                                                false,
                                                RollAction::ChooseClipColor,
                                                cx,
                                            )),
                                    ))
                                    .child(property_action(
                                        ds,
                                        "Canal",
                                        self.roll_button(
                                            format!("{}", clip.channel),
                                            false,
                                            RollAction::Form("midi.clip.channel"),
                                            cx,
                                        ),
                                    ))
                                    .child(property_action(
                                        ds,
                                        "Duración",
                                        self.roll_button(
                                            format!(
                                                "{:.2} compases",
                                                clip.pattern_beats()
                                                    / self.timeline.meter.beats_per_bar()
                                            ),
                                            false,
                                            RollAction::Form("midi.clip.length"),
                                            cx,
                                        ),
                                    ))
                                    .child(property(
                                        ds,
                                        "Start",
                                        self.timeline.meter.position_label(
                                            elevate::musical::time::Tick(
                                                (clip.start_beat
                                                    * elevate::musical::time::PPQ as f64)
                                                    .round()
                                                    as i64,
                                            ),
                                        ),
                                    ))
                                    .child(property(
                                        ds,
                                        "End",
                                        self.timeline.meter.position_label(
                                            elevate::musical::time::Tick(
                                                (clip.end_beat()
                                                    * elevate::musical::time::PPQ as f64)
                                                    .round()
                                                    as i64,
                                            ),
                                        ),
                                    ))
                                    .child(div().h(px(1.0)).my_1().bg(rgb(ds.border)))
                                    .child(self.roll_checkbox(
                                        "Loop",
                                        clip.props.looped,
                                        RollAction::ToggleClipLoop,
                                        cx,
                                    ))
                                    .child(self.roll_checkbox(
                                        "Mute",
                                        clip.props.muted,
                                        RollAction::ToggleClipMute,
                                        cx,
                                    ))
                                    .child(self.roll_checkbox(
                                        "Solo de pista",
                                        self.tracks.get(clip.track).is_some_and(|track| track.solo),
                                        RollAction::TrackSolo,
                                        cx,
                                    )),
                            )
                        },
                    ))
                    .child(div().when(self.piano_roll.inspector_tab == 1, |container| {
                        container.child(
                            section(ds, "Quantize")
                                .child(self.roll_button(
                                    format!(
                                        "Q: 1/{} · Strength…",
                                        self.piano_roll.resolution.denominator()
                                    ),
                                    false,
                                    RollAction::Quantize,
                                    cx,
                                ))
                                .child(self.roll_button("Swing…", false, RollAction::Swing, cx))
                                .child(self.roll_button("Random…", false, RollAction::Random, cx))
                                .child(self.roll_humanize_row(cx)),
                        )
                    }))
                    .child(div().when(self.piano_roll.inspector_tab == 2, |container| {
                        container.child(
                            section(ds, "Scale")
                                .child(self.roll_button(
                                    if self.piano_roll.scale_minor {
                                        "Natural minor ▾"
                                    } else {
                                        "Major ▾"
                                    },
                                    false,
                                    RollAction::ChooseScale,
                                    cx,
                                ))
                                .child(self.roll_button(
                                    format!(
                                        "Root: {} ▾",
                                        [
                                            "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A",
                                            "A♯", "B"
                                        ][usize::from(self.piano_roll.scale_root)]
                                    ),
                                    false,
                                    RollAction::ChooseRoot,
                                    cx,
                                ))
                                .child(self.roll_checkbox(
                                    "Highlight scale notes",
                                    self.piano_roll.highlight_scale,
                                    RollAction::HighlightScale,
                                    cx,
                                ))
                                .child(self.roll_button(
                                    "Clamp selected to scale",
                                    false,
                                    RollAction::ClampScale,
                                    cx,
                                ))
                                .child(self.roll_checkbox(
                                    "Show scale in keyboard",
                                    self.piano_roll.show_scale_keyboard,
                                    RollAction::ToggleScaleKeyboard,
                                    cx,
                                )),
                        )
                    }))
                    .child(div().when(self.piano_roll.inspector_tab == 3, |container| {
                        container.child(
                            section(ds, "View")
                                .child(self.roll_checkbox(
                                    "Ghost notes (other clips)",
                                    self.piano_roll.ghost_notes,
                                    RollAction::GhostNotes,
                                    cx,
                                ))
                                .child(self.roll_checkbox(
                                    "Show lane / velocities",
                                    self.piano_roll.velocities,
                                    RollAction::Velocities,
                                    cx,
                                ))
                                .child(self.roll_button(
                                    format!("{} ▾", self.piano_roll.active_lane.label()),
                                    false,
                                    RollAction::ChooseLane,
                                    cx,
                                ))
                                .child(self.roll_checkbox(
                                    "Note names",
                                    self.piano_roll.show_note_names,
                                    RollAction::NoteNames,
                                    cx,
                                ))
                                .child(self.roll_button(
                                    "Ajustar vista",
                                    false,
                                    RollAction::ResetView,
                                    cx,
                                )),
                        )
                    }))
                    .child(div().when(self.piano_roll.inspector_tab == 4, |container| {
                        container.child(self.render_roll_chord_panel(cx))
                    }))
                    .child(div().when(self.piano_roll.inspector_tab == 5, |container| {
                        container.child(self.render_roll_mpe_panel(cx))
                    })),
            )
    }

    fn roll_checkbox(
        &self,
        label: &'static str,
        checked: bool,
        action: RollAction,
        cx: &mut Context<Self>,
    ) -> Div {
        let ds = &self.ds;
        div()
            .flex()
            .items_center()
            .gap_1()
            .h(px(17.0))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.apply_roll_action(action, window, cx);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(
                badge_tag(ds, if checked { "✓" } else { " " }, ds.accent)
                    .w(px(10.0))
                    .h(px(10.0))
                    .px_0()
                    .text_size(px(9.0))
                    .line_height(px(10.0))
                    .border_1()
                    .border_color(rgb(if checked { ds.accent } else { ds.muted }))
                    .bg(rgb(if checked { ds.accent } else { ds.bg }))
                    .text_color(rgb(ds.bg)),
            )
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(ds.text))
                    .child(label),
            )
    }

    /// Fila de control de intensidad de humanización en el inspector.
    fn roll_humanize_row(&self, cx: &mut Context<Self>) -> Div {
        let ds = &self.ds;
        let amount = self.piano_roll.humanize_amount;
        let amount_down = amount.saturating_sub(10);
        let amount_up = amount.saturating_add(10).min(100);
        div()
            .h(px(16.0))
            .flex()
            .items_center()
            .text_size(px(10.5))
            .child(
                div()
                    .w(px(64.0))
                    .flex_shrink_0()
                    .text_color(rgb(ds.muted))
                    .child("Humanize"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_color(rgb(ds.muted))
                            .text_size(px(10.0))
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.apply_roll_action(
                                        RollAction::HumanizeAmount(amount_down),
                                        window,
                                        cx,
                                    );
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            .child("−"),
                    )
                    .child(
                        div()
                            .min_w(px(32.0))
                            .text_color(rgb(ds.text))
                            .on_scroll(cx.listener(
                                |this, event: &gpui::ScrollEvent, window, cx| {
                                    let delta = -event.delta.pixel_delta(px(1.0)).y;
                                    let step: i16 = if delta > px(0.0) { 5 } else { -5 };
                                    let next = (i16::from(this.piano_roll.humanize_amount) + step)
                                        .clamp(0, 100)
                                        as u8;
                                    this.apply_roll_action(
                                        RollAction::HumanizeAmount(next),
                                        window,
                                        cx,
                                    );
                                    cx.stop_propagation();
                                    cx.notify();
                                },
                            ))
                            .child(format!("{amount}%")),
                    )
                    .child(
                        div()
                            .text_color(rgb(ds.muted))
                            .text_size(px(10.0))
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.apply_roll_action(
                                        RollAction::HumanizeAmount(amount_up),
                                        window,
                                        cx,
                                    );
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            .child("+"),
                    ),
            )
    }
}

fn section(ds: &DesignSystem, label: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(5.0))
        .p_2()
        .mb(px(8.0))
        .rounded(px(ds.radius_sm))
        .border_1()
        .border_color(rgb(ds.border))
        .bg(rgb(ds.panel))
        .child(
            div()
                .h(px(25.0))
                .mb(px(3.0))
                .flex()
                .items_center()
                .border_b_1()
                .border_color(rgb(ds.border))
                .text_size(px(10.5))
                .text_color(rgb(ds.muted))
                .child(label),
        )
}

fn property_action(ds: &DesignSystem, label: &'static str, control: Div) -> Div {
    div()
        .h(px(27.0))
        .flex()
        .items_center()
        .gap_1()
        .child(
            div()
                .w(px(62.0))
                .flex_shrink_0()
                .text_size(px(10.5))
                .text_color(rgb(ds.muted))
                .child(label),
        )
        .child(control.flex_1().min_w_0().overflow_hidden())
}

fn property(ds: &DesignSystem, label: &'static str, value: impl Into<gpui::SharedString>) -> Div {
    div()
        .h(px(24.0))
        .flex()
        .items_center()
        .text_size(px(10.5))
        .child(
            div()
                .w(px(66.0))
                .flex_shrink_0()
                .text_color(rgb(ds.muted))
                .child(label),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_color(rgb(ds.text))
                .child(value.into()),
        )
}
