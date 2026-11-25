//! Panel visual de Hey Convolver: estética de gabinete de madera + motor FFT estéreo real.
use crate::ui::{
    DawApp,
    components::{button::flat_control, slider::param_slider},
    explorer::ExplorerSampleDrag,
    fx::FxTarget,
};
use elevate::musical::{
    fx::{
        FxEdit, FxKind,
        cabinet::{
            CabinetImpulse, DECAY_PARAMETER, GAIN_PARAMETER, ID, MAX_IR_SECONDS, MIC_PARAMETER,
            MIC_POSITIONS, MIX_PARAMETER, MODELS, PREDELAY_PARAMETER, decay_value, gain_db_value,
            mic_index, mix_value, predelay_ms_value,
        },
    },
    track::TrackDevice,
};
use gpui::{AppContext, Context, MouseButton, div, prelude::*, px, rgb, rgba};
use std::rc::Rc;

impl DawApp {
    pub(super) fn cabinet_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let target = self.fx.current?;
        let id = target.node?;
        if self.tracks.get(self.cursor_row)?.id != target.track {
            return None;
        }
        let node = self.fx_node(target)?;
        let FxKind::Effect {
            plugin_id: TrackDevice::Hay { catalog_id },
            parameters,
        } = &node.kind
        else {
            return None;
        };
        if catalog_id != ID {
            return None;
        }
        let model = (parameters.get(&0).copied().unwrap_or(0.) as usize).min(MODELS.len() - 1);
        let ds = &self.ds;
        let custom = node.cabinet_ir.as_ref();
        let mic = mic_index(parameters.get(&MIC_PARAMETER)).unwrap_or(1);
        let mix = mix_value(parameters.get(&MIX_PARAMETER), custom.is_some(), model);
        let gain_db = gain_db_value(parameters.get(&GAIN_PARAMETER));
        let predelay_ms = predelay_ms_value(parameters.get(&PREDELAY_PARAMETER));
        let decay = decay_value(parameters.get(&DECAY_PARAMETER));

        let switches = div()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(MODELS.iter().enumerate().map(|(i, label)| {
                flat_control(ds, custom.is_none() && i == model, ds.accent)
                    .px_2()
                    .py(px(3.))
                    .text_xs()
                    .child(*label)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            if let Err(e) =
                                this.edit_fx(target.track, FxEdit::Cabinet(id, i as u8, None))
                            {
                                this.status_message = e;
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
            }));

        let select_mic = |i: usize| {
            cx.listener(move |this, _: &gpui::MouseDownEvent, _, cx| {
                if mic != i {
                    if let Err(e) =
                        this.edit_fx(target.track, FxEdit::Parameter(id, MIC_PARAMETER, i as f32))
                    {
                        this.status_message = e;
                    }
                    cx.stop_propagation();
                    cx.notify();
                }
            })
        };

        let ring = |diameter: f32, color: u32| {
            div()
                .absolute()
                .left(px((76. - diameter) / 2.))
                .top(px((76. - diameter) / 2.))
                .size(px(diameter))
                .rounded_full()
                .border_1()
                .border_color(rgb(color))
        };

        let speaker = div()
            .relative()
            .flex_shrink_0()
            .size(px(76.))
            .rounded_full()
            .bg(rgb(ds.panel_dark))
            .child(ring(74., ds.muted))
            .child(ring(64., ds.border))
            .child(ring(56., ds.border))
            .child(ring(27., ds.muted))
            .children([38., 54., 70.].into_iter().enumerate().map(|(i, x)| {
                div()
                    .absolute()
                    .left(px(x - 8.))
                    .top(px(30.))
                    .size(px(16.))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .size(px(if i == mic { 10. } else { 5. }))
                            .rounded_full()
                            .bg(rgb(if i == mic { ds.cyan } else { ds.muted })),
                    )
                    .on_mouse_down(MouseButton::Left, select_mic(i))
            }));

        let body = div().flex().gap_2().items_center().child(speaker).child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(ds.muted))
                        .child(if custom.is_some() {
                            "Tono de la IR"
                        } else {
                            "Posición / Tono"
                        }),
                )
                .child(div().flex().flex_wrap().gap_1().children(
                    MIC_POSITIONS.iter().enumerate().map(|(i, label)| {
                        flat_control(ds, i == mic, ds.cyan)
                            .px(px(5.))
                            .py(px(3.))
                            .text_xs()
                            .child(*label)
                            .on_mouse_down(MouseButton::Left, select_mic(i))
                    }),
                ))
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(ds.muted))
                        .child(["Más brillo", "Respuesta natural", "Agudos suaves"][mic]),
                ),
        );

        let set_param = move |param: u32, val: f32| {
            move |this: &mut DawApp, cx: &mut Context<DawApp>| {
                if let Err(e) = this.edit_fx(target.track, FxEdit::Parameter(id, param, val)) {
                    this.status_message = e;
                }
                cx.notify();
            }
        };

        let sliders = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(param_slider(
                ds,
                "Mix".into(),
                format!("{:.0}%", mix * 100.0),
                mix,
                Rc::new(cx.listener(move |this, &f: &f32, _, cx| {
                    set_param(MIX_PARAMETER, f.clamp(0.0, 1.0))(this, cx);
                })),
                Rc::new(cx.listener(move |this, _: &(), _, cx| {
                    let def = if model == 3 { 1.0 } else { 0.35 };
                    set_param(MIX_PARAMETER, def)(this, cx);
                })),
            ))
            .child(param_slider(
                ds,
                "Decay".into(),
                format!("{:.0}%", decay * 100.0),
                (decay - 0.1) / 0.9,
                Rc::new(cx.listener(move |this, &f: &f32, _, cx| {
                    set_param(DECAY_PARAMETER, 0.1 + f.clamp(0.0, 1.0) * 0.9)(this, cx);
                })),
                Rc::new(cx.listener(move |this, _: &(), _, cx| {
                    set_param(DECAY_PARAMETER, 1.0)(this, cx);
                })),
            ))
            .child(param_slider(
                ds,
                "Pre-delay".into(),
                format!("{:.0} ms", predelay_ms),
                predelay_ms / 120.0,
                Rc::new(cx.listener(move |this, &f: &f32, _, cx| {
                    set_param(PREDELAY_PARAMETER, (f.clamp(0.0, 1.0) * 120.0).round())(this, cx);
                })),
                Rc::new(cx.listener(move |this, _: &(), _, cx| {
                    set_param(PREDELAY_PARAMETER, 0.0)(this, cx);
                })),
            ))
            .child(param_slider(
                ds,
                "Salida".into(),
                format!("{gain_db:+.1} dB"),
                (gain_db + 12.0) / 24.0,
                Rc::new(cx.listener(move |this, &f: &f32, _, cx| {
                    let db = ((f.clamp(0.0, 1.0) * 24.0 - 12.0) * 2.0).round() / 2.0;
                    set_param(GAIN_PARAMETER, db)(this, cx);
                })),
                Rc::new(cx.listener(move |this, _: &(), _, cx| {
                    set_param(GAIN_PARAMETER, 0.0)(this, cx);
                })),
            ));

        let stain = crate::ui::design::blend_color(0xb08359, ds.orange, 18);
        let wood = crate::ui::design::blend_color(ds.panel, stain, 32);
        let grain = crate::ui::design::blend_color(wood, ds.text, 23);

        Some(
            div()
                .relative()
                .overflow_hidden()
                .p_2()
                .flex()
                .flex_col()
                .gap_2()
                .bg(rgb(wood))
                .border_t_1()
                .border_color(rgb(ds.border))
                .text_color(rgb(ds.text))
                .child(
                    gpui::svg()
                        .data(
                            include_bytes!("../../../assets/textures/cabinet-grain.svg").as_slice(),
                        )
                        .absolute()
                        .inset_0()
                        .size_full()
                        .text_color(rgb(grain)),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("Hey Convolver"),
                )
                .child(div().flex().items_center().gap_2().child(switches))
                .child(body)
                .child(sliders)
                .child(
                    div()
                        .py(px(5.))
                        .px_2()
                        .rounded(px(ds.radius_sm))
                        .bg(rgb(ds.panel_dark))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .text_xs()
                        .child(if let Some(ir) = custom {
                            div().text_color(rgb(ds.cyan)).truncate().child(format!(
                                "{} · {:.2} s · {:.1} kHz",
                                ir.name,
                                ir.duration_seconds(),
                                ir.rate as f64 / 1000.0
                            ))
                        } else {
                            div().text_color(rgb(ds.muted)).child(format!(
                                "Arrastra una captura WAV · máx. {MAX_IR_SECONDS} s"
                            ))
                        })
                        .when(custom.is_some(), |row| {
                            row.child(
                                div()
                                    .id("convolver-clear-ir")
                                    .px(px(5.))
                                    .rounded(px(3.))
                                    .bg(rgba((ds.text << 8) | 0x15))
                                    .hover(|b| b.bg(rgba((ds.danger << 8) | 0x35)))
                                    .text_color(rgb(ds.text))
                                    .cursor_pointer()
                                    .child("×")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            if let Err(e) = this.edit_fx(
                                                target.track,
                                                FxEdit::Cabinet(id, model as u8, None),
                                            ) {
                                                this.status_message = e;
                                            }
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                        })
                        .on_drop(cx.listener(move |this, drag: &ExplorerSampleDrag, _, cx| {
                            this.load_cabinet_ir(target, drag.path.clone(), cx);
                            cx.stop_propagation();
                        }))
                        .on_drop(
                            cx.listener(move |this, paths: &gpui::ExternalPaths, _, cx| {
                                if let Some(path) = paths.paths().first() {
                                    this.load_cabinet_ir(target, path.clone(), cx);
                                }
                                cx.stop_propagation();
                            }),
                        ),
                ),
        )
    }

    fn load_cabinet_ir(
        &mut self,
        target: FxTarget,
        path: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = target.node else { return };
        let job = cx.background_spawn(async move {
            let file = crate::audio::AudioFile::decode(&path)?;
            if file.frames.len() > file.sample_rate as usize * MAX_IR_SECONDS {
                return Err(format!("La IR supera {MAX_IR_SECONDS} segundos"));
            }
            let mut pcm = Vec::with_capacity(file.frames.len() * 4);
            for x in file.frames.iter().flatten() {
                pcm.extend_from_slice(&((x.clamp(-1., 1.) * 32767.).round() as i16).to_le_bytes());
            }
            CabinetImpulse::new(file.name, file.sample_rate, pcm)
        });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                if let Err(e) = result
                    .and_then(|ir| this.edit_fx(target.track, FxEdit::Cabinet(id, 0, Some(ir))))
                {
                    this.status_message = e;
                }
                cx.notify();
            });
        })
        .detach();
    }
}
