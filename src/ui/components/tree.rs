//! TreeView compartido: extraído del explorador de bibliotecas, sin conocer archivos ni FX.
use crate::ui::{DesignSystem, icons::render_svg_icon};
use gpui::{
    App, Div, ElementId, IntoElement, Stateful, UniformList, UniformListScrollHandle, Window, div,
    prelude::*, px, rgb, uniform_list,
};
use std::ops::Range;

pub const FOLDER: &[u8] = include_bytes!("../../../assets/icons/toolbar/folder.svg");
pub const FILE: &[u8] = include_bytes!("../../../assets/icons/toolbar/file-code.svg");
pub const AUDIO: &[u8] = include_bytes!("../../../assets/icons/toolbar/wave.svg");
pub const CLOSE: &[u8] = include_bytes!("../../../assets/icons/toolbar/x.svg");
pub const ADD: &[u8] = include_bytes!("../../../assets/icons/toolbar/folder-plus.svg");
pub const ROW_HEIGHT: f32 = 27.0;

#[derive(Clone, Copy, PartialEq)]
pub(in crate::ui) struct TreeStyle {
    pub panel: u32,
    pub border: u32,
    pub text: u32,
    pub muted: u32,
    pub accent: u32,
    pub selected: u32,
    pub hover: u32,
    pub radius: f32,
    pub font_size: f32,
}
impl From<&DesignSystem> for TreeStyle {
    fn from(ds: &DesignSystem) -> Self {
        Self {
            panel: ds.panel_dark,
            border: ds.border,
            text: ds.text,
            muted: ds.muted,
            accent: ds.accent,
            selected: ds.selection_background(),
            hover: ds.panel,
            radius: ds.radius_lg,
            font_size: ds.font_size_xs,
        }
    }
}
pub(in crate::ui) fn panel(id: impl Into<ElementId>, style: TreeStyle) -> Stateful<Div> {
    div()
        .id(id)
        .size_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(style.radius))
        .border_1()
        .border_color(rgb(style.border))
        .bg(rgb(style.panel))
        .text_color(rgb(style.text))
        .text_size(px(style.font_size))
}
pub(in crate::ui) fn header() -> Div {
    div()
        .h(px(34.0))
        .flex_shrink_0()
        .px_2()
        .flex()
        .items_center()
        .gap_2()
}
pub(in crate::ui) fn footer(style: TreeStyle) -> Div {
    div()
        .flex_shrink_0()
        .px_2()
        .py_2()
        .border_t_1()
        .border_color(rgb(style.border))
        .flex()
        .flex_col()
        .gap_1()
        .text_color(rgb(style.muted))
}
pub(in crate::ui) fn list<R: IntoElement>(
    id: impl Into<ElementId>,
    count: usize,
    scroll: &UniformListScrollHandle,
    render: impl Fn(Range<usize>, &mut Window, &mut App) -> Vec<R> + 'static,
) -> UniformList {
    uniform_list(id, count, render)
        .track_scroll(scroll)
        .flex_1()
        .min_h_0()
        .w_full()
}
pub(in crate::ui) struct TreeRow<'a> {
    pub depth: usize,
    pub name: &'a str,
    pub expanded: Option<bool>,
    pub selected: bool,
    pub icon: &'static [u8],
    pub color: u32,
    pub icon_color: u32,
    pub status: Option<&'a str>,
}
pub(in crate::ui) fn row(
    id: impl Into<ElementId>,
    style: TreeStyle,
    row: TreeRow<'_>,
) -> Stateful<Div> {
    let selected = row.selected;
    div()
        .id(id)
        .h(px(ROW_HEIGHT))
        .w_full()
        .px_2()
        .flex()
        .items_center()
        .overflow_hidden()
        .cursor_pointer()
        .text_color(rgb(row.color))
        .when(selected, |s| s.bg(rgb(style.selected)))
        .hover(move |s| {
            s.bg(rgb(if selected {
                style.selected
            } else {
                style.hover
            }))
        })
        .children((0..row.depth).map(|_| {
            div()
                .w(px(12.0))
                .h_full()
                .flex_shrink_0()
                .border_l_1()
                .border_color(rgb(style.border))
        }))
        .child(
            div()
                .w(px(15.0))
                .flex_shrink_0()
                .text_color(rgb(style.muted))
                .child(match row.expanded {
                    None => "",
                    Some(true) => "▾",
                    Some(false) => "▸",
                }),
        )
        .child(render_svg_icon(row.icon, 15.0, row.icon_color))
        .child(
            div()
                .ml_2()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(row.name.to_string()),
        )
        .when_some(row.status, |line, status| {
            line.child(
                div()
                    .ml_2()
                    .max_w(px(120.0))
                    .truncate()
                    .text_color(rgb(style.muted))
                    .child(status.to_string()),
            )
        })
}
