//! Guía de imán compartida por Arrangement y piano roll.
//! Intención: la línea destino aparece y se intensifica al acercarse.
//! Captura: el borde se vuelve acento y destella una vez; al soltar, otro destello confirma.
//! Solo dibujo; la posición musical la decide el controlador del gesto.
use crate::ui::{
    DesignSystem,
    time_grid::{TimeGrid, magnet::MagnetHit, musical::beat_label},
};
use elevate::musical::time::Meter;
use gpui::{App, Bounds, Pixels, TextRun, Window, fill, point, px, rgb, rgba, size};
use std::time::{Duration, Instant};

const FLASH: Duration = Duration::from_millis(340);

#[derive(Clone, Default)]
pub(in crate::ui) struct SnapGuide {
    /// Borde actual del bloque arrastrado (inicio del clip o de las notas).
    edge: Option<f64>,
    hit: Option<MagnetHit>,
    flash: Option<(f64, Instant)>,
}

impl SnapGuide {
    /// Actualiza durante el arrastre; destella solo al entrar en una línea nueva.
    pub fn track(&mut self, edge: f64, hit: Option<MagnetHit>) {
        if let Some(hit) = hit.filter(|hit| hit.captured) {
            let already = self
                .hit
                .is_some_and(|previous| previous.captured && previous.target == hit.target);
            if !already {
                self.flash = Some((hit.target, Instant::now()));
            }
        }
        self.edge = Some(edge);
        self.hit = hit;
    }

    /// Fin del gesto: confirma la posición final (si la hay) y retira la guía.
    pub fn land(&mut self, beat: Option<f64>) {
        self.edge = None;
        self.hit = None;
        if let Some(beat) = beat {
            self.flash = Some((beat, Instant::now()));
        }
    }

    /// Fin del gesto sin cambio musical: si estaba pegado, confirma esa línea.
    pub fn release(&mut self) {
        let captured = self.hit.filter(|hit| hit.captured).map(|hit| hit.target);
        self.land(captured);
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Pide frames solo mientras dura el destello.
    pub fn animating(&self) -> bool {
        self.flash.is_some_and(|(_, at)| at.elapsed() < FLASH)
    }

    pub fn paint(
        &self,
        axis: TimeGrid,
        area: Bounds<Pixels>,
        meter: Meter,
        design: &DesignSystem,
        window: &mut Window,
        cx: &mut App,
    ) {
        let x_at = |beat: f64| area.left() + px(axis.x_at(beat).round() as f32);
        let visible = |x: Pixels| x >= area.left() - px(16.0) && x <= area.right() + px(16.0);
        let column = |x: Pixels, width: f32, color: u32, window: &mut Window| {
            window.paint_quad(fill(
                Bounds::new(
                    point(x - px(width / 2.0), area.top()),
                    size(px(width), area.size.height),
                ),
                rgba(color),
            ));
        };
        let accent = design.accent;
        // Destello: anillo que se abre y se apaga desde la línea alcanzada.
        if let Some((beat, at)) = self.flash {
            let t = at.elapsed().as_secs_f32() / FLASH.as_secs_f32();
            let x = x_at(beat);
            if t < 1.0 && visible(x) {
                let fade = (1.0 - t) * (1.0 - t);
                column(
                    x,
                    3.0 + 22.0 * t,
                    (accent << 8) | (fade * 70.0) as u32,
                    window,
                );
                column(
                    x,
                    2.0 + 6.0 * t,
                    (accent << 8) | (fade * 150.0) as u32,
                    window,
                );
            }
        }
        let Some(edge) = self.edge else {
            return;
        };
        let captured = self.hit.is_some_and(|hit| hit.captured);
        // Intención: la línea destino se enciende según la cercanía.
        if let Some(hit) = self.hit.filter(|hit| !hit.captured) {
            let x = x_at(hit.target);
            if visible(x) {
                let alpha = 40.0 + 150.0 * hit.proximity;
                column(x, 1.0, (accent << 8) | alpha as u32, window);
                let cap = 4.0 + 6.0 * hit.proximity;
                for y in [area.top(), area.bottom() - px(2.0)] {
                    window.paint_quad(fill(
                        Bounds::new(point(x - px(cap / 2.0), y), size(px(cap), px(2.0))),
                        rgba((accent << 8) | alpha as u32),
                    ));
                }
            }
        }
        // Borde actual: deja claro dónde empieza el bloque mientras se arrastra.
        let x = x_at(edge);
        if !visible(x) {
            return;
        }
        if captured {
            column(x, 7.0, (accent << 8) | 0x38, window);
            column(x, 2.0, (accent << 8) | 0xff, window);
        } else {
            column(x, 1.0, (design.text << 8) | 0xb0, window);
        }
        let text = beat_label(edge, meter);
        let (background, foreground) = if captured {
            (accent, design.panel_dark)
        } else {
            (design.panel, design.text)
        };
        let run = TextRun {
            len: text.len(),
            font: window.text_style().font(),
            color: rgb(foreground).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let font = px(design.font_size_xs);
        let line = window
            .text_system()
            .shape_line(text.into(), font, &[run], None);
        let origin = point(x + px(4.0), area.top() + px(3.0));
        window.paint_quad(
            fill(
                Bounds::new(origin, size(line.width + px(8.0), font + px(5.0))),
                rgb(background),
            )
            .corner_radii(px(3.0)),
        );
        if let Err(error) = line.paint(
            origin + point(px(4.0), px(2.0)),
            font + px(1.0),
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        ) {
            eprintln!("No se pudo dibujar la guía de snap: {error}");
        }
    }
}
