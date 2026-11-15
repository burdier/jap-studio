use super::{FxEvent, FxTarget, FxTree, drag::FxDrag};
use crate::ui::{
    components::tree::{self, TreeRow},
    icons::render_svg_icon,
};
use elevate::musical::fx::{FxEdit, FxPlacement};
use gpui::{
    Context, Div, IntoElement, MouseButton, MouseDownEvent, Render, Stateful, Window, div,
    prelude::*, rgb,
};

impl Render for FxTree {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let style = self.style;
        let scroll = self
            .track
            .and_then(|track| self.views.get(&track))
            .map(|view| view.scroll.clone())
            .unwrap_or_default();
        tree::panel("track-fx-tree", style)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event, _, cx| this.handle_key(event, cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    window.focus(&this.focus, cx);
                    cx.emit(FxEvent::Focused);
                    cx.stop_propagation();
                }),
            )
            .child(
                tree::header()
                    .child(
                        div()
                            .flex_1()
                            .text_color(rgb(style.muted))
                            .child("TRACK FX"),
                    )
                    .child(
                        div()
                            .id("close-fx")
                            .p_1()
                            .cursor_pointer()
                            .hover(move |s| s.bg(rgb(style.selected)))
                            .child(render_svg_icon(tree::CLOSE, 13.0, style.muted))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|_, _, _, cx| {
                                    cx.emit(FxEvent::Close);
                                    cx.stop_propagation();
                                }),
                            ),
                    ),
            )
            .child(
                div().px_2().pb_2().child(
                    div()
                        .id("fx-add-button")
                        .px_2()
                        .py_1()
                        .flex()
                        .items_center()
                        .justify_between()
                        .border_1()
                        .border_color(rgb(style.border))
                        .bg(rgb(style.hover))
                        .cursor_pointer()
                        .hover(move |s| s.border_color(rgb(style.accent)).bg(rgb(style.selected)))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(gpui::px(6.0))
                                .child(render_svg_icon(tree::EFFECT, 13.0, style.accent))
                                .child(
                                    div()
                                        .text_color(rgb(style.text))
                                        .child("+ Add · Instrumento / Efecto FX"),
                                ),
                        )
                        .child(div().text_color(rgb(style.muted)).child("A"))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                if let Some(target) = this.target() {
                                    cx.emit(FxEvent::Add(target));
                                }
                            }),
                        ),
                ),
            )
            .when(!self.hilo_branches.is_empty(), |panel| {
                panel.child(
                    div()
                        .px_2()
                        .pb_2()
                        .children(self.hilo_branches.iter().map(|branch| {
                            let id = branch.id;
                            let muted = branch.muted;
                            div()
                                .id(format!("hilo-branch-{}", id.0))
                                .flex()
                                .items_center()
                                .gap_2()
                                .py_1()
                                .px_2()
                                .border_1()
                                .border_color(rgb(style.border))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .cursor_pointer()
                                        .text_color(rgb(style.text))
                                        .child(branch.name.clone())
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |_, _, _, cx| {
                                                cx.emit(FxEvent::BranchSelect(id));
                                                cx.stop_propagation();
                                            }),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_color(rgb(style.muted))
                                        .child(format!("{}%", branch.level)),
                                )
                                .child(
                                    div()
                                        .cursor_pointer()
                                        .text_color(rgb(style.accent))
                                        .child("−")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |_, _, _, cx| {
                                                cx.emit(FxEvent::BranchLevel(id, -5));
                                                cx.stop_propagation();
                                            }),
                                        ),
                                )
                                .child(
                                    div()
                                        .cursor_pointer()
                                        .text_color(rgb(style.accent))
                                        .child("+")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |_, _, _, cx| {
                                                cx.emit(FxEvent::BranchLevel(id, 5));
                                                cx.stop_propagation();
                                            }),
                                        ),
                                )
                                .child(
                                    div()
                                        .cursor_pointer()
                                        .text_color(rgb(if muted {
                                            style.muted
                                        } else {
                                            style.text
                                        }))
                                        .child(if muted { "OFF" } else { "ON" })
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |_, _, _, cx| {
                                                cx.emit(FxEvent::BranchMute(id));
                                                cx.stop_propagation();
                                            }),
                                        ),
                                )
                        })),
                )
            })
            .when(self.track.is_none(), |panel| {
                panel.child(
                    div()
                        .p_4()
                        .child("Selecciona una pista para administrar sus efectos."),
                )
            })
            .child(tree::list(
                "fx-nodes",
                self.rows.len(),
                &scroll,
                cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                    range.map(|i| this.render_row(i, cx)).collect()
                }),
            ))
            .child(
                tree::footer(style)
                    .child("Doble clic / Enter: parámetros · A: Add")
                    .child("B: bypass · Delete: quitar · F2: nombre")
                    .child("Arrastra: borde para ordenar, centro para entrar"),
            )
    }
}
impl FxTree {
    fn render_row(&self, index: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let row = &self.rows[index];
        let style = self.style;
        let track = self.track.unwrap();
        let view = &self.views[&track];
        let id = row.id;
        let is_instrument = row.instrument;
        let target = FxTarget { track, node: id };
        let expanded = row
            .container
            .then(|| id.map_or(!view.root_closed, |id| !view.collapsed.contains(&id)));
        let color = if row.enabled { style.text } else { style.muted };
        let icon_color = if !row.enabled {
            style.muted
        } else if row.instrument {
            style.accent
        } else {
            style.text
        };
        let mut line = tree::row(
            if row.instrument {
                usize::MAX
            } else {
                id.map_or(0, |id| id.0) as usize
            },
            style,
            TreeRow {
                depth: row.depth,
                name: &row.name,
                expanded,
                selected: if row.instrument {
                    view.instrument_selected
                } else {
                    !view.instrument_selected && view.selected == id
                },
                icon: if row.instrument {
                    tree::INSTRUMENT
                } else if row.container {
                    tree::FOLDER
                } else {
                    tree::EFFECT
                },
                color,
                icon_color,
                status: if row.instrument {
                    Some("Instrumento")
                } else if row.parallel {
                    Some("Paralelo")
                } else {
                    Some("FX")
                },
            },
        )
        .relative()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                if event.click_count <= 2 {
                    this.activate(index, event.click_count == 2, cx);
                }
            }),
        )
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                this.views.get_mut(&track).unwrap().selected = id;
                this.views.get_mut(&track).unwrap().instrument_selected = is_instrument;
                window.focus(&this.focus, cx);
                if is_instrument {
                    cx.emit(FxEvent::Instrument(track));
                } else {
                    cx.emit(FxEvent::Menu(target, event.position));
                }
                cx.stop_propagation();
                cx.notify();
            }),
        );
        if row.instrument {
            return line;
        }
        let place = if row.container {
            FxPlacement::Inside(id)
        } else {
            FxPlacement::After(id.unwrap())
        };
        let tree = cx.entity();
        line = line
            .can_drop(move |value, _, cx| {
                value
                    .downcast_ref::<FxDrag>()
                    .is_some_and(|drag| tree.read(cx).accepts_drop(drag, id))
            })
            .drag_over::<FxDrag>(move |s, _, _, _| s.bg(rgb(style.selected)))
            .on_drop(cx.listener(move |_, drag: &FxDrag, _, cx| {
                if drag.track == track {
                    cx.emit(FxEvent::Edit(track, FxEdit::Move(drag.node, place)));
                }
                cx.stop_propagation();
            }));
        if let Some(id) = id {
            let drag = FxDrag {
                track,
                node: id,
                label: row.name.clone(),
                style,
            };
            line = line
                .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
                .child(
                    div()
                        .id(("fx-bypass", id.0 as usize))
                        .ml_2()
                        .cursor_pointer()
                        .text_color(rgb(style.muted))
                        .hover(move |s| s.text_color(rgb(style.accent)))
                        .child(if row.enabled { "✓" } else { "Bypass" })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                window.focus(&this.focus, cx);
                                cx.emit(FxEvent::Focused);
                                cx.emit(FxEvent::Edit(track, FxEdit::Toggle(id)));
                                cx.stop_propagation();
                            }),
                        ),
                )
                .child(self.drop_edge(track, id, true, cx))
                .child(self.drop_edge(track, id, false, cx));
        }
        line
    }
}
