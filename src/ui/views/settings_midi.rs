//! Panel de gestión de mapeos MIDI Learn: inspección, rango min/max, inversión y borrado selectivo.
use crate::{
    instruments::piano_params::SPECS,
    ui::{
        DawApp,
        components::button::{chip_option, flat_control, led_dot},
    },
};
use elevate::musical::{midi_map::MidiBindTarget, track::MASTER_TRACK_ID};
use gpui::{
    Context, FontWeight, MouseButton, component::tooltip::Tooltip, div, prelude::*, px, rgb, rgba,
};

impl DawApp {
    fn describe_midi_target(&self, target: MidiBindTarget) -> String {
        match target {
            MidiBindTarget::TrackVolume(id) => self
                .tracks
                .iter()
                .find(|t| t.id == id)
                .map(|t| format!("{} · Volumen", t.name))
                .unwrap_or_else(|| target.label()),
            MidiBindTarget::TrackPan(id) => self
                .tracks
                .iter()
                .find(|t| t.id == id)
                .map(|t| format!("{} · Pan", t.name))
                .unwrap_or_else(|| target.label()),
            MidiBindTarget::TrackSend { track, target: dst } => {
                let src_name: &str = self
                    .tracks
                    .iter()
                    .find(|t| t.id == track)
                    .map(|t| t.name.as_ref())
                    .unwrap_or("Pista");
                let dst_name: &str = self
                    .tracks
                    .iter()
                    .find(|t| t.id == dst)
                    .map(|t| t.name.as_ref())
                    .unwrap_or("Return");
                format!("{src_name} · Envío → {dst_name}")
            }
            MidiBindTarget::MasterGain => "Master · Volumen".into(),
            MidiBindTarget::InstrumentParam(idx) => SPECS
                .get(usize::from(idx))
                .map(|spec| format!("Piano · {}", spec.label))
                .unwrap_or_else(|| target.label()),
            MidiBindTarget::FxParam { track, fx, param } => {
                let track_name: &str = if track == MASTER_TRACK_ID {
                    "Master"
                } else {
                    self.tracks
                        .iter()
                        .find(|t| t.id == track)
                        .map(|t| t.name.as_ref())
                        .unwrap_or("Pista")
                };
                let fx_name = self
                    .tracks
                    .iter()
                    .find_map(|t| t.chain.find(fx))
                    .or_else(|| self.master_chain.find(fx))
                    .map(|n| n.name.as_str())
                    .unwrap_or("FX");
                if let Some(comp) = self.lua_components.instances.get(&fx)
                    && let Some(ctrl) = comp.descriptor().controls.iter().find(|c| c.id == param)
                {
                    return format!("{track_name} · {fx_name} · {}", ctrl.label);
                }
                if let Some(effect) = self.vst_effects.instances.get(&fx)
                    && let Some(touched) = effect.editor.last_touched()
                    && touched.id == param
                {
                    return format!("{track_name} · {fx_name} · {}", touched.title);
                }
                format!("{track_name} · {fx_name} · P{param}")
            }
            MidiBindTarget::VstInstrumentParam { instrument, param } => {
                let name = self.vst_name(instrument);
                let editor = if self.vst.id == instrument {
                    self.vst.editor()
                } else {
                    self.vst_instances
                        .get(&instrument)
                        .and_then(|inst| inst.editor())
                };
                if let Some(touched) = editor.and_then(|e| e.last_touched())
                    && touched.id == param
                {
                    return format!("{name} · {}", touched.title);
                }
                format!("{name} · P{param}")
            }
            MidiBindTarget::Action(action) => format!("Acción · {action:?}"),
        }
    }

    pub(super) fn render_midi_settings(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let ds = &self.ds;
        let dark = ds.is_dark();
        let learn_active = self.midi_mapper.learn_mode();
        let armed = self
            .midi_mapper
            .armed_target()
            .map(|t| self.describe_midi_target(t));
        let bindings = self.midi_mapper.bindings().to_vec();
        let last_vst = self.last_touched_vst_target();
        let vst_label = last_vst
            .as_ref()
            .map(|(_, label)| format!("Learn VST3 · {label}"))
            .unwrap_or_else(|| "Learn último VST3".into());

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_shrink_0()
                    .px_4()
                    .pb_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(ds.muted))
                            .child(
                                "Inspecciona todos los enlaces MIDI Learn del proyecto, ajusta su rango min/max (o inviértelo) y desvincula controles concretos. Se guardan automáticamente en el archivo .loro.",
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .child(
                                chip_option(
                                    ds,
                                    if learn_active {
                                        "● MIDI Learn ACTIVO"
                                    } else {
                                        "Activar MIDI Learn"
                                    },
                                    learn_active,
                                    ds.accent,
                                )
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.toggle_midi_learn();
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                chip_option(ds, vst_label, false, ds.cyan).on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.arm_last_touched_vst_learn();
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                chip_option(ds, "Restaurar MIDI 1.0", false, ds.muted)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.midi_mapper.reset_defaults();
                                            this.refresh_project_dirty();
                                            this.status_message =
                                                "Mapeos MIDI 1.0 estándar restaurados".into();
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .when(!bindings.is_empty(), |row| {
                                row.child(
                                    chip_option(ds, "Limpiar todos", false, ds.danger)
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.midi_mapper.clear_all();
                                                this.refresh_project_dirty();
                                                this.status_message =
                                                    "Todos los enlaces MIDI eliminados".into();
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        ),
                                )
                            }),
                    )
                    .when_some(armed, |header, target_name| {
                        header.child(
                            div()
                                .px_3()
                                .py_2()
                                .rounded(px(ds.radius_sm))
                                .bg(rgba((ds.cyan << 8) | 0x1c))
                                .border_1()
                                .border_color(rgba((ds.cyan << 8) | 0x66))
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_xs()
                                .text_color(rgb(ds.text))
                                .child(led_dot(ds.cyan, 6.0, true))
                                .child(format!(
                                    "Esperando CC o Nota MIDI para «{target_name}» — mueve tu controlador o pulsa ' (CC21)"
                                )),
                        )
                    }),
            )
            .child(
                div()
                    .id("midi-bindings-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_4()
                    .pb_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .when(bindings.is_empty(), |list| {
                        list.child(
                            div()
                                .p_4()
                                .rounded(px(ds.radius_md))
                                .bg(rgb(ds.panel))
                                .border_1()
                                .border_color(rgba(if dark { 0xffffff10 } else { 0x0f172a14 }))
                                .text_sm()
                                .text_color(rgb(ds.muted))
                                .child(
                                    "No hay enlaces MIDI activos. Activa MIDI Learn y toca cualquier fader, pan, envío Return, slider de FX o usa «Learn último VST3».",
                                ),
                        )
                    })
                    .children(bindings.iter().enumerate().map(|(index, binding)| {
                        let source_label = binding.source.label();
                        let target_label = self.describe_midi_target(binding.target);
                        let min_pct = (binding.min * 100.0).round() as i32;
                        let max_pct = (binding.max * 100.0).round() as i32;
                        let inverted = binding.min > binding.max;
                        let min_val = binding.min;
                        let max_val = binding.max;
                        div()
                            .id(("midi-binding-row", index))
                            .px_3()
                            .py_2()
                            .rounded(px(ds.radius_md))
                            .bg(rgb(ds.panel))
                            .border_1()
                            .border_color(rgba(if dark { 0xffffff10 } else { 0x0f172a14 }))
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .px(px(7.0))
                                            .py(px(2.0))
                                            .rounded(px(ds.radius_sm))
                                            .bg(rgba((ds.cyan << 8) | 0x22))
                                            .border_1()
                                            .border_color(rgba((ds.cyan << 8) | 0x66))
                                            .font_family("monospace")
                                            .text_size(px(10.5))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb(ds.cyan))
                                            .flex_shrink_0()
                                            .child(source_label),
                                    )
                                    .child(div().text_xs().text_color(rgb(ds.muted)).child("→"))
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .text_sm()
                                            .text_color(rgb(ds.text))
                                            .child(target_label),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .flex_shrink_0()
                                    .child(
                                        flat_control(ds, false, ds.accent)
                                            .id(("midi-min", index))
                                            .h(px(22.0))
                                            .px(px(7.0))
                                            .rounded(px(ds.radius_sm))
                                            .font_family("monospace")
                                            .text_size(px(10.0))
                                            .text_color(rgb(ds.text))
                                            .tooltip(|window, cx| {
                                                Tooltip::new(
                                                    "Mínimo · clic izq +5% · clic der -5% · rueda: ajustar (Shift: ±1%)",
                                                )
                                                .build(window, cx)
                                            })
                                            .child(format!("Min {min_pct}%"))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    let next = (min_val + 0.05).clamp(0.0, 1.0);
                                                    this.midi_mapper.set_range(index, next, max_val);
                                                    this.refresh_project_dirty();
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                }),
                                            )
                                            .on_mouse_down(
                                                MouseButton::Right,
                                                cx.listener(move |this, _, _, cx| {
                                                    let next = (min_val - 0.05).clamp(0.0, 1.0);
                                                    this.midi_mapper.set_range(index, next, max_val);
                                                    this.refresh_project_dirty();
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                }),
                                            )
                                            .on_scroll(cx.listener(
                                                move |this, event: &gpui::ScrollEvent, _, cx| {
                                                    let delta = f32::from(
                                                        event.delta.pixel_delta(px(24.0)).y,
                                                    );
                                                    if delta != 0.0 {
                                                        let step = if event.modifiers.shift {
                                                            0.01
                                                        } else {
                                                            0.05
                                                        };
                                                        let next = (min_val
                                                            + delta.signum() * step)
                                                            .clamp(0.0, 1.0);
                                                        this.midi_mapper
                                                            .set_range(index, next, max_val);
                                                        this.refresh_project_dirty();
                                                        cx.stop_propagation();
                                                        cx.notify();
                                                    }
                                                },
                                            )),
                                    )
                                    .child(
                                        flat_control(ds, false, ds.accent)
                                            .id(("midi-max", index))
                                            .h(px(22.0))
                                            .px(px(7.0))
                                            .rounded(px(ds.radius_sm))
                                            .font_family("monospace")
                                            .text_size(px(10.0))
                                            .text_color(rgb(ds.text))
                                            .tooltip(|window, cx| {
                                                Tooltip::new(
                                                    "Máximo · clic izq +5% · clic der -5% · rueda: ajustar (Shift: ±1%)",
                                                )
                                                .build(window, cx)
                                            })
                                            .child(format!("Max {max_pct}%"))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    let next = (max_val + 0.05).clamp(0.0, 1.0);
                                                    this.midi_mapper.set_range(index, min_val, next);
                                                    this.refresh_project_dirty();
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                }),
                                            )
                                            .on_mouse_down(
                                                MouseButton::Right,
                                                cx.listener(move |this, _, _, cx| {
                                                    let next = (max_val - 0.05).clamp(0.0, 1.0);
                                                    this.midi_mapper.set_range(index, min_val, next);
                                                    this.refresh_project_dirty();
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                }),
                                            )
                                            .on_scroll(cx.listener(
                                                move |this, event: &gpui::ScrollEvent, _, cx| {
                                                    let delta = f32::from(
                                                        event.delta.pixel_delta(px(24.0)).y,
                                                    );
                                                    if delta != 0.0 {
                                                        let step = if event.modifiers.shift {
                                                            0.01
                                                        } else {
                                                            0.05
                                                        };
                                                        let next = (max_val
                                                            + delta.signum() * step)
                                                            .clamp(0.0, 1.0);
                                                        this.midi_mapper
                                                            .set_range(index, min_val, next);
                                                        this.refresh_project_dirty();
                                                        cx.stop_propagation();
                                                        cx.notify();
                                                    }
                                                },
                                            )),
                                    )
                                    .child(
                                        flat_control(ds, inverted, ds.warning)
                                            .id(("midi-invert", index))
                                            .h(px(22.0))
                                            .px(px(6.0))
                                            .rounded(px(ds.radius_sm))
                                            .text_size(px(10.0))
                                            .text_color(rgb(if inverted {
                                                ds.warning
                                            } else {
                                                ds.muted
                                            }))
                                            .tooltip(|window, cx| {
                                                Tooltip::new("Invertir rango (Min ↔ Max)")
                                                    .build(window, cx)
                                            })
                                            .child("⇄")
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    this.midi_mapper.invert_range(index);
                                                    this.refresh_project_dirty();
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                }),
                                            ),
                                    )
                                    .child(
                                        flat_control(ds, false, ds.danger)
                                            .id(("midi-remove", index))
                                            .h(px(22.0))
                                            .px(px(6.0))
                                            .rounded(px(ds.radius_sm))
                                            .text_size(px(10.0))
                                            .text_color(rgb(ds.danger))
                                            .tooltip(|window, cx| {
                                                Tooltip::new("Desvincular este control MIDI")
                                                    .build(window, cx)
                                            })
                                            .child("✕")
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    if this.midi_mapper.remove_at(index).is_some() {
                                                        this.refresh_project_dirty();
                                                        this.status_message =
                                                            "Enlace MIDI eliminado".into();
                                                    }
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                }),
                                            ),
                                    ),
                            )
                    })),
            )
            .into_any_element()
    }
}
