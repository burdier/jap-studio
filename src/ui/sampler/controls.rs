use crate::ui::{
    DawApp,
    components::knob::{KnobSpec, rotary_knob},
};
use elevate::devices::sampler::{PAN, PARAMS, ROOT, TUNE, VOICES};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, rgb};
impl DawApp {
    pub(super) fn sample_knob(
        &self,
        id: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let spec = PARAMS[id];
        let value = self.sampler.params.get(id);
        let text = if id == ROOT {
            elevate::musical::pitch::note_name(value as u8)
        } else if id == VOICES {
            format!("{}", value as u32)
        } else {
            format!("{value:.1} {}", spec.unit)
        };
        div()
            .id(("sampler-param", id))
            .w(px(87.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_1()
            .items_center()
            .cursor(gpui::CursorStyle::ResizeUpDown)
            .child(div().text_color(rgb(ds.muted)).child(spec.name))
            .child(rotary_knob(
                ds,
                KnobSpec {
                    fraction: self.sampler.params.fraction(id),
                    bipolar: matches!(id, TUNE | PAN),
                    color: ds.accent,
                    diameter: 26.,
                },
            ))
            .child(div().text_color(rgb(ds.cyan)).child(text))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    if event.click_count == 2 {
                        this.set_sample_param(id, PARAMS[id].default, cx);
                    } else {
                        this.sampler_ui.drag =
                            Some((id, event.position.y, this.sampler.params.fraction(id)));
                    }
                    cx.stop_propagation();
                }),
            )
            .on_scroll(cx.listener(move |this, event: &gpui::ScrollEvent, _, cx| {
                let delta = f32::from(event.delta.pixel_delta(px(20.)).y);
                let step = match id {
                    ROOT | VOICES => 1.,
                    TUNE => {
                        if event.modifiers.shift {
                            0.1
                        } else {
                            1.
                        }
                    }
                    _ => {
                        (PARAMS[id].max - PARAMS[id].min)
                            / if event.modifiers.shift { 1000. } else { 100. }
                    }
                };
                if delta != 0. {
                    this.set_sample_param(
                        id,
                        this.sampler.params.get(id) + delta.signum() * step,
                        cx,
                    );
                }
                cx.stop_propagation();
            }))
    }
}
