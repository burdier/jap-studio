//! Headers del Arrangement configurables estilo REAPER TCP (con perillas rotatorias o barras).
mod mix_controls;
pub(in crate::ui) use mix_controls::TrackControlDrag;

use crate::ui::{
    DawApp,
    components::{button::toggle_badge, meter::track_peak_meter},
    design::blend_color,
};
use gpui::{
    AnyElement, Context, IntoElement, MouseButton, MouseDownEvent, div, prelude::*, px, rgb,
};

/// Nivel de fader (0–100) que equivale a ganancia 1.0x.
pub(in crate::ui) const TRACK_UNITY_LEVEL: i32 = elevate::musical::track::UNITY_LEVEL;
pub(in crate::ui) const DEMO_LEVELS: [i32; 8] = [78, 65, 52, 48, 70, 58, 42, 55];
pub(in crate::ui) const DEMO_PANS: [i32; 8] = [50, 50, 30, 50, 50, 70, 50, 50];
const MAX_ROWS: usize = 3;
const MAX_COLS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::ui) enum TcpControlStyle {
    Knobs,
    #[default]
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
    Midi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ui) enum TcpGroup {
    Identity,
    Mix,
    State,
    Fx,
}

impl TcpGroup {
    pub const ALL: [Self; 4] = [Self::Identity, Self::Mix, Self::State, Self::Fx];

    pub fn label(self) -> &'static str {
        match self {
            Self::Identity => "Identidad",
            Self::Mix => "Mezcla",
            Self::State => "Estado",
            Self::Fx => "Instrumentos y FX",
        }
    }
}

impl TcpWidget {
    pub const TOGGLEABLE: [Self; 9] = [
        Self::Type,
        Self::Pan,
        Self::Vol,
        Self::Pct,
        Self::Mute,
        Self::Solo,
        Self::Rec,
        Self::Add,
        Self::Midi,
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
            "midi" | "input" => Some(Self::Midi),
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
            Self::Midi => "MIDI",
        }
    }

    pub fn group(self) -> TcpGroup {
        match self {
            Self::Name | Self::Type => TcpGroup::Identity,
            Self::Pan | Self::Vol | Self::Pct => TcpGroup::Mix,
            Self::Mute | Self::Solo | Self::Rec | Self::Midi => TcpGroup::State,
            Self::Add => TcpGroup::Fx,
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
                [Some(Vol), Some(Pan), None, None, None, None],
                [
                    Some(Mute),
                    Some(Solo),
                    Some(Midi),
                    Some(Rec),
                    Some(Add),
                    None,
                ],
            ],
            TcpLayoutPreset::Compact => [
                [Some(Name), Some(Type), None, None, None, None],
                [
                    Some(Mute),
                    Some(Solo),
                    Some(Midi),
                    Some(Rec),
                    Some(Add),
                    None,
                ],
                [None; MAX_COLS],
            ],
            TcpLayoutPreset::Mixer => [
                [Some(Name), Some(Type), Some(Midi), Some(Add), None, None],
                [Some(Pan), Some(Vol), Some(Pct), None, None, None],
                [None; MAX_COLS],
            ],
            TcpLayoutPreset::Minimal => [
                [Some(Name), Some(Type), Some(Midi), Some(Add), None, None],
                [None; MAX_COLS],
                [None; MAX_COLS],
            ],
        };
        Self {
            rows,
            hidden_mask: 0,
            preset: Some(preset),
            control_style: TcpControlStyle::default(),
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
            control_style: TcpControlStyle::default(),
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
        let dragged = self
            .track_reorder_drag
            .is_some_and(|drag| drag.moved && drag.source == self.tracks[i].id);
        let hovered = self.timeline.hover_track() == Some(i);
        let drag_glow = hovered && self.timeline.hover_dragging();
        // El color de pista identifica la cabecera sin colorear todos sus controles.
        let header = ds.arrangement_header();
        let base = if selected {
            ds.arrangement_selection(header)
        } else if hovered {
            blend_color(header, ds.text, 8)
        } else {
            header
        };
        let background = if ds.track_color_override.get(i).copied().flatten().is_some() {
            blend_color(base, color, 10)
        } else {
            base
        };
        let stripe = if selected || drag_glow {
            color
        } else {
            blend_color(header, color, 62)
        };
        let mut track_box = div()
            .relative()
            .w_full()
            .h(px(height))
            .flex_shrink_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .justify_between()
            .pl(px(9.0))
            .pr(px(13.0))
            .when(self.tracks[i].parent.is_some(), |header| {
                header.pl(px(18.0))
            })
            .py(px(if height < 47.0 { 2.0 } else { 4.0 }))
            .bg(rgb(background))
            .border_b_1()
            .border_color(gpui::rgba((ds.text << 8) | 0x23))
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(if selected { 3.0 } else { 2.0 }))
                    .bg(rgb(stripe)),
            )
            .child(track_peak_meter(
                ds,
                self.player
                    .as_ref()
                    .map_or(0.0, |player| player.track_peak(self.tracks[i].id)),
            ))
            .when(dragged, |header| header.opacity(0.25))
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
                    if !event.modifiers.control && !event.modifiers.shift {
                        this.begin_track_reorder_drag(i, event.position);
                    }
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
        // La altura normal de 58 px admite las tres filas. Las pistas muy bajas
        // priorizan identidad y estados sin recortar botones.
        let rows = if height < 54.0 && cfg.preset() == Some(TcpLayoutPreset::Standard) {
            TrackHeaderConfig::from_preset(TcpLayoutPreset::Compact).rows
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
            let mut row_el = div()
                .h(px(if height < 36.0 { 13.0 } else { 15.0 }))
                .flex()
                .items_center()
                .min_w_0();
            row_el = if is_title_pair {
                row_el.justify_between()
            } else {
                row_el.gap(px(5.0))
            };
            let mut start = 0;
            while start < active.len() {
                let group = active[start].group();
                let mut end = start + 1;
                while end < active.len() && active[end].group() == group {
                    end += 1;
                }
                let mut group_el = div().flex().items_center().min_w_0().gap(px(2.0));
                if active[start..end].contains(&TcpWidget::Name) {
                    group_el = group_el.flex_1();
                }
                if !is_title_pair && end == active.len() && start > 0 && group == TcpGroup::Fx {
                    group_el = group_el.ml_auto();
                }
                for widget in &active[start..end] {
                    group_el = group_el.child(self.render_tcp_widget(i, *widget, cx));
                }
                row_el = row_el.child(group_el);
                start = end;
            }
            track_box = track_box.child(row_el);
        }
        track_box
    }

    fn render_tcp_widget(&self, i: usize, widget: TcpWidget, cx: &mut Context<Self>) -> AnyElement {
        let ds = &self.ds;
        let track = &self.tracks[i];
        let has_audio = self
            .timeline
            .clip
            .as_ref()
            .is_some_and(|clip| clip.track == i);
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
                        .truncate()
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(ds.text))
                        .child(self.track_title(i)),
                )
                .into_any_element(),
            TcpWidget::Type => {
                let label = if track.instrument.is_some_and(|id| self.is_vst_id(id)) {
                    self.vst_name(track.instrument.unwrap()).to_string()
                } else if track.instrument.is_some_and(|id| self.is_sampler_id(id)) {
                    let id = track.instrument.unwrap();
                    let name = if id == self.active_sampler_id {
                        self.sampler.name()
                    } else {
                        self.sampler_instances
                            .get(&id)
                            .map_or("Hay Sampler", |s| s.name())
                    };
                    format!("Sampler · {name}")
                } else if track.instrument.is_some_and(|id| self.is_piano_id(id)) {
                    let id = track.instrument.unwrap();
                    if id == self.instrument.id() {
                        self.instrument.active_name().to_string()
                    } else {
                        self.piano_instances[&id].active_name().to_string()
                    }
                } else if has_audio {
                    "Audio".to_string()
                } else {
                    track.kind.label().to_string()
                };
                let el = div()
                    .max_w(px(74.0))
                    .min_w_0()
                    .truncate()
                    .text_size(px(9.0))
                    .text_color(rgb(if track.instrument.is_some() {
                        blend_color(ds.muted, ds.accent, 45)
                    } else {
                        ds.muted
                    }))
                    .child(label);
                el.into_any_element()
            }
            TcpWidget::Pan => self.render_track_mix_control(i, true, cx),
            TcpWidget::Vol => self.render_track_mix_control(i, false, cx),
            TcpWidget::Pct => {
                let volume_percent = self.track_levels[i];
                div()
                    .text_xs()
                    .text_color(rgb(ds.muted))
                    .font_family("monospace")
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
            TcpWidget::Midi => toggle_badge(
                ds,
                "MIDI",
                self.midi_armed_tracks.contains(&track.id),
                ds.cyan,
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.toggle_track_midi(i);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .into_any_element(),
            TcpWidget::Add => {
                let add = toggle_badge(ds, "+", false, ds.accent).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.open_palette_for_track(i);
                        cx.stop_propagation();
                        cx.notify();
                    }),
                );
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(add)
                    .child(
                        toggle_badge(
                            ds,
                            "FX",
                            i == self.cursor_row && self.fx.width > 0.0,
                            ds.cyan,
                        )
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
                    )
                    .into_any_element()
            }
        }
    }
}
