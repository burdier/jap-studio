//! Panel MPE del inspector del Piano Roll: curvas de pitch, vibrato, legato slide, Slide Y y Pressure Z.
use crate::ui::{
    DawApp, DesignSystem,
    piano_roll::{MpePreset, RollAction, RollLane, RollTool},
};
use gpui::{Context, Div, FontWeight, IntoElement, div, prelude::*, px, rgb};

impl DawApp {
    pub(super) fn render_roll_mpe_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let clip = self.timeline.midi_clip.as_ref().expect("clip visible");
        let pitch_active = self.piano_roll.tool == RollTool::Pitch;
        let selected_count = if !self.piano_roll.selected_notes.is_empty() {
            self.piano_roll.selected_notes.len()
        } else if self.piano_roll.selected.is_some() {
            1
        } else {
            0
        };
        let target_summary = if selected_count > 0 {
            self.tr_fmt(
                "piano_roll.mpe.target_selected",
                &[("count", &selected_count.to_string())],
            )
        } else {
            self.tr_fmt(
                "piano_roll.mpe.target_all",
                &[("count", &clip.notes().len().to_string())],
            )
        };
        let active_lane = self.piano_roll.active_lane;

        div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(8.0))
            // 1. Estado y conmutador de edición directa de curva MPE
            .child(
                mpe_section(ds, self.tr("piano_roll.mpe.title"))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(ds.muted))
                            .child(target_summary),
                    )
                    .child(
                        self.roll_button(
                            if pitch_active {
                                self.tr("piano_roll.mpe.edit_pitch_active")
                            } else {
                                self.tr("piano_roll.mpe.edit_pitch")
                            },
                            pitch_active,
                            RollAction::Tool(RollTool::Pitch),
                            cx,
                        )
                        .w_full()
                        .justify_center(),
                    )
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(rgb(ds.muted))
                            .child(self.tr("piano_roll.mpe.gesture_hint")),
                    ),
            )
            // 2. Carriles de expresión por nota (Y / Z / Pan / Release)
            .child(
                mpe_section(ds, self.tr("piano_roll.mpe.lanes_heading")).child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(4.0))
                        .child(self.roll_button(
                            "Slide (Y)",
                            self.piano_roll.velocities && active_lane == RollLane::MpeSlide,
                            RollAction::MpeOpenLane(RollLane::MpeSlide),
                            cx,
                        ))
                        .child(self.roll_button(
                            "Pressure (Z)",
                            self.piano_roll.velocities && active_lane == RollLane::MpePressure,
                            RollAction::MpeOpenLane(RollLane::MpePressure),
                            cx,
                        ))
                        .child(self.roll_button(
                            "Note Pan",
                            self.piano_roll.velocities && active_lane == RollLane::Pan,
                            RollAction::MpeOpenLane(RollLane::Pan),
                            cx,
                        ))
                        .child(self.roll_button(
                            "Rel Vel",
                            self.piano_roll.velocities && active_lane == RollLane::ReleaseVelocity,
                            RollAction::MpeOpenLane(RollLane::ReleaseVelocity),
                            cx,
                        )),
                ),
            )
            // 3. Generadores de curva de Pitch MPE
            .child(
                mpe_section(ds, self.tr("piano_roll.mpe.curves_heading"))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(4.0))
                            .child(self.roll_button(
                                "↗ +2 st",
                                false,
                                RollAction::MpePreset(MpePreset::GlideUp2),
                                cx,
                            ))
                            .child(self.roll_button(
                                "↘ −2 st",
                                false,
                                RollAction::MpePreset(MpePreset::GlideDown2),
                                cx,
                            ))
                            .child(self.roll_button(
                                "↗ +12 st",
                                false,
                                RollAction::MpePreset(MpePreset::GlideUp12),
                                cx,
                            ))
                            .child(self.roll_button(
                                "↘ −12 st",
                                false,
                                RollAction::MpePreset(MpePreset::GlideDown12),
                                cx,
                            ))
                            .child(self.roll_button(
                                "⤴ Scoop",
                                false,
                                RollAction::MpePreset(MpePreset::Scoop),
                                cx,
                            ))
                            .child(self.roll_button(
                                "⤵ Fall",
                                false,
                                RollAction::MpePreset(MpePreset::Fall),
                                cx,
                            ))
                            .child(self.roll_button(
                                "〜 Vib ±30c",
                                false,
                                RollAction::MpePreset(MpePreset::VibratoSubtle),
                                cx,
                            ))
                            .child(self.roll_button(
                                "〜 Vib ±75c",
                                false,
                                RollAction::MpePreset(MpePreset::VibratoWide),
                                cx,
                            )),
                    )
                    .child(
                        self.roll_button(
                            self.tr("piano_roll.mpe.legato_slide"),
                            false,
                            RollAction::MpePreset(MpePreset::LegatoSlide),
                            cx,
                        )
                        .w_full()
                        .justify_center(),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(4.0))
                            .child(self.roll_button(
                                self.tr("piano_roll.mpe.quantize_st"),
                                false,
                                RollAction::MpePreset(MpePreset::QuantizePitch),
                                cx,
                            ))
                            .child(self.roll_button(
                                self.tr("piano_roll.mpe.invert"),
                                false,
                                RollAction::MpePreset(MpePreset::InvertPitch),
                                cx,
                            ))
                            .child(self.roll_button(
                                self.tr("piano_roll.mpe.reset_pitch"),
                                false,
                                RollAction::MpePreset(MpePreset::ClearPitch),
                                cx,
                            )),
                    ),
            )
            // 4. Niveles rápidos Slide (Y) y Pressure (Z) para notas seleccionadas
            .child(
                mpe_section(ds, self.tr("piano_roll.mpe.quick_expr_heading"))
                    .child(
                        div()
                            .text_size(px(9.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(ds.cyan))
                            .child("Slide (Y · CC 74)"),
                    )
                    .child(div().flex().flex_wrap().gap(px(4.0)).children(
                        [0u8, 32, 64, 96, 127].into_iter().map(|v| {
                            self.roll_button(format!("{v}"), false, RollAction::MpeSetSlide(v), cx)
                        }),
                    ))
                    .child(
                        div()
                            .mt(px(3.0))
                            .text_size(px(9.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(ds.accent))
                            .child("Pressure (Z · Aftertouch)"),
                    )
                    .child(div().flex().flex_wrap().gap(px(4.0)).children(
                        [0u8, 32, 64, 96, 127].into_iter().map(|v| {
                            self.roll_button(
                                format!("{v}"),
                                false,
                                RollAction::MpeSetPressure(v),
                                cx,
                            )
                        }),
                    )),
            )
    }
}

fn mpe_section(ds: &DesignSystem, label: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(5.0))
        .p_2()
        .rounded(px(ds.radius_sm))
        .border_1()
        .border_color(rgb(ds.border))
        .bg(rgb(ds.panel))
        .child(
            div()
                .h(px(22.0))
                .mb(px(2.0))
                .flex()
                .items_center()
                .border_b_1()
                .border_color(rgb(ds.border))
                .text_size(px(10.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(ds.cyan))
                .child(label),
        )
}
