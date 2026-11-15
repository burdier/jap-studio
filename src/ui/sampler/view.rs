use crate::ui::{
    DawApp,
    components::{
        button::pill_button,
        keyboard::{NotePress, midi_keyboard},
    },
};
use elevate::devices::sampler::{
    ATTACK, CUTOFF, DECAY, END, GAIN, HOLD, LOOP_START, PAN, PITCH_DECAY, PITCH_ENV, PlayMode,
    RELEASE, RESONANCE, ROOT, START, SUSTAIN, TUNE, VEL_FILTER, VEL_VOLUME, VOICES,
};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, rgb};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_sampler(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let params = &self.sampler.params;
        let loaded = self.sampler.asset.is_some();
        let info = self
            .sampler
            .asset
            .as_ref()
            .map(|a| format!("{:.2} s · {} Hz", a.file.duration(), a.file.sample_rate))
            .unwrap_or_else(|| "WAV · FLAC · MP3 · OGG · AAC".into());
        let header = div().flex_shrink_0().flex().flex_wrap().items_center().justify_between().gap_3().px_3().py_2().bg(rgb(ds.panel))
            .child(div().flex().items_center().gap_3()
                .child(div().text_color(rgb(ds.accent)).child("HAY SAMPLER"))
                .child(div().text_color(rgb(ds.text)).child(self.sampler.name().to_owned()))
                .child(div().text_color(rgb(ds.muted)).child(info)))
            .child(div().flex().gap_2()
                .child(pill_button(ds, if self.sampler_ui.busy { "Preparando…" } else { "Cargar sample…" }, false, ds.accent)
                    .id("sampler-load").on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.choose_sample(cx); cx.stop_propagation(); })))
                .child(pill_button(ds, "Recargar", false, ds.muted).id("sampler-reload")
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.prepare_sampler(cx); cx.stop_propagation(); cx.notify(); })))
                .child(pill_button(ds, "Desde biblioteca", false, ds.accent).id("sampler-library")
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        if let Some(path) = this.explorer.view.read(cx).selected_file() { this.load_sample(path, cx); }
                        else { this.status_message = "Selecciona un archivo de audio en el explorador y vuelve al sampler".into(); }
                        cx.stop_propagation(); cx.notify();
                    }))));
        let modes = div()
            .w(px(92.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .child(
                pill_button(
                    ds,
                    "Drum Kit",
                    !params.keytrack && params.mode == PlayMode::OneShot,
                    ds.cyan,
                )
                .id("sampler-drum-preset")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.sampler.params.apply_drum_preset();
                        this.prepare_sampler(cx);
                        this.status_message =
                            "Simpler Drum · 1-Shot + Pitch fijo + Choke + AHD listos para programar"
                                .into();
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
            )
            .child(
                pill_button(ds, "Classic", params.mode == PlayMode::Classic, ds.accent)
                    .id("sampler-classic")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.sample_mode(PlayMode::Classic, cx);
                            cx.stop_propagation();
                        }),
                    ),
            )
            .child(
                pill_button(ds, "1-Shot", params.mode == PlayMode::OneShot, ds.accent)
                    .id("sampler-oneshot")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.sample_mode(PlayMode::OneShot, cx);
                            cx.stop_propagation();
                        }),
                    ),
            )
            .child(
                pill_button(
                    ds,
                    "Loop",
                    params.looped && params.mode == PlayMode::Classic,
                    ds.cyan,
                )
                .id("sampler-loop")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.sampler.params.mode = PlayMode::Classic;
                        this.sampler.params.looped = !this.sampler.params.looped;
                        this.prepare_sampler(cx);
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
            )
            .child(
                pill_button(ds, "Keytrack", params.keytrack, ds.accent)
                    .id("sampler-keytrack")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.sampler.params.keytrack = !this.sampler.params.keytrack;
                            this.prepare_sampler(cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                pill_button(ds, "Choke", params.choke, ds.cyan)
                    .id("sampler-choke")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.sampler.params.choke = !this.sampler.params.choke;
                            this.prepare_sampler(cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                pill_button(ds, "Reverse", params.reverse, ds.accent)
                    .id("sampler-reverse")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.sampler.params.reverse = !this.sampler.params.reverse;
                            this.prepare_sampler(cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            );
        let wave = div()
            .flex_1()
            .min_w_0()
            .relative()
            .h(px(175.))
            .bg(rgb(ds.bg))
            .child(self.sample_waveform(cx))
            .when(!loaded, |el| {
                el.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .child(
                            div()
                                .text_color(rgb(ds.accent))
                                .child("Arrastra aquí un sample"),
                        )
                        .child(
                            div()
                                .text_color(rgb(ds.muted))
                                .child("o usa Cargar sample…"),
                        ),
                )
            });
        let playback = div()
            .flex_shrink_0()
            .flex()
            .flex_wrap()
            .gap_2()
            .p_2()
            .bg(rgb(ds.panel_dark))
            .children(
                [
                    START,
                    END,
                    LOOP_START,
                    TUNE,
                    ROOT,
                    VOICES,
                    PITCH_ENV,
                    PITCH_DECAY,
                    VEL_VOLUME,
                    VEL_FILTER,
                ]
                .map(|id| self.sample_knob(id, cx).into_any_element()),
            );
        let filter = div()
            .flex()
            .items_start()
            .gap_2()
            .p_2()
            .child(self.sample_filter_card(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(self.sample_knob(CUTOFF, cx))
                    .child(self.sample_knob(RESONANCE, cx)),
            );
        let envelope = div().flex().flex_wrap().gap_2().p_2().children(
            [ATTACK, HOLD, DECAY, SUSTAIN, RELEASE, GAIN, PAN]
                .map(|id| self.sample_knob(id, cx).into_any_element()),
        );
        let press: NotePress = Rc::new(cx.listener(|this, note: &u8, _, cx| {
            this.release_mouse_note();
            this.sampler.mouse = Some(*note);
            this.sample_note_on(*note, 100);
            cx.notify();
        }));
        let keyboard = midi_keyboard(
            self.instrument.layout(),
            ds,
            &|note| self.sampler.held(note),
            press,
        );
        div().id("hay-sampler").flex_1().min_h_0().min_w_0().flex().flex_col().text_size(px(11.)).bg(rgb(ds.bg))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if !this.instrument_detached() {
                    return;
                }
                this.cursor_fx.on_mouse_move(event.position);
                if let Some((id, bounds)) = this.sampler_ui.wave_drag {
                    if event.dragging() {
                        let percent = f32::from(event.position.x - bounds.left()) / f32::from(bounds.size.width).max(1.) * 100.;
                        this.set_sample_param(id, percent, cx);
                    } else { this.finish_sample_drag(cx); }
                }
                if let Some((id, origin, fraction)) = this.sampler_ui.drag {
                    if event.dragging() {
                        let delta = f32::from(origin - event.position.y) / if event.modifiers.shift { 1600. } else { 160. };
                        this.set_sample_param(id, elevate::devices::sampler::SamplerParams::value_at(id, fraction + delta), cx);
                    } else { this.finish_sample_drag(cx); }
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| { this.finish_sample_drag(cx); this.release_mouse_note(); cx.notify(); }))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, cx| { this.finish_sample_drag(cx); this.release_mouse_note(); cx.notify(); }))
            .on_drop(cx.listener(|this, paths: &gpui::ExternalPaths, _, cx| {
                if paths.paths().len() == 1 { this.load_sample(paths.paths()[0].clone(), cx); }
                else { this.status_message = "Carga un sample a la vez".into(); }
                cx.stop_propagation(); cx.notify();
            }))
            .child(header)
            .when(loaded, |el| el.child(div().px_3().py_1().text_color(rgb(ds.muted)).child(self.sampler.asset.as_ref().unwrap().source.display().to_string())))
            .child(div().id("sampler-body").overflow_y_scroll().flex_1().min_h_0().flex().flex_col()
                .child(div().flex().flex_shrink_0().child(modes).child(wave))
                .child(playback)
                .child(div().flex().flex_wrap().border_t_1().border_color(rgb(ds.border)).bg(rgb(ds.panel)).child(filter).child(envelope))
                .child(div().px_3().py_2().text_color(rgb(ds.muted)).child("Arrastra controles/marcadores · Alt: inicio del loop · Shift: precisión · doble clic: restablecer"))
                .child(div().h(px(135.)).flex_shrink_0().p_3().flex().child(keyboard)))
            .child(div().px_3().py_2().text_color(rgb(ds.muted)).child(self.status_message.clone()))
    }
}
