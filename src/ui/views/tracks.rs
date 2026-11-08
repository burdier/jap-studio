//! Headers del Arrangement configurables estilo REAPER TCP (con perillas rotatorias o barras).
use super::super::{
    DawApp,
    components::knob::{KnobSpec, rotary_knob},
};
use gpui::{AnyElement, Context, IntoElement, MouseButton, div, prelude::*, px, rgb};

pub(in crate::ui) const DEMO_LEVELS: [i32; 8] = [78, 65, 52, 48, 70, 58, 42, 55];
pub(in crate::ui) const DEMO_PANS: [i32; 8] = [50, 50, 30, 50, 50, 70, 50, 50];
pub(super) const TRACK_ADD_GUTTER_WIDTH: f32 = 24.0;
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
                [Some(Mute), Some(Solo), Some(Rec), None, None, None],
            ],
            TcpLayoutPreset::Compact => [
                [Some(Name), Some(Type), None, None, None, None],
                [Some(Mute), Some(Solo), Some(Rec), None, None, None],
                [None; MAX_COLS],
            ],
            TcpLayoutPreset::Mixer => [
                [Some(Name), Some(Type), None, None, None, None],
                [Some(Pan), Some(Vol), Some(Pct), None, None, None],
                [None; MAX_COLS],
            ],
            TcpLayoutPreset::Minimal => [
                [Some(Name), Some(Type), None, None, None, None],
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
        if (self.hidden_mask & widget.bit()) != 0 {
            return false;
        }
        if widget == TcpWidget::Add {
            return true;
        }
        self.rows.iter().flatten().any(|slot| *slot == Some(widget))
    }

    pub fn toggle_widget(&mut self, widget: TcpWidget) {
        if widget != TcpWidget::Name {
            self.hidden_mask ^= widget.bit();
        }
    }
}

impl DawApp {
    pub(super) fn render_track_header(
        &self,
        i: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let cfg = ds.track_header;
        let selected = i == self.cursor_row;
        let mut track_box = div()
            .w(px(ds.track_header_width))
            .h_full()
            .flex_shrink_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .justify_between()
            .px_3()
            .py(px(ds.padding_xs))
            .bg(rgb(if selected { ds.panel_hover } else { ds.panel }))
            .border_r_1()
            .border_color(rgb(ds.border))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.cursor_row = i;
                    this.sync_midi_focus();
                    cx.notify();
                }),
            );
        for row in cfg.rows {
            let active: Vec<TcpWidget> = row
                .into_iter()
                .flatten()
                .filter(|w| *w != TcpWidget::Add && cfg.is_visible(*w))
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
                let push_right =
                    !is_title_pair && idx == last_idx && idx > 0 && widget == TcpWidget::Type;
                row_el = row_el.child(self.render_tcp_widget(i, widget, push_right, cx));
            }
            track_box = track_box.child(row_el);
        }

        // `[track]+`: la caja del track a la izquierda y el botón `+` justo delante, pegado por fuera
        div()
            .w_full()
            .h(px(ds.track_header_height))
            .flex_shrink_0()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(rgb(ds.border))
            .child(track_box)
            .child(
                div()
                    .w(px(TRACK_ADD_GUTTER_WIDTH))
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(ds.panel_dark))
                    .when(cfg.is_visible(TcpWidget::Add), |gutter| {
                        gutter.child(
                            div()
                                .w(px(18.0))
                                .h(px(22.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(ds.radius_sm))
                                .bg(rgb(if selected { ds.panel_hover } else { ds.panel }))
                                .border_1()
                                .border_color(rgb(if selected { ds.accent } else { ds.border }))
                                .text_xs()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(rgb(ds.accent))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.open_palette_for_track(i);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                )
                                .child("+"),
                        )
                    }),
            )
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
        let color = [
            ds.accent, ds.success, ds.warning, ds.danger, ds.cyan, ds.orange, ds.pink, ds.accent,
        ][i];
        let has_audio = self
            .timeline
            .clip
            .as_ref()
            .is_some_and(|clip| clip.track == i);
        let style = ds.track_header.control_style();
        match widget {
            TcpWidget::Name => div()
                .text_xs()
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(rgb(color))
                .child(track.name)
                .into_any_element(),
            TcpWidget::Type => {
                let label = if track.instrument == Some(self.instrument.id()) {
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
                    TcpControlStyle::Bars => div()
                        .w(px(ds.pan_bar_width))
                        .h(px(ds.control_bar_height))
                        .bg(rgb(ds.panel_dark))
                        .rounded(px(ds.radius_sm))
                        .child(
                            div()
                                .w(px(pan_frac * ds.pan_bar_width))
                                .h_full()
                                .bg(rgb(color))
                                .rounded(px(ds.radius_sm)),
                        )
                        .into_any_element(),
                };
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .on_scroll(cx.listener(move |this, event: &gpui::ScrollEvent, _, cx| {
                        let delta = f32::from(event.delta.pixel_delta(px(24.0)).y);
                        let step = if delta > 0.0 { 5 } else { -5 };
                        this.track_pans[i] = (this.track_pans[i] + step).clamp(0, 100);
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, _, _, cx| {
                            this.track_pans[i] = 50;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .child(div().text_xs().text_color(rgb(ds.muted)).child("P"))
                    .child(control)
                    .into_any_element()
            }
            TcpWidget::Vol => {
                let vol_frac = if has_audio {
                    (self.gain / 2.0).clamp(0.0, 1.0)
                } else {
                    (self.track_levels[i] as f32 / 100.0).clamp(0.0, 1.0)
                };
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
                    TcpControlStyle::Bars => div()
                        .w(px(ds.vol_bar_width))
                        .h(px(ds.control_bar_height))
                        .bg(rgb(ds.panel_dark))
                        .rounded(px(ds.radius_sm))
                        .child(
                            div()
                                .w(px(vol_frac * ds.vol_bar_width))
                                .h_full()
                                .bg(rgb(ds.success))
                                .rounded(px(ds.radius_sm)),
                        )
                        .into_any_element(),
                };
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .on_scroll(cx.listener(move |this, event: &gpui::ScrollEvent, _, cx| {
                        let delta = f32::from(event.delta.pixel_delta(px(24.0)).y);
                        if this
                            .timeline
                            .clip
                            .as_ref()
                            .is_some_and(|clip| clip.track == i)
                        {
                            this.change_gain(delta.signum() * 0.05);
                        } else {
                            let step = if delta > 0.0 { 4 } else { -4 };
                            this.track_levels[i] = (this.track_levels[i] + step).clamp(0, 100);
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, _, _, cx| {
                            if this
                                .timeline
                                .clip
                                .as_ref()
                                .is_some_and(|clip| clip.track == i)
                            {
                                let reset_delta = 1.0 - this.gain;
                                this.change_gain(reset_delta);
                            } else {
                                this.track_levels[i] = DEMO_LEVELS[i];
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .child(div().text_xs().text_color(rgb(ds.muted)).child("V"))
                    .child(control)
                    .into_any_element()
            }
            TcpWidget::Pct => {
                let volume_percent = if has_audio {
                    (self.gain * 100.0).round() as i32
                } else {
                    self.track_levels[i]
                };
                div()
                    .text_xs()
                    .text_color(rgb(ds.muted))
                    .child(format!("{volume_percent}%"))
                    .into_any_element()
            }
            TcpWidget::Mute => div()
                .px_1()
                .rounded(px(ds.radius_sm))
                .bg(rgb(ds.panel_dark))
                .text_xs()
                .text_color(rgb(ds.danger))
                .child("M")
                .into_any_element(),
            TcpWidget::Solo => div()
                .px_1()
                .rounded(px(ds.radius_sm))
                .bg(rgb(ds.panel_dark))
                .text_xs()
                .text_color(rgb(ds.warning))
                .child("S")
                .into_any_element(),
            TcpWidget::Rec => div()
                .px_1()
                .rounded(px(ds.radius_sm))
                .bg(rgb(ds.panel_dark))
                .text_xs()
                .text_color(rgb(ds.muted))
                .child("R")
                .into_any_element(),
            TcpWidget::Add => {
                let mut el = div()
                    .px_1()
                    .rounded(px(ds.radius_sm))
                    .bg(rgb(ds.panel_dark))
                    .text_xs()
                    .text_color(rgb(ds.accent))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.open_palette_for_track(i);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .child("[+]");
                if push_right {
                    el = el.ml_auto();
                }
                el.into_any_element()
            }
        }
    }
}
