//! Marcas y etiquetas compartidas por Arrangement y piano roll; sin cambiar el snap.
use super::TimeGrid;
use elevate::musical::{
    grid::NoteGrid,
    time::{Meter, PPQ},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ui) enum MarkKind {
    Bar,
    Beat,
    Subdivision,
}

pub(in crate::ui) struct MusicalMark {
    pub beat: f64,
    pub kind: MarkKind,
    pub label: Option<String>,
}

/// Una sola secuencia de ticks enteros evita líneas duplicadas o desplazadas
/// entre la regla y los carriles, incluso en compases de numerador impar.
pub(in crate::ui) fn marks(
    axis: TimeGrid,
    meter: Meter,
    width: f64,
    font_size: f64,
    resolution: Option<NoteGrid>,
) -> Vec<MusicalMark> {
    if width <= 0.0 || !width.is_finite() || axis.pixels_per_beat <= 0.0 {
        return Vec::new();
    }
    let bar = meter.ticks_per_bar();
    let beat = meter.ticks_per_beat();
    let pixels = |ticks: i64| ticks as f64 / PPQ as f64 * axis.pixels_per_beat;
    let mut line_step = if pixels(beat) >= 12.0 { beat } else { bar };
    while pixels(line_step) < 12.0 {
        line_step *= 2;
    }
    while line_step <= beat && line_step % 2 == 0 && pixels(line_step / 2) >= 12.0 {
        line_step /= 2;
    }
    // En piano roll no sugerir subdivisiones más finas que la rejilla elegida.
    // Compases y pulsos siguen siendo referencias estructurales con cualquier snap.
    if let Some(resolution) = resolution {
        line_step = line_step.max(resolution.ticks().0.min(beat));
    }

    // Reservar sitio según la posición visible, no solo para números de un dígito.
    let last_bar = meter.bar_at(axis.beat_at(width)).to_string().len();
    let beat_digits = meter.numerator().to_string().len();
    let spacing = |characters: usize| characters as f64 * font_size * 0.65 + 12.0;
    let mut label_step = if pixels(beat) >= spacing(last_bar + beat_digits + 1) {
        beat
    } else {
        let mut step = bar;
        while pixels(step) < spacing(last_bar) {
            step *= 2;
        }
        step
    };
    while label_step <= beat
        && label_step % 2 == 0
        && pixels(label_step / 2) >= spacing(last_bar + beat_digits + 5)
    {
        label_step /= 2;
    }

    let first = (axis.first_beat.max(0.0) * PPQ as f64 / line_step as f64).ceil() as i64;
    let last = (axis.beat_at(width) * PPQ as f64 / line_step as f64).floor() as i64;
    (first..=last.min(first.saturating_add(4096)))
        .map(|index| {
            let tick = index * line_step;
            let kind = if tick % bar == 0 {
                MarkKind::Bar
            } else if tick % beat == 0 {
                MarkKind::Beat
            } else {
                MarkKind::Subdivision
            };
            MusicalMark {
                beat: tick as f64 / PPQ as f64,
                kind,
                label: (tick % label_step == 0).then(|| position_label(tick, meter)),
            }
        })
        .collect()
}

/// Etiqueta compás.pulso[.tick] de un beat cualquiera, con el formato de la regla.
pub(in crate::ui) fn beat_label(beat: f64, meter: Meter) -> String {
    position_label((beat.max(0.0) * PPQ as f64).round() as i64, meter)
}

fn position_label(tick: i64, meter: Meter) -> String {
    let bar = tick / meter.ticks_per_bar() + 1;
    let within = tick % meter.ticks_per_bar();
    let beat = within / meter.ticks_per_beat() + 1;
    let offset = within % meter.ticks_per_beat();
    if within == 0 {
        bar.to_string()
    } else if offset == 0 {
        format!("{bar}.{beat}")
    } else {
        format!("{bar}.{beat}.{offset:03}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_reveals_beats_then_tick_offsets_without_moving_existing_marks() {
        let axis = |pixels_per_beat| TimeGrid {
            first_beat: 0.0,
            pixels_per_beat,
        };
        let labels = |zoom| {
            marks(axis(zoom), Meter::default(), 1024.0, 11.0, None)
                .into_iter()
                .filter_map(|mark| mark.label.map(|label| (mark.beat, label)))
                .collect::<Vec<_>>()
        };
        assert!(labels(32.0).contains(&(4.0, "2".into())));
        assert!(!labels(32.0).iter().any(|(_, text)| text.contains('.')));
        assert!(labels(64.0).contains(&(1.0, "1.2".into())));
        assert!(labels(256.0).contains(&(0.5, "1.1.480".into())));
        assert!(labels(256.0).contains(&(1.0, "1.2".into())));
    }

    #[test]
    fn scroll_and_odd_meters_keep_marks_aligned_and_strictly_ordered() {
        for (n, d) in [(3, 4), (6, 8), (7, 8), (1, 16)] {
            let meter = Meter::new(n, d).unwrap();
            for zoom in [8.0, 32.0, 64.0, 256.0] {
                let axis = TimeGrid {
                    first_beat: 5.17,
                    pixels_per_beat: zoom,
                };
                let visible = marks(axis, meter, 800.0, 11.0, None);
                assert!(
                    visible
                        .windows(2)
                        .all(|pair| { axis.x_at(pair[1].beat) - axis.x_at(pair[0].beat) >= 12.0 })
                );
                for mark in visible {
                    assert!(axis.x_at(mark.beat) >= 0.0);
                    if mark.kind == MarkKind::Bar {
                        assert!((mark.beat / meter.beats_per_bar()).fract().abs() < 1e-9);
                    }
                    if let Some(label) = mark.label {
                        assert_eq!(
                            label.split('.').next().unwrap(),
                            meter.bar_at(mark.beat).to_string()
                        );
                    }
                }
            }
        }
        let meter = Meter::new(6, 8).unwrap();
        assert_eq!(position_label(3 * PPQ, meter), "2");
        assert_eq!(position_label(3 * PPQ + PPQ / 2, meter), "2.2");
        assert_eq!(position_label(3 * PPQ + PPQ / 4, meter), "2.1.240");
    }

    #[test]
    fn piano_roll_shares_labels_and_keeps_subdivisions_within_its_selected_grid() {
        let axis = TimeGrid {
            first_beat: 0.0,
            pixels_per_beat: 256.0,
        };
        let labels = |resolution| {
            marks(axis, Meter::default(), 1024.0, 11.0, resolution)
                .into_iter()
                .filter_map(|mark| mark.label.map(|label| (mark.beat, label)))
                .collect::<Vec<_>>()
        };
        assert_eq!(labels(None), labels(Some(NoteGrid::default())));
        let coarse = NoteGrid::new(4).unwrap();
        assert!(
            marks(axis, Meter::default(), 1024.0, 11.0, Some(coarse))
                .iter()
                .all(|mark| mark.kind != MarkKind::Subdivision)
        );
        let axis = TimeGrid {
            pixels_per_beat: 1024.0,
            ..axis
        };
        for denominator in [8, 16, 32, 64, 128] {
            let resolution = NoteGrid::new(denominator).unwrap();
            let visible = marks(axis, Meter::default(), 1024.0, 11.0, Some(resolution));
            assert_eq!(visible[1].beat, resolution.ticks().0 as f64 / PPQ as f64);
            assert!(visible.iter().all(|mark| {
                (mark.beat * PPQ as f64).round() as i64 % resolution.ticks().0 == 0
            }));
        }
    }
}
