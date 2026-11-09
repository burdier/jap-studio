//! Arrastre entre nodos: centro inserta dentro, bordes reordenan sin cambiar identidad.
use super::{FxEvent, FxTree};
use crate::ui::components::tree::{self, TreeRow, TreeStyle};
use elevate::musical::{
    fx::{FxEdit, FxId, FxPlacement},
    track::TrackId,
};
use gpui::{Context, Div, IntoElement, Render, Stateful, Window, div, prelude::*, px, rgb};
#[derive(Clone)]
pub(super) struct FxDrag {
    pub track: TrackId,
    pub node: FxId,
    pub label: String,
    pub style: TreeStyle,
}
impl Render for FxDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        tree::panel("fx-drag", self.style)
            .w(px(230.0))
            .h(px(tree::ROW_HEIGHT))
            .child(tree::row(
                "preview",
                self.style,
                TreeRow {
                    depth: 0,
                    name: &self.label,
                    expanded: None,
                    selected: true,
                    icon: tree::AUDIO,
                    color: self.style.text,
                    icon_color: self.style.text,
                    status: None,
                },
            ))
    }
}
impl FxTree {
    pub(super) fn accepts_drop(&self, drag: &FxDrag, target: Option<FxId>) -> bool {
        self.track == Some(drag.track)
            && self
                .chain
                .find(drag.node)
                .is_some_and(|node| target.is_none_or(|id| node.find(id).is_none()))
    }

    pub(super) fn drop_edge(
        &self,
        track: TrackId,
        id: FxId,
        before: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let accent = self.style.accent;
        let tree = cx.entity();
        div()
            .id((if before { "fx-before" } else { "fx-after" }, id.0 as usize))
            .absolute()
            .left_0()
            .right_0()
            .h(px(6.0))
            .when(before, |s| s.top_0())
            .when(!before, |s| s.bottom_0())
            .can_drop(move |value, _, cx| {
                value
                    .downcast_ref::<FxDrag>()
                    .is_some_and(|drag| tree.read(cx).accepts_drop(drag, Some(id)))
            })
            .drag_over::<FxDrag>(move |s, _, _, _| {
                if before {
                    s.border_t_1().border_color(rgb(accent))
                } else {
                    s.border_b_1().border_color(rgb(accent))
                }
            })
            .on_drop(cx.listener(move |_, drag: &FxDrag, _, cx| {
                if drag.track == track {
                    cx.emit(FxEvent::Edit(
                        track,
                        FxEdit::Move(
                            drag.node,
                            if before {
                                FxPlacement::Before(id)
                            } else {
                                FxPlacement::After(id)
                            },
                        ),
                    ));
                }
                cx.stop_propagation();
            }))
    }
}
