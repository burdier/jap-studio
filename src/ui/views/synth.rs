//! Vista de Hay Synth: 3 pestañas (`Stage` XY humano · `Voice` laboratorio estilo Vital · `Matrix` 12 ranuras).
use crate::ui::{
    DawApp,
    components::{
        button::pill_button,
        keyboard::{NotePress, midi_keyboard},
        knob::{KnobSpec, rotary_knob},
    },
    synth::{SynthKnobDrag, SynthTab, format_synth_param_value},
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

        // ── Barra superior: identidad, presets, pestañas Stage | Voice | Matrix y guardado ──
        let matrix_tab_label = if active_routes > 0 {
            format!("Matrix ({active_routes})")
        } else {
            "Matrix".into()
        };
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
                            .px(px(6.))
                            .py(px(1.))
                            .rounded(px(3.))
                            .bg(rgb(ds.panel_dark))
                            .text_size(px(10.))
                            .text_color(rgb(ds.cyan))
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
                                    .min_w(px(165.))
                                    .px_2()
                                    .py(px(3.))
                                    .rounded(px(ds.radius_sm))
                                    .bg(rgb(ds.panel_dark))
                                    .border_1()
                                    .border_color(rgb(ds.border))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(rgb(ds.text))
                                            .truncate()
                                            .child(preset_label),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(9.5))
                                            .text_color(rgb(ds.muted))
                                            .child(preset_category),
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
                    .gap_2()
                    .child(
                        pill_button(ds, "Importar .vital…", false, ds.accent)
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
                        pill_button(ds, "Guardar preset", false, ds.cyan)
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

        // ── Selector de los 8 motores de síntesis + navegador de librería .vital ──
        let vital_count = self.synth_ui.vital_library.len();
        let vital_label = if let Some(idx) = self.synth_ui.vital_index
            && let Some(entry) = self.synth_ui.vital_library.get(idx)
        {
            format!("Vital ({}/{vital_count}): {}", idx + 1, entry.name)
        } else if vital_count > 0 {
            format!("Vital Bank ({vital_count} presets)")
        } else {
            String::new()
        };
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
                            .child("MOTOR"),
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
            .when(vital_count > 0, |bar| {
                bar.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(self.step_selector(
                            "vital-lib",
                            &vital_label,
                            ds.cyan,
                            cx,
                            |this, delta| this.step_vital_preset(delta),
                        )),
                )
            });

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

        div()
            .id("hay-synth")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .text_size(px(11.))
            .bg(rgb(ds.bg))
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
                    .child(self.status_message.clone()),
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
                    .child(self.step_selector(
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
                    ))
                    .child(self.step_selector(
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
                    ))
                    .when(!is_wt, |el| {
                        el.child(
                            pill_button(ds, "Usar Wavetable", false, ds.cyan)
                                .id("osc-a-enable-wt")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.set_synth_engine(SynthEngineKind::Wavetable);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                        )
                    }),
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
                    .child(self.step_selector(
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
                    ))
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
                        pill_button(
                            ds,
                            if vp.env1_override {
                                "ENV 1: ADSR Manual"
                            } else {
                                "ENV 1: Usar ADSR Manual"
                            },
                            vp.env1_override,
                            ds.accent,
                        )
                        .id("synth-env1-override")
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
                    .child(format!("Dist {:.0}%", params.distance * 100.0)),
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
                    .text_size(px(9.5))
                    .text_color(rgb(ds.cyan))
                    .child(format!("Tone {:.0}%", params.tone * 100.0))
                    .child(format!("Body {:.0}%", params.body * 100.0))
                    .child(format!("Motion {:.0}%", params.motion * 100.0)),
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
                    .min_w(px(64.))
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
            .w(px(72.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .items_center()
            .cursor(gpui::CursorStyle::ResizeUpDown)
            .child(
                div()
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
                    .text_size(px(9.5))
                    .text_color(rgb(color))
                    .child(readout),
            )
            .child(
                div()
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
