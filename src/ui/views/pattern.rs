//! Drums Latex: Canción arriba, conmutación de pasos, Pads MPC y Piano Roll Light abajo.
mod inspector;
mod mpc;
mod roll;
mod song;
mod steps;

use crate::ui::{
    DawApp, DesignSystem,
    components::{
        button::{flat_control, led_dot, panel_shadow},
        meter::track_peak_meter,
    },
    explorer::ExplorerSampleDrag,
    pattern_editor::{DrumSampleTarget, DrumViewMode, PatternAction, PatternOptions},
    workspace_focus::Panel,
};
use elevate::musical::{drums::DrumPlaybackSource, grid::NoteGrid};
use gpui::{
    Context, Div, FontWeight, MouseButton, SharedString, Stateful, component::tooltip::Tooltip,
    div, prelude::*, px, rgb, rgba,
};

fn grid_note_name(denominator: u16) -> &'static str {
    match denominator {
        1 => "Redonda",
        2 => "Blanca",
        4 => "Negra",
        8 => "Corchea",
        16 => "Semicorchea",
        32 => "Fusa",
        64 => "Semifusa",
        _ => "Garrapatea",
    }
}

fn drum_capsule(ds: &DesignSystem) -> Div {
    div()
        .h(px(30.0))
        .px(px(4.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(4.0))
        .rounded(px(9.0))
        .bg(rgb(ds.panel))
        .border_1()
        .border_color(rgba(if ds.is_dark() { 0xffffff0e } else { 0x0f172a14 }))
        .shadow(panel_shadow(ds))
}

fn duration_label(length: i64, bar: i64, beat: i64) -> String {
    let bars = length / bar;
    let remainder = length % bar;
    let pulses = remainder / beat;
    let measures = if bars == 1 {
        "1 compás".into()
    } else {
        format!("{bars} compases")
    };
    if remainder == 0 {
        measures
    } else if remainder % beat != 0 {
        if bars == 0 {
            "Parcial".into()
        } else {
            format!("{measures}+")
        }
    } else {
        let beats = if pulses == 1 {
            "1 pulso".into()
        } else {
            format!("{pulses} pulsos")
        };
        if bars == 0 {
            beats
        } else {
            format!("{measures} + {beats}")
        }
    }
}

impl DawApp {
    pub(in crate::ui) fn render_pattern(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let editor = &self.pattern_editor;
        let has_kit = !editor.session.kit.pieces.is_empty();
        let selected = editor.roll.piece.and_then(|id| {
            editor
                .session
                .kit
                .pieces
                .iter()
                .find(|piece| piece.id == id)
        });
        let instrument = selected.or_else(|| editor.session.kit.pieces.get(editor.cursor_lane));
        let show_song =
            has_kit && !editor.mixer_open && editor.session.source == DrumPlaybackSource::Song;

        let show_toolbar = has_kit || editor.mixer_open || editor.view_mode == DrumViewMode::Pads;
        div()
            .id("drums-latex")
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .bg(rgb(self.ds.bg))
            .capture_any_mouse_down(cx.listener(|this, _, window, cx| {
                if this.workspace_focus.active != Panel::Center {
                    this.workspace_focus.active = Panel::Center;
                    this.sync_midi_focus();
                }
                window.focus(&this.focus_handle, cx);
            }))
            .when(show_toolbar, |panel| {
                panel
                    .child(self.render_drum_toolbar(cx))
                    .when(!editor.mixer_open && editor.options.is_some(), |p| {
                        p.child(self.render_grid_picker_drawer(cx))
                    })
                    .when(show_song, |p| p.child(self.render_drum_song(cx)))
                    .when_some(
                        instrument.filter(|_| editor.inspector_open && !editor.mixer_open),
                        |p, piece| p.child(self.render_drum_inspector(piece, cx)),
                    )
            })
            .child(
                div()
                    .id("drum-sample-drop")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .on_drop(
                        cx.listener(|this, files: &gpui::ExternalPaths, window, cx| {
                            this.drop_drum_files(files.paths(), None, window, cx);
                        }),
                    )
                    .drag_over::<ExplorerSampleDrag>({
                        let cyan = self.ds.cyan;
                        move |style, _, _, _| style.border_2().border_color(rgb(cyan))
                    })
                    .on_drop(
                        cx.listener(|this, sample: &ExplorerSampleDrag, window, cx| {
                            this.drop_drum_sample(sample.path.clone(), None, window, cx);
                        }),
                    )
                    .child(if editor.mixer_open {
                        self.render_drum_mixer(cx).into_any_element()
                    } else if editor.view_mode == DrumViewMode::Pads && selected.is_none() {
                        self.render_mpc_pads(cx).into_any_element()
                    } else if !has_kit {
                        self.render_drum_empty_state(cx).into_any_element()
                    } else if let Some(piece) = selected {
                        self.render_drum_roll(piece, cx).into_any_element()
                    } else {
                        self.render_drum_steps(cx).into_any_element()
                    }),
            )
    }

    /// Estado inicial limpio: sin paneles ni controles hasta que se suelte o cargue un sample.
    fn render_drum_empty_state(&self, cx: &mut Context<Self>) -> Div {
        let ds = &self.ds;
        let dark = ds.is_dark();
        let pad = |lit: bool| {
            div()
                .size(px(14.0))
                .rounded(px(4.0))
                .bg(rgb(if lit { ds.accent } else { ds.panel_dark }))
                .border_1()
                .border_color(rgba(if lit {
                    (ds.accent << 8) | 0x99
                } else if dark {
                    0xffffff12
                } else {
                    0x0f172a18
                }))
        };
        let pad_emblem = div()
            .p(px(8.0))
            .rounded(px(12.0))
            .bg(rgb(ds.bg))
            .border_1()
            .border_color(rgba(if dark { 0xffffff10 } else { 0x0f172a14 }))
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(div().flex().gap(px(4.0)).child(pad(true)).child(pad(false)))
            .child(
                div()
                    .flex()
                    .gap(px(4.0))
                    .child(pad(false))
                    .child(pad(false)),
            );

        div()
            .flex_1()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p(px(24.0))
            .child(
                div()
                    .w(px(400.0))
                    .max_w_full()
                    .px(px(28.0))
                    .py(px(26.0))
                    .rounded(px(14.0))
                    .bg(rgb(ds.panel))
                    .border_1()
                    .border_color(rgba(if dark { 0xffffff12 } else { 0x0f172a16 }))
                    .shadow(panel_shadow(ds))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(12.0))
                    .child(pad_emblem)
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(ds.text))
                            .child("Empieza tu ritmo"),
                    )
                    .child(
                        div()
                            .text_center()
                            .text_size(px(11.5))
                            .text_color(rgb(ds.muted))
                            .child(
                                "Suelta un sample aquí desde el Explorador o desde tu equipo para activar el secuenciador de Drums.",
                            ),
                    )
                    .child(
                        div()
                            .pt(px(4.0))
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                flat_control(ds, true, ds.accent)
                                    .id("empty-add-drum-sample")
                                    .h(px(30.0))
                                    .px(px(14.0))
                                    .rounded(px(ds.radius_md))
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(ds.text))
                                    .child("+ Cargar muestra")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.choose_drum_sample(
                                                DrumSampleTarget::NewPiece,
                                                cx,
                                            );
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                flat_control(ds, false, ds.warning)
                                    .id("empty-load-drum-kit")
                                    .h(px(30.0))
                                    .px(px(14.0))
                                    .rounded(px(ds.radius_md))
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(ds.text))
                                    .child("Cargar kit")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.load_drum_kit_dialog(cx);
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                flat_control(ds, false, ds.orange)
                                    .id("empty-open-mpc")
                                    .h(px(30.0))
                                    .px(px(14.0))
                                    .rounded(px(ds.radius_md))
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(ds.orange))
                                    .child("Pads MPC")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.pattern_editor.view_mode = DrumViewMode::Pads;
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .when(!self.explorer_visible(), |row| {
                                row.child(
                                    flat_control(ds, false, ds.cyan)
                                        .id("empty-open-explorer")
                                        .h(px(30.0))
                                        .px(px(14.0))
                                        .rounded(px(ds.radius_md))
                                        .text_size(px(11.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(rgb(ds.text))
                                        .child("Abrir explorador")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.toggle_explorer();
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        ),
                                )
                            }),
                    ),
            )
    }

    /// Barra única y organizada en 4 cápsulas limpias (Patrón · Ritmo · Modo · Kit).
    fn render_drum_toolbar(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let ds = &self.ds;
        let editor = &self.pattern_editor;
        let session = &editor.session;
        let pattern = session.active_pattern();
        let meter = self.timeline.meter;
        let bar_ticks = meter.ticks_per_bar();
        let beat_ticks = meter.ticks_per_beat();
        let length = pattern.notes.length().0;
        let can_shrink = length > bar_ticks;
        let peak = self.mpc_bus_peak();
        let multiple_patterns = session.patterns.len() > 1;

        // 1. Cápsula de patrones (selector + crear + duplicar)
        let mut pattern_capsule = drum_capsule(ds);
        for (idx, pat) in session.patterns.iter().enumerate() {
            let active = pat.id == session.active;
            let short = if multiple_patterns {
                format!("P{}", idx + 1)
            } else {
                pat.name.clone()
            };
            let full_name = pat.name.clone();
            let id = pat.id;
            pattern_capsule = pattern_capsule.child(
                flat_control(ds, active, ds.accent)
                    .id(("drum-pat-tab", idx))
                    .h(px(22.0))
                    .px(px(8.0))
                    .gap(px(5.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(11.0))
                    .font_weight(if active {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(rgb(if active { ds.text } else { ds.muted }))
                    .tooltip(move |window, cx| Tooltip::new(full_name.clone()).build(window, cx))
                    .when(active, |el| el.child(led_dot(ds.accent, 4.5, true)))
                    .child(short)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.apply_pattern_action(PatternAction::Select(id));
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            );
        }
        let has_notes = !pattern.notes.notes().is_empty();
        let active_id = session.active;
        pattern_capsule = pattern_capsule
            .child(
                flat_control(ds, false, ds.accent)
                    .id("drum-new-pattern")
                    .w(px(22.0))
                    .h(px(22.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(12.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(ds.text))
                    .tooltip(|window, cx| Tooltip::new("Nuevo patrón").build(window, cx))
                    .child("+")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.apply_pattern_action(PatternAction::NewPattern);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                flat_control(ds, false, ds.accent)
                    .id("drum-dup-pattern")
                    .h(px(22.0))
                    .px(px(8.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(10.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(ds.muted))
                    .tooltip(|window, cx| Tooltip::new("Duplicar patrón actual").build(window, cx))
                    .child("Duplicar")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.apply_pattern_action(PatternAction::Duplicate);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                flat_control(ds, false, ds.warning)
                    .id("drum-clear-pattern")
                    .h(px(22.0))
                    .px(px(7.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(10.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(ds.muted))
                    .when(!has_notes, |el| el.opacity(0.45))
                    .tooltip(|window, cx| {
                        Tooltip::new("Vaciar todos los pasos del patrón actual").build(window, cx)
                    })
                    .child("Limpiar")
                    .when(has_notes, |el| {
                        el.on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.apply_pattern_action(PatternAction::ClearPattern);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                    }),
            )
            .when(multiple_patterns, |cap| {
                cap.child(
                    flat_control(ds, false, ds.danger)
                        .id("drum-delete-pattern")
                        .w(px(22.0))
                        .h(px(22.0))
                        .rounded(px(ds.radius_sm))
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(ds.danger))
                        .tooltip(|window, cx| {
                            Tooltip::new("Eliminar patrón actual").build(window, cx)
                        })
                        .child("×")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.apply_pattern_action(PatternAction::DeletePattern(active_id));
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ),
                )
            });

        // 2. Cápsula de ritmo (duración en compases + rejilla/pasos)
        let rhythm_capsule = drum_capsule(ds)
            .child(
                flat_control(ds, false, ds.accent)
                    .id("drum-bars-dec")
                    .w(px(22.0))
                    .h(px(22.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(11.5))
                    .text_color(rgb(if can_shrink { ds.text } else { ds.muted }))
                    .when(!can_shrink, |el| el.opacity(0.45))
                    .tooltip(|window, cx| Tooltip::new("Quitar 1 compás").build(window, cx))
                    .child("−")
                    .when(can_shrink, |el| {
                        el.on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.apply_pattern_action(PatternAction::Bars(-1));
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                    }),
            )
            .child(
                div()
                    .px(px(4.0))
                    .font_family("monospace")
                    .text_size(px(10.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(ds.text))
                    .child(duration_label(length, bar_ticks, beat_ticks)),
            )
            .child(
                flat_control(ds, false, ds.accent)
                    .id("drum-bars-inc")
                    .w(px(22.0))
                    .h(px(22.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(11.5))
                    .text_color(rgb(ds.text))
                    .tooltip(|window, cx| Tooltip::new("Añadir 1 compás").build(window, cx))
                    .child("+")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.apply_pattern_action(PatternAction::Bars(1));
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .w(px(1.0))
                    .h(px(14.0))
                    .mx(px(2.0))
                    .bg(rgba(if ds.is_dark() { 0xffffff14 } else { 0x0f172a18 })),
            )
            .child(
                flat_control(ds, editor.options == Some(PatternOptions::Grid), ds.cyan)
                    .id("drum-grid-menu")
                    .h(px(22.0))
                    .px(px(8.0))
                    .gap(px(5.0))
                    .rounded(px(ds.radius_sm))
                    .font_family("monospace")
                    .text_size(px(10.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(if editor.options == Some(PatternOptions::Grid) {
                        ds.text
                    } else {
                        ds.muted
                    }))
                    .tooltip({
                        let name = grid_note_name(pattern.grid.denominator());
                        let steps = pattern.step_count();
                        move |window, cx| {
                            Tooltip::new(format!(
                                "Rejilla: {name} ({steps} pasos) · clic para cambiar subdivisión"
                            ))
                            .build(window, cx)
                        }
                    })
                    .when(editor.options == Some(PatternOptions::Grid), |el| {
                        el.child(led_dot(ds.cyan, 4.5, true))
                    })
                    .child(format!(
                        "1/{} · {}p",
                        pattern.grid.denominator(),
                        pattern.step_count()
                    ))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            let options = &mut this.pattern_editor.options;
                            *options = (*options != Some(PatternOptions::Grid))
                                .then_some(PatternOptions::Grid);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child({
                let swing = pattern.swing;
                let has_swing = swing > 0;
                flat_control(ds, has_swing, ds.warning)
                    .id("drum-pattern-swing")
                    .h(px(22.0))
                    .px(px(8.0))
                    .gap(px(4.0))
                    .rounded(px(ds.radius_sm))
                    .font_family("monospace")
                    .text_size(px(10.0))
                    .font_weight(if has_swing {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(rgb(if has_swing { ds.text } else { ds.muted }))
                    .tooltip(|window, cx| {
                        Tooltip::new(
                            "Swing del patrón (0–75%) · rueda o clic izq/der: ±5% · doble clic o Alt+clic: 0%",
                        )
                        .build(window, cx)
                    })
                    .when(has_swing, |el| el.child(led_dot(ds.warning, 4.5, true)))
                    .child(format!("Swing {swing}%"))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                            let delta = if event.click_count >= 2 || event.modifiers.alt {
                                i16::MIN
                            } else {
                                5
                            };
                            this.apply_pattern_action(PatternAction::Swing(delta));
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, _, cx| {
                            this.apply_pattern_action(PatternAction::Swing(-5));
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_scroll(cx.listener(|this, event: &gpui::ScrollEvent, _, cx| {
                        let delta = f32::from(event.delta.pixel_delta(px(24.0)).y);
                        if delta != 0.0 {
                            this.apply_pattern_action(PatternAction::Swing(
                                delta.signum() as i16 * 5,
                            ));
                            cx.stop_propagation();
                            cx.notify();
                        }
                    }))
            });

        let total_bars = editor.step_total_bars(meter);
        let bars_per_page = editor.step_bars_per_page();
        let page_count = editor.step_page_count(meter);
        let play_tick = self
            .drum_pattern_playhead()
            .map(|beat| (beat * elevate::musical::time::PPQ as f64) as i64);
        let (visible_page, _, _) = editor.visible_step_window(meter, play_tick);
        let playing_page = play_tick.map(|t| {
            let page_ticks = bars_per_page as i64 * bar_ticks.max(1);
            ((t.max(0) / page_ticks) as usize).min(page_count - 1)
        });
        let page_capsule = (total_bars > 1 && !editor.mixer_open).then(|| {
            let mut cap = drum_capsule(ds);
            for (bars, label) in [(1usize, "1c"), (2usize, "2c")] {
                let active = bars_per_page == bars;
                cap = cap.child(
                    flat_control(ds, active, ds.cyan)
                        .id(("drum-view-bars", bars))
                        .h(px(22.0))
                        .px(px(6.0))
                        .rounded(px(ds.radius_sm))
                        .font_family("monospace")
                        .text_size(px(10.0))
                        .font_weight(if active {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .text_color(rgb(if active { ds.text } else { ds.muted }))
                        .tooltip(move |window, cx| {
                            Tooltip::new(if bars == 1 {
                                "Ver 1 compás por página (pads grandes)"
                            } else {
                                "Ver 2 compases por página al ancho completo"
                            })
                            .build(window, cx)
                        })
                        .child(label)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                let meter = this.timeline.meter;
                                this.pattern_editor.set_bars_per_page(bars, meter);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ),
                );
            }
            cap = cap.child(
                div()
                    .w(px(1.0))
                    .h(px(14.0))
                    .mx(px(1.0))
                    .bg(rgba(if ds.is_dark() { 0xffffff14 } else { 0x0f172a18 })),
            );
            for page in 0..page_count {
                let first_bar = page * bars_per_page + 1;
                let last_bar = (first_bar + bars_per_page - 1).min(total_bars);
                let label = if first_bar == last_bar {
                    format!("{first_bar}")
                } else {
                    format!("{first_bar}–{last_bar}")
                };
                let start_tick = (first_bar - 1) as i64 * bar_ticks;
                let end_tick = (last_bar as i64 * bar_ticks).min(length);
                let has_notes = editor.page_has_notes(start_tick, end_tick);
                let is_visible = page == visible_page;
                let is_playing = playing_page == Some(page);
                let tip_label = label.clone();
                cap = cap.child(
                    flat_control(ds, is_visible, if is_playing { ds.cyan } else { ds.accent })
                        .id(("drum-step-page", page))
                        .min_w(px(24.0))
                        .h(px(22.0))
                        .px(px(6.0))
                        .gap(px(4.0))
                        .rounded(px(ds.radius_sm))
                        .font_family("monospace")
                        .text_size(px(10.0))
                        .font_weight(if is_visible {
                            FontWeight::BOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .text_color(rgb(if is_visible {
                            ds.text
                        } else if has_notes {
                            ds.accent
                        } else {
                            ds.muted
                        }))
                        .tooltip(move |window, cx| {
                            Tooltip::new(format!("Ir a compás {tip_label}")).build(window, cx)
                        })
                        .when(is_playing, |el| el.child(led_dot(ds.cyan, 4.5, true)))
                        .when(!is_playing && has_notes, |el| {
                            el.child(led_dot(
                                if is_visible { ds.accent } else { ds.muted },
                                3.5,
                                false,
                            ))
                        })
                        .child(label)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.pattern_editor.step_page = page;
                                if this.player.is_some() {
                                    this.pattern_editor.follow_playhead = false;
                                }
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ),
                );
            }
            let follow = editor.follow_playhead;
            cap.child(
                flat_control(ds, follow, ds.cyan)
                    .id("drum-page-follow")
                    .h(px(22.0))
                    .px(px(7.0))
                    .gap(px(4.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(10.0))
                    .font_weight(if follow {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(rgb(if follow { ds.text } else { ds.muted }))
                    .tooltip(|window, cx| {
                        Tooltip::new("Cambiar de página automáticamente siguiendo el cabezal")
                            .build(window, cx)
                    })
                    .when(follow, |el| el.child(led_dot(ds.cyan, 4.5, true)))
                    .child("Seguir")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.pattern_editor.follow_playhead =
                                !this.pattern_editor.follow_playhead;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
        });

        // 3. Cápsula de modo de reproducción (Patrón · Canción · Timeline)
        let mode_capsule = drum_capsule(ds).children(
            [
                (
                    "drum-mode-pattern",
                    "Patrón",
                    "Reproduce el patrón activo en bucle",
                    DrumPlaybackSource::Pattern,
                ),
                (
                    "drum-mode-song",
                    "Canción",
                    "Abre y reproduce la secuencia de patrones",
                    DrumPlaybackSource::Song,
                ),
                (
                    "drum-mode-timeline",
                    "Timeline",
                    "Reproduce sincronizado con el Arrangement",
                    DrumPlaybackSource::Arrangement,
                ),
            ]
            .into_iter()
            .map(|(id, label, tip, source)| {
                let active = session.source == source;
                flat_control(ds, active, ds.accent)
                    .id(id)
                    .h(px(22.0))
                    .px(px(8.0))
                    .gap(px(5.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(10.5))
                    .font_weight(if active {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(rgb(if active { ds.text } else { ds.muted }))
                    .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
                    .when(active, |el| el.child(led_dot(ds.accent, 4.5, true)))
                    .child(label)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.apply_pattern_action(PatternAction::UseSource(source));
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
            }),
        );

        // 4. Cápsula de acciones del kit (+ Muestra · Volcar · Ajustes · Mixer · Pico)
        let kit_capsule = drum_capsule(ds)
            .child(
                flat_control(ds, false, ds.cyan)
                    .id("add-drum-sample")
                    .h(px(22.0))
                    .px(px(8.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(10.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(ds.cyan))
                    .tooltip(|window, cx| {
                        Tooltip::new("Añadir una muestra de audio al kit").build(window, cx)
                    })
                    .child("+ Muestra")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.choose_drum_sample(DrumSampleTarget::NewPiece, cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                flat_control(ds, false, ds.accent)
                    .id("send-drums-to-arrangement")
                    .h(px(22.0))
                    .px(px(8.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(10.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(ds.text))
                    .tooltip(|window, cx| {
                        Tooltip::new("Volcar las piezas al cursor del Arrangement")
                            .build(window, cx)
                    })
                    .child("Volcar")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.send_drums_to_arrangement();
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                flat_control(ds, editor.inspector_open, ds.accent)
                    .id("drum-inspector-toggle")
                    .h(px(22.0))
                    .px(px(8.0))
                    .gap(px(5.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(10.5))
                    .font_weight(if editor.inspector_open {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(rgb(if editor.inspector_open {
                        ds.text
                    } else {
                        ds.muted
                    }))
                    .tooltip(|window, cx| {
                        Tooltip::new("Ajustes y envolvente de la pieza seleccionada")
                            .build(window, cx)
                    })
                    .when(editor.inspector_open, |el| {
                        el.child(led_dot(ds.accent, 4.5, true))
                    })
                    .child("Ajustes")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.pattern_editor.inspector_open =
                                !this.pattern_editor.inspector_open;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                flat_control(ds, editor.mixer_open, ds.cyan)
                    .id("drums-mixer-toggle")
                    .h(px(22.0))
                    .px(px(8.0))
                    .gap(px(5.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(10.5))
                    .font_weight(if editor.mixer_open {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(rgb(if editor.mixer_open { ds.text } else { ds.muted }))
                    .tooltip(|window, cx| {
                        Tooltip::new("Mezclador interno del kit").build(window, cx)
                    })
                    .when(editor.mixer_open, |el| {
                        el.child(led_dot(ds.cyan, 4.5, true))
                    })
                    .child("Mixer")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.pattern_editor.mixer_open = !this.pattern_editor.mixer_open;
                            this.mixer_drag = None;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                flat_control(ds, false, ds.cyan)
                    .id("drum-save-kit")
                    .h(px(22.0))
                    .px(px(7.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(10.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(ds.muted))
                    .tooltip(|window, cx| {
                        Tooltip::new(
                            "Guardar kit completo (.haykit) con muestras, filtros y mezcla",
                        )
                        .build(window, cx)
                    })
                    .child("Guardar kit")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.save_drum_kit_dialog(cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                flat_control(ds, false, ds.cyan)
                    .id("drum-load-kit")
                    .h(px(22.0))
                    .px(px(7.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(10.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(ds.muted))
                    .tooltip(|window, cx| {
                        Tooltip::new("Cargar un kit guardado (.haykit)").build(window, cx)
                    })
                    .child("Cargar kit")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.load_drum_kit_dialog(cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .relative()
                    .id("drums-bus-peak")
                    .h(px(20.0))
                    .w(px(11.0))
                    .flex_shrink_0()
                    .tooltip(|window, cx| Tooltip::new("Pico del bus Drums").build(window, cx))
                    .child(track_peak_meter(ds, peak)),
            );

        // 0. Selector de superficie (Pasos · Pads MPC)
        let view_capsule = drum_capsule(ds).children(
            [
                (
                    "drum-view-steps",
                    "Pasos",
                    "Secuenciador por pasos clásico",
                    DrumViewMode::Steps,
                    ds.accent,
                ),
                (
                    "drum-view-pads",
                    "Pads MPC",
                    "Matriz 4×4 MPC con 16 Levels, Note Repeat, Overdub y Chop",
                    DrumViewMode::Pads,
                    ds.orange,
                ),
            ]
            .into_iter()
            .map(|(id, label, tip, mode, color)| {
                let active = !editor.mixer_open && editor.view_mode == mode;
                flat_control(ds, active, color)
                    .id(id)
                    .h(px(22.0))
                    .px(px(8.0))
                    .gap(px(5.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(10.5))
                    .font_weight(if active {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(rgb(if active { ds.text } else { ds.muted }))
                    .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
                    .when(active, |el| el.child(led_dot(color, 4.5, true)))
                    .child(label)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.pattern_editor.mixer_open = false;
                            this.pattern_editor.roll.piece = None;
                            this.pattern_editor.view_mode = mode;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
            }),
        );

        div()
            .id("drum-pattern-controls")
            .flex()
            .flex_wrap()
            .flex_shrink_0()
            .w_full()
            .min_w_0()
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .py(px(6.0))
            .bg(rgb(ds.bg))
            .border_b_1()
            .border_color(rgba(if ds.is_dark() { 0xffffff0d } else { 0x0f172a12 }))
            .child(view_capsule)
            .child(pattern_capsule)
            .when(!editor.mixer_open, |row| row.child(rhythm_capsule))
            .when_some(page_capsule, |row, cap| row.child(cap))
            .child(mode_capsule)
            .child(kit_capsule)
    }

    fn render_grid_picker_drawer(&self, cx: &mut Context<Self>) -> Div {
        let ds = &self.ds;
        let pattern = self.pattern_editor.session.active_pattern();
        div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .px(px(10.0))
            .py(px(5.0))
            .flex_shrink_0()
            .bg(rgb(ds.panel))
            .border_b_1()
            .border_color(rgba(if ds.is_dark() { 0xffffff0d } else { 0x0f172a12 }))
            .child(
                div()
                    .text_size(px(10.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(ds.muted))
                    .child("Subdivisión de pasos:"),
            )
            .children([8u16, 16, 32, 64].into_iter().map(|denominator| {
                let grid = NoteGrid::new(denominator).expect("subdivisión predefinida");
                let active = grid == pattern.grid;
                flat_control(ds, active, ds.cyan)
                    .h(px(22.0))
                    .px(px(9.0))
                    .gap(px(5.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(10.5))
                    .font_weight(if active {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(rgb(if active { ds.text } else { ds.muted }))
                    .when(active, |el| el.child(led_dot(ds.cyan, 4.5, true)))
                    .child(format!("1/{denominator} · {}", grid_note_name(denominator)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.apply_pattern_action(PatternAction::Grid(grid));
                            this.pattern_editor.options = None;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
            }))
    }

    fn drum_action_chip(
        &self,
        label: impl Into<SharedString>,
        active: bool,
        action: PatternAction,
        cx: &mut Context<Self>,
    ) -> Div {
        flat_control(&self.ds, active, self.ds.accent)
            .h(px(22.0))
            .px(px(8.0))
            .rounded(px(self.ds.radius_sm))
            .text_size(px(10.5))
            .font_weight(if active {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::MEDIUM
            })
            .text_color(rgb(if active { self.ds.text } else { self.ds.muted }))
            .child(label.into())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.apply_pattern_action(action.clone());
                    cx.notify();
                }),
            )
    }

    pub(in crate::ui) fn render_drum_lane_menu(
        &self,
        window: &gpui::Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        use crate::ui::components::context_menu::{
            context_menu, menu_panel, menu_row, menu_separator,
        };
        let ds = &self.ds;
        let menu = self.pattern_editor.lane_menu.expect("menú de fila abierto");
        let lane = menu.lane;
        let piece_name = self
            .pattern_editor
            .session
            .kit
            .pieces
            .get(lane)
            .map(|piece| piece.name.clone())
            .unwrap_or_else(|| "Fila".into());
        let has_copied = self.pattern_editor.copied_lane.is_some();

        let action_row = |label: &'static str, action: PatternAction, cx: &mut Context<Self>| {
            menu_row(ds, label, false, false)
                .hover(|row| row.bg(rgb(ds.selection_background())))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.apply_pattern_action(action.clone());
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
        };

        let section = |title: &'static str| {
            div()
                .px_2()
                .pt(px(4.0))
                .pb(px(2.0))
                .text_size(px(ds.font_size_xs - 1.0))
                .text_color(rgb(ds.accent))
                .child(title)
        };

        let mut panel = menu_panel(ds, 248.0)
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .px_2()
                    .py(px(3.0))
                    .text_xs()
                    .text_color(rgb(ds.text))
                    .truncate()
                    .child(piece_name),
            )
            .child(menu_separator(ds));

        if let Some(step) = menu.step {
            panel = panel
                .child(action_row(
                    "Dividir golpe (Roll)",
                    PatternAction::SplitStep { lane, step },
                    cx,
                ))
                .child(menu_separator(ds));
        }

        panel = panel
            .child(section("Rellenar (Fill)"))
            .child(action_row(
                "Rellenar cada 2 pasos",
                PatternAction::FillLane { lane, every: 2 },
                cx,
            ))
            .child(action_row(
                "Rellenar cada 4 pasos",
                PatternAction::FillLane { lane, every: 4 },
                cx,
            ))
            .child(action_row(
                "Rellenar cada 8 pasos",
                PatternAction::FillLane { lane, every: 8 },
                cx,
            ))
            .child(action_row(
                "Rellenar cada 16 pasos",
                PatternAction::FillLane { lane, every: 16 },
                cx,
            ))
            .child(menu_separator(ds))
            .child(section("Clonar y MIDI"))
            .child(action_row(
                "Shadow Clone (solo MIDI)",
                PatternAction::CloneLane {
                    lane,
                    keep_layers: false,
                    copy_midi: true,
                },
                cx,
            ))
            .child(action_row(
                "Clone (pieza + patrón)",
                PatternAction::CloneLane {
                    lane,
                    keep_layers: true,
                    copy_midi: true,
                },
                cx,
            ))
            .child(action_row(
                "Clonar instrumento (vacío)",
                PatternAction::CloneLane {
                    lane,
                    keep_layers: true,
                    copy_midi: false,
                },
                cx,
            ))
            .child(action_row(
                "Copiar MIDI de la fila",
                PatternAction::CopyLaneMidi(lane),
                cx,
            ));

        if has_copied {
            panel = panel.child(action_row(
                "Pegar MIDI en esta fila",
                PatternAction::PasteLaneMidi(lane),
                cx,
            ));
        }

        let can_delete_pattern = self.pattern_editor.session.patterns.len() > 1;
        let active_pattern = self.pattern_editor.session.active;
        panel = panel
            .child(menu_separator(ds))
            .child(section("Edición de pasos"))
            .child(action_row(
                "Desplazar ← 1 paso",
                PatternAction::ShiftLane { lane, steps: -1 },
                cx,
            ))
            .child(action_row(
                "Desplazar → 1 paso",
                PatternAction::ShiftLane { lane, steps: 1 },
                cx,
            ))
            .child(action_row(
                "Invertir pasos",
                PatternAction::InvertLane(lane),
                cx,
            ))
            .child(action_row(
                "Vaciar fila",
                PatternAction::ClearLane(lane),
                cx,
            ))
            .child(menu_separator(ds))
            .child(action_row(
                "Duplicar patrón completo",
                PatternAction::Duplicate,
                cx,
            ))
            .child(action_row(
                "Vaciar patrón completo",
                PatternAction::ClearPattern,
                cx,
            ))
            .when(can_delete_pattern, |panel| {
                panel.child(action_row(
                    "Eliminar patrón actual",
                    PatternAction::DeletePattern(active_pattern),
                    cx,
                ))
            })
            .child(menu_separator(ds))
            .child(
                menu_row(ds, "Eliminar pieza del kit", false, false)
                    .text_color(rgb(ds.danger))
                    .hover({
                        let danger = ds.danger;
                        move |row| row.bg(rgba((danger << 8) | 0x26))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.apply_pattern_action(PatternAction::RemovePiece(lane));
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            );

        let close = std::rc::Rc::new(cx.listener(|this, _, _, cx| {
            this.pattern_editor.lane_menu = None;
            cx.notify();
        }));
        let viewport = window.viewport_size();
        let x = menu
            .position
            .x
            .min((viewport.width - px(256.0)).max(px(8.0)))
            .max(px(8.0));
        let y = menu
            .position
            .y
            .min((viewport.height - px(450.0)).max(px(8.0)))
            .max(px(8.0));
        context_menu(gpui::point(x, y), close, panel)
    }
}
