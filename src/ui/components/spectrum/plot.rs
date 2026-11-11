//! Canvas único; la historia es un mapa frecuencia/tiempo, no miles de elementos de layout.
use super::{SpectrumData, SpectrumMode, db};
use gpui::{Bounds, IntoElement, Pixels, Styled, Window, canvas, fill, point, px, rgb, rgba, size};

pub(super) fn graph(data: SpectrumData, mode: SpectrumMode) -> impl IntoElement {
    let height = data.height;
    canvas(
        move |_, _, _| (),
        move |bounds, (), window, _| {
            let w = f32::from(bounds.size.width).max(1.);
            let h = f32::from(bounds.size.height).max(1.);
            let mut rect = |x: f32, y: f32, width: f32, height: f32, color| {
                window.paint_quad(fill(
                    Bounds::new(
                        point(bounds.left() + px(x), bounds.top() + px(y)),
                        size(px(width), px(height)),
                    ),
                    color,
                ));
            };
            match mode {
                SpectrumMode::Spectrum => {
                    let dx = w / data.spectrum.len().max(1) as f32;
                    for (i, level) in data.spectrum.iter().enumerate() {
                        let height = level.clamp(0., 1.) * h;
                        rect(
                            i as f32 * dx,
                            h - height,
                            (dx - 0.6).max(0.5),
                            height,
                            rgb(data.color),
                        );
                    }
                }
                SpectrumMode::Visualizer => {
                    let dx = w / data.wave_min.len().max(1) as f32;
                    for (i, (low, high)) in data.wave_min.iter().zip(&data.wave_max).enumerate() {
                        let top = (1. - high.clamp(-1., 1.)) * h * 0.5;
                        let bottom = (1. - low.clamp(-1., 1.)) * h * 0.5;
                        rect(
                            i as f32 * dx,
                            top,
                            dx.max(1.),
                            (bottom - top).max(1.),
                            rgb(data.color),
                        );
                    }
                }
                SpectrumMode::SpectrogramLog | SpectrumMode::SpectrogramLinear => {
                    let bins = data.history_bins.max(1);
                    let frames = data.history.len() / bins;
                    let capacity = data.history_capacity.max(frames).max(1);
                    let dx = w / capacity as f32;
                    let offset = capacity - frames;
                    for (time, frame) in data.history.chunks_exact(bins).enumerate() {
                        let dy = h / frame.len().max(1) as f32;
                        for (bin, level) in frame.iter().enumerate() {
                            if *level <= 0.001 {
                                continue;
                            }
                            let color = heat(*level, data.background, data.color, data.hot);
                            rect(
                                (offset + time) as f32 * dx,
                                h - (bin + 1) as f32 * dy,
                                dx + 0.1,
                                dy + 0.1,
                                rgb(color),
                            );
                        }
                    }
                }
                SpectrumMode::Level => {
                    for channel in 0..2 {
                        let top = (channel as f32 + 0.15) * h / 2.;
                        let peak = ((db(data.peak[channel]) + 80.) / 80.).clamp(0., 1.);
                        let rms = ((db(data.rms[channel]) + 80.) / 80.).clamp(0., 1.);
                        rect(0., top, rms * w, h * 0.3, rgb(data.color));
                        rect((peak * w).min(w - 2.), top, 2., h * 0.3, rgb(data.hot));
                    }
                }
            }
            guides(bounds, &data, mode, window);
        },
    )
    .w_full()
    .h(px(height))
}
fn guides(bounds: Bounds<Pixels>, data: &SpectrumData, mode: SpectrumMode, window: &mut Window) {
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    for fraction in [0.25, 0.5, 0.75] {
        window.paint_quad(fill(
            Bounds::new(
                point(bounds.left(), bounds.top() + px(h * fraction)),
                size(px(w), px(1.)),
            ),
            rgba((data.border << 8) | 0x50),
        ));
    }
    if mode == SpectrumMode::Spectrum {
        if let Some(hz) = data.guide_hz {
            let fraction =
                ((hz.max(20.) / 20.).ln() / (data.upper_hz.max(21.) / 20.).ln()).clamp(0., 1.);
            window.paint_quad(fill(
                Bounds::new(
                    point(bounds.left() + px((fraction * w).min(w - 1.)), bounds.top()),
                    size(px(1.), px(h)),
                ),
                rgba((data.hot << 8) | 0xb0),
            ));
        }
    }
}
fn heat(level: f32, background: u32, cool: u32, hot: u32) -> u32 {
    let level = level.clamp(0., 1.);
    let (from, to, t) = if level < 0.7 {
        (background, cool, level / 0.7)
    } else {
        (cool, hot, (level - 0.7) / 0.3)
    };
    [16, 8, 0].into_iter().fold(0, |color, shift| {
        let a = ((from >> shift) & 255) as f32;
        let b = ((to >> shift) & 255) as f32;
        color | (((a + (b - a) * t) as u32) << shift)
    })
}
