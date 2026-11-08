//! Perilla rotatoria (knob) nativa en GPU para cabeceras TCP e instrumentos.
use crate::ui::DesignSystem;
use gpui::{Bounds, IntoElement, Styled, canvas, fill, point, px, rgb, rgba, size};

const ARC_STEPS: usize = 19;
const START_ANGLE_DEG: f32 = -135.0;
const SWEEP_ANGLE_DEG: f32 = 270.0;

#[derive(Clone, Copy)]
pub(in crate::ui) struct KnobSpec {
    pub fraction: f32,
    pub bipolar: bool,
    pub color: u32,
    pub diameter: f32,
}

pub(in crate::ui) fn rotary_knob(ds: &DesignSystem, spec: KnobSpec) -> impl IntoElement + use<> {
    let ds = *ds;
    let fraction = spec.fraction.clamp(0.0, 1.0);
    let diameter = spec.diameter.max(14.0);
    canvas(
        move |_, _, _| (),
        move |bounds, (), window, _| {
            let cx = bounds.left() + bounds.size.width * 0.5;
            let cy = bounds.top() + bounds.size.height * 0.5;
            let r =
                (f32::from(bounds.size.width).min(f32::from(bounds.size.height)) * 0.5).max(7.0);

            // 1. Anillo exterior de segmentos LED (270 grados)
            let arc_r = r - 1.6;
            for step in 0..ARC_STEPS {
                let t = step as f32 / (ARC_STEPS - 1) as f32;
                let deg = START_ANGLE_DEG + t * SWEEP_ANGLE_DEG;
                let rad = (deg - 90.0).to_radians();
                let px_x = cx + px(rad.cos() * arc_r);
                let px_y = cy + px(rad.sin() * arc_r);

                let lit = if spec.bipolar {
                    let lo = fraction.min(0.5);
                    let hi = fraction.max(0.5);
                    (t >= lo - 0.02 && t <= hi + 0.02) || (step == ARC_STEPS / 2)
                } else {
                    t <= fraction + 0.01
                };

                let dot_r = if lit { 1.35 } else { 1.0 };
                if lit {
                    let glow_r = dot_r * 1.8;
                    window.paint_quad(
                        fill(
                            Bounds::new(
                                point(px_x - px(glow_r), px_y - px(glow_r)),
                                size(px(glow_r * 2.0), px(glow_r * 2.0)),
                            ),
                            rgba((spec.color << 8) | 0x44),
                        )
                        .corner_radii(px(glow_r)),
                    );
                }
                let dot_color = if lit { spec.color } else { ds.border };
                window.paint_quad(
                    fill(
                        Bounds::new(
                            point(px_x - px(dot_r), px_y - px(dot_r)),
                            size(px(dot_r * 2.0), px(dot_r * 2.0)),
                        ),
                        rgb(dot_color),
                    )
                    .corner_radii(px(dot_r)),
                );
            }

            // 2. Cuerpo metálico de la perilla (borde + tapa oscura + brillo superior)
            let body_r = (r - 4.0).max(4.0);
            window.paint_quad(
                fill(
                    Bounds::new(
                        point(cx - px(body_r), cy - px(body_r)),
                        size(px(body_r * 2.0), px(body_r * 2.0)),
                    ),
                    rgb(ds.border),
                )
                .corner_radii(px(body_r)),
            );
            let cap_r = (body_r - 1.0).max(3.0);
            window.paint_quad(
                fill(
                    Bounds::new(
                        point(cx - px(cap_r), cy - px(cap_r)),
                        size(px(cap_r * 2.0), px(cap_r * 2.0)),
                    ),
                    rgb(ds.panel_dark),
                )
                .corner_radii(px(cap_r)),
            );
            let inner_r = (cap_r - 2.0).max(2.0);
            window.paint_quad(
                fill(
                    Bounds::new(
                        point(cx - px(inner_r), cy - px(inner_r)),
                        size(px(inner_r * 2.0), px(inner_r * 2.0)),
                    ),
                    rgba((ds.panel_hover << 8) | 0x90),
                )
                .corner_radii(px(inner_r)),
            );

            // 3. Aguja indicadora desde el centro hacia el borde
            let angle_deg = START_ANGLE_DEG + fraction * SWEEP_ANGLE_DEG;
            let angle_rad = (angle_deg - 90.0).to_radians();
            let dir_x = angle_rad.cos();
            let dir_y = angle_rad.sin();
            let needle_steps = 6;
            for i in 1..=needle_steps {
                let dist = (i as f32 / needle_steps as f32) * (cap_r - 1.4);
                let nx = cx + px(dir_x * dist);
                let ny = cy + px(dir_y * dist);
                let nr = if i == needle_steps { 1.3 } else { 1.05 };
                let ncolor = if i == needle_steps {
                    ds.text
                } else {
                    spec.color
                };
                window.paint_quad(
                    fill(
                        Bounds::new(
                            point(nx - px(nr), ny - px(nr)),
                            size(px(nr * 2.0), px(nr * 2.0)),
                        ),
                        rgb(ncolor),
                    )
                    .corner_radii(px(nr)),
                );
            }
        },
    )
    .w(px(diameter))
    .h(px(diameter))
}
