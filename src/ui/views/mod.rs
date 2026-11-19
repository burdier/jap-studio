//! Una vista por archivo. El comportamiento reutilizable vive en los controladores de ui/.
mod action_form;
mod arrangement;
mod chrome;
mod clip_menu;
mod command_bar;
mod empty_arrangement;
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
mod settings;
mod settings_appearance;
mod settings_audio;
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
mod workspace_focus;

use super::{CenterView, DawApp};
use gpui::{Context, IntoElement, div, prelude::*, px, rgb};

impl DawApp {
    pub(super) fn render_center(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if let Some(expanded) = self.render_expanded_workspace(cx) {
            return expanded;
        }
        let ds = &self.ds;
        let (label, help) = match self.center_view {
            CenterView::Arrangement => ("ARRANGEMENT", ""),
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
            .bg(rgb(ds.gutter_color()))
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
                    .border_color(rgb(ds.frame_border()))
                    .child(div().text_xs().text_color(rgb(ds.accent)).child(label))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when(!help.is_empty(), |strip| {
                                strip.child(div().text_xs().text_color(rgb(ds.muted)).child(help))
                            })
                            .child(self.workspace_zoom_button(cx)),
                    ),
            )
            .child(match self.center_view {
                CenterView::Arrangement => self.render_arrangement_workspace(cx).into_any_element(),
                CenterView::Session => div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .rounded(px(ds.radius_lg))
                    .border_1()
                    .border_color(rgb(ds.frame_border()))
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
                    .border_color(rgb(ds.frame_border()))
                    .overflow_hidden()
                    .child(self.render_mixer())
                    .into_any_element(),
            })
            .into_any_element()
    }
}
