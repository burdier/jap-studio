//! Barra de Locators, Región de Tiempo y Loop Brace estilo Ableton Live.
use crate::ui::{
    DawApp,
    components::modal::{ModalClose, modal_backdrop},
    timeline::{MarkerEdit, TimelineArea, loop_beats},
};
use elevate::musical::time::PPQ;
use gpui::{
    Context, Div, MouseButton, MouseDownEvent, Window,
    component::{
        Sizable,
        button::{Button, ButtonVariants},
        input::{Input, InputState},
        tooltip::Tooltip,
    },
    div,
    prelude::*,
    px, rgb, rgba,
};
use std::rc::Rc;

impl DawApp {
    pub(super) fn render_arrangement_navigation(&self, cx: &mut Context<Self>) -> Div {
        let ds = &self.ds;
        let (first, scale, width) = self.timeline.navigation_view();
        let x_at = |tick: i64| (tick as f64 / PPQ as f64 - first) * scale;
        let markers = &self.timeline.navigation.markers;
        let first_tick = (first * PPQ as f64).ceil() as i64;
        let visible_markers = markers
            .range(first_tick..)
            .take_while(|&(&tick, _)| x_at(tick) < width);
        let loop_region = self.timeline.visible_loop();
        let time_sel = self.timeline.visible_time_selection();
        let has_region = self.timeline.active_region_ticks().is_some();
        let region_tool = self.timeline.region_tool;
        let auto_mode = self.timeline.automation_mode;
        let auto_param = self.timeline.automation_param;
        let selected_track_has_auto = self
            .tracks
            .get(self.cursor_row)
            .is_some_and(|t| self.timeline.automation.has_points(t.id, auto_param));
        let auto_color = match auto_param {
            elevate::musical::track_automation::AutomationParam::Volume => ds.warning,
            elevate::musical::track_automation::AutomationParam::Pan => ds.cyan,
        };
        let header_bg = ds.arrangement_header();

        let lane = div()
            .id("arrangement-marker-lane")
            .relative()
            .w_full()
            .h_full()
            .overflow_hidden()
            .bg(rgb(header_bg))
            .cursor(gpui::CursorStyle::IBeam)
            .tooltip(|window, cx| {
                Tooltip::new(
                    "Arrastra: marcar región · Doble clic: añadir Locator · Ctrl+L: Loop · Ctrl+Shift+D: Duplicar tiempo",
                )
                .build(window, cx)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if event.click_count == 2 {
                        let beat = this.timeline.ruler_beat(event.position);
                        this.edit_arrangement_marker(beat, window, cx);
                    } else {
                        this.timeline
                            .begin_region_drag(event.position, event.modifiers.shift);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, _, cx| {
                    this.timeline.time_selection = None;
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            // Resaltado de región de tiempo marcada
            .when_some(time_sel, |lane, (start, end)| {
                let (left, right) = (x_at(start), x_at(end));
                let visible_left = left.max(0.);
                let visible_right = right.min(width);
                lane.when(visible_right > visible_left, |lane| {
                    lane.child(
                        div()
                            .absolute()
                            .left(px(visible_left as f32))
                            .top_0()
                            .bottom_0()
                            .w(px((visible_right - visible_left) as f32))
                            .bg(rgba((ds.accent << 8) | 0x2a))
                            .when(left >= 0., |r| r.border_l_2())
                            .when(right <= width, |r| r.border_r_2())
                            .border_color(rgba((ds.accent << 8) | 0xcc)),
                    )
                })
            })
            // Loop Brace estilo Ableton (franja inferior con extremos marcados)
            .when_some(loop_region, |lane, (start, end)| {
                let (left, right) = (x_at(start), x_at(end));
                let visible_left = left.max(0.);
                let visible_right = right.min(width);
                lane.when(visible_right > visible_left, |lane| {
                    let active = self.audio_loop.is_some()
                        || loop_region != self.timeline.navigation.loop_region;
                    let color = if active { ds.cyan } else { ds.muted };
                    let w = (visible_right - visible_left) as f32;
                    lane.child(
                        div()
                            .id("arrangement-loop-brace")
                            .absolute()
                            .left(px(visible_left as f32))
                            .bottom(px(1.))
                            .h(px(10.))
                            .w(px(w))
                            .cursor(gpui::CursorStyle::OpenHand)
                            .bg(rgba((color << 8) | if active { 0x44 } else { 0x20 }))
                            .border_t_2()
                            .border_color(rgb(color))
                            .tooltip(|window, cx| {
                                Tooltip::new(
                                    "Loop Brace · Arrastra extremos o centro · Clic derecho: quitar loop",
                                )
                                .build(window, cx)
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                    this.timeline.time_selection = Some((start, end));
                                    this.timeline
                                        .begin_loop_drag(event.position, event.modifiers.shift);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(|this, _, _, cx| {
                                    if let Err(error) = this.set_audio_loop(None) {
                                        this.status_message = error;
                                    } else {
                                        this.timeline.navigation.loop_region = None;
                                    }
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            // Corchete izquierdo
                            .when(left >= 0., |brace| {
                                brace.child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .top_0()
                                        .bottom_0()
                                        .w(px(5.))
                                        .bg(rgb(color)),
                                )
                            })
                            // Corchete derecho
                            .when(right <= width, |brace| {
                                brace.child(
                                    div()
                                        .absolute()
                                        .right_0()
                                        .top_0()
                                        .bottom_0()
                                        .w(px(5.))
                                        .bg(rgb(color)),
                                )
                            }),
                    )
                })
            })
            // Locators estilo banderita Ableton en la mitad superior
            .children(visible_markers.map(|(&tick, name)| {
                let x = x_at(tick);
                let next = self
                    .timeline
                    .navigation
                    .markers
                    .range((std::ops::Bound::Excluded(tick), std::ops::Bound::Unbounded))
                    .next()
                    .map_or(width, |(&next, _)| x_at(next).min(width));
                div()
                    .id(("arrangement-marker", tick as usize))
                    .absolute()
                    .left(px(x as f32))
                    .top(px(1.))
                    .h(px(14.))
                    .max_w(px((next - x - 3.).max(10.) as f32))
                    .flex()
                    .items_center()
                    .gap(px(3.))
                    .pr(px(5.))
                    .overflow_hidden()
                    .rounded_r(px(2.))
                    .bg(rgba((ds.warning << 8) | 0x30))
                    .border_l_2()
                    .border_color(rgb(ds.warning))
                    .hover(|marker| marker.bg(rgba((ds.warning << 8) | 0x55)))
                    .text_size(px(9.5))
                    .text_color(rgb(ds.text))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                            if let Err(error) = this.seek_audio(tick as f64 / PPQ as f64) {
                                this.status_message = error;
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, _, window, cx| {
                            this.edit_arrangement_marker(tick as f64 / PPQ as f64, window, cx);
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .pl(px(2.))
                            .text_size(px(8.5))
                            .text_color(rgb(ds.warning))
                            .child("▸"),
                    )
                    .child(div().truncate().child(name.clone()))
            }));

        div()
            .h(px(26.))
            .flex()
            .flex_shrink_0()
            .text_size(px(ds.font_size_xs))
            .child(
                div()
                    .w(px(ds.track_header_width))
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_1()
                    .px_2()
                    .bg(rgb(header_bg))
                    .rounded_tl(px(ds.radius_lg))
                    .border_t_1()
                    .border_x_1()
                    .border_color(rgb(ds.frame_border()))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            // Herramienta Región (estilo Ableton)
                            .child(
                                div()
                                    .id("arrangement-region-tool")
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded(px(3.))
                                    .bg(if region_tool {
                                        rgba((ds.accent << 8) | 0x38)
                                    } else {
                                        rgba((ds.text << 8) | 0x0d)
                                    })
                                    .border_1()
                                    .border_color(if region_tool {
                                        rgb(ds.accent)
                                    } else {
                                        rgba((ds.text << 8) | 0x22)
                                    })
                                    .hover(|b| b.bg(rgba((ds.accent << 8) | 0x2e)))
                                    .text_size(px(10.))
                                    .text_color(rgb(if region_tool { ds.accent } else { ds.text }))
                                    .cursor_pointer()
                                    .tooltip(|window, cx| {
                                        Tooltip::new(
                                            "Herramienta Región · Arrastra en pistas para marcar tiempo (o Alt+Arrastra)",
                                        )
                                        .build(window, cx)
                                    })
                                    .child("⌶ Región")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.timeline.region_tool = !this.timeline.region_tool;
                                            this.status_message = if this.timeline.region_tool {
                                                "Herramienta Región activa · arrastra en las pistas (Ctrl+L loop · Ctrl+Shift+D duplicar tiempo)".into()
                                            } else {
                                                "Herramienta Selección normal".into()
                                            };
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            // Botón Set (Locator estilo Ableton)
                            .child(
                                div()
                                    .id("new-arrangement-marker")
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded(px(3.))
                                    .bg(rgba((ds.warning << 8) | 0x1e))
                                    .border_1()
                                    .border_color(rgba((ds.warning << 8) | 0x55))
                                    .hover(|btn| {
                                        btn.bg(rgba((ds.warning << 8) | 0x38))
                                            .border_color(rgb(ds.warning))
                                    })
                                    .text_size(px(10.))
                                    .text_color(rgb(ds.warning))
                                    .cursor_pointer()
                                    .tooltip(|window, cx| {
                                        Tooltip::new("Set Locator en cursor o inicio de región (Ctrl+Shift+M)")
                                            .build(window, cx)
                                    })
                                    .child("Set")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            let beat = this
                                                .timeline
                                                .active_region_ticks()
                                                .map(|(s, _)| s as f64 / PPQ as f64)
                                                .unwrap_or_else(|| this.timeline.edit_beat());
                                            this.edit_arrangement_marker(beat, window, cx);
                                            cx.stop_propagation();
                                        }),
                                    ),
                            )
                            // Modo Automatización (Volumen / Pan)
                            .child(
                                div()
                                    .id("arrangement-automation-toggle")
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded(px(3.))
                                    .bg(if auto_mode {
                                        rgba((auto_color << 8) | 0x38)
                                    } else {
                                        rgba((ds.text << 8) | 0x0d)
                                    })
                                    .border_1()
                                    .border_color(if auto_mode {
                                        rgb(auto_color)
                                    } else {
                                        rgba((ds.text << 8) | 0x22)
                                    })
                                    .hover(|b| b.bg(rgba((auto_color << 8) | 0x2e)))
                                    .text_size(px(10.))
                                    .text_color(rgb(if auto_mode { auto_color } else { ds.text }))
                                    .cursor_pointer()
                                    .tooltip(|window, cx| {
                                        Tooltip::new(
                                            "Modo Automatización (Ctrl+Shift+A) · Clic: añadir/arrastrar punto · Doble clic o clic derecho: borrar punto · Shift: libre sin imán",
                                        )
                                        .build(window, cx)
                                    })
                                    .child(if auto_mode {
                                        format!("⚡ {}", auto_param.short_label())
                                    } else {
                                        "⚡ Auto".into()
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.toggle_automation_mode();
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        cx.listener(|this, _, _, cx| {
                                            this.set_automation_param(
                                                this.timeline.automation_param.toggle(),
                                            );
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .when(auto_mode, |row| {
                                row.child(
                                    div()
                                        .id("arrangement-automation-param")
                                        .px(px(5.))
                                        .py(px(2.))
                                        .rounded(px(3.))
                                        .bg(rgba((ds.text << 8) | 0x10))
                                        .border_1()
                                        .border_color(rgba((auto_color << 8) | 0x66))
                                        .hover(|b| b.bg(rgba((auto_color << 8) | 0x2a)))
                                        .text_size(px(9.5))
                                        .text_color(rgb(ds.text))
                                        .cursor_pointer()
                                        .tooltip(|window, cx| {
                                            Tooltip::new(
                                                "Alternar parámetro de automatización (Volumen / Pan)",
                                            )
                                            .build(window, cx)
                                        })
                                        .child(auto_param.toggle().short_label())
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.set_automation_param(
                                                    this.timeline.automation_param.toggle(),
                                                );
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        ),
                                )
                                .when(selected_track_has_auto, |row| {
                                    row.child(
                                        div()
                                            .id("arrangement-automation-clear")
                                            .px(px(4.))
                                            .py(px(2.))
                                            .rounded(px(3.))
                                            .bg(rgba((ds.danger << 8) | 0x22))
                                            .hover(|b| b.bg(rgba((ds.danger << 8) | 0x45)))
                                            .text_size(px(9.5))
                                            .text_color(rgb(ds.danger))
                                            .cursor_pointer()
                                            .tooltip(|window, cx| {
                                                Tooltip::new(
                                                    "Borrar automatización de la pista seleccionada",
                                                )
                                                .build(window, cx)
                                            })
                                            .child("✕")
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _, cx| {
                                                    this.clear_selected_track_automation();
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                }),
                                            ),
                                    )
                                })
                            }),
                    )
                    // Acciones rápidas cuando hay región o selección
                    .when(has_region, |left| {
                        left.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(3.))
                                .child(
                                    div()
                                        .id("region-action-loop")
                                        .px(px(5.))
                                        .py(px(1.))
                                        .rounded(px(3.))
                                        .bg(rgba((ds.cyan << 8) | 0x22))
                                        .border_1()
                                        .border_color(rgba((ds.cyan << 8) | 0x66))
                                        .hover(|b| b.bg(rgba((ds.cyan << 8) | 0x44)))
                                        .text_size(px(10.))
                                        .text_color(rgb(ds.cyan))
                                        .cursor_pointer()
                                        .tooltip(|window, cx| {
                                            Tooltip::new("Loop de selección (Ctrl+L)")
                                                .build(window, cx)
                                        })
                                        .child("↻")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.toggle_loop_from_selection();
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("region-action-dup-time")
                                        .px(px(5.))
                                        .py(px(1.))
                                        .rounded(px(3.))
                                        .bg(rgba((ds.accent << 8) | 0x22))
                                        .border_1()
                                        .border_color(rgba((ds.accent << 8) | 0x66))
                                        .hover(|b| b.bg(rgba((ds.accent << 8) | 0x44)))
                                        .text_size(px(10.))
                                        .text_color(rgb(ds.accent))
                                        .cursor_pointer()
                                        .tooltip(|window, cx| {
                                            Tooltip::new("Duplicar tiempo (Ctrl+Shift+D)")
                                                .build(window, cx)
                                        })
                                        .child("⧉")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.duplicate_arrangement_time();
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        ),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .w(px(7.))
                    .h_full()
                    .flex_shrink_0()
                    .bg(rgb(ds.gutter_color())),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .rounded_tr(px(ds.radius_lg))
                    .border_t_1()
                    .border_x_1()
                    .border_color(rgb(ds.frame_border()))
                    .overflow_hidden()
                    .on_scroll(cx.listener(|this, event, _, cx| {
                        this.timeline.scroll(event, TimelineArea::Ruler);
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .on_pinch(cx.listener(|this, event, _, cx| {
                        this.timeline.pinch(event, TimelineArea::Ruler);
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(lane),
            )
    }

    pub(super) fn render_wave_zoom(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(div().text_xs().text_color(rgb(self.ds.muted)).child("Onda"))
            .children([("−", 0.5), ("+", 2.0)].into_iter().enumerate().map(
                |(i, (label, factor))| {
                    Button::new(("arrangement-wave-zoom", i))
                        .small()
                        .ghost()
                        .label(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.timeline.change_wave_zoom(factor);
                            cx.notify();
                        }))
                },
            ))
            .when(self.timeline.focused_audio.is_some(), |row| {
                row.child(
                    Button::new("arrangement-zoom-clip")
                        .small()
                        .ghost()
                        .label("Ver clip")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.timeline.zoom_to_clip();
                            cx.notify();
                        })),
                )
            })
    }

    pub(in crate::ui) fn toggle_automation_mode(&mut self) {
        self.timeline.automation_mode = !self.timeline.automation_mode;
        if self.timeline.automation_mode {
            self.timeline.region_tool = false;
            self.status_message = format!(
                "Automatización {} activa · clic para añadir/mover puntos · doble clic o clic derecho para borrar",
                self.timeline.automation_param.label()
            );
        } else {
            self.timeline.end_automation_drag();
            self.status_message = "Modo Automatización desactivado".into();
        }
    }

    pub(in crate::ui) fn set_automation_param(
        &mut self,
        param: elevate::musical::track_automation::AutomationParam,
    ) {
        self.timeline.automation_param = param;
        self.timeline.automation_mode = true;
        self.timeline.region_tool = false;
        self.status_message = format!(
            "Automatización: {} · clic en una pista para dibujar la curva",
            param.label()
        );
    }

    pub(in crate::ui) fn clear_selected_track_automation(&mut self) {
        let Some(track) = self.tracks.get(self.cursor_row) else {
            return;
        };
        let id = track.id;
        let name = track.name.clone();
        let param = self.timeline.automation_param;
        self.timeline.automation.clear_lane(id, param);
        self.publish_track_automation(id);
        self.refresh_project_dirty();
        self.status_message = format!("{name}: automatización de {} borrada", param.label());
    }

    /// Ctrl+L estilo Ableton: ajusta el Loop Brace a la región o clips seleccionados y activa/alterna el loop.
    pub(in crate::ui) fn toggle_loop_from_selection(&mut self) {
        let Some(range_ticks) = self.timeline.active_region_ticks() else {
            self.toggle_audio_loop();
            return;
        };
        let beats = loop_beats(range_ticks);
        let already_same = self
            .audio_loop
            .is_some_and(|(a, b)| (a - beats.0).abs() < 1e-3 && (b - beats.1).abs() < 1e-3);
        let next = if already_same { None } else { Some(beats) };
        if let Err(error) = self.set_audio_loop(next) {
            self.status_message = error;
        }
    }

    /// Ctrl+Shift+D estilo Ableton: duplica el tiempo de la región activa desplazando todo lo posterior.
    pub(in crate::ui) fn duplicate_arrangement_time(&mut self) {
        let Some(range_ticks) = self.timeline.active_region_ticks() else {
            self.status_message =
                "Marca una región o selecciona clips para duplicar tiempo (Ctrl+Shift+D)".into();
            return;
        };
        let (start, end) = loop_beats(range_ticks);
        self.timeline.end_clip_drag();
        self.stop_audio();
        let cloned = self.timeline.duplicate_time(start, end);
        self.publish_clip_edit();
        self.refresh_project_dirty();
        self.status_message = format!(
            "Tiempo duplicado ({:.2}–{:.2} negras · {cloned} clips clonados)",
            start, end
        );
    }

    /// Ctrl+Shift+Delete estilo Ableton: elimina el tiempo de la región y contrae lo posterior.
    pub(in crate::ui) fn delete_arrangement_time(&mut self) {
        let Some(range_ticks) = self.timeline.active_region_ticks() else {
            self.status_message =
                "Marca una región para eliminar tiempo (Ctrl+Shift+Delete)".into();
            return;
        };
        let (start, end) = loop_beats(range_ticks);
        self.timeline.end_clip_drag();
        self.stop_audio();
        let affected = self.timeline.delete_time(start, end);
        self.publish_clip_edit();
        self.refresh_project_dirty();
        self.status_message = format!(
            "Tiempo eliminado ({:.2}–{:.2} negras · {affected} clips ajustados)",
            start, end
        );
    }

    /// Ctrl+I estilo Ableton: inserta silencio del largo de la región (o 1 compás) desplazando todo.
    pub(in crate::ui) fn insert_arrangement_time(&mut self) {
        let (start, len) = if let Some(range_ticks) = self.timeline.active_region_ticks() {
            let (s, e) = loop_beats(range_ticks);
            (s, e - s)
        } else {
            (
                self.timeline.edit_beat(),
                self.timeline.meter.beats_per_bar(),
            )
        };
        self.timeline.end_clip_drag();
        self.stop_audio();
        self.timeline.insert_silence(start, len);
        self.publish_clip_edit();
        self.refresh_project_dirty();
        self.status_message = format!("Tiempo insertado: +{len:.2} negras en {start:.2}");
    }

    pub(in crate::ui) fn edit_arrangement_marker(
        &mut self,
        beat: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tick = (beat * PPQ as f64).round() as i64;
        let label = self
            .timeline
            .navigation
            .markers
            .get(&tick)
            .cloned()
            .unwrap_or_else(|| format!("Compás {}", self.timeline.meter.bar_at(beat)));
        let field = cx.new(|cx| InputState::new(window, cx).default_value(label));
        field.update(cx, |field, cx| {
            field.focus(window, cx);
            field.select_all(window, cx);
        });
        self.timeline.marker_edit = Some(MarkerEdit { tick, field });
        cx.notify();
    }

    fn close_arrangement_marker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.timeline.marker_edit = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    fn apply_arrangement_marker(
        &mut self,
        remove: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = &self.timeline.marker_edit else {
            return;
        };
        if remove {
            self.timeline.navigation.markers.remove(&edit.tick);
        } else {
            let label = edit.field.read(cx).value();
            let label = label.trim();
            if label.is_empty() {
                return;
            }
            self.timeline
                .navigation
                .markers
                .insert(edit.tick, label.chars().take(80).collect());
        }
        self.refresh_project_dirty();
        self.close_arrangement_marker(window, cx);
    }

    pub(in crate::ui) fn finish_arrangement_loop(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
    ) -> bool {
        if !self.timeline.move_loop_drag(position) {
            return false;
        }
        if let Some(range) = self.timeline.finish_loop_drag()
            && let Err(error) = self.set_audio_loop(Some(loop_beats(range)))
        {
            self.status_message = error;
        }
        true
    }

    pub(in crate::ui) fn finish_arrangement_region(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
    ) -> bool {
        if !self.timeline.move_region_drag(position) {
            return false;
        }
        if let Some(Some(range)) = self.timeline.finish_region_drag() {
            let (a, b) = loop_beats(range);
            self.status_message = format!(
                "Región: {a:.2}–{b:.2} negras · Ctrl+L loop · Ctrl+Shift+D duplicar · Ctrl+Shift+Del borrar tiempo"
            );
        }
        true
    }

    pub(in crate::ui) fn arrangement_marker_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.timeline.marker_edit.is_none() {
            return false;
        }
        match event.keystroke.key.as_str() {
            "escape" => self.close_arrangement_marker(window, cx),
            "enter" => self.apply_arrangement_marker(false, window, cx),
            _ => return true,
        }
        cx.stop_propagation();
        true
    }

    pub(in crate::ui) fn render_arrangement_marker_editor(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let edit = self.timeline.marker_edit.as_ref()?;
        let exists = self.timeline.navigation.markers.contains_key(&edit.tick);
        let close: ModalClose = Rc::new(cx.listener(|this, _, window, cx| {
            this.close_arrangement_marker(window, cx);
            cx.stop_propagation();
        }));
        let card = div()
            .w(px(362.))
            .p_4()
            .rounded(px(self.ds.radius_lg))
            .bg(rgb(self.ds.panel_dark))
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_lg().child(if exists {
                "Editar Locator"
            } else {
                "Nuevo Locator"
            }))
            .child(
                Input::new(&edit.field)
                    .w_full()
                    .aria_label("Nombre del Locator"),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .when(exists, |row| {
                        row.child(
                            Button::new("delete-arrangement-marker")
                                .small()
                                .ghost()
                                .label("Eliminar")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.apply_arrangement_marker(true, window, cx);
                                })),
                        )
                    })
                    .child(
                        Button::new("close-arrangement-marker")
                            .small()
                            .label("Cancelar")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_arrangement_marker(window, cx)
                            })),
                    )
                    .child(
                        Button::new("save-arrangement-marker")
                            .small()
                            .primary()
                            .label("Guardar")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.apply_arrangement_marker(false, window, cx)
                            })),
                    ),
            );
        Some(modal_backdrop(0x05070bd9, close, card))
    }
}
