use super::band_name;
use crate::ui::{
    DawApp,
    components::{
        button::flat_control,
        curve_overlay::{CurvePath, paint::paint_curve},
    },
    fx::FxTarget,
};
use elevate::musical::{
    fx::{Band, FxEdit, FxId, FxKind, FxNode},
    track::TrackId,
};
use gpui::{
    Bounds, Context, Div, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    canvas, div, fill, point, prelude::*, px, rgb, rgba, size,
};
use std::{cell::Cell, rc::Rc};

fn range(b: Band) -> String {
    if b.high == 24000 {
        format!("{} Hz ↑", b.low)
    } else {
        format!("{}–{} Hz", b.low, b.high)
    }
}

fn x_at(hz: u16) -> f32 {
    (f32::from(hz).clamp(20., 20000.) / 20.).log10() / 3.
}
fn hz_at(x: f32) -> u16 {
    (20. * 1000f32.powf(x.clamp(0., 1.))).round() as u16
}

// Sucesión de filtros complementarios que alimentan la rama, antes de sus efectos.
fn cuts(nodes: &[FxNode], target: FxId, path: &mut Vec<(FxId, u16, bool)>) -> bool {
    for n in nodes {
        if n.id == target {
            return true;
        }
        if let FxKind::Crossover { hz, branches } = &n.kind {
            for (i, child) in branches.iter().enumerate() {
                path.push((n.id, *hz, i == 1));
                if cuts(std::slice::from_ref(child), target, path) {
                    return true;
                }
                path.pop();
            }
        } else if let Some(children) = n.children()
            && cuts(children, target, path)
        {
            return true;
        }
    }
    false
}

impl DawApp {
    pub(in crate::ui) fn tejer_preview(
        &self,
        track: &elevate::musical::track::Track,
    ) -> Option<(FxId, u16)> {
        if let Some(draft) = self
            .fx
            .tejer
            .filter(|t| t.track == track.id)
            .and(self.fx.cut_draft)
        {
            return Some(draft);
        }
        #[cfg(feature = "p2p")]
        return self.peer_band_cut(track);
        #[cfg(not(feature = "p2p"))]
        None
    }

    pub(super) fn tejer_display_chain<'a>(
        &self,
        track: &'a elevate::musical::track::Track,
    ) -> std::borrow::Cow<'a, elevate::musical::fx::FxChain> {
        let mut chain = std::borrow::Cow::Borrowed(&track.chain);
        if let Some((cut, hz)) = self.tejer_preview(track) {
            let _ = chain
                .to_mut()
                .apply(FxEdit::Parameter(cut, 0, f32::from(hz)));
        }
        chain
    }

    pub(in crate::ui) fn select_tejer_band(&mut self, index: usize, band: Option<FxId>) {
        let Some(t) = self.tracks.get(index) else {
            return;
        };
        let id = t.id;
        let bands = t.chain.bands();
        let previous = self.fx.tejer.filter(|t| t.track == id).and_then(|t| t.node);
        let node = band
            .or(previous)
            .filter(|id| bands.iter().any(|b| b.chain == *id))
            .or_else(|| bands.first().map(|b| b.chain));
        self.select_track(index, false, false);
        self.timeline.sample_editor.silence();
        self.fx.editor_track = None;
        self.fx.cut_draft = None;
        self.fx.tejer = Some(FxTarget { track: id, node });
    }

    fn finish_tejer_cut(&mut self, track: TrackId, cx: &mut Context<Self>) {
        if let Some((id, hz)) = self.fx.cut_draft.take() {
            if self.fx.tejer.is_some_and(|t| t.track == track)
                && self
                    .tracks
                    .iter()
                    .find(|t| t.id == track)
                    .and_then(|t| t.chain.find(id))
                    .is_some_and(|n| matches!(n.kind,FxKind::Crossover{hz:old,..} if old!=hz))
                && let Err(error) = self.edit_fx(track, FxEdit::Parameter(id, 0, f32::from(hz)))
            {
                self.status_message = error;
            }
            cx.notify();
        }
    }

    pub(in crate::ui) fn render_tejer_selector(
        &self,
        index: usize,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        let track = &self.tracks[index];
        let id = track.id;
        let chain = self.tejer_display_chain(track);
        let bands = chain.bands();
        let selected = self
            .fx
            .tejer
            .and_then(|t| t.node)
            .and_then(|id| bands.iter().position(|b| b.chain == id))
            .unwrap_or(0);
        let active = bands.get(selected).copied();
        let spectrum = active
            .map(|b| {
                self.fx
                    .analyzers
                    .comparison(b.input_crossover, b.chain, &self.ds)
            })
            .unwrap_or_default();
        let name = if active.is_some() {
            band_name(selected, bands.len())
        } else {
            "Rango completo".into()
        };
        let ds = &self.ds;
        let accent = ds.accent;
        let grid = ds.grid;
        let panel = div()
            .id("tejer-editor")
            .size_full()
            .overflow_y_scroll()
            .p_3()
            .bg(rgb(ds.panel))
            .text_color(rgb(ds.text));
        let mut body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .child(format!("{}  /  Track FX", track.name)),
                    )
                    .child(
                        super::toggle_badge(&self.ds, "Cerrar", false, self.ds.text).on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|s, _, _, cx| {
                                s.fx.tejer = None;
                                s.fx.cut_draft = None;
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .child(div().text_lg().child("Tejer EQ"));
        let mut row = div().flex().flex_wrap().gap_1();
        let mut boundaries = Vec::new();
        let mut curves = Vec::new();
        let rate = self.settings.audio.saved.sample_rate.unwrap_or(48000) as f32;
        let warped = |hz: f32| (std::f32::consts::PI * (hz / rate).min(0.49)).tan();
        for (i, b) in bands.iter().copied().enumerate() {
            let chosen = i == selected;
            let color = ds.band_color(chosen);
            let mut path = Vec::new();
            cuts(chain.nodes(), b.chain, &mut path);
            for &(cut, hz, _) in &path {
                if !boundaries.iter().any(|&(other, _)| other == cut) {
                    boundaries.push((cut, hz));
                }
            }
            let points = (0..=120)
                .map(|n| {
                    let x = n as f32 / 120.;
                    let f = warped(f32::from(hz_at(x)));
                    let gain = path.iter().fold(1f32, |g, (_, hz, high)| {
                        let cutoff = warped(f32::from(*hz));
                        g * (if *high { f } else { cutoff }) / f.hypot(cutoff)
                    });
                    (x, 0.05 + 0.82 * gain)
                })
                .collect();
            curves.push(CurvePath {
                points,
                color,
                stroke_px: if chosen { 2. } else { 1. },
                fill_alpha: if chosen { 36 } else { 10 },
            });
            row = row.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        flat_control(ds, chosen, color)
                            .flex_col()
                            .items_start()
                            .px_2()
                            .py_1()
                            .text_color(rgb(ds.text))
                            .border_l_2()
                            .border_color(rgb(color))
                            .child(div().text_sm().child(band_name(i, bands.len())))
                            .child(div().text_xs().font_family("monospace").child(range(b)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |s, _, _, cx| {
                                    s.select_tejer_band(index, Some(b.chain));
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(self.band_solo_button(index, b.chain, cx)),
            );
        }
        if bands.is_empty() {
            curves.push(CurvePath {
                points: vec![(0., 0.87), (1., 0.87)],
                color: ds.cyan,
                stroke_px: 2.,
                fill_alpha: 18,
            });
        }
        boundaries.sort_by_key(|(_, hz)| *hz);
        let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let layout = bounds.clone();
        let read = bounds.clone();
        let markers = boundaries.clone();
        let moves = boundaries.clone();
        let hit_bands = bands.clone();
        let graph = div()
            .w_full()
            .h(px(148.))
            .bg(rgb(ds.panel_dark))
            .cursor_pointer()
            .child(
                canvas(
                    move |rect, _, _| layout.set(rect),
                    move |rect, _, window, _| {
                        let width = f32::from(rect.size.width);
                        let stripe = |x: f32, w: f32, c: u32| {
                            fill(
                                Bounds::new(
                                    point(rect.left() + px(x), rect.top()),
                                    size(px(w), rect.size.height),
                                ),
                                rgba(c),
                            )
                        };
                        for curve in &spectrum {
                            paint_curve(rect, curve, 0., window);
                        }
                        if let Some(b) = active {
                            window.paint_quad(stripe(
                                width * x_at(b.low),
                                width * (x_at(b.high) - x_at(b.low)),
                                (accent << 8) | 0x18,
                            ));
                        }
                        for n in 1..12 {
                            window.paint_quad(stripe(
                                width * n as f32 / 12.,
                                1.,
                                (grid << 8) | 0x90,
                            ));
                        }
                        for curve in &curves {
                            paint_curve(rect, curve, 0., window);
                        }
                        for (_, hz) in &markers {
                            let x = x_at(*hz) * width;
                            window.paint_quad(stripe(x, 1.5, (accent << 8) | 0xba));
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(rect.left() + px(x - 4.), rect.top() + px(5.)),
                                    size(px(9.), px(10.)),
                                ),
                                rgb(accent),
                            ));
                        }
                    },
                )
                .size_full(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |s, ev: &MouseDownEvent, _, cx| {
                    let rect = read.get();
                    let w = f32::from(rect.size.width).max(1.);
                    let x = f32::from(ev.position.x - rect.left()) / w;
                    if let Some(&(cut, hz)) = boundaries
                        .iter()
                        .min_by(|a, b| (x_at(a.1) - x).abs().total_cmp(&(x_at(b.1) - x).abs()))
                        .filter(|(_, hz)| (x_at(*hz) - x).abs() * w < 7.)
                    {
                        s.fx.cut_draft = Some((cut, hz));
                    } else if let Some(b) = hit_bands
                        .iter()
                        .find(|b| x >= x_at(b.low) && x <= x_at(b.high))
                    {
                        s.select_tejer_band(index, Some(b.chain));
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(move |s, ev: &MouseMoveEvent, _, cx| {
                if !ev.dragging() {
                    return;
                }
                let Some((cut, _)) = s.fx.cut_draft else {
                    return;
                };
                let Some(i) = moves.iter().position(|m| m.0 == cut) else {
                    return;
                };
                let lo = if i == 0 { 20 } else { moves[i - 1].1 + 1 };
                let hi = moves
                    .get(i + 1)
                    .map_or(20000, |(_, hz)| hz.saturating_sub(1));
                if lo > hi {
                    return;
                }
                let r = bounds.get();
                let hz =
                    hz_at(f32::from(ev.position.x - r.left()) / f32::from(r.size.width).max(1.))
                        .clamp(lo, hi);
                if s.fx.cut_draft != Some((cut, hz)) {
                    s.fx.cut_draft = Some((cut, hz));
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |s, _, _, cx| s.finish_tejer_cut(id, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(move |s, ev: &MouseUpEvent, _, cx| {
                    if ev.button == MouseButton::Left {
                        s.finish_tejer_cut(id, cx);
                    }
                }),
            );
        body = body
            .child(graph)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_xs()
                    .font_family("monospace")
                    .children(["20 Hz", "200 Hz", "2 kHz", "20 kHz"].map(|s| div().child(s))),
            )
            .when(active.is_some(), |el| {
                el.child(crate::ui::components::curve_overlay::spectrum_legend(
                    ds,
                    "Entrada a Tejer EQ",
                    format!("Salida de {name}").into(),
                    "Respuesta de la banda",
                ))
            })
            .child(row);
        if !bands.is_empty() {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(rgb(ds.muted))
                    .child("Arrastra un corte · S para aislar una banda"),
            );
        }
        let node = active.map(|b| b.chain);
        let mut controls = div().flex().gap_2().child(
            super::toggle_badge(&self.ds, "+ Banda", false, ds.cyan).on_mouse_down(
                MouseButton::Left,
                cx.listener(move |s, _, _, cx| {
                    s.split_track_band(index, node);
                    cx.notify();
                }),
            ),
        );
        if node.is_some() {
            controls = controls.child(
                super::toggle_badge(&self.ds, "+ FX", true, accent).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |s, _, _, cx| {
                        s.open_fx_palette(FxTarget { track: id, node });
                        cx.notify();
                    }),
                ),
            );
        }
        body = body.child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_sm()
                        .child(format!("{}  /  {}", name, track.name)),
                )
                .child(controls),
        );
        if let Some(n) = node.and_then(|id| chain.find(id)) {
            for child in n.children().unwrap_or_default() {
                body = body.child(
                    div()
                        .text_sm()
                        .text_color(rgb(ds.cyan))
                        .child(child.name.clone()),
                );
            }
        }
        panel.child(body)
    }
}
