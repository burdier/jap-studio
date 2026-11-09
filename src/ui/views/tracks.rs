//! Headers del Arrangement configurables estilo REAPER TCP (con perillas rotatorias o barras).
use super::super::{
    DawApp,
    components::{
        button::{badge_tag, toggle_badge},
        knob::{KnobSpec, rotary_knob},
        meter::horizontal_bar,
    },
};
use gpui::{
    AnyElement, Context, IntoElement, MouseButton, MouseDownEvent, Pixels, Point, div, prelude::*,
    px, rgb,
};

/// Nivel de fader (0–100) que equivale a ganancia 1.0x.
pub(in crate::ui) const TRACK_UNITY_LEVEL: i32 = 75;
pub(in crate::ui) const DEMO_LEVELS: [i32; 8] = [78, 65, 52, 48, 70, 58, 42, 55];
pub(in crate::ui) const DEMO_PANS: [i32; 8] = [50, 50, 30, 50, 50, 70, 50, 50];
const MAX_ROWS: usize = 3;
const MAX_COLS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::ui) enum TcpControlStyle {
    #[default]
    Knobs,
    Bars,
}

impl TcpControlStyle {
    pub const ALL: [Self; 2] = [Self::Knobs, Self::Bars];

    pub fn from_token(token: &str) -> Self {
        match token.trim().to_ascii_lowercase().as_str() {
            "bars" | "barras" | "faders" | "sliders" => Self::Bars,
            _ => Self::Knobs,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Knobs => "◉ Perillas (Knobs)",
            Self::Bars => "▬ Barras",
        }
    }

    pub fn token(self) -> &'static str {
        match self {
            Self::Knobs => "knobs",
            Self::Bars => "bars",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ui) enum TcpWidget {
    Name,
    Type,
    Pan,
    Vol,
    Pct,
    Mute,
    Solo,
    Rec,
    Add,
}

impl TcpWidget {
    pub const TOGGLEABLE: [Self; 8] = [
        Self::Type,
        Self::Pan,
        Self::Vol,
        Self::Pct,
        Self::Mute,
        Self::Solo,
        Self::Rec,
        Self::Add,
    ];

    pub fn from_token(token: &str) -> Option<Self> {
        match token.trim().to_ascii_lowercase().as_str() {
            "name" | "title" | "nombre" => Some(Self::Name),
            "type" | "kind" | "inst" | "tipo" => Some(Self::Type),
            "pan" | "p" => Some(Self::Pan),
            "vol" | "volume" | "v" => Some(Self::Vol),
            "pct" | "percent" | "%" => Some(Self::Pct),
            "mute" | "m" => Some(Self::Mute),
            "solo" | "s" => Some(Self::Solo),
            "rec" | "record" | "r" => Some(Self::Rec),
            "add" | "fx" | "plus" | "+" => Some(Self::Add),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Nombre",
            Self::Type => "Tipo",
            Self::Pan => "Pan",
            Self::Vol => "Volumen",
            Self::Pct => "%",
            Self::Mute => "M",
            Self::Solo => "S",
            Self::Rec => "R",
            Self::Add => "[+]",
        }
    }

    fn bit(self) -> u16 {
        1 << (self as u16)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ui) enum TcpLayoutPreset {
    Standard,
    Compact,
    Mixer,
    Minimal,
}

impl TcpLayoutPreset {
    pub const ALL: [Self; 4] = [Self::Standard, Self::Compact, Self::Mixer, Self::Minimal];

    pub fn from_token(token: &str) -> Option<Self> {
        match token.trim().to_ascii_lowercase().as_str() {
            "standard" | "default" | "estandar" | "full" => Some(Self::Standard),
            "compact" | "compacto" | "buttons" => Some(Self::Compact),
            "mixer" | "meters" | "mezcla" => Some(Self::Mixer),
            "minimal" | "min" | "strip" => Some(Self::Minimal),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "Estándar (3 filas)",
            Self::Compact => "Compacto (M/S/R)",
            Self::Mixer => "Mezcla (Pan/Vol)",
            Self::Minimal => "Minimal (1 fila)",
        }
    }

    pub fn token(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Compact => "compact",
            Self::Mixer => "mixer",
            Self::Minimal => "minimal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ui) struct TrackHeaderConfig {
    rows: [[Option<TcpWidget>; MAX_COLS]; MAX_ROWS],
    hidden_mask: u16,
    preset: Option<TcpLayoutPreset>,
    control_style: TcpControlStyle,
}

impl Default for TrackHeaderConfig {
    fn default() -> Self {
        Self::from_preset(TcpLayoutPreset::Standard)
    }
}

impl TrackHeaderConfig {
    pub fn from_preset(preset: TcpLayoutPreset) -> Self {
        use TcpWidget::*;
        let rows = match preset {
            TcpLayoutPreset::Standard => [
                [Some(Name), Some(Type), None, None, None, None],
                [Some(Pan), Some(Vol), Some(Pct), None, None, None],
                [Some(Mute), Some(Solo), Some(Rec), Some(Add), None, None],
            ],
            TcpLayoutPreset::Compact => [
                [Some(Name), Some(Type), None, None, None, None],
                [Some(Mute), Some(Solo), Some(Rec), Some(Add), None, None],
                [None; MAX_COLS],
            ],
            TcpLayoutPreset::Mixer => [
                [Some(Name), Some(Type), Some(Add), None, None, None],
                [Some(Pan), Some(Vol), Some(Pct), None, None, None],
                [None; MAX_COLS],
            ],
            TcpLayoutPreset::Minimal => [
                [Some(Name), Some(Type), Some(Add), None, None, None],
                [None; MAX_COLS],
                [None; MAX_COLS],
            ],
        };
        Self {
            rows,
            hidden_mask: 0,
            preset: Some(preset),
            control_style: TcpControlStyle::Knobs,
        }
    }

    pub fn from_profile(layout: Option<&str>, hide: Option<&str>, style: Option<&str>) -> Self {
        let mut cfg = match layout {
            None | Some("") => Self::default(),
            Some(token) => {
                if let Some(preset) = TcpLayoutPreset::from_token(token) {
                    Self::from_preset(preset)
                } else {
                    Self::from_custom_rows(token)
                }
            }
        };
        if let Some(hide_list) = hide {
            for item in hide_list.split(',') {
                if let Some(w) = TcpWidget::from_token(item)
                    && w != TcpWidget::Name
                {
                    cfg.hidden_mask |= w.bit();
                }
            }
        }
        if let Some(style_token) = style {
            cfg.control_style = TcpControlStyle::from_token(style_token);
        }
        cfg
    }

    fn from_custom_rows(spec: &str) -> Self {
        let mut rows = [[None; MAX_COLS]; MAX_ROWS];
        let mut any = false;
        for (r_idx, row_str) in spec.split('|').take(MAX_ROWS).enumerate() {
            let mut col = 0;
            for item in row_str.split(',') {
                if col >= MAX_COLS {
                    break;
                }
                if let Some(widget) = TcpWidget::from_token(item) {
                    rows[r_idx][col] = Some(widget);
                    col += 1;
                    any = true;
                }
            }
        }
        if !any {
            return Self::default();
        }
        Self {
            rows,
            hidden_mask: 0,
            preset: None,
            control_style: TcpControlStyle::Knobs,
        }
    }

    pub fn preset(&self) -> Option<TcpLayoutPreset> {
        self.preset
    }

    pub fn control_style(&self) -> TcpControlStyle {
        self.control_style
    }

    pub fn set_control_style(&mut self, style: TcpControlStyle) {
        self.control_style = style;
    }

    pub fn set_preset_preserving_style(&mut self, preset: TcpLayoutPreset) {
        let style = self.control_style;
        let hidden = self.hidden_mask;
        *self = Self::from_preset(preset);
        self.control_style = style;
        self.hidden_mask = hidden;
    }

    pub fn is_visible(&self, widget: TcpWidget) -> bool {
        (self.hidden_mask & widget.bit()) == 0
            && self.rows.iter().flatten().any(|slot| *slot == Some(widget))
    }

    pub fn toggle_widget(&mut self, widget: TcpWidget) {
        if widget != TcpWidget::Name {
            self.hidden_mask ^= widget.bit();
        }
    }
}

impl DawApp {
    pub(super) fn begin_track_control_drag(
        &mut self,
        track: usize,
        pan: bool,
        position: Point<Pixels>,
    ) {
        self.cursor_row = track;
        let value = if pan {
            self.track_pans.get(track)
        } else {
            self.track_levels.get(track)
        };
        if let Some(value) = value {
            self.track_control_drag = Some((track, pan, position.y, *value, false));
        }
    }

    pub(in crate::ui) fn update_track_control_drag(
        &mut self,
        position: Point<Pixels>,
        dragging: bool,
    ) {
        let Some((track, pan, start_y, initial, _)) = self.track_control_drag else {
            return;
        };
        if !dragging {
            self.end_track_control_drag();
            return;
        }
        let delta = f32::from(start_y - position.y);
        let next = (initial + (delta * 0.8).round() as i32).clamp(0, 100);
        if pan {
            self.set_track_pan(track, next);
        } else if let Some(level) = self.track_levels.get_mut(track) {
            *level = next;
            self.apply_track_gains();
        }
        self.track_control_drag = Some((track, pan, start_y, initial, next != initial));
    }

    pub(in crate::ui) fn end_track_control_drag(&mut self) {
        let Some((track, pan, _, _, moved)) = self.track_control_drag.take() else {
            return;
        };
        if moved {
            return;
        }
        if pan {
            let value = self.track_pans.get(track).copied().unwrap_or(50);
            self.set_track_pan(track, (value + 5).clamp(0, 100));
        } else if let Some(level) = self.track_levels.get_mut(track) {
            *level = (*level + 4).clamp(0, 100);
            self.apply_track_gains();
        }
    }

    pub(super) fn render_track_header(
        &self,
        i: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let cfg = ds.track_header;
        let height = self.track_height(i);
        let color = ds.track_color(i);
        let selected = self.track_selected(i);
        let hovered = self.timeline.hover_track() == Some(i);
        let drag_glow = hovered && self.timeline.hover_dragging();
        // Mezcla sutil para conservar el panel oscuro y la legibilidad del encabezado.
        let track_tint = if self
            .ds
            .track_color_override
            .get(i)
            .copied()
            .flatten()
            .is_some()
        {
            let base = if selected {
                ds.selection_background()
            } else if hovered {
                ds.panel_hover
            } else {
                ds.panel
            };
            let tint =
                |shift: u32| ((base >> shift) & 0xff) * 3 / 4 + ((color >> shift) & 0xff) / 4;
            Some((tint(16) << 16) | (tint(8) << 8) | tint(0))
        } else {
            None
        };
        let border_col = if selected || drag_glow {
            color
        } else {
            ds.border
        };
        let mut track_box = div()
            .w_full()
            .h(px(height))
            .flex_shrink_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .justify_between()
            .px_3()
            .when(self.tracks[i].parent.is_some(), |header| {
                header.pl(px(24.0))
            })
            .py(px(ds.padding_xs))
            .bg(rgb(track_tint.unwrap_or(if selected {
                ds.selection_background()
            } else if hovered {
                ds.panel_hover
            } else {
                ds.panel
            })))
            .border_b_1()
            .border_l_2()
            .border_color(rgb(border_col))
            .drag_over::<gpui::ExternalPaths>(move |style, _, _, _| {
                style
                    .bg(gpui::rgba((color << 8) | 0x2e))
                    .border_color(rgb(color))
            })
            .on_drop(
                cx.listener(move |this, paths: &gpui::ExternalPaths, _, cx| {
                    if let Some(path) = paths.paths().first() {
                        this.cursor_row = i;
                        this.import_audio(path.clone(), i, this.timeline.edit_beat(), cx);
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.select_track(i, event.modifiers.control, event.modifiers.shift);
                    window.focus(&this.focus_handle, cx);
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                    this.open_track_menu(i, event.position, window.viewport_size());
                    window.focus(&this.focus_handle, cx);
                    cx.stop_propagation();
                    cx.notify();
                }),
            );
        let rows = if height < 56.0 {
            [
                [
                    Some(TcpWidget::Name),
                    Some(TcpWidget::Mute),
                    Some(TcpWidget::Solo),
                    Some(TcpWidget::Add),
                    None,
                    None,
                ],
                [None; MAX_COLS],
                [None; MAX_COLS],
            ]
        } else if height < 72.0 {
            [
                [
                    Some(TcpWidget::Name),
                    Some(TcpWidget::Type),
                    None,
                    None,
                    None,
                    None,
                ],
                [
                    Some(TcpWidget::Mute),
                    Some(TcpWidget::Solo),
                    Some(TcpWidget::Rec),
                    Some(TcpWidget::Add),
                    None,
                    None,
                ],
                [None; MAX_COLS],
            ]
        } else {
            cfg.rows
        };
        for row in rows {
            let active: Vec<TcpWidget> = row
                .into_iter()
                .flatten()
                .filter(|w| {
                    cfg.is_visible(*w) && (*w != TcpWidget::Rec || self.tracks[i].accepts_clips())
                })
                .collect();
            if active.is_empty() {
                continue;
            }
            let is_title_pair = active.as_slice() == [TcpWidget::Name, TcpWidget::Type];
            let mut row_el = div().flex().items_center();
            row_el = if is_title_pair {
                row_el.justify_between()
            } else {
                row_el.gap_2()
            };
            let last_idx = active.len() - 1;
            for (idx, widget) in active.into_iter().enumerate() {
                let push_right = !is_title_pair
                    && idx == last_idx
                    && idx > 0
                    && matches!(widget, TcpWidget::Add | TcpWidget::Type);
                row_el = row_el.child(self.render_tcp_widget(i, widget, push_right, cx));
            }
            track_box = track_box.child(row_el);
        }
        track_box
    }

    fn render_tcp_widget(
        &self,
        i: usize,
        widget: TcpWidget,
        push_right: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ds = &self.ds;
        let track = &self.tracks[i];
        let color = ds.track_color(i);
        let has_audio = self
            .timeline
            .clip
            .as_ref()
            .is_some_and(|clip| clip.track == i);
        let style = ds.track_header.control_style();
        match widget {
            TcpWidget::Name => div()
                .flex()
                .items_center()
                .gap_1()
                .flex_1()
                .min_w_0()
                .when(
                    track.kind == elevate::musical::track::TrackKind::Group,
                    |el| {
                        el.child(
                            div()
                                .px_1()
                                .cursor_pointer()
                                .text_color(rgb(ds.text))
                                .child(if self.track_collapsed(i) {
                                    "▸"
                                } else {
                                    "▾"
                                })
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, window, cx| {
                                        this.toggle_track_fold(i);
                                        window.focus(&this.focus_handle, cx);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                        )
                    },
                )
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(if self.track_selected(i) {
                            ds.text
                        } else {
                            color
                        }))
                        .child(self.track_title(i)),
                )
                .into_any_element(),
            TcpWidget::Type => {
                let label = if track.instrument == Some(crate::instruments::sampler::SAMPLER_ID) {
                    format!("Sampler · {}", self.sampler.name())
                } else if track.instrument == Some(self.instrument.id()) {
                    self.instrument.active_name().to_string()
                } else if has_audio {
                    "Audio".to_string()
                } else {
                    track.kind.label().to_string()
                };
                let mut el = div()
                    .text_xs()
                    .text_color(rgb(if track.instrument.is_some() {
                        ds.accent
                    } else {
                        ds.muted
                    }))
                    .child(label);
                if push_right {
                    el = el.ml_auto();
                }
                el.into_any_element()
            }
            TcpWidget::Pan => {
                let pan_val = self.track_pans[i];
                let pan_frac = (pan_val as f32 / 100.0).clamp(0.0, 1.0);
                let control = match style {
                    TcpControlStyle::Knobs => rotary_knob(
                        ds,
                        KnobSpec {
                            fraction: pan_frac,
                            bipolar: true,
                            color,
                            diameter: ds.knob_size,
                        },
                    )
                    .into_any_element(),
                    TcpControlStyle::Bars => {
                        horizontal_bar(ds, pan_frac, ds.pan_bar_width, color).into_any_element()
                    }
                };
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .cursor_ns_resize()
                    .on_scroll(cx.listener(move |this, event: &gpui::ScrollEvent, _, cx| {
                        let delta = f32::from(event.delta.pixel_delta(px(24.0)).y);
                        let step = if delta > 0.0 { 5 } else { -5 };
                        let value = (this.track_pans[i] + step).clamp(0, 100);
                        this.set_track_pan(i, value);
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            if this.midi_mapper.learn_mode() {
                                this.arm_midi_learn(
                                    elevate::musical::midi_map::MidiBindTarget::TrackPan(i),
                                );
                                cx.stop_propagation();
                                cx.notify();
                            } else {
                                this.begin_track_control_drag(i, true, event.position);
                                cx.stop_propagation();
                            }
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, _, _, cx| {
                            this.set_track_pan(i, 50);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .child(div().text_xs().text_color(rgb(ds.muted)).child("P"))
                    .child(control)
                    .into_any_element()
            }
            TcpWidget::Vol => {
                let vol_frac = (self.track_levels[i] as f32 / 100.0).clamp(0.0, 1.0);
                let control = match style {
                    TcpControlStyle::Knobs => rotary_knob(
                        ds,
                        KnobSpec {
                            fraction: vol_frac,
                            bipolar: false,
                            color: ds.success,
                            diameter: ds.knob_size,
                        },
                    )
                    .into_any_element(),
                    TcpControlStyle::Bars => {
                        horizontal_bar(ds, vol_frac, ds.vol_bar_width, ds.success)
                            .into_any_element()
                    }
                };
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .cursor_ns_resize()
                    .on_scroll(cx.listener(move |this, event: &gpui::ScrollEvent, _, cx| {
                        let delta = f32::from(event.delta.pixel_delta(px(24.0)).y);
                        let step = if delta > 0.0 { 4 } else { -4 };
                        this.track_levels[i] = (this.track_levels[i] + step).clamp(0, 100);
                        this.apply_track_gains();
                        this.status_message =
                            format!("{}: {}%", this.tracks[i].name, this.track_levels[i]);
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            if this.midi_mapper.learn_mode() {
                                this.arm_midi_learn(
                                    elevate::musical::midi_map::MidiBindTarget::TrackVolume(i),
                                );
                                cx.stop_propagation();
                                cx.notify();
                            } else {
                                this.begin_track_control_drag(i, false, event.position);
                                cx.stop_propagation();
                            }
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, _, _, cx| {
                            this.track_levels[i] = TRACK_UNITY_LEVEL;
                            this.apply_track_gains();
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .child(div().text_xs().text_color(rgb(ds.muted)).child("V"))
                    .child(control)
                    .into_any_element()
            }
            TcpWidget::Pct => {
                let volume_percent = self.track_levels[i];
                div()
                    .text_xs()
                    .text_color(rgb(ds.muted))
                    .child(format!("{volume_percent}%"))
                    .into_any_element()
            }
            TcpWidget::Mute => toggle_badge(ds, "M", track.muted, ds.danger)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.toggle_track_mute(i);
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .into_any_element(),
            TcpWidget::Solo => toggle_badge(ds, "S", track.solo, ds.warning)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.toggle_track_solo(i);
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .into_any_element(),
            TcpWidget::Rec => {
                let recording = self.midi_capture.is_recording() && self.cursor_row == i;
                toggle_badge(ds, "R", recording, ds.danger)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.toggle_track_record(i);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .into_any_element()
            }
            TcpWidget::Add => {
                let add = badge_tag(ds, "[+]", ds.accent)
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.open_palette_for_track(i);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    );
                let mut el = div().flex().items_center().gap_1().child(add).child(
                    badge_tag(ds, "FX", ds.muted)
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.select_track(i, false, false);
                                this.show_fx();
                                window.focus(&this.fx.view.read(cx).focus.clone(), cx);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ),
                );
                if push_right {
                    el = el.ml_auto();
                }
                el.into_any_element()
            }
        }
    }
}
