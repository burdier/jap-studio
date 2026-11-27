//! Vista de Hay Synth: 3 pestañas (`Stage` XY humano · `Voice` laboratorio estilo Vital · `Matrix` 12 ranuras).
use crate::ui::{
    DawApp,
    components::{
        button::pill_button,
        keyboard::{NotePress, midi_keyboard},
        knob::{KnobSpec, rotary_knob},
    },
    synth::{PresetCategory, PresetSource, SynthKnobDrag, SynthTab, format_synth_param_value},
};
use elevate::devices::synth::{
    DistMode, EnvelopeMode, FACTORY_PRESETS, FilterKind, LfoShape, MATRIX_SLOTS, ModSource,
    ModTarget, RoomKind, SynthEngineKind, WarpMode, WavetableKind, cc, param,
    vital_factory_presets,
};
use gpui::{
    Bounds, Context, Corners, Edges, FontWeight, IntoElement, MouseButton, canvas, div, fill,
    point, prelude::*, px, quad, rgb, rgba, size,
};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_synth(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let params = &self.synth.params;
        let active_notes = self.synth.active_note_count();
        let last_vel = self.synth.last_velocity;
        let tab = self.synth_ui.tab;

        let active_routes = params
            .voice
            .matrix
            .iter()
            .filter(|s| {
                s.source != ModSource::None && s.target != ModTarget::None && s.amount.abs() > 0.005
            })
            .count();

        let preset_category = FACTORY_PRESETS
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(&params.preset_name))
            .map(|p| p.category)
            .or_else(|| {
                vital_factory_presets()
                    .into_iter()
                    .find(|(n, _, _)| n.eq_ignore_ascii_case(&params.preset_name))
                    .map(|(_, c, _)| c)
            })
            .unwrap_or("User");
        let preset_label = if params.preset_name.is_empty() {
            "Init".to_owned()
        } else {
            params.preset_name.clone()
        };

        // ── Barra superior: identidad, presets (clic abre menú), pestañas, Reset y Guardar ──
        let matrix_tab_label = if active_routes > 0 {
            format!("Matrix ({active_routes})")
        } else {
            "Matrix".into()
        };
        let menu_open = self.synth_ui.preset_menu_open;
        let header = div()
            .flex_shrink_0()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .px_3()
            .py_2()
            .bg(rgb(ds.panel))
            .border_b_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(ds.accent))
                            .child("HAY SYNTH"),
                    )
                    .child(
                        div()
                            .w(px(80.))
                            .px(px(6.))
                            .py(px(1.))
                            .rounded(px(3.))
                            .bg(rgb(ds.panel_dark))
                            .text_center()
                            .text_size(px(10.))
                            .text_color(rgb(ds.cyan))
                            .truncate()
                            .child(params.engine.label()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .ml_1()
                            .child(
                                pill_button(ds, "◀", false, ds.muted)
                                    .id("synth-preset-prev")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.step_synth_preset(-1);
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .id("synth-preset-menu-trigger")
                                    .w(px(216.))
                                    .px_2()
                                    .py(px(3.))
                                    .rounded(px(ds.radius_sm))
                                    .bg(rgb(ds.panel_dark))
                                    .border_1()
                                    .border_color(rgb(if menu_open { ds.cyan } else { ds.border }))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.synth_ui.ensure_user_presets_loaded();
                                            this.synth_ui.preset_menu_open =
                                                !this.synth_ui.preset_menu_open;
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(rgb(ds.text))
                                            .truncate()
                                            .child(preset_label),
                                    )
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .text_size(px(9.5))
                                            .text_color(rgb(if menu_open {
                                                ds.cyan
                                            } else {
                                                ds.muted
                                            }))
                                            .child(format!("{preset_category} ▾")),
                                    ),
                            )
                            .child(
                                pill_button(ds, "▶", false, ds.muted)
                                    .id("synth-preset-next")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.step_synth_preset(1);
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .ml_2()
                            .child(
                                pill_button(ds, "Stage", tab == SynthTab::Stage, ds.accent)
                                    .id("synth-tab-stage")
                                    .w(px(58.))
                                    .justify_center()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.synth_ui.tab = SynthTab::Stage;
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                pill_button(ds, "Voice", tab == SynthTab::Voice, ds.cyan)
                                    .id("synth-tab-voice")
                                    .w(px(58.))
                                    .justify_center()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.synth_ui.tab = SynthTab::Voice;
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                pill_button(
                                    ds,
                                    &matrix_tab_label,
                                    tab == SynthTab::Matrix,
                                    ds.accent,
                                )
                                .id("synth-tab-matrix")
                                .w(px(86.))
                                .justify_center()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.synth_ui.tab = SynthTab::Matrix;
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        pill_button(ds, "Reset", false, ds.muted)
                            .id("synth-reset-init")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.reset_synth_init();
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        pill_button(ds, "Importar…", false, ds.accent)
                            .id("synth-import-vital")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.choose_vital_file(cx);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        pill_button(ds, "Guardar", false, ds.cyan)
                            .id("synth-save-preset")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.save_user_synth_preset();
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(self.render_window_button(cx)),
            );

        // ── Selector de motor + Flujo de señal claro (qué capas están sonando hacia Stage) ──
        let osc_b_on = params.voice.osc_b_level > 0.01;
        let sub_on = params.voice.sub_level > 0.01;
        let noise_on = params.voice.noise_level > 0.01;
        let engine_bar = div()
            .flex_shrink_0()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .px_3()
            .py(px(6.))
            .bg(rgb(ds.panel_dark))
            .border_b_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(9.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(ds.muted))
                            .mr_1()
                            .child("FUENTE"),
                    )
                    .children(SynthEngineKind::ALL.into_iter().map(|engine| {
                        let active = params.engine == engine;
                        pill_button(
                            ds,
                            engine.label(),
                            active,
                            if engine == SynthEngineKind::Wavetable {
                                ds.cyan
                            } else {
                                ds.accent
                            },
                        )
                        .id(("synth-engine", engine as usize))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.set_synth_engine(engine);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(9.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(ds.muted))
                            .child("CAPAS:"),
                    )
                    .child(
                        pill_button(ds, if osc_b_on { "OSC B ON" } else { "OSC B" }, osc_b_on, ds.cyan)
                            .id("synth-flow-oscb")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    let next = if this.synth.params.voice.osc_b_level > 0.01 { 0.0 } else { 0.48 };
                                    this.set_synth_voice_param(param::OSC_B_LEVEL, next);
                                    this.status_message = format!(
                                        "Hay Synth · OSC B {}",
                                        if next > 0.0 { "activado" } else { "silenciado" }
                                    );
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        pill_button(ds, if sub_on { "SUB ON" } else { "SUB" }, sub_on, ds.accent)
                            .id("synth-flow-sub")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    let next = if this.synth.params.voice.sub_level > 0.01 { 0.0 } else { 0.45 };
                                    this.set_synth_voice_param(param::SUB_LEVEL, next);
                                    this.status_message = format!(
                                        "Hay Synth · SUB {}",
                                        if next > 0.0 { "activado" } else { "silenciado" }
                                    );
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        pill_button(ds, if noise_on { "NOISE ON" } else { "NOISE" }, noise_on, ds.muted)
                            .id("synth-flow-noise")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    let next = if this.synth.params.voice.noise_level > 0.01 { 0.0 } else { 0.18 };
                                    this.set_synth_voice_param(param::NOISE_LEVEL, next);
                                    this.status_message = format!(
                                        "Hay Synth · Noise {}",
                                        if next > 0.0 { "activado" } else { "silenciado" }
                                    );
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(rgb(ds.cyan))
                            .ml(px(2.))
                            .child("→ STAGE"),
                    ),
            );

        let tab_content = match tab {
            SynthTab::Stage => self
                .render_synth_stage_tab(active_notes, last_vel, cx)
                .into_any_element(),
            SynthTab::Voice => self.render_synth_voice_tab(cx).into_any_element(),
            SynthTab::Matrix => self.render_synth_matrix_tab(cx).into_any_element(),
        };

        // ── Teclado clicable inferior ──
        let press: NotePress = Rc::new(cx.listener(|this, note: &u8, _, cx| {
            this.release_mouse_note();
            this.synth.mouse = Some(*note);
            this.synth_note_on(*note, 102);
            cx.notify();
        }));
        let keyboard = midi_keyboard(
            self.instrument.layout(),
            ds,
            &|note| self.synth.held(note),
            press,
        );

        let preset_overlay = menu_open.then(|| self.render_synth_preset_menu(cx));

        div()
            .id("hay-synth")
            .relative()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .text_size(px(11.))
            .bg(rgb(ds.bg))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.synth_ui.preset_menu_open {
                        this.synth_ui.preset_menu_open = false;
                        cx.notify();
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if !this.instrument_detached() {
                    return;
                }
                if this.update_synth_drag(event.position, event.dragging(), event.modifiers.shift)
                {
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    let changed = this.finish_synth_drag();
                    this.release_mouse_note();
                    if changed {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    let changed = this.finish_synth_drag();
                    this.release_mouse_note();
                    if changed {
                        cx.notify();
                    }
                }),
            )
            .child(header)
            .child(engine_bar)
            .child(self.render_synth_live_strip())
            .child(
                div()
                    .id("synth-body")
                    .overflow_y_scroll()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(tab_content)
                    .child(
                        div()
                            .flex_shrink_0()
                            .px_3()
                            .pb_1()
                            .text_size(px(10.))
                            .text_color(rgb(ds.muted))
                            .child("Arrastra perillas o usa la rueda · Shift: precisión fina · doble clic: restablecer"),
                    )
                    .child(
                        div()
                            .h(px(112.))
                            .flex_shrink_0()
                            .py_2()
                            .px_3()
                            .flex()
                            .child(keyboard),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .px_3()
                    .py_1()
                    .border_t_1()
                    .border_color(rgb(ds.border))
                    .text_color(rgb(ds.muted))
                    .truncate()
                    .child(self.status_message.clone()),
            )
            .when_some(preset_overlay, |root, overlay| root.child(overlay))
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Visor en vivo (60 fps): Osciloscopio + Transiente deslizante + Curva ADSR/LFO viva
    // Altura fija (74 px) -> cero saltos de layout.
    // ──────────────────────────────────────────────────────────────────────────
    fn render_synth_live_strip(&self) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let params = &self.synth.params;
        let vp = &params.voice;
        let active_notes = self.synth.active_note_count();

        let wave: Vec<f32> = if self.synth_ui.scope_active() && !self.synth_ui.scope_wave.is_empty()
        {
            self.synth_ui.scope_wave.clone()
        } else if let Some(custom) = &params.custom_wt_a {
            let fi = ((params.shape.clamp(0.0, 1.0) * 7.0).round() as usize).min(7);
            (0..128).map(|i| custom.frames[fi][(i * 2).min(255)] * 0.72).collect()
        } else {
            (0..128)
                .map(|i| {
                    let ph = i as f32 / 128.0;
                    ((ph * std::f32::consts::TAU).sin() * (1.0 - params.shape * 0.5)
                        + (2.0 * ph - 1.0) * (params.shape * 0.5))
                        * 0.60
                })
                .collect()
        };
        let live_audio = self.synth_ui.scope_active();
        let trail = if self.synth_ui.transient_trail.is_empty() {
            vec![0.0; 56]
        } else {
            self.synth_ui.transient_trail.clone()
        };
        let meter_l = self.synth_ui.meter_l;
        let meter_r = self.synth_ui.meter_r;

        // Tiempos ADSR reales para dibujar la curva y calcular dónde va el punto luminoso.
        let (atk_s, dec_s, sus_lvl, rel_s) = if vp.env1_override {
            (
                0.0035 + vp.env1_attack * vp.env1_attack * 2.4,
                0.02 + vp.env1_decay * vp.env1_decay * 3.5,
                vp.env1_sustain.clamp(0.0, 1.0),
                0.02 + vp.env1_release * vp.env1_release * 4.0,
            )
        } else {
            let t = params.time;
            match params.mode {
                EnvelopeMode::Hit => (0.004, 0.08 + t * t * 2.2, 0.0, 0.04 + t * 0.45),
                EnvelopeMode::Hold => (0.004 + t * 0.035, 0.14 + t * 0.9, 0.76, 0.08 + t * 1.1),
                EnvelopeMode::Swell => (0.12 + t * t * 1.6, 0.30 + t * 1.2, 0.85, 0.28 + t * t * 2.6),
            }
        };

        // Posición normalizada [0..1] del playhead sobre la curva ADSR [A:0..0.25, D:0.25..0.55, S:0.55..0.72, R:0.72..1.0].
        let env_playhead: Option<(f32, f32)> = if active_notes > 0 {
            let dt = self
                .synth_ui
                .note_started_at
                .map_or(0.0, |t| t.elapsed().as_secs_f32());
            if dt < atk_s {
                let f = (dt / atk_s.max(0.004)).clamp(0.0, 1.0);
                Some((f * 0.25, f))
            } else if dt < atk_s + dec_s {
                let f = ((dt - atk_s) / dec_s.max(0.02)).clamp(0.0, 1.0);
                let y = 1.0 - (1.0 - sus_lvl) * f;
                Some((0.25 + f * 0.30, y))
            } else {
                Some((0.64, sus_lvl))
            }
        } else if let Some(rel_t) = self.synth_ui.note_released_at {
            let dt = rel_t.elapsed().as_secs_f32();
            if dt < rel_s.min(1.6) {
                let f = (dt / rel_s.max(0.03)).clamp(0.0, 1.0);
                let y = sus_lvl * (1.0 - f).powi(2);
                Some((0.72 + f * 0.28, y))
            } else {
                None
            }
        } else {
            None
        };

        // Pulso vivo del LFO 1 (0..1)
        let lfo_hz = if vp.lfo1_sync {
            const MULT: [f32; 8] = [0.0625, 0.125, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0];
            let idx = ((vp.lfo1_rate * 7.0).round() as usize).min(7);
            (self.timeline.tempo.bpm() as f32 / 60.0) * MULT[idx]
        } else {
            0.05 * (320.0_f32).powf(vp.lfo1_rate)
        };
        let lfo_phase = self
            .synth_ui
            .lfo_epoch
            .map_or(0.0, |t| (t.elapsed().as_secs_f32() * lfo_hz).fract());
        let lfo_val = ((lfo_phase * std::f32::consts::TAU).sin() * 0.5 + 0.5).clamp(0.0, 1.0);

        let bg_col = ds.bg;
        let border_col = ds.border;
        let cyan_col = ds.cyan;
        let accent_col = ds.accent;
        let muted_col = ds.muted;

        let scope_canvas = canvas(
            move |_, _, _| (),
            move |bounds, (), window, _| {
                let w = f32::from(bounds.size.width).max(1.0);
                let h = f32::from(bounds.size.height).max(1.0);
                let mid = h * 0.5;
                // Línea base central
                window.paint_quad(fill(
                    Bounds::new(
                        point(bounds.left(), bounds.top() + px(mid)),
                        size(px(w), px(1.0)),
                    ),
                    rgba((border_col << 8) | 0x90),
                ));
                let n = wave.len().max(2);
                let dx = w / n as f32;
                let glow = if live_audio {
                    rgba((cyan_col << 8) | 0x38)
                } else {
                    rgba((muted_col << 8) | 0x24)
                };
                let stroke = if live_audio { rgb(cyan_col) } else { rgba((cyan_col << 8) | 0x88) };
                for i in 0..n {
                    let s = wave[i].clamp(-1.0, 1.0);
                    let y = mid - s * (mid - 4.0);
                    let prev_y = if i > 0 {
                        mid - wave[i - 1].clamp(-1.0, 1.0) * (mid - 4.0)
                    } else {
                        y
                    };
                    let x = i as f32 * dx;
                    // Relleno suave hacia el eje central
                    let fill_top = y.min(mid);
                    let fill_h = (y - mid).abs().max(1.0);
                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.left() + px(x), bounds.top() + px(fill_top)),
                            size(px(dx.max(1.2)), px(fill_h)),
                        ),
                        glow,
                    ));
                    // Trazo continuo entre muestras consecutivas
                    let seg_top = y.min(prev_y);
                    let seg_h = (y - prev_y).abs().max(2.0);
                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.left() + px(x), bounds.top() + px(seg_top - 0.5)),
                            size(px((dx + 0.8).max(1.8)), px(seg_h)),
                        ),
                        stroke,
                    ));
                }
            },
        )
        .w_full()
        .h(px(46.));

        let transient_canvas = canvas(
            move |_, _, _| (),
            move |bounds, (), window, _| {
                let w = f32::from(bounds.size.width).max(1.0);
                let h = f32::from(bounds.size.height).max(1.0);
                let meter_w = 18.0;
                let plot_w = (w - meter_w - 6.0).max(10.0);
                let n = trail.len().max(1);
                let dx = plot_w / n as f32;
                for (i, &lv) in trail.iter().enumerate() {
                    let shaped = lv.clamp(0.0, 1.0).sqrt();
                    let bh = (shaped * (h - 4.0)).max(1.5);
                    let is_hot = lv > 0.45;
                    let is_recent = i + 4 >= n;
                    let col = if is_hot {
                        rgb(accent_col)
                    } else if is_recent && live_audio {
                        rgb(cyan_col)
                    } else {
                        rgba((cyan_col << 8) | 0x9a)
                    };
                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.left() + px(i as f32 * dx), bounds.top() + px(h - bh)),
                            size(px((dx - 1.0).max(1.2)), px(bh)),
                        ),
                        col,
                    ));
                }
                // Medidores estéreo L / R en tiempo real
                let mx = plot_w + 6.0;
                for (ch, lv) in [(0, meter_l), (1, meter_r)] {
                    let cx_pos = mx + ch as f32 * 8.0;
                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.left() + px(cx_pos), bounds.top() + px(2.0)),
                            size(px(5.0), px(h - 4.0)),
                        ),
                        rgba((border_col << 8) | 0x70),
                    ));
                    let mh = (lv.clamp(0.0, 1.0).sqrt() * (h - 4.0)).max(1.0);
                    let mcol = if lv > 0.85 { rgb(accent_col) } else { rgb(cyan_col) };
                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.left() + px(cx_pos), bounds.top() + px(h - 2.0 - mh)),
                            size(px(5.0), px(mh)),
                        ),
                        mcol,
                    ));
                }
            },
        )
        .w_full()
        .h(px(46.));

        let adsr_canvas = canvas(
            move |_, _, _| (),
            move |bounds, (), window, _| {
                let w = f32::from(bounds.size.width).max(1.0);
                let h = f32::from(bounds.size.height).max(1.0);
                let pad_y = 4.0;
                let usable_h = (h - pad_y * 2.0).max(6.0);
                // Muestrear curva ADSR en 48 columnas
                let cols = 48usize;
                let dx = w / cols as f32;
                let sample_env = |u: f32| -> f32 {
                    if u < 0.25 {
                        (u / 0.25).clamp(0.0, 1.0)
                    } else if u < 0.55 {
                        let f = ((u - 0.25) / 0.30).clamp(0.0, 1.0);
                        1.0 - (1.0 - sus_lvl) * f.powf(0.75)
                    } else if u < 0.72 {
                        sus_lvl
                    } else {
                        let f = ((u - 0.72) / 0.28).clamp(0.0, 1.0);
                        sus_lvl * (1.0 - f).powi(2)
                    }
                };
                for i in 0..cols {
                    let u = i as f32 / (cols - 1) as f32;
                    let val = sample_env(u);
                    let y = pad_y + (1.0 - val) * usable_h;
                    let fill_h = (h - y).max(1.0);
                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.left() + px(i as f32 * dx), bounds.top() + px(y)),
                            size(px(dx.max(1.2)), px(fill_h)),
                        ),
                        rgba((accent_col << 8) | 0x30),
                    ));
                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.left() + px(i as f32 * dx), bounds.top() + px(y - 1.0)),
                            size(px((dx + 0.8).max(1.6)), px(2.0)),
                        ),
                        rgb(accent_col),
                    ));
                }
                // Punto vivo recorriendo el transiente ADSR
                if let Some((pu, py_val)) = env_playhead {
                    let px_x = (pu * w).clamp(2.0, w - 3.0);
                    let py_y = pad_y + (1.0 - py_val.clamp(0.0, 1.0)) * usable_h;
                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.left() + px(px_x - 0.5), bounds.top() + px(2.0)),
                            size(px(1.5), px(h - 4.0)),
                        ),
                        rgba((cyan_col << 8) | 0x80),
                    ));
                    window.paint_quad(quad(
                        Bounds::new(
                            point(bounds.left() + px(px_x - 4.0), bounds.top() + px(py_y - 4.0)),
                            size(px(8.0), px(8.0)),
                        ),
                        Corners::all(px(4.0)),
                        rgb(cyan_col),
                        Edges::all(px(1.0)),
                        rgb(bg_col),
                        gpui::BorderStyle::default(),
                    ));
                }
            },
        )
        .w_full()
        .h(px(46.));

        let peak_db = if self.synth_ui.transient_peak > 1e-4 {
            format!("{:+.0} dB", 20.0 * self.synth_ui.transient_peak.log10())
        } else {
            "-∞ dB".into()
        };

        div()
            .flex_shrink_0()
            .h(px(74.))
            .px_3()
            .py(px(5.))
            .bg(rgb(ds.panel))
            .border_b_1()
            .border_color(rgb(ds.border))
            .flex()
            .items_stretch()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px_2()
                    .py(px(3.))
                    .rounded(px(ds.radius_sm))
                    .bg(rgb(ds.bg))
                    .border_1()
                    .border_color(rgb(if live_audio { ds.cyan } else { ds.border }))
                    .flex()
                    .flex_col()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_size(px(9.))
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(ds.cyan))
                                    .child(if live_audio {
                                        "OSCILOSCOPIO · EN VIVO"
                                    } else {
                                        "WAVETABLE · OSC A"
                                    }),
                            )
                            .child(
                                div()
                                    .font_family("monospace")
                                    .text_color(rgb(ds.muted))
                                    .child(if active_notes > 0 {
                                        format!("{active_notes} voz")
                                    } else {
                                        "listo".into()
                                    }),
                            ),
                    )
                    .child(scope_canvas),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px_2()
                    .py(px(3.))
                    .rounded(px(ds.radius_sm))
                    .bg(rgb(ds.bg))
                    .border_1()
                    .border_color(rgb(ds.border))
                    .flex()
                    .flex_col()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_size(px(9.))
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(ds.accent))
                                    .child("TRANSIENTE · L/R"),
                            )
                            .child(
                                div()
                                    .w(px(46.))
                                    .text_right()
                                    .font_family("monospace")
                                    .text_color(rgb(if live_audio { ds.text } else { ds.muted }))
                                    .child(peak_db),
                            ),
                    )
                    .child(transient_canvas),
            )
            .child(
                div()
                    .w(px(210.))
                    .flex_shrink_0()
                    .px_2()
                    .py(px(3.))
                    .rounded(px(ds.radius_sm))
                    .bg(rgb(ds.bg))
                    .border_1()
                    .border_color(rgb(ds.border))
                    .flex()
                    .flex_col()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_size(px(9.))
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(ds.accent))
                                    .child("ENV 1 · ADSR"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .child(
                                        div()
                                            .text_color(rgb(ds.muted))
                                            .child("LFO"),
                                    )
                                    .child(
                                        div()
                                            .w(px(34.))
                                            .h(px(5.))
                                            .rounded(px(3.))
                                            .bg(rgb(ds.panel_dark))
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .h_full()
                                                    .w(px((lfo_val * 34.0).clamp(2.0, 34.0)))
                                                    .bg(rgb(ds.cyan)),
                                            ),
                                    ),
                            ),
                    )
                    .child(adsr_canvas),
            )
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Menú flotante estilo Add: pestañas separadas por origen (Hay Synth | Bancos | Serum | User | FX)
    // y categorías limpias dentro de cada origen.
    // ──────────────────────────────────────────────────────────────────────────
    fn render_synth_preset_menu(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let active_src = self.synth_ui.preset_source;
        let active_cat = self.synth_ui.preset_category;
        let active_name = self.synth.params.preset_name.clone();
        let all_native = self.synth_ui.all_presets();
        let factory_count = FACTORY_PRESETS.len() + vital_factory_presets().len();
        let user_count = self.synth_ui.user_presets.len();
        let vital_count = self
            .synth_ui
            .vital_library
            .iter()
            .filter(|e| {
                e.path
                    .extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| x.eq_ignore_ascii_case("vital"))
            })
            .count();
        let serum_count = self
            .synth_ui
            .vital_library
            .iter()
            .filter(|e| {
                e.path
                    .extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| x.eq_ignore_ascii_case("fxp"))
            })
            .count();

        let source_tabs = div()
            .flex_shrink_0()
            .px_2()
            .py(px(5.))
            .bg(rgb(ds.panel_dark))
            .border_b_1()
            .border_color(rgb(ds.border))
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .children(PresetSource::ALL.into_iter().map(|src| {
                let selected = active_src == src;
                let count_tag = match src {
                    PresetSource::HayNative => format!("{} ({factory_count})", src.label()),
                    PresetSource::ExternalVital => format!("{} ({vital_count})", src.label()),
                    PresetSource::SerumFxp => format!("{} ({serum_count})", src.label()),
                    PresetSource::User => format!("{} ({user_count})", src.label()),
                    PresetSource::Components => src.label().to_owned(),
                };
                let color = match src {
                    PresetSource::HayNative => ds.accent,
                    PresetSource::ExternalVital | PresetSource::SerumFxp => ds.cyan,
                    PresetSource::User => ds.text,
                    PresetSource::Components => ds.cyan,
                };
                pill_button(ds, &count_tag, selected, color)
                    .id(("synth-menu-src", src as usize))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.synth_ui.preset_source = src;
                            this.synth_ui.preset_category = PresetCategory::All;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
            }));

        let show_categories = active_src != PresetSource::Components;
        let categories_col = div()
            .w(px(124.))
            .flex_shrink_0()
            .p_2()
            .border_r_1()
            .border_color(rgb(ds.border))
            .bg(rgb(ds.panel_dark))
            .flex()
            .flex_col()
            .gap(px(3.))
            .children(PresetCategory::ALL.into_iter().map(|cat| {
                let selected = active_cat == cat;
                div()
                    .id(("synth-menu-cat", cat as usize))
                    .w_full()
                    .px_2()
                    .py(px(4.))
                    .rounded(px(ds.radius_sm))
                    .cursor_pointer()
                    .bg(rgb(if selected { ds.panel } else { ds.panel_dark }))
                    .border_1()
                    .border_color(rgb(if selected { ds.accent } else { ds.panel_dark }))
                    .text_size(px(10.5))
                    .font_weight(if selected {
                        FontWeight::BOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(rgb(if selected { ds.accent } else { ds.text }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.synth_ui.preset_category = cat;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .child(cat.label())
            }));

        let list_body: gpui::AnyElement = match active_src {
            PresetSource::Components => {
                let track_chain = self
                    .synth_track()
                    .and_then(|idx| self.tracks.get(idx))
                    .map(|t| &t.chain);
                let mut components: Vec<(String, String, String)> = self
                    .lua_components
                    .registry
                    .values()
                    .map(|c| (c.id.clone(), c.name.clone(), c.description.clone()))
                    .collect();
                components.push((
                    elevate::musical::fx::cabinet::ID.to_owned(),
                    "Hey Convolver".into(),
                    "Convolución de cabinas e impulsos IR".into(),
                ));
                components.sort_by(|a, b| a.1.cmp(&b.1));
                div()
                    .id("synth-menu-fx-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(rgb(ds.muted))
                            .pb_1()
                            .child("Haz clic en un componente para insertarlo en la cadena FX de esta pista:"),
                    )
                    .children(components.into_iter().enumerate().map(|(i, (id, name, desc))| {
                        let already = track_chain.is_some_and(|chain| {
                            chain.nodes().iter().any(|n| match &n.kind {
                                elevate::musical::fx::FxKind::Effect {
                                    plugin_id:
                                        elevate::musical::track::TrackDevice::Hay { catalog_id },
                                    ..
                                } => catalog_id == &id,
                                _ => false,
                            })
                        });
                        let fx_id = id.clone();
                        div()
                            .id(("synth-menu-fx", i))
                            .w_full()
                            .px_2()
                            .py(px(6.))
                            .rounded(px(ds.radius_sm))
                            .bg(rgb(ds.panel_dark))
                            .border_1()
                            .border_color(rgb(if already { ds.cyan } else { ds.border }))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.add_synth_track_fx(&fx_id);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(rgb(ds.text))
                                            .child(name),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(9.))
                                            .text_color(rgb(ds.muted))
                                            .truncate()
                                            .child(desc),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded(px(3.))
                                    .bg(rgb(ds.bg))
                                    .text_size(px(9.5))
                                    .text_color(rgb(if already { ds.cyan } else { ds.accent }))
                                    .child(if already { "+ Otro" } else { "+ Añadir" }),
                            )
                    }))
                    .into_any_element()
            }
            PresetSource::HayNative | PresetSource::User => {
                let mut rows: Vec<gpui::AnyElement> = Vec::new();
                for (idx, p) in all_native.iter().enumerate() {
                    let is_user = idx >= factory_count;
                    if (active_src == PresetSource::User && !is_user)
                        || (active_src == PresetSource::HayNative && is_user)
                    {
                        continue;
                    }
                    if !active_cat.matches(&p.name, &p.category) {
                        continue;
                    }
                    let is_cur = p.name.eq_ignore_ascii_case(&active_name);
                    let name = p.name.clone();
                    let cat_lbl = p.category.clone();
                    let eng_lbl = p.params.engine.label();
                    rows.push(
                        div()
                            .id(("synth-menu-native", idx))
                            .w_full()
                            .px_2()
                            .py(px(5.))
                            .rounded(px(ds.radius_sm))
                            .bg(rgb(if is_cur { ds.panel } else { ds.panel_dark }))
                            .border_1()
                            .border_color(rgb(if is_cur { ds.accent } else { ds.border }))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.synth_ui.vital_index = None;
                                    this.apply_synth_preset_index(idx);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_color(rgb(if is_cur { ds.accent } else { ds.text }))
                                    .font_weight(if is_cur {
                                        FontWeight::BOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .truncate()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px(9.))
                                    .text_color(rgb(ds.muted))
                                    .child(format!("{cat_lbl} · {eng_lbl}")),
                            )
                            .into_any_element(),
                    );
                }
                if rows.is_empty() {
                    rows.push(
                        div()
                            .p_3()
                            .text_color(rgb(ds.muted))
                            .child(if active_src == PresetSource::User {
                                "Aún no guardaste presets propios · pulsa «Guardar»"
                            } else {
                                "Sin presets en esta categoría"
                            })
                            .into_any_element(),
                    );
                }
                div()
                    .id("synth-menu-preset-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .children(rows)
                    .into_any_element()
            }
            PresetSource::ExternalVital | PresetSource::SerumFxp => {
                let want_ext = if active_src == PresetSource::SerumFxp {
                    "fxp"
                } else {
                    "vital"
                };
                let mut rows: Vec<gpui::AnyElement> = Vec::new();
                let mut shown = 0usize;
                for (vidx, entry) in self.synth_ui.vital_library.iter().enumerate() {
                    if shown >= 200 {
                        break;
                    }
                    let ext_ok = entry
                        .path
                        .extension()
                        .and_then(|x| x.to_str())
                        .is_some_and(|x| x.eq_ignore_ascii_case(want_ext));
                    if !ext_ok || !active_cat.matches(&entry.name, &entry.pack) {
                        continue;
                    }
                    shown += 1;
                    let is_cur = self.synth_ui.vital_index == Some(vidx);
                    let path = entry.path.clone();
                    let name = entry.name.clone();
                    let pack = entry.pack.clone();
                    rows.push(
                        div()
                            .id(("synth-menu-lib", vidx))
                            .w_full()
                            .px_2()
                            .py(px(5.))
                            .rounded(px(ds.radius_sm))
                            .bg(rgb(if is_cur { ds.panel } else { ds.panel_dark }))
                            .border_1()
                            .border_color(rgb(if is_cur { ds.cyan } else { ds.border }))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.synth_ui.vital_index = Some(vidx);
                                    this.import_vital_path(path.clone());
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_color(rgb(if is_cur { ds.cyan } else { ds.text }))
                                    .truncate()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .w(px(140.))
                                    .flex_shrink_0()
                                    .text_right()
                                    .text_size(px(9.))
                                    .text_color(rgb(ds.muted))
                                    .truncate()
                                    .child(pack),
                            )
                            .into_any_element(),
                    );
                }
                if rows.is_empty() {
                    rows.push(
                        div()
                            .p_3()
                            .text_color(rgb(ds.muted))
                            .child("Sin presets en esta categoría · usa «Importar…» para añadir bancos")
                            .into_any_element(),
                    );
                }
                div()
                    .id("synth-menu-preset-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .children(rows)
                    .into_any_element()
            }
        };

        // Overlay posicionado absolutamente: nunca empuja ni altera el layout subyacente.
        div()
            .id("synth-preset-overlay")
            .absolute()
            .top(px(40.))
            .left(px(12.))
            .w(px(620.))
            .h(px(360.))
            .rounded(px(ds.radius_md))
            .bg(rgb(ds.bg))
            .border_1()
            .border_color(rgb(ds.cyan))
            .flex()
            .flex_col()
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .px_3()
                    .py(px(6.))
                    .bg(rgb(ds.panel))
                    .border_b_1()
                    .border_color(rgb(ds.border))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(10.5))
                            .text_color(rgb(ds.cyan))
                            .child(format!("NAVEGADOR · {}", active_src.label())),
                    )
                    .child(
                        pill_button(ds, "Cerrar ✕", false, ds.muted)
                            .id("synth-menu-close")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.synth_ui.preset_menu_open = false;
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    ),
            )
            .child(source_tabs)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .when(show_categories, |row| row.child(categories_col))
                    .child(list_body),
            )
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Pestaña 1: STAGE (Espacio XY + Cuerpo + Intención)
    // ──────────────────────────────────────────────────────────────────────────
    fn render_synth_stage_tab(
        &self,
        active_notes: usize,
        last_vel: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let params = &self.synth.params;

        let stage = self.render_synth_stage(active_notes, last_vel, cx);

        let room_pills = div().flex().flex_wrap().gap_1().children(
            RoomKind::ALL.into_iter().map(|room| {
                let active = params.room == room;
                pill_button(ds, room.label(), active, ds.cyan)
                    .id(("synth-room", room as usize))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.set_synth_room(room);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
            }),
        );

        let mode_pills = div().flex().flex_wrap().gap_1().children(
            EnvelopeMode::ALL.into_iter().map(|mode| {
                let active = params.mode == mode;
                pill_button(ds, mode.label(), active, ds.accent)
                    .id(("synth-mode", mode as usize))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.set_synth_mode(mode);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
            }),
        );

        let space_card = div()
            .flex()
            .flex_col()
            .gap_2()
            .p(px(10.))
            .rounded(px(ds.radius_md))
            .bg(rgb(ds.panel_dark))
            .border_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(ds.cyan))
                            .child("ESPACIO"),
                    )
                    .child(room_pills),
            )
            .child(div().flex().flex_wrap().items_center().gap_3().child(
                self.synth_param_knob(
                    param::DISTANCE,
                    "Distance",
                    "Pecho ↔ Aire",
                    0.0,
                    1.0,
                    0.25,
                    false,
                    ds.cyan,
                    cx,
                ),
            ));

        let body_card = div()
            .flex()
            .flex_col()
            .gap_2()
            .p(px(10.))
            .rounded(px(ds.radius_md))
            .bg(rgb(ds.panel_dark))
            .border_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(ds.accent))
                            .child("CUERPO"),
                    )
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(rgb(ds.muted))
                            .child(format!(
                                "{} · Shape = {}",
                                params.engine.label(),
                                params.engine.shape_hint()
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(self.synth_param_knob(
                        param::SHAPE,
                        "Shape",
                        params.engine.shape_hint(),
                        0.0,
                        1.0,
                        0.45,
                        false,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::TONE,
                        "Tone",
                        "Oscuro ↔ Brillo",
                        0.0,
                        1.0,
                        0.55,
                        false,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::BODY,
                        "Body",
                        "Mono ↔ Denso",
                        0.0,
                        1.0,
                        0.55,
                        false,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::MOTION,
                        "Motion",
                        "Fijo ↔ Vivo",
                        0.0,
                        1.0,
                        0.30,
                        false,
                        ds.cyan,
                        cx,
                    )),
            );

        let intention_card = div()
            .flex()
            .flex_col()
            .gap_2()
            .p(px(10.))
            .rounded(px(ds.radius_md))
            .bg(rgb(ds.panel_dark))
            .border_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(ds.text))
                            .child("INTENCIÓN"),
                    )
                    .child(mode_pills),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(self.synth_param_knob(
                        param::TIME,
                        "Time",
                        "Corto ↔ Largo",
                        0.0,
                        1.0,
                        0.50,
                        false,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::TOUCH,
                        "Touch",
                        "Dinámica",
                        0.0,
                        1.0,
                        0.55,
                        false,
                        ds.cyan,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::GLIDE,
                        "Glide",
                        "Portamento",
                        0.0,
                        1.0,
                        0.0,
                        false,
                        ds.muted,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::GAIN,
                        "Out",
                        "Nivel",
                        0.0,
                        1.0,
                        0.80,
                        false,
                        ds.text,
                        cx,
                    )),
            );

        let controls_col = div()
            .flex_1()
            .min_w(px(340.))
            .flex()
            .flex_col()
            .gap_2()
            .child(body_card)
            .child(intention_card)
            .child(space_card);

        div()
            .flex()
            .flex_wrap()
            .items_stretch()
            .gap_3()
            .p_3()
            .child(stage)
            .child(controls_col)
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Pestaña 2: VOICE LAB (OSC A/B Wavetable + Warp + Unison + Sub/Noise + Filter 24dB + Envs + LFOs + FX)
    // ──────────────────────────────────────────────────────────────────────────
    fn render_synth_voice_tab(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let params = &self.synth.params;
        let vp = &params.voice;
        let is_wt = params.engine == SynthEngineKind::Wavetable;

        let card = |title: &'static str, accent: u32| {
            div()
                .flex_1()
                .min_w(px(260.))
                .flex()
                .flex_col()
                .gap_2()
                .p(px(10.))
                .rounded(px(ds.radius_md))
                .bg(rgb(ds.panel_dark))
                .border_1()
                .border_color(rgb(ds.border))
                .child(
                    div()
                        .text_size(px(10.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(accent))
                        .child(title),
                )
        };

        // ── OSC A ──
        let osc_a_wt_label = params
            .custom_wt_a
            .as_ref()
            .map(|w| format!("★ {}", w.name))
            .unwrap_or_else(|| vp.osc_a_table.label().to_owned());
        let osc_a_card = card("OSC A · WAVETABLE / WARP / UNISON", ds.accent)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .child(div().w(px(148.)).flex_shrink_0().child(self.step_selector(
                        "osc-a-table",
                        &osc_a_wt_label,
                        ds.cyan,
                        cx,
                        |this, delta| {
                            let cur = this.synth.params.voice.osc_a_table as u8 as isize;
                            let next = (cur + delta).rem_euclid(WavetableKind::ALL.len() as isize);
                            this.set_synth_voice_param(param::OSC_A_TABLE, next as f32);
                            this.status_message = format!(
                                "OSC A · Tabla: {}",
                                this.synth.params.voice.osc_a_table.label()
                            );
                        },
                    )))
                    .child(div().w(px(110.)).flex_shrink_0().child(self.step_selector(
                        "osc-a-warp",
                        vp.osc_a_warp_mode.label(),
                        ds.accent,
                        cx,
                        |this, delta| {
                            let cur = this.synth.params.voice.osc_a_warp_mode as u8 as isize;
                            let next = (cur + delta).rem_euclid(WarpMode::ALL.len() as isize);
                            this.set_synth_voice_param(param::OSC_A_WARP_MODE, next as f32);
                            this.status_message = format!(
                                "OSC A · Warp: {}",
                                this.synth.params.voice.osc_a_warp_mode.label()
                            );
                        },
                    )))
                    .child(
                        pill_button(ds, "Wavetable", is_wt, ds.cyan)
                            .id("osc-a-enable-wt")
                            .w(px(76.))
                            .justify_center()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.set_synth_engine(SynthEngineKind::Wavetable);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(self.synth_param_knob(
                        param::SHAPE,
                        "Frame",
                        "WT Pos",
                        0.0,
                        1.0,
                        0.45,
                        false,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::OSC_A_WARP,
                        "Warp",
                        vp.osc_a_warp_mode.label(),
                        0.0,
                        1.0,
                        0.0,
                        false,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::OSC_A_UNISON,
                        "Unison",
                        "1–7 voces",
                        1.0,
                        7.0,
                        1.0,
                        false,
                        ds.cyan,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::OSC_A_DETUNE,
                        "Detune",
                        "Cents",
                        0.0,
                        1.0,
                        0.22,
                        false,
                        ds.cyan,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::OSC_A_SPREAD,
                        "Spread",
                        "Estéreo",
                        0.0,
                        1.0,
                        0.70,
                        false,
                        ds.cyan,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::OSC_A_SEMI,
                        "Semi",
                        "±24 st",
                        -24.0,
                        24.0,
                        0.0,
                        true,
                        ds.muted,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::OSC_A_LEVEL,
                        "Level",
                        "Nivel A",
                        0.0,
                        1.0,
                        1.0,
                        false,
                        ds.text,
                        cx,
                    )),
            );

        // ── OSC B + SUB + NOISE ──
        let osc_b_wt_label = params
            .custom_wt_b
            .as_ref()
            .map(|w| format!("★ {}", w.name))
            .unwrap_or_else(|| vp.osc_b_table.label().to_owned());
        let osc_b_card = card("OSC B · WAVETABLE / FM SOURCE + SUB / NOISE", ds.cyan)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .child(div().w(px(148.)).flex_shrink_0().child(self.step_selector(
                        "osc-b-table",
                        &osc_b_wt_label,
                        ds.cyan,
                        cx,
                        |this, delta| {
                            let cur = this.synth.params.voice.osc_b_table as u8 as isize;
                            let next = (cur + delta).rem_euclid(WavetableKind::ALL.len() as isize);
                            this.set_synth_voice_param(param::OSC_B_TABLE, next as f32);
                            this.status_message = format!(
                                "OSC B · Tabla: {}",
                                this.synth.params.voice.osc_b_table.label()
                            );
                        },
                    )))
                    .child(
                        pill_button(
                            ds,
                            if vp.sub_octave == 0 {
                                "Sub -1 Oct"
                            } else {
                                "Sub -2 Oct"
                            },
                            vp.sub_level > 0.01,
                            ds.accent,
                        )
                        .id("synth-sub-oct")
                        .w(px(80.))
                        .justify_center()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                let next = if this.synth.params.voice.sub_octave == 0 {
                                    1.0
                                } else {
                                    0.0
                                };
                                this.set_synth_voice_param(param::SUB_OCTAVE, next);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(self.synth_param_knob(
                        param::OSC_B_LEVEL,
                        "Level B",
                        "0 = solo FM",
                        0.0,
                        1.0,
                        0.0,
                        false,
                        ds.cyan,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::OSC_B_POS,
                        "Frame B",
                        "WT Pos",
                        0.0,
                        1.0,
                        0.35,
                        false,
                        ds.cyan,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::OSC_B_SEMI,
                        "Semi B",
                        "±24 st",
                        -24.0,
                        24.0,
                        0.0,
                        true,
                        ds.muted,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::OSC_B_FINE,
                        "Fine B",
                        "±50 ct",
                        -1.0,
                        1.0,
                        0.0,
                        true,
                        ds.muted,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::OSC_B_UNISON,
                        "Uni B",
                        "1–7",
                        1.0,
                        7.0,
                        1.0,
                        false,
                        ds.cyan,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::SUB_LEVEL,
                        "Sub",
                        "Seno grave",
                        0.0,
                        1.0,
                        0.0,
                        false,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::NOISE_LEVEL,
                        "Noise",
                        "Aire / Grano",
                        0.0,
                        1.0,
                        0.0,
                        false,
                        ds.text,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::NOISE_COLOR,
                        "Color",
                        "Osc ↔ Aire",
                        0.0,
                        1.0,
                        0.55,
                        false,
                        ds.muted,
                        cx,
                    )),
            );

        // ── FILTER (TPT SVF) ──
        let filter_card = card("FILTER · TPT SVF 12 / 24 dB", ds.accent)
            .child(div().flex().flex_wrap().gap_1().children(
                FilterKind::ALL.into_iter().map(|kind| {
                    pill_button(ds, kind.label(), vp.filter_kind == kind, ds.accent)
                        .id(("synth-filter-kind", kind as usize))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.set_synth_voice_param(param::FILTER_KIND, kind as u8 as f32);
                                this.status_message =
                                    format!("Hay Synth · Filtro: {}", kind.label());
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                }),
            ))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(self.synth_param_knob(
                        param::TONE,
                        "Cutoff",
                        "70–19k Hz",
                        0.0,
                        1.0,
                        0.55,
                        false,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::FILTER_RES,
                        "Res",
                        "Q / Pico",
                        0.0,
                        1.0,
                        0.12,
                        false,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::FILTER_DRIVE,
                        "Drive",
                        "Pre-sat",
                        0.0,
                        1.0,
                        0.0,
                        false,
                        ds.cyan,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::FILTER_KEYTRACK,
                        "Keytrack",
                        "Seguimiento",
                        0.0,
                        1.0,
                        0.25,
                        false,
                        ds.muted,
                        cx,
                    )),
            );

        // ── FX BUS (Chorus + Distortion + Space) ──
        let fx_card = card("FX BUS · DISTORTION / CHORUS / SPACE", ds.cyan)
            .child(div().flex().flex_wrap().gap_1().children(
                DistMode::ALL.into_iter().map(|mode| {
                    pill_button(
                        ds,
                        mode.label(),
                        vp.dist_mode == mode && vp.dist_mix > 0.01,
                        ds.accent,
                    )
                    .id(("synth-dist-mode", mode as usize))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.set_synth_voice_param(param::DIST_MODE, mode as u8 as f32);
                            if this.synth.params.voice.dist_mix < 0.05 {
                                this.set_synth_voice_param(param::DIST_MIX, 0.40);
                                this.set_synth_voice_param(param::DIST_DRIVE, 0.35);
                            }
                            this.status_message = format!("Distortion · {}", mode.label());
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                }),
            ))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(self.synth_param_knob(
                        param::DIST_DRIVE,
                        "Dist Drv",
                        vp.dist_mode.label(),
                        0.0,
                        1.0,
                        0.0,
                        false,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::DIST_MIX,
                        "Dist Mix",
                        "Dry/Wet",
                        0.0,
                        1.0,
                        0.0,
                        false,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::CHORUS_MIX,
                        "Chorus",
                        "Dimensión",
                        0.0,
                        1.0,
                        0.0,
                        false,
                        ds.cyan,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::CHORUS_RATE,
                        "Ch Rate",
                        "Velocidad",
                        0.0,
                        1.0,
                        0.32,
                        false,
                        ds.cyan,
                        cx,
                    )),
            );

        // ── ENVELOPES (ENV 1 + ENV 2 + ENV 3) ──
        let env_card = card("ENVELOPES · ENV 1 (AMP) / ENV 2 (MOD) / ENV 3 (PUNCH)", ds.text)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        pill_button(ds, "ADSR Manual", vp.env1_override, ds.accent)
                            .id("synth-env1-override")
                            .w(px(96.))
                            .justify_center()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                let next = if this.synth.params.voice.env1_override {
                                    0.0
                                } else {
                                    1.0
                                };
                                this.set_synth_voice_param(param::ENV1_OVERRIDE, next);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(rgb(ds.muted))
                            .child("ENV 2 y ENV 3 siempre disponibles en Matrix"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(self.synth_param_knob(
                        param::ENV1_ATTACK,
                        "E1 Atk",
                        "Amp",
                        0.0,
                        1.0,
                        0.02,
                        false,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::ENV1_DECAY,
                        "E1 Dec",
                        "Amp",
                        0.0,
                        1.0,
                        0.35,
                        false,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::ENV1_SUSTAIN,
                        "E1 Sus",
                        "Amp",
                        0.0,
                        1.0,
                        0.75,
                        false,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::ENV1_RELEASE,
                        "E1 Rel",
                        "Amp",
                        0.0,
                        1.0,
                        0.30,
                        false,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::ENV2_ATTACK,
                        "E2 Atk",
                        "Mod",
                        0.0,
                        1.0,
                        0.01,
                        false,
                        ds.cyan,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::ENV2_DECAY,
                        "E2 Dec",
                        "Mod",
                        0.0,
                        1.0,
                        0.32,
                        false,
                        ds.cyan,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::ENV2_SUSTAIN,
                        "E2 Sus",
                        "Mod",
                        0.0,
                        1.0,
                        0.0,
                        false,
                        ds.cyan,
                        cx,
                    ))
                    .child(self.synth_param_knob(
                        param::ENV3_DECAY,
                        "E3 Punch",
                        "Pitch",
                        0.0,
                        1.0,
                        0.12,
                        false,
                        ds.text,
                        cx,
                    )),
            );

        // ── LFO 1 & LFO 2 ──
        let lfo_card = card("LFO 1 & LFO 2 · MODULADORES", ds.cyan).child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
                .child(self.render_lfo_block(
                    1,
                    vp.lfo1_shape,
                    vp.lfo1_sync,
                    vp.lfo1_retrigger,
                    param::LFO1_SHAPE,
                    param::LFO1_RATE,
                    param::LFO1_SYNC,
                    param::LFO1_RETRIGGER,
                    cx,
                ))
                .child(self.render_lfo_block(
                    2,
                    vp.lfo2_shape,
                    vp.lfo2_sync,
                    vp.lfo2_retrigger,
                    param::LFO2_SHAPE,
                    param::LFO2_RATE,
                    param::LFO2_SYNC,
                    param::LFO2_RETRIGGER,
                    cx,
                )),
        );

        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .child(div().flex().flex_wrap().gap_2().child(osc_a_card).child(osc_b_card))
            .child(div().flex().flex_wrap().gap_2().child(filter_card).child(fx_card))
            .child(div().flex().flex_wrap().gap_2().child(env_card).child(lfo_card))
    }

    #[allow(clippy::too_many_arguments)]
    fn render_lfo_block(
        &self,
        num: usize,
        shape: LfoShape,
        sync: bool,
        retrig: bool,
        shape_pid: u16,
        rate_pid: u16,
        sync_pid: u16,
        retrig_pid: u16,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ds = &self.ds;
        div()
            .flex()
            .items_center()
            .gap_2()
            .p_2()
            .rounded(px(ds.radius_sm))
            .bg(rgb(ds.bg))
            .border_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(9.5))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(ds.cyan))
                            .child(format!("LFO {num}")),
                    )
                    .child(self.step_selector(
                        if num == 1 { "lfo1-shape" } else { "lfo2-shape" },
                        shape.label(),
                        ds.accent,
                        cx,
                        move |this, delta| {
                            let cur = this.synth.params.get_param(shape_pid).round() as isize;
                            let next = (cur + delta).rem_euclid(LfoShape::ALL.len() as isize);
                            this.set_synth_voice_param(shape_pid, next as f32);
                        },
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(
                                pill_button(ds, "BPM", sync, ds.cyan)
                                    .id(("lfo-sync", num))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            let cur = this.synth.params.get_param(sync_pid) >= 0.5;
                                            this.set_synth_voice_param(
                                                sync_pid,
                                                if cur { 0.0 } else { 1.0 },
                                            );
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                pill_button(ds, "Retrig", retrig, ds.accent)
                                    .id(("lfo-retrig", num))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            let cur =
                                                this.synth.params.get_param(retrig_pid) >= 0.5;
                                            this.set_synth_voice_param(
                                                retrig_pid,
                                                if cur { 0.0 } else { 1.0 },
                                            );
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    ),
            )
            .child(self.synth_param_knob(
                rate_pid,
                "Rate",
                if sync { "Sync" } else { "Hz" },
                0.0,
                1.0,
                0.35,
                false,
                ds.cyan,
                cx,
            ))
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Pestaña 3: MOD MATRIX (12 ranuras de acoplamiento bipolar + plantillas)
    // ──────────────────────────────────────────────────────────────────────────
    fn render_synth_matrix_tab(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let matrix = &self.synth.params.voice.matrix;

        let toolbar = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .p(px(8.))
            .rounded(px(ds.radius_md))
            .bg(rgb(ds.panel_dark))
            .border_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(ds.accent))
                            .child("MOD MATRIX · 12 RANURAS DE ACOPLAMIENTO"),
                    )
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(rgb(ds.muted))
                            .child("Conecta ENV/LFO/Velocity o tus propios macros (Distance, Tone, Shape, Body, Motion) hacia cualquier parámetro"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        pill_button(ds, "Vital Wobble", false, ds.cyan)
                            .id("matrix-tpl-0")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.apply_matrix_template(0);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        pill_button(ds, "XY Space Morph", false, ds.accent)
                            .id("matrix-tpl-1")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.apply_matrix_template(1);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        pill_button(ds, "Expressive Touch", false, ds.cyan)
                            .id("matrix-tpl-2")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.apply_matrix_template(2);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    ),
            );

        let render_row = |slot: usize, cx: &mut Context<Self>| {
            let entry = matrix[slot];
            let active = entry.source != ModSource::None
                && entry.target != ModTarget::None
                && entry.amount.abs() > 0.005;
            let amt_pid = param::matrix_amount(slot);
            let amt_frac = (entry.amount + 1.0) * 0.5;
            div()
                .id(("matrix-row", slot))
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px_2()
                .py(px(5.))
                .rounded(px(ds.radius_sm))
                .bg(rgb(ds.panel_dark))
                .border_1()
                .border_color(rgb(if active { ds.cyan } else { ds.border }))
                .child(
                    div()
                        .w(px(20.))
                        .text_size(px(9.5))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(if active { ds.cyan } else { ds.muted }))
                        .child(format!("#{}", slot + 1)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(120.))
                        .child(self.step_selector(
                            match slot {
                                0 => "mx-s0",
                                1 => "mx-s1",
                                2 => "mx-s2",
                                3 => "mx-s3",
                                4 => "mx-s4",
                                5 => "mx-s5",
                                6 => "mx-s6",
                                7 => "mx-s7",
                                8 => "mx-s8",
                                9 => "mx-s9",
                                10 => "mx-s10",
                                _ => "mx-s11",
                            },
                            entry.source.label(),
                            if entry.source != ModSource::None {
                                ds.accent
                            } else {
                                ds.muted
                            },
                            cx,
                            move |this, delta| this.step_matrix_source(slot, delta),
                        )),
                )
                .child(
                    div()
                        .id(("matrix-amt", slot))
                        .w(px(84.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap_1()
                        .cursor(gpui::CursorStyle::ResizeUpDown)
                        .child(rotary_knob(
                            ds,
                            KnobSpec {
                                fraction: amt_frac,
                                bipolar: true,
                                color: if active { ds.cyan } else { ds.muted },
                                diameter: 22.,
                            },
                        ))
                        .child(
                            div()
                                .w(px(44.))
                                .flex_shrink_0()
                                .font_family("monospace")
                                .text_right()
                                .text_size(px(10.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(if active { ds.cyan } else { ds.muted }))
                                .child(format!("{:+.0}%", entry.amount * 100.0)),
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                                if event.click_count == 2 {
                                    this.set_synth_voice_param(amt_pid, 0.0);
                                } else {
                                    this.synth_ui.knob_drag = Some(SynthKnobDrag {
                                        param_id: amt_pid,
                                        origin_y: event.position.y,
                                        start_val: this.synth.params.get_param(amt_pid),
                                        min: -1.0,
                                        max: 1.0,
                                    });
                                }
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                        .on_scroll(cx.listener(move |this, event: &gpui::ScrollEvent, _, cx| {
                            let delta = f32::from(event.delta.pixel_delta(px(20.)).y);
                            if delta != 0.0 {
                                let step = if event.modifiers.shift { 0.02 } else { 0.06 };
                                let cur = this.synth.params.get_param(amt_pid);
                                this.set_synth_voice_param(amt_pid, cur + delta.signum() * step);
                                cx.stop_propagation();
                                cx.notify();
                            }
                        })),
                )
                .child(div().text_color(rgb(ds.muted)).child("→"))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(130.))
                        .child(self.step_selector(
                            match slot {
                                0 => "mx-t0",
                                1 => "mx-t1",
                                2 => "mx-t2",
                                3 => "mx-t3",
                                4 => "mx-t4",
                                5 => "mx-t5",
                                6 => "mx-t6",
                                7 => "mx-t7",
                                8 => "mx-t8",
                                9 => "mx-t9",
                                10 => "mx-t10",
                                _ => "mx-t11",
                            },
                            entry.target.label(),
                            if entry.target != ModTarget::None {
                                ds.cyan
                            } else {
                                ds.muted
                            },
                            cx,
                            move |this, delta| this.step_matrix_target(slot, delta),
                        )),
                )
                .child(
                    pill_button(ds, "×", false, ds.muted)
                        .id(("matrix-clear", slot))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.clear_matrix_slot(slot);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ),
                )
        };

        let left_col = div()
            .flex_1()
            .min_w(px(410.))
            .flex()
            .flex_col()
            .gap_1()
            .children((0..MATRIX_SLOTS / 2).map(|i| render_row(i, cx)));
        let right_col = div()
            .flex_1()
            .min_w(px(410.))
            .flex()
            .flex_col()
            .gap_1()
            .children((MATRIX_SLOTS / 2..MATRIX_SLOTS).map(|i| render_row(i, cx)));

        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .child(toolbar)
            .child(div().flex().flex_wrap().gap_2().child(left_col).child(right_col))
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Mapa XY interactivo (Stage)
    // ──────────────────────────────────────────────────────────────────────────
    fn render_synth_stage(
        &self,
        active_notes: usize,
        last_vel: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let params = self.synth.params.clone();
        let bounds_cell = self.synth_ui.stage_bounds.clone();
        let bg_col = ds.arrangement_surface();
        let grid_col = ds.border;
        let accent = ds.accent;
        let cyan = ds.cyan;
        let pulse = if active_notes > 0 {
            0.35 + last_vel * 0.65
        } else {
            0.0
        };

        let xy_canvas = div()
            .id("synth-xy-stage")
            .w_full()
            .h(px(230.))
            .rounded(px(ds.radius_md))
            .bg(rgb(bg_col))
            .border_1()
            .border_color(rgb(ds.border))
            .overflow_hidden()
            .cursor(gpui::CursorStyle::Crosshair)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    if event.click_count == 2 {
                        this.set_synth_param(cc::TONE, 0.55);
                        this.set_synth_param(cc::DISTANCE, 0.25);
                        this.synth_ui.stage_drag = false;
                    } else {
                        this.synth_ui.stage_drag = true;
                        this.update_synth_stage_drag(event.position);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(
                canvas(
                    move |bounds, _, _| {
                        bounds_cell.set(Some(bounds));
                    },
                    move |bounds, _, window, _| {
                        let w = f32::from(bounds.size.width).max(1.0);
                        let h = f32::from(bounds.size.height).max(1.0);

                        for step in 1..4 {
                            let f = step as f32 / 4.0;
                            let x = bounds.left() + px(w * f);
                            let y = bounds.top() + px(h * f);
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(x, bounds.top()),
                                    size(px(1.0), bounds.size.height),
                                ),
                                rgba((grid_col << 8) | 0x38),
                            ));
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(bounds.left(), y),
                                    size(bounds.size.width, px(1.0)),
                                ),
                                rgba((grid_col << 8) | 0x38),
                            ));
                        }

                        let cx_px =
                            (params.tone.clamp(0.0, 1.0) * w).clamp(12.0, (w - 12.0).max(12.0));
                        let cy_px = ((1.0 - params.distance.clamp(0.0, 1.0)) * h)
                            .clamp(12.0, (h - 12.0).max(12.0));

                        window.paint_quad(fill(
                            Bounds::new(
                                point(bounds.left() + px(cx_px), bounds.top()),
                                size(px(1.0), bounds.size.height),
                            ),
                            rgba((accent << 8) | 0x32),
                        ));
                        window.paint_quad(fill(
                            Bounds::new(
                                point(bounds.left(), bounds.top() + px(cy_px)),
                                size(bounds.size.width, px(1.0)),
                            ),
                            rgba((cyan << 8) | 0x32),
                        ));

                        let room_scale = match params.room {
                            RoomKind::Close => 0.55,
                            RoomKind::Wood => 0.85,
                            RoomKind::Hall => 1.20,
                            RoomKind::Open => 1.45,
                        };
                        let halo_r = (20.0
                            + params.distance * 36.0 * room_scale
                            + params.body * 14.0
                            + pulse * 14.0)
                            .min(w.min(h) * 0.45);
                        let halo_alpha =
                            ((0x16 as f32) + params.distance * 24.0 + pulse * 36.0)
                                .clamp(16.0, 110.0) as u32;
                        window.paint_quad(quad(
                            Bounds::new(
                                point(
                                    bounds.left() + px(cx_px - halo_r),
                                    bounds.top() + px(cy_px - halo_r),
                                ),
                                size(px(halo_r * 2.0), px(halo_r * 2.0)),
                            ),
                            Corners::all(px(halo_r)),
                            rgba((cyan << 8) | halo_alpha),
                            Edges::all(px(1.0)),
                            rgba((cyan << 8) | (halo_alpha + 0x24).min(0xaa)),
                            Default::default(),
                        ));

                        let body_r = 10.0 + params.body * 14.0 + pulse * 6.0;
                        let body_alpha =
                            ((0x2c as f32) + pulse * 52.0).clamp(32.0, 150.0) as u32;
                        window.paint_quad(quad(
                            Bounds::new(
                                point(
                                    bounds.left() + px(cx_px - body_r),
                                    bounds.top() + px(cy_px - body_r),
                                ),
                                size(px(body_r * 2.0), px(body_r * 2.0)),
                            ),
                            Corners::all(px(body_r)),
                            rgba((accent << 8) | body_alpha),
                            Edges::all(px(1.0)),
                            rgba((accent << 8) | 0x99),
                            Default::default(),
                        ));

                        let core_r = 5.5 + pulse * 2.5;
                        window.paint_quad(quad(
                            Bounds::new(
                                point(
                                    bounds.left() + px(cx_px - core_r),
                                    bounds.top() + px(cy_px - core_r),
                                ),
                                size(px(core_r * 2.0), px(core_r * 2.0)),
                            ),
                            Corners::all(px(core_r)),
                            rgb(if active_notes > 0 { 0xffffff } else { accent }),
                            Edges::all(px(1.5)),
                            rgb(0x111318),
                            Default::default(),
                        ));
                    },
                )
                .size_full(),
            );

        div()
            .w(px(270.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .p(px(10.))
            .rounded(px(ds.radius_md))
            .bg(rgb(ds.panel_dark))
            .border_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(ds.cyan))
                            .child("STAGE XY · ESPACIO × TONO"),
                    )
                    .child(
                        div()
                            .w(px(58.))
                            .flex_shrink_0()
                            .text_right()
                            .font_family("monospace")
                            .text_size(px(9.5))
                            .text_color(rgb(if active_notes > 0 { ds.accent } else { ds.muted }))
                            .child(if active_notes > 0 {
                                format!("{active_notes} voz")
                            } else {
                                params.room.label().to_owned()
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_size(px(9.))
                    .text_color(rgb(ds.muted))
                    .child("FAR · Aire")
                    .child(
                        div()
                            .w(px(64.))
                            .text_right()
                            .font_family("monospace")
                            .child(format!("Dist {:3.0}%", params.distance * 100.0)),
                    ),
            )
            .child(xy_canvas)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_size(px(9.))
                    .text_color(rgb(ds.muted))
                    .child("DARK")
                    .child("CLOSE · Pecho")
                    .child("BRIGHT"),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .font_family("monospace")
                    .text_size(px(9.5))
                    .text_color(rgb(ds.cyan))
                    .child(div().w(px(76.)).child(format!("Tone {:3.0}%", params.tone * 100.0)))
                    .child(div().w(px(76.)).text_center().child(format!("Body {:3.0}%", params.body * 100.0)))
                    .child(div().w(px(84.)).text_right().child(format!("Motion {:3.0}%", params.motion * 100.0))),
            )
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Controles reutilizables (perilla paramérica + selector ◀ Valor ▶)
    // ──────────────────────────────────────────────────────────────────────────
    fn step_selector<F: Fn(&mut DawApp, isize) + Copy + 'static>(
        &self,
        id: &'static str,
        label: &str,
        color: u32,
        cx: &mut Context<Self>,
        step: F,
    ) -> impl IntoElement + use<F> {
        let ds = &self.ds;
        div()
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .flex()
            .items_center()
            .gap(px(2.))
            .rounded(px(ds.radius_sm))
            .bg(rgb(ds.bg))
            .border_1()
            .border_color(rgb(ds.border))
            .px(px(3.))
            .py(px(2.))
            .child(
                div()
                    .id((id, 0usize))
                    .flex_shrink_0()
                    .px(px(4.))
                    .cursor_pointer()
                    .text_size(px(10.))
                    .text_color(rgb(ds.muted))
                    .child("◀")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            step(this, -1);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .id((id, 1usize))
                    .flex_1()
                    .min_w_0()
                    .cursor_pointer()
                    .text_center()
                    .text_size(px(10.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(color))
                    .truncate()
                    .child(label.to_owned())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            step(this, 1);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .id((id, 2usize))
                    .flex_shrink_0()
                    .px(px(4.))
                    .cursor_pointer()
                    .text_size(px(10.))
                    .text_color(rgb(ds.muted))
                    .child("▶")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            step(this, 1);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
    }

    #[allow(clippy::too_many_arguments)]
    fn synth_param_knob(
        &self,
        param_id: u16,
        label: &'static str,
        hint: &'static str,
        min: f32,
        max: f32,
        default_val: f32,
        bipolar: bool,
        color: u32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let value = self.synth.params.get_param(param_id);
        let span = (max - min).max(1e-4);
        let fraction = ((value - min) / span).clamp(0.0, 1.0);
        let readout = format_synth_param_value(param_id, value);
        div()
            .id(("synth-vparam", param_id as usize))
            .w(px(76.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .items_center()
            .cursor(gpui::CursorStyle::ResizeUpDown)
            .child(
                div()
                    .w_full()
                    .text_center()
                    .truncate()
                    .text_size(px(10.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(ds.text))
                    .child(label),
            )
            .child(rotary_knob(
                ds,
                KnobSpec {
                    fraction,
                    bipolar,
                    color,
                    diameter: 26.,
                },
            ))
            .child(
                div()
                    .w_full()
                    .text_center()
                    .font_family("monospace")
                    .text_size(px(9.5))
                    .text_color(rgb(color))
                    .child(readout),
            )
            .child(
                div()
                    .w_full()
                    .text_center()
                    .text_size(px(8.5))
                    .text_color(rgb(ds.muted))
                    .truncate()
                    .child(hint),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    if event.click_count == 2 {
                        this.set_synth_voice_param(param_id, default_val);
                    } else {
                        let cur = this.synth.params.get_param(param_id);
                        this.synth_ui.knob_drag = Some(SynthKnobDrag {
                            param_id,
                            origin_y: event.position.y,
                            start_val: cur,
                            min,
                            max,
                        });
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_scroll(cx.listener(move |this, event: &gpui::ScrollEvent, _, cx| {
                let delta = f32::from(event.delta.pixel_delta(px(20.)).y);
                if delta != 0.0 {
                    let step = if matches!(
                        param_id,
                        param::OSC_A_UNISON
                            | param::OSC_B_UNISON
                            | param::OSC_A_SEMI
                            | param::OSC_B_SEMI
                    ) {
                        1.0
                    } else if event.modifiers.shift {
                        span * 0.01
                    } else {
                        span * 0.04
                    };
                    let cur = this.synth.params.get_param(param_id);
                    let next = (cur + delta.signum() * step).clamp(min, max);
                    this.set_synth_voice_param(param_id, next);
                    this.status_message = format!(
                        "Hay Synth · {label}: {}",
                        format_synth_param_value(param_id, next)
                    );
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
    }
}
