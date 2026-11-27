//! Una vista por archivo. El comportamiento reutilizable vive en los controladores de ui/.
mod action_form;
mod arrangement;
mod arrangement_navigation;
mod chrome;
mod clip_menu;
mod command_bar;
mod convolver;
mod empty_arrangement;
mod export_dialog;
mod fx_analyzers;
mod general_note;
mod icon_picker;
mod icon_picker_browser;
mod instrument;
mod instrument_actions;
mod instrument_banks;
pub(super) mod instrument_params;
mod lua_component;
pub(super) mod menu_bar;
mod mixer;
mod palette;
mod pattern;
mod piano_roll;
mod sample_editor;
mod settings;
mod settings_appearance;
mod settings_audio;
mod settings_export;
mod settings_midi;
mod settings_shortcuts;
mod settings_tracks;
mod synth;
mod tejer;
mod toolbar;
mod track_color_menu;
mod track_menu;
pub(super) mod tracks;
mod transport;
mod utility;
mod workspace;
mod workspace_focus;

use super::{
    CenterView, DawApp,
    components::{
        button::{flat_control, led_dot},
        splitter::{SplitDrag, vertical_splitter},
    },
    toolbar::ToolbarAction,
};
use gpui::{
    Context, FontWeight, IntoElement, MouseButton, MouseDownEvent, component::tooltip::Tooltip,
    div, prelude::*, px, relative, rgb,
};

impl DawApp {
    pub(super) fn render_center(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if let Some(expanded) = self.render_expanded_workspace(cx) {
            return expanded;
        }
        let ds = &self.ds;
        let content = match self.center_view {
            CenterView::Arrangement | CenterView::Mixer => {
                self.render_arrangement_workspace(cx).into_any_element()
            }
            CenterView::Session => self.render_drum_workspace(cx).into_any_element(),
        };
        let explorer_open = self.explorer_visible();
        let fx_open = self.fx.width > 0.0;
        let roll_open = self.piano_roll_window.is_some()
            || (self.piano_roll.open && self.timeline.midi_clip.is_some());
        let active_panel = self.workspace_focus.active.label(self.center_view);
        let context_target = if self.fx.master_focus && fx_open {
            "Bus Master".to_string()
        } else if let Some(track) = self.tracks.get(self.cursor_row) {
            format!("Pista {}: {}", self.cursor_row + 1, track.name)
        } else {
            "Sin pistas".to_string()
        };
        let view_tab = |id: &'static str,
                        label: &'static str,
                        shortcut: &'static str,
                        view: CenterView,
                        cx: &mut Context<Self>| {
            let active = self.center_view == view;
            flat_control(ds, active, ds.accent)
                .id(id)
                .h(px(24.0))
                .px(px(10.0))
                .gap(px(5.0))
                .rounded(px(ds.radius_md))
                .tooltip(move |window, cx| {
                    Tooltip::new(format!("Cambiar a {label} ({shortcut})")).build(window, cx)
                })
                .when(active, |el| el.child(led_dot(ds.accent, 5.0, true)))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(if active {
                            FontWeight::BOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .text_color(rgb(if active { ds.text } else { ds.muted }))
                        .child(label),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .text_color(rgb(if active { ds.accent } else { ds.muted }))
                        .child(shortcut),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        this.set_center_view(view);
                        window.focus(&this.focus_handle, cx);
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
        };
        let panel_toggle = |id: &'static str,
                            label: &'static str,
                            tip: &'static str,
                            active: bool,
                            accent: u32| {
            flat_control(ds, active, accent)
                .id(id)
                .h(px(24.0))
                .px(px(9.0))
                .gap(px(5.0))
                .rounded(px(ds.radius_md))
                .text_size(px(10.5))
                .font_weight(if active {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(rgb(if active { ds.text } else { ds.muted }))
                .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
                .when(active, |el| el.child(led_dot(accent, 4.5, true)))
                .child(label)
        };
        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .bg(rgb(ds.gutter_color()))
            .child(
                // Barra continua sobre el chasis: pestañas y botones emergen como objetos físicos.
                div()
                    .h(px(ds.timeline_bar_height.max(30.0)))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px(px(6.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .flex_shrink_0()
                            .child(view_tab(
                                "view-tab-arrangement",
                                "Arrangement",
                                "F1",
                                CenterView::Arrangement,
                                cx,
                            ))
                            .child(view_tab(
                                "view-tab-drums",
                                "Drums",
                                "F2",
                                CenterView::Session,
                                cx,
                            ))
                            .child(view_tab(
                                "view-tab-mixer",
                                "Mixer",
                                "F3",
                                CenterView::Mixer,
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(6.0))
                            .text_size(px(11.0))
                            .child(led_dot(ds.accent, 6.0, true))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(ds.text))
                                    .child(active_panel),
                            )
                            .child(div().text_color(rgb(ds.muted)).child("›"))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(rgb(ds.muted))
                                    .child(context_target),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .flex_shrink_0()
                            .child(
                                panel_toggle(
                                    "panel-toggle-explorer",
                                    "Explorador",
                                    "Mostrar / ocultar explorador de bibliotecas (Ctrl+E)",
                                    explorer_open,
                                    ds.cyan,
                                )
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.toggle_explorer();
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                panel_toggle(
                                    "panel-toggle-fx",
                                    "Track FX",
                                    "Mostrar / ocultar cadena de efectos de la pista",
                                    fx_open,
                                    ds.cyan,
                                )
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        if fx_open {
                                            this.resize_fx(0.0);
                                        } else {
                                            this.show_fx();
                                        }
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                panel_toggle(
                                    "panel-toggle-piano-roll",
                                    "Piano Roll",
                                    "Abrir / cerrar editor Piano Roll",
                                    roll_open,
                                    ds.accent,
                                )
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, window, cx| {
                                        if roll_open {
                                            this.execute_toolbar_action(
                                                ToolbarAction::RollClose,
                                                window,
                                                cx,
                                            );
                                        } else {
                                            this.set_center_view(CenterView::Arrangement);
                                            this.fx.editor_track = None;
                                            this.execute_toolbar_action(
                                                ToolbarAction::OpenPianoRoll,
                                                window,
                                                cx,
                                            );
                                        }
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                panel_toggle(
                                    "panel-open-command",
                                    ": Cmd",
                                    "Abrir paleta de comandos (Ctrl+P o :)",
                                    self.command_open,
                                    ds.pink,
                                )
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, window, cx| {
                                        if this.command_open {
                                            this.close_command_bar();
                                        } else {
                                            this.open_command_bar();
                                        }
                                        window.focus(&this.focus_handle, cx);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(self.workspace_zoom_button(cx)),
                    ),
            )
            .child(self.workspace_with_explorer(cx).child(content))
            .into_any_element()
    }
}

impl DawApp {
    fn render_drum_workspace(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ratio = self.pattern_editor.workspace_ratio.unwrap_or(0.5);
        let dragging = matches!(self.split_drag, Some(SplitDrag::DrumWorkspace { .. }));
        div()
            .flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .child(
                self.render_pattern(cx)
                    .flex_basis(relative(ratio))
                    .flex_grow_0()
                    .flex_shrink_0(),
            )
            .child(vertical_splitter(&self.ds, dragging).on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.split_drag = Some(SplitDrag::DrumWorkspace {
                        start_x: f32::from(event.position.x),
                        start_ratio: ratio,
                    });
                    cx.stop_propagation();
                    cx.notify();
                }),
            ))
            .child(self.render_arrangement_workspace(cx))
    }
}
