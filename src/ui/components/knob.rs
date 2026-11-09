//! Perilla rotatoria (knob) nativa en GPU para cabeceras TCP e instrumentos.
//! Los arcos se tiñen con geometría vectorial continua: sin estelas de puntos.
use crate::ui::DesignSystem;
use gpui::{
    Bounds, ColorSpace, IntoElement, PathBuilder, Styled, canvas, fill, linear_color_stop,
    linear_gradient, point, px, rgb, rgba, size,
};

const START_ANGLE_DEG: f32 = 135.0;
const END_ANGLE_DEG: f32 = 405.0;

#[derive(Clone, Copy)]
pub(in crate::ui) struct KnobSpec {
    pub fraction: f32,
    pub bipolar: bool,
    pub color: u32,
    pub diameter: f32,
}

pub(in crate::ui) fn rotary_knob(ds: &DesignSystem, spec: KnobSpec) -> impl IntoElement + use<> {
    let ds = ds.clone();
    let fraction = spec.fraction.clamp(0.0, 1.0);
    let diameter = spec.diameter.max(14.0);
    canvas(
        move |_, _, _| (),
        move |bounds, (), window, _| {
            let cx = bounds.left() + bounds.size.width * 0.5;
            let cy = bounds.top() + bounds.size.height * 0.5;
            let r =
                (f32::from(bounds.size.width).min(f32::from(bounds.size.height)) * 0.5).max(7.0);
            let track_r = r - 1.8;
            let track_w = track_w(r);
            let body_r = (r - 4.4).max(4.4);
            let cap_r = (body_r - 0.9).max(3.2);
            let hub_r = (cap_r * 0.52).max(2.1);
            let needle_angle = angle_deg(fraction);
            let (active_from, active_to) = active_range(fraction, spec.bipolar);

            // 1. Riel exterior: trazo continuo suave.
            stroke_arc(
                window,
                cx,
                cy,
                track_r,
                START_ANGLE_DEG,
                END_ANGLE_DEG,
                track_w,
                rgba((ds.border << 8) | 0x66),
            );

            // 2. Arco activo con halo discreto; el remate marca el valor exacto.
            if active_to - active_from > f32::EPSILON {
                stroke_arc(
                    window,
                    cx,
                    cy,
                    track_r,
                    angle_deg(active_from),
                    angle_deg(active_to),
                    track_w * 1.8,
                    rgba((spec.color << 8) | 0x1d),
                );
                stroke_arc(
                    window,
                    cx,
                    cy,
                    track_r,
                    angle_deg(active_from),
                    angle_deg(active_to),
                    track_w,
                    rgba((spec.color << 8) | 0xe4),
                );
            }
            dot(
                window,
                point_at(cx, cy, track_r, needle_angle),
                track_w * 0.48,
                rgba((spec.color << 8) | 0xf2),
            );

            // 3. Cuerpo: sombra, bisel y tapa con menos contraste vertical.
            let bezel = linear_gradient(
                180.0,
                linear_color_stop(rgb(ds.panel_dark), 0.0),
                linear_color_stop(rgba((ds.border << 8) | 0x80), 1.0),
            )
            .color_space(ColorSpace::Oklab);
            let cap = linear_gradient(
                180.0,
                linear_color_stop(rgba((ds.panel_hover << 8) | 0xff), 0.0),
                linear_color_stop(rgba((ds.panel_dark << 8) | 0xff), 1.0),
            )
            .color_space(ColorSpace::Oklab);
            dot(
                window,
                point(cx, cy),
                body_r + 0.75,
                rgba((ds.bg << 8) | 0x3f),
            );
            dot(window, point(cx, cy), body_r, bezel);
            dot(window, point(cx, cy), cap_r, cap);

            // 4. Marcador: aguja desde el eje hasta el borde, con remate centrado.
            let tip_radius = (cap_r - 0.7).max(hub_r + 1.0);
            let tip = point_at(cx, cy, tip_radius, needle_angle);
            let inner = point_at(cx, cy, hub_r * 0.45, needle_angle);
            stroke_line(
                window,
                inner,
                tip,
                (track_w * 0.38).max(1.3),
                rgba((ds.text << 8) | 0xd9),
            );
            dot(window, point(cx, cy), hub_r, rgba((ds.bg << 8) | 0xaa));
            dot(
                window,
                point(cx, cy),
                hub_r * 0.48,
                rgba((ds.border << 8) | 0xd2),
            );
        },
    )
    .w(px(diameter))
    .h(px(diameter))
}

/// Grosor mínimo legible y proporcional al tamaño de la perilla.
fn track_w(r: f32) -> f32 {
    (r * 0.2).clamp(2.1, 2.75)
}

fn angle_deg(value: f32) -> f32 {
    START_ANGLE_DEG + value * (END_ANGLE_DEG - START_ANGLE_DEG)
}

/// Rango visible del arco: continuo en volumen y centrado en pan.
fn active_range(value: f32, bipolar: bool) -> (f32, f32) {
    if bipolar {
        (value.min(0.5), value.max(0.5))
    } else {
        (0.0, value)
    }
}

fn point_at(
    cx: gpui::Pixels,
    cy: gpui::Pixels,
    radius: f32,
    angle_deg: f32,
) -> gpui::Point<gpui::Pixels> {
    let rad = angle_deg.to_radians();
    point(cx + px(rad.cos() * radius), cy + px(rad.sin() * radius))
}

fn dot(
    window: &mut gpui::Window,
    center: gpui::Point<gpui::Pixels>,
    radius: f32,
    color: impl Into<gpui::Background>,
) {
    window.paint_quad(
        fill(
            Bounds::new(
                point(center.x - px(radius), center.y - px(radius)),
                size(px(radius * 2.0), px(radius * 2.0)),
            ),
            color,
        )
        .corner_radii(px(radius)),
    );
}

/// Dibuja el arco como geometría vectorial continua, en tramos de a 90°.
fn stroke_arc(
    window: &mut gpui::Window,
    cx: gpui::Pixels,
    cy: gpui::Pixels,
    radius: f32,
    start_deg: f32,
    end_deg: f32,
    width: f32,
    color: impl Into<gpui::Background>,
) {
    if end_deg <= start_deg {
        return;
    }
    let mut builder = PathBuilder::stroke(px(width));
    let mut cursor = start_deg;
    builder.move_to(point_at(cx, cy, radius, cursor));
    while end_deg > cursor {
        let next = (cursor + 90.0).min(end_deg);
        builder.arc_to(
            point(px(radius), px(radius)),
            px(0.0),
            false,
            true,
            point_at(cx, cy, radius, next),
        );
        cursor = next;
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn stroke_line(
    window: &mut gpui::Window,
    origin: gpui::Point<gpui::Pixels>,
    end: gpui::Point<gpui::Pixels>,
    width: f32,
    color: impl Into<gpui::Background>,
) {
    let mut builder = PathBuilder::stroke(px(width));
    builder.move_to(origin);
    builder.line_to(end);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}
