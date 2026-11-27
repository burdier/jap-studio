//! Superficie MPC 4×4: 16 pads con bancos A–D, Full Level, 16 Levels, Note Repeat, Overdub y Chop.
use crate::ui::{
    DawApp, DesignSystem,
    components::button::{flat_control, led_dot, panel_shadow},
    design::blend_color,
    explorer::ExplorerSampleDrag,
    pattern_editor::{
        DrumSampleTarget, MPC_BANK_COUNT, MPC_KEY_LABELS, MPC_PAD_COUNT, MPC_VISUAL_ROWS, MpcState,
        NoteRepeatRate, SixteenLevelsMode,
    },
};
use elevate::musical::pitch::note_name;
use gpui::{
    Context, Div, FontWeight, MouseButton, Stateful, component::tooltip::Tooltip, div, prelude::*,
    px, rgb, rgba,
};

fn mpc_capsule(ds: &DesignSystem) -> Div {
    div()
        .h(px(30.0))
        .px(px(5.0))
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

impl DawApp {
    pub(super) fn render_mpc_pads(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let ds = &self.ds;
        let editor = &self.pattern_editor;
        let mpc = &editor.mpc;
        let pieces = &editor.session.kit.pieces;
        let dark = ds.is_dark();

        // 1. Bancos A / B / C / D
        let tip_full_level = self.tr("mpc.full_level_tooltip");
        let tip_16_off = self.tr("mpc.16lvl_off_tooltip");
        let tip_16_vel = self.tr("mpc.16lvl_vel_tooltip");
        let tip_16_pitch = self.tr("mpc.16lvl_pitch_tooltip");
        let tip_overdub = self.tr("mpc.overdub_tooltip");
        let tip_chop = self.tr("mpc.chop_tooltip");
        let mut bank_cap = mpc_capsule(ds).child(
            div()
                .px(px(4.0))
                .text_size(px(9.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(ds.muted))
                .child(self.tr("mpc.bank")),
        );
        for bank in 0..MPC_BANK_COUNT {
            let active = mpc.bank == bank;
            let start = bank * MPC_PAD_COUNT;
            let has_pieces = start < pieces.len();
            let label = MpcState::bank_label(bank);
            let bank_tip = self.tr_fmt(
                "mpc.bank_tooltip",
                &[
                    ("bank", label),
                    ("start", &(start + 1).to_string()),
                    ("end", &(start + 16).to_string()),
                ],
            );
            bank_cap = bank_cap.child(
                flat_control(ds, active, ds.accent)
                    .id(("mpc-bank", bank))
                    .w(px(26.0))
                    .h(px(22.0))
                    .gap(px(3.0))
                    .rounded(px(ds.radius_sm))
                    .font_family("monospace")
                    .text_size(px(11.0))
                    .font_weight(if active {
                        FontWeight::BOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(rgb(if active {
                        ds.text
                    } else if has_pieces {
                        ds.accent
                    } else {
                        ds.muted
                    }))
                    .tooltip(move |window, cx| Tooltip::new(bank_tip.clone()).build(window, cx))
                    .when(has_pieces && !active, |el| {
                        el.child(led_dot(ds.accent, 3.5, false))
                    })
                    .child(label)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.pattern_editor.mpc.bank = bank;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            );
        }

        // 2. Full Level + 16 Levels
        let mut levels_cap = mpc_capsule(ds)
            .child(
                flat_control(ds, mpc.full_level, ds.warning)
                    .id("mpc-full-level")
                    .h(px(22.0))
                    .px(px(8.0))
                    .gap(px(5.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(10.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(if mpc.full_level { ds.text } else { ds.muted }))
                    .tooltip(move |window, cx| Tooltip::new(tip_full_level).build(window, cx))
                    .when(mpc.full_level, |el| {
                        el.child(led_dot(ds.warning, 4.5, true))
                    })
                    .child(self.tr("mpc.full_level"))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.pattern_editor.mpc.full_level =
                                !this.pattern_editor.mpc.full_level;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(div().w(px(1.0)).h(px(14.0)).mx(px(2.0)).bg(rgba(if dark {
                0xffffff14
            } else {
                0x0f172a18
            })))
            .child(
                div()
                    .px(px(3.0))
                    .text_size(px(9.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(ds.muted))
                    .child(self.tr("mpc.sixteen_levels")),
            );
        for mode in [
            SixteenLevelsMode::Off,
            SixteenLevelsMode::Velocity,
            SixteenLevelsMode::Pitch,
        ] {
            let active = mpc.sixteen_levels == mode;
            let color = if mode == SixteenLevelsMode::Off {
                ds.muted
            } else {
                ds.cyan
            };
            let mode_tip = match mode {
                SixteenLevelsMode::Off => tip_16_off,
                SixteenLevelsMode::Velocity => tip_16_vel,
                SixteenLevelsMode::Pitch => tip_16_pitch,
            };
            levels_cap = levels_cap.child(
                flat_control(ds, active && mode != SixteenLevelsMode::Off, color)
                    .id(("mpc-16lvl", mode as usize))
                    .h(px(22.0))
                    .px(px(7.0))
                    .gap(px(4.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(10.0))
                    .font_weight(if active {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(rgb(if active { ds.text } else { ds.muted }))
                    .tooltip(move |window, cx| Tooltip::new(mode_tip).build(window, cx))
                    .when(active && mode != SixteenLevelsMode::Off, |el| {
                        el.child(led_dot(ds.cyan, 4.5, true))
                    })
                    .child(mode.label())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.pattern_editor.mpc.sixteen_levels = mode;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            );
        }

        // 3. Note Repeat (sincronizado al reloj RT de audio)
        let mut repeat_cap = mpc_capsule(ds).child(
            div()
                .px(px(3.0))
                .text_size(px(9.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(if mpc.note_repeat != NoteRepeatRate::Off {
                    ds.orange
                } else {
                    ds.muted
                }))
                .child(self.tr("mpc.repeat")),
        );
        for rate in NoteRepeatRate::ALL {
            let active = mpc.note_repeat == rate;
            let lit = active && rate != NoteRepeatRate::Off;
            let rate_tip = self.tr_fmt("mpc.repeat_tooltip", &[("rate", rate.label())]);
            repeat_cap = repeat_cap.child(
                flat_control(ds, lit, ds.orange)
                    .id(("mpc-repeat", rate as usize))
                    .h(px(22.0))
                    .px(px(6.0))
                    .gap(px(3.0))
                    .rounded(px(ds.radius_sm))
                    .font_family("monospace")
                    .text_size(px(10.0))
                    .font_weight(if active {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(rgb(if active { ds.text } else { ds.muted }))
                    .tooltip(move |window, cx| Tooltip::new(rate_tip.clone()).build(window, cx))
                    .when(lit, |el| el.child(led_dot(ds.orange, 4.0, true)))
                    .child(rate.label())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.pattern_editor.mpc.note_repeat = rate;
                            if rate == NoteRepeatRate::Off {
                                this.drum_note_repeat.stop();
                            } else {
                                this.sync_mpc_note_repeat_bpm();
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            );
        }

        // 4. Overdub + Trocear sample (Chop)
        let overdub = mpc.overdub;
        let chop_cap = mpc_capsule(ds)
            .child(
                flat_control(ds, overdub, ds.danger)
                    .id("mpc-overdub")
                    .h(px(22.0))
                    .px(px(8.0))
                    .gap(px(5.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(10.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(if overdub { ds.danger } else { ds.muted }))
                    .tooltip(move |window, cx| Tooltip::new(tip_overdub).build(window, cx))
                    .child(led_dot(
                        if overdub { ds.danger } else { ds.muted },
                        5.0,
                        overdub,
                    ))
                    .child(self.tr("mpc.overdub"))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.pattern_editor.mpc.overdub = !this.pattern_editor.mpc.overdub;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(div().w(px(1.0)).h(px(14.0)).mx(px(2.0)).bg(rgba(if dark {
                0xffffff14
            } else {
                0x0f172a18
            })))
            .child(
                flat_control(ds, false, ds.cyan)
                    .id("mpc-chop-open")
                    .h(px(22.0))
                    .px(px(8.0))
                    .rounded(px(ds.radius_sm))
                    .text_size(px(10.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(ds.cyan))
                    .tooltip(move |window, cx| Tooltip::new(tip_chop).build(window, cx))
                    .child(self.tr("mpc.chop"))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.chop_sample_dialog(cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            );

        let control_bar = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.0))
            .px(px(12.0))
            .py(px(8.0))
            .flex_shrink_0()
            .bg(rgb(ds.panel_dark))
            .border_b_1()
            .border_color(rgba(if dark { 0xffffff0d } else { 0x0f172a12 }))
            .child(bank_cap)
            .child(levels_cap)
            .child(repeat_cap)
            .child(chop_cap);

        // Matriz 4×4 centrada de 16 Pads MPC
        let mut grid = div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .w_full()
            .max_w(px(760.0))
            .mx_auto()
            .p(px(14.0))
            .flex()
            .flex_col()
            .gap(px(10.0));

        for visual_row in MPC_VISUAL_ROWS {
            let mut row_el = div().flex_1().min_h(px(74.0)).flex().gap(px(10.0));
            for pad in visual_row {
                row_el = row_el.child(self.render_mpc_pad_tile(pad, cx));
            }
            grid = grid.child(row_el);
        }

        // Pie con guía de teclado rápida y pieza seleccionada
        let selected_lane = mpc.lane_for_pad(mpc.selected_pad);
        let no_choke = self.tr("mpc.no_choke");
        let selected_label = pieces.get(selected_lane).map_or_else(
            || self.tr("mpc.no_pad_loaded").into(),
            |p| {
                let choke = p
                    .choke_group
                    .map_or_else(|| no_choke.into(), |g| format!("Choke G{g}"));
                format!(
                    "Pad {}{:02}: {} · {} · {choke}",
                    MpcState::bank_label(mpc.bank),
                    mpc.selected_pad + 1,
                    p.name,
                    note_name(p.base_note())
                )
            },
        );
        let footer = div()
            .flex_shrink_0()
            .px(px(14.0))
            .py(px(6.0))
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(8.0))
            .bg(rgb(ds.panel))
            .border_t_1()
            .border_color(rgba(if dark { 0xffffff0d } else { 0x0f172a12 }))
            .font_family("monospace")
            .text_size(px(10.0))
            .child(div().text_color(rgb(ds.text)).child(selected_label))
            .child(
                div()
                    .text_color(rgb(ds.muted))
                    .child(self.tr("mpc.footer_keys")),
            );

        div()
            .id("drum-mpc-pads")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .bg(rgb(ds.bg))
            .child(control_bar)
            .child(
                div()
                    .id("drum-mpc-pads-scroll")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .overflow_y_scroll()
                    .child(grid),
            )
            .child(footer)
    }

    fn render_mpc_pad_tile(&self, pad: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let ds = &self.ds;
        let dark = ds.is_dark();
        let editor = &self.pattern_editor;
        let mpc = &editor.mpc;
        let bank_lbl = MpcState::bank_label(mpc.bank);
        let key_lbl = MPC_KEY_LABELS[pad];
        let midi_lbl = note_name(36 + pad as u8);
        let lane = mpc.lane_for_pad(pad);
        let direct_piece = editor.session.kit.pieces.get(lane);
        let resolved = editor.resolve_mpc_pad(pad, 112);
        let active_piece = resolved.and_then(|(l, _, _, _)| editor.session.kit.pieces.get(l));
        let loaded = active_piece.is_some();
        let is_selected = mpc.selected_pad == pad && mpc.sixteen_levels == SixteenLevelsMode::Off;
        let is_held = mpc.held.is_some_and(|h| h.pad == pad);
        let flash = mpc
            .pad_flash_alpha(pad)
            .max(if is_held { 0.85 } else { 0.0 });
        let peak = direct_piece.map_or(0.0, |p| self.mpc_piece_peak(p.id));

        // Colores por fila estilo MPC (abajo: acento cálido/naranja, medio: cyan/acento, arriba: verde/warning)
        let row_colors = [ds.accent, ds.orange, ds.cyan, ds.warning];
        let pad_color = match mpc.sixteen_levels {
            SixteenLevelsMode::Off => row_colors[(pad / 4) % row_colors.len()],
            SixteenLevelsMode::Velocity => ds.warning,
            SixteenLevelsMode::Pitch => ds.cyan,
        };

        let base_bg = if loaded {
            let idle = blend_color(ds.panel, pad_color, if is_selected { 22 } else { 10 });
            if flash > 0.01 {
                blend_color(idle, pad_color, (flash * 75.0).round() as u32)
            } else {
                idle
            }
        } else {
            ds.panel_dark
        };

        let border_col = if flash > 0.05 {
            pad_color
        } else if is_selected && loaded {
            blend_color(pad_color, ds.text, 24)
        } else if loaded {
            blend_color(ds.border, pad_color, 45)
        } else {
            ds.border
        };

        let empty_pad_lbl = self.tr("mpc.empty_pad");
        let no_base_lbl = self.tr("mpc.no_base_pad");
        let remove_pad_tip = self.tr("mpc.remove_pad_tooltip");
        let title = match mpc.sixteen_levels {
            SixteenLevelsMode::Off => direct_piece
                .map(|p| p.name.clone())
                .unwrap_or_else(|| empty_pad_lbl.into()),
            SixteenLevelsMode::Velocity => {
                let vel = (16 + (pad as u32 * 111) / 15).clamp(1, 127);
                active_piece.map_or_else(|| no_base_lbl.into(), |_| format!("Vel {vel}"))
            }
            SixteenLevelsMode::Pitch => {
                let semi = pad as i16 - 7;
                active_piece.map_or_else(|| no_base_lbl.into(), |_| format!("{semi:+} st"))
            }
        };

        let subtitle = match mpc.sixteen_levels {
            SixteenLevelsMode::Off => direct_piece.map(|p| {
                if let Some(g) = p.choke_group {
                    format!("G{g} · {midi_lbl}")
                } else {
                    midi_lbl.clone()
                }
            }),
            SixteenLevelsMode::Velocity | SixteenLevelsMode::Pitch => {
                active_piece.map(|p| p.name.clone())
            }
        };

        let cyan = ds.cyan;
        let show_remove = mpc.sixteen_levels == SixteenLevelsMode::Off && direct_piece.is_some();

        div()
            .id(("mpc-pad", pad))
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .rounded(px(10.0))
            .bg(rgb(base_bg))
            .border_2()
            .border_color(rgb(border_col))
            .shadow(panel_shadow(ds))
            .p(px(8.0))
            .flex()
            .flex_col()
            .justify_between()
            .cursor_pointer()
            .overflow_hidden()
            .drag_over::<ExplorerSampleDrag>(move |style, _, _, _| {
                style.border_2().border_color(rgb(cyan))
            })
            .on_drop(
                cx.listener(move |this, sample: &ExplorerSampleDrag, window, cx| {
                    this.drop_sample_on_mpc_pad(pad, sample.path.clone(), window, cx);
                }),
            )
            .on_drop(
                cx.listener(move |this, files: &gpui::ExternalPaths, window, cx| {
                    if let Some(path) = files.paths().first() {
                        this.drop_sample_on_mpc_pad(pad, path.clone(), window, cx);
                    }
                }),
            )
            // Barra inferior de nivel/pico del pad
            .when(loaded && (peak > 0.01 || flash > 0.05), |el| {
                let level = peak.max(flash * 0.9).clamp(0.0, 1.0);
                el.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(px(3.5))
                        .bg(rgba(
                            (pad_color << 8) | ((level * 235.0).round() as u32).min(0xff),
                        )),
                )
            })
            // Fila superior del pad: número + tecla + botón quitar
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(4.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.0))
                            .child(
                                div()
                                    .font_family("monospace")
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(if loaded { pad_color } else { ds.muted }))
                                    .child(format!("{bank_lbl}{:02}", pad + 1)),
                            )
                            .child(
                                div()
                                    .px(px(4.0))
                                    .py(px(1.0))
                                    .rounded(px(3.0))
                                    .bg(rgba(if dark { 0xffffff10 } else { 0x0f172a12 }))
                                    .font_family("monospace")
                                    .text_size(px(9.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(ds.muted))
                                    .child(key_lbl),
                            ),
                    )
                    .when(show_remove, |top| {
                        let danger = ds.danger;
                        top.child(
                            div()
                                .id(("mpc-pad-rm", pad))
                                .w(px(16.0))
                                .h(px(16.0))
                                .rounded(px(3.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(11.0))
                                .text_color(rgb(ds.muted))
                                .hover(move |s| {
                                    s.bg(rgba((danger << 8) | 0x26)).text_color(rgb(danger))
                                })
                                .tooltip(move |window, cx| {
                                    Tooltip::new(remove_pad_tip).build(window, cx)
                                })
                                .child("×")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.clear_mpc_pad(pad);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                        )
                    }),
            )
            // Centro: nombre del sample o valor 16 Levels
            .child(
                div()
                    .w_full()
                    .truncate()
                    .text_size(px(if loaded { 12.5 } else { 10.5 }))
                    .font_weight(if loaded {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(rgb(if loaded { ds.text } else { ds.muted }))
                    .child(title),
            )
            // Pie del pad: grupo choke / MIDI o nombre base en 16 Levels
            .child(
                div()
                    .w_full()
                    .truncate()
                    .font_family("monospace")
                    .text_size(px(9.5))
                    .text_color(rgb(ds.muted))
                    .child(subtitle.unwrap_or_else(|| midi_lbl.clone())),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    let has_piece = this.pattern_editor.resolve_mpc_pad(pad, 112).is_some();
                    if has_piece {
                        this.mpc_pad_press(pad, 114, None);
                    } else if this.pattern_editor.mpc.sixteen_levels == SixteenLevelsMode::Off {
                        this.pattern_editor.mpc.selected_pad = pad;
                        this.choose_drum_sample(DrumSampleTarget::NewPiece, cx);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.mpc_pad_release();
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _, _, cx| {
                    let lane = this.pattern_editor.mpc.lane_for_pad(pad);
                    if lane < this.pattern_editor.session.kit.pieces.len() {
                        this.pattern_editor.mpc.selected_pad = pad;
                        this.pattern_editor.cursor_lane = lane;
                        this.pattern_editor.inspector_open = true;
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
    }
}
