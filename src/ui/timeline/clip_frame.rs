//! Marco y cabecera compartidos por clips de audio y MIDI.
use super::DesignSystem;
use gpui::{Bounds, Pixels, Window, fill, point, px, rgb, rgba, size};

pub(super) const CONTROL_LEFT: f32 = 4.0;
pub(super) const CONTROL_TOP: f32 = 0.0;
pub(super) const CONTROL_WIDTH: f32 = 92.0;
pub(super) const CONTROL_HEIGHT: f32 = 15.0;
pub(super) const CONTROL_SLOT: f32 = 22.0;
pub(super) const CONTROLS_MIN_WIDTH: f32 = 130.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ClipEdge {
    Left,
    Right,
}

/// Alto de la franja de título dentro del clip.
pub(super) fn title_height(design: &DesignSystem) -> f32 {
    (design.font_size_xs + 8.0).max(21.0)
}

fn tint(color: u32, alpha: u32) -> gpui::Rgba {
    rgba((color << 8) | alpha)
}

/// Contorno cuadrado de 1 px, barra fina y tiradores de recorte.
pub(super) fn paint_frame_without_top_border(
    clip: Bounds<Pixels>,
    color: u32,
    selected: bool,
    muted: bool,
    hover_edge: Option<ClipEdge>,
    design: &DesignSystem,
    window: &mut Window,
) {
    let accent = if muted { design.muted } else { color };
    let frame = design.clip_border.clamp(0.75, 1.0);
    let border = tint(
        if selected { accent } else { design.border },
        if selected { 0xe0 } else { 0xb0 },
    );
    let side_width = px(frame.max(1.0));
    window.paint_quad(fill(
        Bounds::new(clip.origin, size(clip.size.width, side_width)),
        border,
    ));
    window.paint_quad(fill(
        Bounds::new(clip.origin, size(side_width, clip.size.height)),
        border,
    ));
    window.paint_quad(fill(
        Bounds::new(
            point(clip.right() - side_width, clip.top()),
            size(side_width, clip.size.height),
        ),
        border,
    ));
    window.paint_quad(fill(
        Bounds::new(
            point(clip.left(), clip.bottom() - side_width),
            size(clip.size.width, side_width),
        ),
        border,
    ));

    let inner = Bounds::new(
        point(clip.left() + px(frame), clip.top() + px(frame)),
        size(
            (clip.size.width - px(frame * 2.0)).max(px(1.0)),
            (clip.size.height - px(frame * 2.0)).max(px(1.0)),
        ),
    );
    window.paint_quad(
        fill(
            inner,
            rgb(if muted {
                design.panel_dark
            } else {
                design.panel_hover
            }),
        )
        .corner_radii(px(0.0)),
    );
    // Barra superior integrada: solo la línea separadora, sin panel añadido.
    let title_h = title_height(design).min(f32::from(inner.size.height) - 1.0);
    // Un hilo tenue identifica el clip; la selección refuerza el contorno.
    if !muted {
        window.paint_quad(fill(
            Bounds::new(inner.origin, size(inner.size.width, px(1.0))),
            tint(accent, if selected { 0x80 } else { 0x38 }),
        ));
        window.paint_quad(fill(
            Bounds::new(
                point(inner.left(), inner.bottom() - px(1.0)),
                size(inner.size.width, px(1.0)),
            ),
            tint(accent, if selected { 0x60 } else { 0x28 }),
        ));
    }
    window.paint_quad(fill(
        Bounds::new(
            point(inner.left(), inner.top() + px(title_h)),
            size(inner.size.width, px(1.0)),
        ),
        border,
    ));
    // Tiradores verticales en ambos extremos para señalar el recorte sin encerrar el clip.
    let grip_h = (f32::from(inner.size.height) - title_h - 3.0)
        .max(2.0)
        .min((f32::from(inner.size.height) * 0.34).max(5.0));
    let grip_top = inner.top() + px(title_h + 2.0);
    for x in [inner.left() + px(1.0), inner.right() - px(2.0)] {
        window.paint_quad(
            fill(
                Bounds::new(point(x, grip_top), size(px(1.0), px(grip_h))),
                tint(accent, if selected { 0xff } else { 0xc0 }),
            )
            .corner_radii(px(0.5)),
        );
    }
    if let Some(edge) = hover_edge.filter(|_| !muted) {
        let x = match edge {
            ClipEdge::Left => inner.left() + px(1.0),
            ClipEdge::Right => inner.right() - px(5.0),
        };
        let height = (f32::from(inner.bottom() - grip_top) - 1.0).max(1.0);
        window.paint_quad(fill(
            Bounds::new(point(x, grip_top), size(px(4.0), px(height))),
            tint(accent, 0x30),
        ));
        window.paint_quad(fill(
            Bounds::new(point(x + px(1.0), grip_top), size(px(1.0), px(height))),
            tint(accent, 0xf0),
        ));
    }
}
