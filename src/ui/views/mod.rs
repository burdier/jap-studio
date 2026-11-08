//! Una vista por archivo. El comportamiento reutilizable vive en los controladores de ui/.
mod arrangement;
mod chrome;
mod instrument;
mod instrument_params;
mod mixer;
mod palette;
mod pattern;
mod settings;
pub(super) mod tracks;
mod transport;

use super::{CenterView, DawApp};
use gpui::{Context, IntoElement, div, prelude::*, px, rgb};

impl DawApp {
    pub(super) fn render_center(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let (label, help) = match self.center_view {
            CenterView::Arrangement => (
                "ARRANGEMENT",
                "Click: cursor | Shift+wheel: scroll | Ctrl+wheel: zoom",
            ),
            CenterView::Instrument => ("INSTRUMENT", elevate::musical::controller::HINT),
            CenterView::Session => (
                "PATTERN / PROTOTIPO",
                "Click/Enter: nota | rueda: velocity | Space: play/stop",
            ),
            CenterView::Mixer => ("MIXER / MOCK", "Tab: cycle | 1/2/3/4: jump"),
        };
        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .bg(rgb(ds.bg))
            .child(
                div()
                    .h(px(ds.timeline_bar_height))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .bg(rgb(ds.panel))
                    .border_b_1()
                    .border_color(rgb(ds.border))
                    .child(div().text_xs().text_color(rgb(ds.accent)).child(label))
                    .child(div().text_xs().text_color(rgb(ds.muted)).child(help)),
            )
            .child(match self.center_view {
                CenterView::Arrangement => self.render_arrangement(cx).into_any_element(),
                CenterView::Instrument if self.instrument_detached() => {
                    self.render_instrument_placeholder(cx).into_any_element()
                }
                CenterView::Instrument => self.render_instrument(cx).into_any_element(),
                CenterView::Session => self.render_pattern(cx).into_any_element(),
                CenterView::Mixer => self.render_mixer().into_any_element(),
            })
    }
}
