//! Lienzo 2D reutilizable para curvas parametrizadas, nodos arrastrables y palitos conectados.
//! Quien lo implementa aporta la forma de la curva; el componente gestiona ejes, pintura y gestos.
pub mod paint;
pub mod shapes;

use crate::ui::DesignSystem;
use gpui::{
    App, Bounds, DispatchPhase, ElementId, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollEvent, SharedString, Styled,
    Window, canvas, div, prelude::*, px, rgb,
};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Debug)]
pub struct CurvePath {
    pub points: Vec<(f32, f32)>,
    pub color: u32,
    pub stroke_px: f32,
    pub fill_alpha: u8,
}

pub(in crate::ui) fn spectrum_legend(
    ds: &DesignSystem,
    input: &'static str,
    output: SharedString,
    response: &'static str,
) -> gpui::Div {
    let key = |label: SharedString, color, height| {
        div()
            .flex()
            .items_center()
            .gap_1()
            .flex_shrink_0()
            .child(div().w(px(12.)).h(px(height)).bg(rgb(color)))
            .child(label)
    };
    div()
        .flex()
        .items_center()
        .gap_2()
        .text_xs()
        .text_color(rgb(ds.text))
        .flex_wrap()
        .child(key(input.into(), ds.muted, 2.))
        .child(key(output, ds.cyan, 6.))
        .child(key(response.into(), ds.accent, 2.))
}

#[derive(Clone, Debug)]
pub struct CurveAnchor {
    pub x: f32,
    pub y: f32,
    pub stem: bool,
    pub crosshair: bool,
    pub color: u32,
}

#[derive(Clone, Debug, Default)]
pub struct CurveModel {
    pub grid_x: Vec<f32>,
    pub grid_y: Vec<f32>,
    pub baseline: Option<f32>,
    /// Picos `[min, max]` normalizados (-1..1) de un audio, dibujados espejados al fondo.
    pub wave: Vec<[f32; 2]>,
    /// Fracción inicial `0..1` de la onda que no suena (se atenúa).
    pub wave_dim_until: f32,
    pub spectrum: Vec<CurvePath>,
    pub curves: Vec<CurvePath>,
    pub anchors: Vec<CurveAnchor>,
    pub header: SharedString,
    pub readout: SharedString,
    pub x_labels: Vec<SharedString>,
    pub height_px: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum CurveGesture {
    /// Arrastre o clic de un nodo normalizado `x, y` en `0.0..=1.0`.
    Move { x: f32, y: f32 },
    /// Rueda sobre el lienzo para ajustar el eje secundario o Q/tensión.
    Wheel { delta: f32, fine: bool },
    /// Doble clic para restaurar los valores por defecto del nodo.
    Reset,
    /// Fin del gesto (permite preparar recursos fuera de RT al soltar).
    Commit,
}

pub type GestureHandler = Rc<dyn Fn(&CurveGesture, &mut Window, &mut App)>;

#[derive(Clone, Copy)]
struct DragSession {
    last_pos: Point<Pixels>,
    x: f32,
    y: f32,
}

pub fn interactive_curve_box(
    id: ElementId,
    ds: &DesignSystem,
    model: CurveModel,
    on_gesture: GestureHandler,
) -> impl IntoElement + use<> {
    let element_id = id.clone();
    let state_id = id;

    let anchors = model.anchors.clone();
    let on_down = on_gesture.clone();
    let on_move = on_gesture.clone();
    let on_up = on_gesture.clone();
    let on_wheel = on_gesture;

    let ds_paint = ds.clone();
    let border = ds.frame_border();
    let height = if model.height_px > 0.0 {
        model.height_px
    } else {
        134.0
    };
    let header = model.header.clone();
    let readout = model.readout.clone();
    let x_labels = model.x_labels.clone();

    div()
        .id(element_id)
        .flex()
        .flex_col()
        .gap_1()
        .w_full()
        .p_2()
        .bg(rgb(ds.panel_dark))
        .border_1()
        .border_color(rgb(border))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .text_xs()
                .child(div().text_color(rgb(ds.muted)).child(header))
                .child(div().text_color(rgb(ds.cyan)).child(readout)),
        )
        .child(
            div()
                .w_full()
                .h(px(height))
                .cursor_pointer()
                .child(
                    canvas(
                        move |bounds, window, cx| {
                            let state = window.use_keyed_state(state_id.clone(), cx, |_, _| {
                                Rc::new(Cell::new(None::<DragSession>))
                            });
                            (bounds, state.read(cx).clone())
                        },
                        move |bounds, (_, session_cell), window, _| {
                            paint::paint_model(bounds, &ds_paint, &model, window);

                            let down_session = session_cell.clone();
                            let down_anchors = anchors.clone();
                            let on_down = on_down.clone();
                            window.on_mouse_event(
                                move |event: &MouseDownEvent, phase, window, cx| {
                                    if phase != DispatchPhase::Bubble
                                        || event.button != MouseButton::Left
                                        || !bounds.contains(&event.position)
                                    {
                                        return;
                                    }
                                    if event.click_count >= 2 {
                                        down_session.set(None);
                                        on_down(&CurveGesture::Reset, window, cx);
                                        return;
                                    }
                                    let (nx, ny) = normalized_point(bounds, event.position);
                                    let target = nearest_anchor(&down_anchors, nx, ny);
                                    let (x, y) = if event.modifiers.shift {
                                        (target.0, target.1)
                                    } else {
                                        (nx, ny)
                                    };
                                    down_session.set(Some(DragSession {
                                        last_pos: event.position,
                                        x,
                                        y,
                                    }));
                                    on_down(&CurveGesture::Move { x, y }, window, cx);
                                },
                            );

                            let move_session = session_cell.clone();
                            let on_move = on_move.clone();
                            window.on_mouse_event(
                                move |event: &MouseMoveEvent, phase, window, cx| {
                                    if phase != DispatchPhase::Capture {
                                        return;
                                    }
                                    let Some(mut session) = move_session.get() else {
                                        return;
                                    };
                                    if !event.dragging() {
                                        move_session.set(None);
                                        on_move(&CurveGesture::Commit, window, cx);
                                        return;
                                    }
                                    let w = f32::from(bounds.size.width).max(1.0);
                                    let h = f32::from(bounds.size.height).max(1.0);
                                    if event.modifiers.shift {
                                        let dx = f32::from(event.position.x - session.last_pos.x)
                                            / w
                                            * 0.15;
                                        let dy = f32::from(session.last_pos.y - event.position.y)
                                            / h
                                            * 0.15;
                                        session.x = (session.x + dx).clamp(0.0, 1.0);
                                        session.y = (session.y + dy).clamp(0.0, 1.0);
                                    } else {
                                        let (nx, ny) = normalized_point(bounds, event.position);
                                        session.x = nx;
                                        session.y = ny;
                                    }
                                    session.last_pos = event.position;
                                    move_session.set(Some(session));
                                    on_move(
                                        &CurveGesture::Move {
                                            x: session.x,
                                            y: session.y,
                                        },
                                        window,
                                        cx,
                                    );
                                },
                            );

                            let up_session = session_cell;
                            let on_up = on_up.clone();
                            window.on_mouse_event(
                                move |event: &MouseUpEvent, phase, window, cx| {
                                    if phase == DispatchPhase::Capture
                                        && event.button == MouseButton::Left
                                        && up_session.take().is_some()
                                    {
                                        on_up(&CurveGesture::Commit, window, cx);
                                    }
                                },
                            );
                        },
                    )
                    .size_full(),
                )
                .on_scroll(move |event: &ScrollEvent, window, cx| {
                    let delta = f32::from(event.delta.pixel_delta(px(20.0)).y);
                    if delta.abs() <= f32::EPSILON {
                        return;
                    }
                    on_wheel(
                        &CurveGesture::Wheel {
                            delta: delta.signum(),
                            fine: event.modifiers.shift,
                        },
                        window,
                        cx,
                    );
                }),
        )
        .when(!x_labels.is_empty(), |el| {
            el.child(
                div()
                    .flex()
                    .justify_between()
                    .text_xs()
                    .text_color(rgb(ds.muted))
                    .children(x_labels.into_iter().map(|s| div().child(s))),
            )
        })
}

fn normalized_point(bounds: Bounds<Pixels>, position: Point<Pixels>) -> (f32, f32) {
    let w = f32::from(bounds.size.width).max(1.0);
    let h = f32::from(bounds.size.height).max(1.0);
    let nx = (f32::from(position.x - bounds.left()) / w).clamp(0.0, 1.0);
    let ny = (1.0 - f32::from(position.y - bounds.top()) / h).clamp(0.0, 1.0);
    (nx, ny)
}

fn nearest_anchor(anchors: &[CurveAnchor], nx: f32, ny: f32) -> (f32, f32) {
    anchors
        .iter()
        .min_by(|a, b| {
            let da = (a.x - nx).powi(2) + (a.y - ny).powi(2);
            let db = (b.x - nx).powi(2) + (b.y - ny).powi(2);
            da.total_cmp(&db)
        })
        .map_or((nx, ny), |a| (a.x, a.y))
}
