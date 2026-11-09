//! Gestos visuales compartidos por Arrangement y piano roll. GPUI entrega deltas
//! con la dirección natural y la inercia del sistema; no añadimos otro temporizador.
use gpui::{PinchEvent, ScrollEvent, TouchPhase, px};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Navigation {
    Pan { x: f64, y: f64 },
    Zoom(f64),
}

pub(super) fn scroll_navigation(event: &ScrollEvent) -> Navigation {
    let delta = event.delta.pixel_delta(px(24.0));
    let finite = |value: f32| {
        if value.is_finite() {
            f64::from(value)
        } else {
            0.0
        }
    };
    let x = finite(delta.x.into());
    let y = finite(delta.y.into());
    if event.modifiers.control {
        let amount = if y.abs() >= x.abs() { y } else { x };
        Navigation::Zoom((amount * 0.005).clamp(-2.0, 2.0).exp())
    } else if event.modifiers.shift {
        // Algunos backends ya convierten Shift+rueda a un delta horizontal.
        Navigation::Pan {
            x: if x.abs() > y.abs() { x } else { y },
            y: 0.0,
        }
    } else {
        // No elegir un eje dominante: un touchpad puede desplazar ambos a la vez.
        Navigation::Pan { x, y }
    }
}

pub(super) fn pinch_factor(event: &PinchEvent) -> Option<f64> {
    if matches!(event.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
        return None;
    }
    let factor = 1.0 + f64::from(event.delta);
    (factor.is_finite() && factor > 0.0).then_some(factor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Modifiers, ScrollDelta, point};

    #[test]
    fn precise_diagonal_motion_keeps_both_axes_and_fractional_pixels() {
        let event = ScrollEvent {
            delta: ScrollDelta::Pixels(point(px(-0.25), px(0.5))),
            ..Default::default()
        };
        assert_eq!(
            scroll_navigation(&event),
            Navigation::Pan { x: -0.25, y: 0.5 }
        );
    }

    #[test]
    fn shift_wheel_works_with_raw_or_already_swapped_axes() {
        for delta in [point(0.0, -2.0), point(-2.0, 0.0)] {
            let event = ScrollEvent {
                delta: ScrollDelta::Lines(delta),
                modifiers: Modifiers {
                    shift: true,
                    ..Default::default()
                },
                ..Default::default()
            };
            assert_eq!(
                scroll_navigation(&event),
                Navigation::Pan { x: -48.0, y: 0.0 }
            );
        }
    }

    #[test]
    fn pinch_accepts_real_updates_but_ignores_cancelled_or_invalid_values() {
        let mut event = PinchEvent {
            delta: 0.25,
            ..Default::default()
        };
        assert_eq!(pinch_factor(&event), Some(1.25));
        for delta in [f32::NAN, f32::INFINITY, -1.0, -2.0] {
            event.delta = delta;
            assert_eq!(pinch_factor(&event), None);
        }
        event.delta = 0.25;
        for phase in [TouchPhase::Ended, TouchPhase::Cancelled] {
            event.phase = phase;
            assert_eq!(pinch_factor(&event), None);
        }
    }
}
