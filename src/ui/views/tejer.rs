mod selector;
use crate::ui::{
    DawApp, components::button::toggle_badge, explorer::ExplorerSampleDrag, fx::FxTarget,
    track_layout::TrackRow,
};
use elevate::musical::fx::{Band, FxEdit, FxId, FxNode, FxPlacement};
use gpui::{Context, Div, MouseButton, div, prelude::*, px, rgb};

pub(super) fn band_name(i: usize, count: usize) -> String {
    match (i, count) {
        (0, _) => "Sub".into(),
        (1, 3) => "Cuerpo".into(),
        (last, n) if last + 1 == n => "Ataque".into(),
        _ => format!("Banda {}", i + 1),
    }
}

impl DawApp {
    pub(super) fn split_track_band(&mut self, index: usize, chain: Option<FxId>) {
        let Some(track) = self.tracks.get(index) else {
            return;
        };
        let id = track.id;
        let bands = track.chain.bands();
        let selected = match chain {
            Some(id) => {
                let Some(b) = bands.iter().find(|b| b.chain == id) else {
                    return;
                };
                Some(b)
            }
            None => bands.last(),
        };
        let hz = selected.map_or(103, |b| {
            if b.low < 700 && b.high > 700 {
                700
            } else {
                ((f32::from(b.low.max(20)) * f32::from(b.high.min(20000))).sqrt()).round() as u16
            }
        });
        let placement = FxPlacement::Inside(selected.map(|b| b.chain));
        let split = FxNode::crossover(hz);
        let upper = split.children().and_then(|c| c.last()).map(|n| n.id);
        match self.edit_fx(id, FxEdit::Insert(split, placement)) {
            Ok(()) => {
                self.track_presentation.entry(id).or_default().z_folded = false;
                self.select_tejer_band(index, upper);
                self.refresh_track_layout();
            }
            Err(e) => self.status_message = e,
        }
    }

    pub(super) fn render_tejer_header(&self, row: TrackRow, cx: &mut Context<Self>) -> Div {
        let h = row.height / row.lanes as f32;
        let track = row.track;
        let color = self.ds.track_color(track);
        let mut view = div()
            .w_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .drag_over::<gpui::ExternalPaths>(move |s, _, _, _| {
                s.border_l_2().border_color(rgb(color))
            })
            .drag_over::<ExplorerSampleDrag>(move |s, _, _, _| {
                s.border_l_2().border_color(rgb(color))
            })
            .on_drop(
                cx.listener(move |this, paths: &gpui::ExternalPaths, win, cx| {
                    this.drop_arrangement_files(paths.paths(), Some(track), win, cx);
                }),
            )
            .on_drop(
                cx.listener(move |this, sample: &ExplorerSampleDrag, win, cx| {
                    this.drop_arrangement_files(
                        std::slice::from_ref(&sample.path),
                        Some(track),
                        win,
                        cx,
                    );
                }),
            )
            .child(self.render_track_header(row.track, row.height - row.bands as f32 * h, cx));
        if row.bands > 0 {
            for (i, band) in self
                .tejer_display_chain(&self.tracks[row.track])
                .bands()
                .into_iter()
                .enumerate()
            {
                view = view.child(self.render_band(row, i, band, h, cx));
            }
        }
        view
    }

    fn band_solo_button(&self, index: usize, band: FxId, cx: &mut Context<Self>) -> Div {
        let track = self.tracks[index].id;
        let active = self
            .track_presentation
            .get(&track)
            .is_some_and(|p| p.band_solo == Some(band));
        toggle_badge(&self.ds, "S", active, self.ds.warning).on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                let slot = &mut this.track_presentation.entry(track).or_default().band_solo;
                *slot = if *slot == Some(band) {
                    None
                } else {
                    Some(band)
                };
                this.select_tejer_band(index, Some(band));
                cx.stop_propagation();
                cx.notify();
            }),
        )
    }

    fn render_band(
        &self,
        row: TrackRow,
        i: usize,
        band: Band,
        h: f32,
        cx: &mut Context<Self>,
    ) -> Div {
        let ds = &self.ds;
        let index = row.track;
        let track = &self.tracks[index];
        let id = track.id;
        let selected = row.selected_band == Some(i);
        let range = if band.high == 24000 {
            format!("{} Hz ↑", band.low)
        } else {
            format!("{}–{} Hz", band.low, band.high)
        };
        let mut controls = div()
            .flex()
            .items_center()
            .gap_1()
            .child(self.band_solo_button(index, band.chain, cx));
        for (label, factor) in [("−", 0.9_f32), ("+", 1.1)] {
            controls = controls.child(toggle_badge(ds, label, false, ds.muted).on_mouse_down(
                MouseButton::Left,
                cx.listener(move |s, _, _, cx| {
                    if let Err(e) = s.edit_fx(
                        id,
                        FxEdit::Parameter(band.crossover, 0, (f32::from(band.hz) * factor).round()),
                    ) {
                        s.status_message = e;
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            ));
        }
        controls = controls
            .child(toggle_badge(ds, "+ Banda", false, ds.cyan).on_mouse_down(
                MouseButton::Left,
                cx.listener(move |s, _, _, cx| {
                    s.split_track_band(index, Some(band.chain));
                    cx.stop_propagation();
                    cx.notify();
                }),
            ))
            .child(toggle_badge(ds, "+ FX", false, ds.accent).on_mouse_down(
                MouseButton::Left,
                cx.listener(move |s, _, _, cx| {
                    s.open_fx_palette(FxTarget {
                        track: id,
                        node: Some(band.chain),
                    });
                    cx.stop_propagation();
                    cx.notify();
                }),
            ));
        div()
            .w_full()
            .h(px(h))
            .flex_shrink_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .justify_center()
            .pl_2()
            .bg(rgb(if selected {
                crate::ui::design::blend_color(ds.arrangement_header(), ds.accent, 18)
            } else {
                ds.arrangement_header()
            }))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |s, _, _, cx| {
                    s.select_tejer_band(index, Some(band.chain));
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .border_l_2()
            .border_color(rgb(ds.band_color(selected)))
            .child(div().text_xs().text_color(rgb(ds.text)).child(format!(
                "Z{}   {}",
                i + 1,
                range
            )))
            .child(controls)
    }
}
