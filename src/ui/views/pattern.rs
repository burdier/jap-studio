//! Vista de pasos del modelo de notas común. La audición usa percusión sintetizada.
use super::super::{DawApp, pattern_editor::PatternAction};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, rgb};

impl DawApp {
    pub(in crate::ui) fn render_pattern(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let steps = self.pattern_editor.step_count();
        let playing_step = self.pattern_play_step();
        let ruler = div()
            .flex()
            .h(px(ds.timeline_bar_height))
            .flex_shrink_0()
            .bg(rgb(ds.panel))
            .child(
                div()
                    .w(px(ds.track_header_width))
                    .flex_shrink_0()
                    .px_3()
                    .text_xs()
                    .child("INSTRUMENTO"),
            )
            .children((0..steps).map(|step| {
                div()
                    .flex_1()
                    .text_center()
                    .text_xs()
                    .text_color(rgb(if playing_step == Some(step) {
                        ds.success
                    } else {
                        ds.muted
                    }))
                    .child(format!("{:02}", step + 1))
            }));
        let lanes = div()
            .id("pattern-lanes")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .children(
                (0..self.pattern_editor.lanes.len())
                    .map(|lane| self.render_pattern_lane(lane, playing_step, cx)),
            );
        let playback = if self.pattern_player.is_some() {
            "Reproduciendo en loop"
        } else {
            "Detenido"
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(ruler)
            .child(lanes)
            .child(
                div()
                    .p_3()
                    .text_xs()
                    .text_color(rgb(ds.muted))
                    .child(format!(
                        "{} notas · {playback} · Space: play/stop · Enter: alternar paso · editar no cambia el plan activo",
                        self.pattern_editor.pattern.notes().len(),
                    )),
            )
    }

    fn render_pattern_lane(
        &self,
        lane: usize,
        playing_step: Option<usize>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let colors = [ds.accent, ds.danger, ds.cyan, ds.warning];
        let color = colors[lane % colors.len()];
        let instrument = &self.pattern_editor.lanes[lane];
        let header = div()
            .w(px(ds.track_header_width))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .justify_center()
            .px_3()
            .bg(rgb(ds.panel))
            .child(div().text_color(rgb(color)).child(instrument.name))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(ds.muted))
                    .child("Sintetizado"),
            );
        div()
            .flex()
            .h(px(ds.track_header_height))
            .border_b_1()
            .border_color(rgb(ds.border))
            .child(header)
            .children(
                (0..self.pattern_editor.step_count())
                    .map(|step| self.render_step(lane, step, color, playing_step, cx)),
            )
    }

    fn render_step(
        &self,
        lane: usize,
        step: usize,
        color: u32,
        playing_step: Option<usize>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let note = self.pattern_editor.note_at(lane, step);
        let selected = lane == self.cursor_row && step == self.cursor_col;
        let playing = playing_step == Some(step);
        let background = if note.is_some() {
            color
        } else if playing {
            ds.panel_hover
        } else if step % 4 == 0 {
            ds.panel
        } else {
            ds.panel_dark
        };
        div()
            .flex_1()
            .min_w_0()
            .m_1()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(background))
            .border_1()
            .border_color(rgb(if playing {
                ds.success
            } else if selected {
                ds.text
            } else {
                ds.border
            }))
            .rounded(px(ds.radius_sm))
            .text_xs()
            .text_color(rgb(if note.is_some() { ds.bg } else { ds.muted }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.cursor_row = lane;
                    this.cursor_col = step;
                    this.apply_pattern_action(PatternAction::Toggle { lane, step });
                    cx.notify();
                }),
            )
            .on_scroll(cx.listener(move |this, event: &gpui::ScrollEvent, _, cx| {
                let delta = f32::from(event.delta.pixel_delta(px(24.0)).y);
                if delta != 0.0 {
                    this.apply_pattern_action(PatternAction::Velocity {
                        lane,
                        step,
                        delta: delta.signum() as i16 * 5,
                    });
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(
                note.map(|note| note.velocity.to_string())
                    .unwrap_or_default(),
            )
    }
}
