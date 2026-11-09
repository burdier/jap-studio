//! Una vista por archivo. El comportamiento reutilizable vive en los controladores de ui/.
mod action_form;
mod arrangement;
mod chrome;
mod clip_menu;
mod empty_arrangement;
mod icon_picker;
mod icon_picker_browser;
mod instrument;
mod instrument_params;
mod mixer;
mod palette;
mod pattern;
mod piano_roll;
mod settings;
mod settings_appearance;
mod settings_export;
mod settings_shortcuts;
mod settings_tracks;
mod toolbar;
mod track_color_menu;
mod track_menu;
pub(super) mod tracks;
mod transport;
mod utility;
mod workspace;

use super::{CenterView, DawApp};
use gpui::{Context, IntoElement, div, prelude::*, px, rgb};

impl DawApp {
    pub(super) fn render_center(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let (label, help) = match self.center_view {
            CenterView::Arrangement => (
                "ARRANGEMENT",
                "Touchpad: 2 dedos desplazar · pellizcar zoom | Ctrl+rueda: zoom",
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
            .gap(px(4.0))
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
                    .rounded(px(ds.radius_lg))
                    .border_1()
                    .border_color(rgb(ds.border))
                    .child(div().text_xs().text_color(rgb(ds.accent)).child(label))
                    .child(div().text_xs().text_color(rgb(ds.muted)).child(help)),
            )
            .child(match self.center_view {
                CenterView::Arrangement => self.render_arrangement_workspace(cx).into_any_element(),
                CenterView::Instrument if self.instrument_detached() => div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .rounded(px(ds.radius_lg))
                    .border_1()
                    .border_color(rgb(ds.border))
                    .overflow_hidden()
                    .child(self.render_instrument_placeholder(cx))
                    .into_any_element(),
                CenterView::Instrument => div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .rounded(px(ds.radius_lg))
                    .border_1()
                    .border_color(rgb(ds.border))
                    .overflow_hidden()
                    .child(self.render_instrument(cx))
                    .into_any_element(),
                CenterView::Session => div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .rounded(px(ds.radius_lg))
                    .border_1()
                    .border_color(rgb(ds.border))
                    .overflow_hidden()
                    .child(self.render_pattern(cx))
                    .into_any_element(),
                CenterView::Mixer => div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .rounded(px(ds.radius_lg))
                    .border_1()
                    .border_color(rgb(ds.border))
                    .overflow_hidden()
                    .child(self.render_mixer())
                    .into_any_element(),
            })
    }
}
