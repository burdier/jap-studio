//! Recorrido de señal de un contenedor FX. Las acciones editan el mismo árbol de la pista.
use super::{FxEvent, FxTarget, FxTree};
use crate::ui::components::slider::slider_bar;
use elevate::musical::{
    fx::{BranchMix, FxEdit, FxId, FxKind, FxNode},
    track::TrackId,
};
use gpui::{Context, Div, MouseButton, MouseDownEvent, div, prelude::*, px, rgb};
use std::rc::Rc;

fn path_to(nodes: &[FxNode], id: FxId, path: &mut Vec<FxId>) -> bool {
    for node in nodes {
        path.push(node.id);
        if node.id == id
            || node
                .children()
                .is_some_and(|children| path_to(children, id, path))
        {
            return true;
        }
        path.pop();
    }
    false
}

impl FxTree {
    pub(super) fn render_route(&self, target: FxTarget, cx: &mut Context<Self>) -> Div {
        let style = self.style;
        let mut path = Vec::new();
        if let Some(id) = target.node {
            path_to(self.chain.nodes(), id, &mut path);
            if self.chain.find(id).is_some_and(|n| n.children().is_none()) {
                path.pop();
            }
        }
        let container = path.last().and_then(|id| self.chain.find(*id));
        let group = FxTarget {
            track: target.track,
            node: container.map(|n| n.id),
        };
        let nodes = container
            .and_then(FxNode::children)
            .unwrap_or(self.chain.nodes());
        let parallel = matches!(
            container.map(|n| &n.kind),
            Some(FxKind::ParallelChain { .. })
        );
        let cut = match container.map(|n| &n.kind) {
            Some(FxKind::Crossover { hz, .. }) => Some(*hz),
            _ => None,
        };
        let fork = parallel || cut.is_some();
        let bypass = path
            .iter()
            .any(|id| self.chain.find(*id).is_some_and(|n| !n.enabled));
        let kind = if let Some(hz) = cut {
            format!("Bandas · {hz} Hz")
        } else if parallel {
            "Paralelo".into()
        } else {
            "Serie".into()
        };
        let name = container.map_or(self.name.as_str(), |n| n.name.as_str());
        let parent = FxTarget {
            track: target.track,
            node: path.iter().rev().nth(1).copied(),
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .px_2()
            .pb_2()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(container.is_some(), |line| {
                        line.child(self.route_button("↑ Volver", FxEvent::Reveal(parent), cx))
                    })
                    .child(div().min_w_0().flex_1().truncate().child(name.to_owned()))
                    .child(div().text_color(rgb(style.accent)).child(kind)),
            )
            .child(div().text_color(rgb(style.muted)).child(if bypass {
                "Bypass · la señal pasa directa"
            } else if cut.is_some() {
                "La entrada se divide por frecuencia"
            } else if parallel {
                "Cada rama recibe la misma entrada"
            } else {
                "La señal pasa de arriba abajo"
            }))
            .child(
                div()
                    .id("fx-signal-path")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .py_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_color(rgb(style.muted))
                                    .child("Entrada ↓")
                                    .when(
                                        group.node.is_none() && self.instrument.is_some(),
                                        |row| {
                                            row.child(
                                                div()
                                                    .id("fx-route-instrument")
                                                    .flex_1()
                                                    .min_w_0()
                                                    .truncate()
                                                    .cursor_pointer()
                                                    .text_color(rgb(style.accent))
                                                    .child(self.instrument_name.clone())
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(move |_, _, _, cx| {
                                                            cx.emit(FxEvent::Instrument(
                                                                target.track,
                                                            ));
                                                            cx.stop_propagation();
                                                        }),
                                                    ),
                                            )
                                        },
                                    ),
                            )
                            .when(nodes.is_empty(), |lane| {
                                lane.child(
                                    div()
                                        .pl_2()
                                        .text_color(rgb(style.muted))
                                        .child("│  Señal directa"),
                                )
                            })
                            .children(nodes.iter().enumerate().map(|(i, node)| {
                                let to = FxTarget {
                                    track: target.track,
                                    node: Some(node.id),
                                };
                                let label = if cut.is_some() {
                                    if i == 0 {
                                        "Graves".to_owned()
                                    } else {
                                        "Agudos".to_owned()
                                    }
                                } else if parallel {
                                    format!("Rama {}", i + 1)
                                } else {
                                    format!("↓ {}", i + 1)
                                };
                                let detail = match &node.kind {
                                    FxKind::SerialChain { children } if children.is_empty() => {
                                        "Señal directa".to_string()
                                    }
                                    FxKind::SerialChain { children } => children
                                        .iter()
                                        .map(|n| n.name.as_str())
                                        .collect::<Vec<_>>()
                                        .join(" → "),
                                    FxKind::ParallelChain { branches } => {
                                        format!("{} ramas en paralelo", branches.len())
                                    }
                                    FxKind::Crossover { hz, .. } => format!("División a {hz} Hz"),
                                    FxKind::Effect { .. } => String::new(),
                                };
                                let leaf = node.children().is_none();
                                let header = div()
                                    .id(("fx-route-node", node.id.0 as usize))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .px_2()
                                    .py_1()
                                    .border_l_2()
                                    .border_color(rgb(if bypass || !node.enabled {
                                        style.border
                                    } else {
                                        style.accent
                                    }))
                                    .when(fork, |row| row.border_r_2())
                                    .rounded(px(style.radius))
                                    .bg(rgb(if target.node == Some(node.id) {
                                        style.selected
                                    } else {
                                        style.hover
                                    }))
                                    .cursor_pointer()
                                    .hover(move |s| s.bg(rgb(style.selected)))
                                    .child(div().text_color(rgb(style.muted)).child(label))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .child(div().truncate().child(node.name.clone()))
                                            .when(!detail.is_empty(), |col| {
                                                col.child(
                                                    div()
                                                        .truncate()
                                                        .text_color(rgb(style.muted))
                                                        .child(detail),
                                                )
                                            }),
                                    )
                                    .when(!node.enabled, |row| row.child("Bypass"))
                                    .when(!leaf, |row| row.child("›"))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |_, event: &MouseDownEvent, _, cx| {
                                            cx.emit(FxEvent::Reveal(to));
                                            if leaf && event.click_count == 2 {
                                                cx.emit(FxEvent::Action(to, "fx.parameters"));
                                            }
                                            cx.stop_propagation();
                                        }),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        cx.listener(move |_, event: &MouseDownEvent, _, cx| {
                                            cx.emit(FxEvent::Menu(to, event.position));
                                            cx.stop_propagation();
                                        }),
                                    );
                                div().flex().flex_col().child(header).when(parallel, |row| {
                                    row.child(self.branch_controls(
                                        target.track,
                                        container.unwrap().id,
                                        node,
                                        cx,
                                    ))
                                })
                            }))
                            .child(div().text_color(rgb(style.muted)).child(
                                if fork && !nodes.is_empty() {
                                    "Σ  Suma → Salida"
                                } else {
                                    "↓ Salida"
                                },
                            )),
                    ),
            )
            .when(cut.is_none(), |panel| {
                panel.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_1()
                        .child(self.route_button("+ FX", FxEvent::Add(group), cx))
                        .child(self.route_button(
                            if parallel { "+ Rama" } else { "+ Serie" },
                            FxEvent::Action(group, "fx.serial"),
                            cx,
                        ))
                        .when(!parallel, |line| {
                            line.child(self.route_button(
                                "+ Paralelo",
                                FxEvent::Action(group, "fx.parallel"),
                                cx,
                            ))
                        }),
                )
            })
    }
    fn branch_controls(
        &self,
        track: TrackId,
        parallel: FxId,
        node: &FxNode,
        cx: &mut Context<Self>,
    ) -> Div {
        let style = self.style;
        let branch = node.id;
        let mix = node.branch_mix;
        let peak = 10f32.powf(f32::from(BranchMix::MAX) / 200.);
        let fraction = (mix.gain(false) / peak).sqrt();
        let value = if mix.db10 == BranchMix::MIN {
            "−∞ dB".to_owned()
        } else {
            format!("{:+.1} dB", f32::from(mix.db10) / 10.)
        };
        let set = Rc::new(cx.listener(move |this, &f: &f32, _, cx| {
            let db10 = if f <= 0. {
                BranchMix::MIN
            } else {
                ((200. * (peak * f * f).log10()).round() as i16)
                    .clamp(BranchMix::MIN + 1, BranchMix::MAX)
            };
            if this
                .chain
                .find(branch)
                .is_some_and(|b| b.branch_mix.db10 != db10)
            {
                cx.emit(FxEvent::Edit(
                    track,
                    FxEdit::BranchLevel(parallel, branch, db10),
                ));
            }
        }));
        let reset = Rc::new(cx.listener(move |_, _: &(), _, cx| {
            cx.emit(FxEvent::Edit(
                track,
                FxEdit::BranchLevel(parallel, branch, 0),
            ));
        }));
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .bg(rgb(style.hover))
            .child(div().text_color(rgb(style.muted)).child("Vol"))
            .child(div().min_w_0().flex_1().child(slider_bar(
                style,
                style.text,
                ("fx-branch-fader", branch.0 as usize).into(),
                fraction,
                set,
                reset,
            )))
            .child(div().w(px(64.)).font_family("monospace").child(value))
            .child(
                div()
                    .id(("fx-branch-solo", branch.0 as usize))
                    .px_2()
                    .rounded(px(style.radius))
                    .cursor_pointer()
                    .bg(rgb(if mix.solo { style.accent } else { style.panel }))
                    .text_color(rgb(if mix.solo { style.panel } else { style.text }))
                    .child("Solo")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |_, _, _, cx| {
                            cx.emit(FxEvent::Edit(track, FxEdit::BranchSolo(parallel, branch)));
                            cx.stop_propagation();
                        }),
                    ),
            )
    }

    fn route_button(
        &self,
        label: &'static str,
        event: FxEvent,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let style = self.style;
        div()
            .id(label)
            .px_2()
            .py_1()
            .rounded(px(style.radius))
            .bg(rgb(style.hover))
            .text_color(rgb(style.accent))
            .cursor_pointer()
            .hover(move |s| s.bg(rgb(style.selected)))
            .child(label)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |_, _, _, cx| {
                    cx.emit(event.clone());
                    cx.stop_propagation();
                }),
            )
    }
}
