//! Vista de Hay Synth: mapa espacial XY (Espacio × Tono) + 3 bloques sobrios (Espacio, Cuerpo, Intención).
use crate::ui::{
    DawApp,
    components::{
        button::pill_button,
        keyboard::{NotePress, midi_keyboard},
        knob::{KnobSpec, rotary_knob},
    },
};
use elevate::devices::synth::{
    EnvelopeMode, FACTORY_PRESETS, RoomKind, SynthEngineKind, cc,
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

        let preset_category = FACTORY_PRESETS
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(&params.preset_name))
            .map(|p| p.category)
            .unwrap_or("User");
        let preset_label = if params.preset_name.is_empty() {
            "Init".to_owned()
        } else {
            params.preset_name.clone()
        };

        // ── Barra superior: identidad, presets y guardado ──
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
                            .ml_2()
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
                                    .min_w(px(170.))
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
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
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

        // ── Selector de los 7 motores de síntesis ──
        let engine_bar = div()
            .flex_shrink_0()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .px_3()
            .py(px(6.))
            .bg(rgb(ds.panel_dark))
            .border_b_1()
            .border_color(rgb(ds.border))
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
                pill_button(ds, engine.label(), active, ds.accent)
                    .id(("synth-engine", engine as usize))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.set_synth_engine(engine);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
            }));

        // ── Columna izquierda: Mapa XY de Espacio (Proximidad × Tono) ──
        let stage = self.render_synth_stage(active_notes, last_vel, cx);

        // ── Columna derecha: 3 bloques sobrios (Espacio · Cuerpo · Intención) ──
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
            .child(
                div().flex().flex_wrap().items_center().gap_3().child(
                    self.synth_knob(
                        cc::DISTANCE,
                        "Distance",
                        "Pecho ↔ Aire",
                        0.25,
                        ds.cyan,
                        cx,
                    ),
                ),
            );

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
                    .child(self.synth_knob(
                        cc::SHAPE,
                        "Shape",
                        params.engine.shape_hint(),
                        0.45,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_knob(
                        cc::TONE,
                        "Tone",
                        "Oscuro ↔ Brillo",
                        0.55,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_knob(
                        cc::BODY,
                        "Body",
                        "Mono ↔ Denso",
                        0.55,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_knob(
                        cc::MOTION,
                        "Motion",
                        "Fijo ↔ Vivo",
                        0.30,
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
                    .child(self.synth_knob(
                        cc::TIME,
                        "Time",
                        "Corto ↔ Largo",
                        0.50,
                        ds.accent,
                        cx,
                    ))
                    .child(self.synth_knob(
                        cc::TOUCH,
                        "Touch",
                        "Dinámica",
                        0.55,
                        ds.cyan,
                        cx,
                    ))
                    .child(self.synth_knob(
                        cc::GLIDE,
                        "Glide",
                        "Portamento",
                        0.0,
                        ds.muted,
                        cx,
                    ))
                    .child(self.synth_knob(
                        cc::GAIN,
                        "Out",
                        "Nivel",
                        0.80,
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

        let main_row = div()
            .flex()
            .flex_wrap()
            .items_stretch()
            .gap_3()
            .p_3()
            .child(stage)
            .child(controls_col);

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
                    .child(main_row)
                    .child(
                        div()
                            .flex_shrink_0()
                            .px_3()
                            .pb_1()
                            .text_size(px(10.))
                            .text_color(rgb(ds.muted))
                            .child("Arrastra el punto en Espacio XY o las perillas · Shift: precisión · doble clic: restablecer"),
                    )
                    .child(
                        div()
                            .h(px(120.))
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

                        // Cuadrícula sutil 4×4
                        for step in 1..4 {
                            let f = step as f32 / 4.0;
                            let x = bounds.left() + px(w * f);
                            let y = bounds.top() + px(h * f);
                            window.paint_quad(fill(
                                Bounds::new(point(x, bounds.top()), size(px(1.0), bounds.size.height)),
                                rgba((grid_col << 8) | 0x38),
                            ));
                            window.paint_quad(fill(
                                Bounds::new(point(bounds.left(), y), size(bounds.size.width, px(1.0))),
                                rgba((grid_col << 8) | 0x38),
                            ));
                        }

                        // Posición de la entidad sonora: X = Tone, Y = 1 - Distance (abajo = cerca/pecho).
                        let cx_px = (params.tone.clamp(0.0, 1.0) * w).clamp(12.0, (w - 12.0).max(12.0));
                        let cy_px = ((1.0 - params.distance.clamp(0.0, 1.0)) * h)
                            .clamp(12.0, (h - 12.0).max(12.0));

                        // Líneas guía hacia los ejes
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

                        // Halo exterior: refleja la sala (`room`) y la distancia (`distance`)
                        let room_scale = match params.room {
                            RoomKind::Close => 0.55,
                            RoomKind::Wood => 0.85,
                            RoomKind::Hall => 1.20,
                            RoomKind::Open => 1.45,
                        };
                        let halo_r = (20.0 + params.distance * 36.0 * room_scale + params.body * 14.0 + pulse * 14.0)
                            .min(w.min(h) * 0.45);
                        let halo_alpha = ((0x16 as f32) + params.distance * 24.0 + pulse * 36.0)
                            .clamp(16.0, 110.0) as u32;
                        window.paint_quad(quad(
                            Bounds::new(
                                point(bounds.left() + px(cx_px - halo_r), bounds.top() + px(cy_px - halo_r)),
                                size(px(halo_r * 2.0), px(halo_r * 2.0)),
                            ),
                            Corners::all(px(halo_r)),
                            rgba((cyan << 8) | halo_alpha),
                            Edges::all(px(1.0)),
                            rgba((cyan << 8) | (halo_alpha + 0x24).min(0xaa)),
                            Default::default(),
                        ));

                        // Halo medio: densidad (`body`)
                        let body_r = 10.0 + params.body * 14.0 + pulse * 6.0;
                        let body_alpha = ((0x2c as f32) + pulse * 52.0).clamp(32.0, 150.0) as u32;
                        window.paint_quad(quad(
                            Bounds::new(
                                point(bounds.left() + px(cx_px - body_r), bounds.top() + px(cy_px - body_r)),
                                size(px(body_r * 2.0), px(body_r * 2.0)),
                            ),
                            Corners::all(px(body_r)),
                            rgba((accent << 8) | body_alpha),
                            Edges::all(px(1.0)),
                            rgba((accent << 8) | 0x99),
                            Default::default(),
                        ));

                        // Núcleo táctil de posición
                        let core_r = 5.5 + pulse * 2.5;
                        window.paint_quad(quad(
                            Bounds::new(
                                point(bounds.left() + px(cx_px - core_r), bounds.top() + px(cy_px - core_r)),
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

    fn synth_knob(
        &self,
        cc_id: u8,
        label: &'static str,
        hint: &'static str,
        default_val: f32,
        color: u32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let value = self.synth_param_value(cc_id);
        div()
            .id(("synth-param", cc_id as usize))
            .w(px(78.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .items_center()
            .cursor(gpui::CursorStyle::ResizeUpDown)
            .child(
                div()
                    .text_size(px(10.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(ds.text))
                    .child(label),
            )
            .child(rotary_knob(
                ds,
                KnobSpec {
                    fraction: value,
                    bipolar: false,
                    color,
                    diameter: 28.,
                },
            ))
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(rgb(color))
                    .child(format!("{:.0}%", value * 100.0)),
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
                        this.set_synth_param(cc_id, default_val);
                    } else {
                        let cur = this.synth_param_value(cc_id);
                        this.synth_ui.slider_drag = Some((cc_id, event.position.y, cur));
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_scroll(cx.listener(move |this, event: &gpui::ScrollEvent, _, cx| {
                let delta = f32::from(event.delta.pixel_delta(px(20.)).y);
                if delta != 0.0 {
                    let step = if event.modifiers.shift { 0.01 } else { 0.04 };
                    let cur = this.synth_param_value(cc_id);
                    this.set_synth_param(cc_id, cur + delta.signum() * step);
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
    }
}
