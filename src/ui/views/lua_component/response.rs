//! Respuesta del filtro/EQ y espectro Pre/Post sobre el mismo eje logarítmico de frecuencias.
use crate::ui::{
    DesignSystem,
    components::curve_overlay::{
        CurveAnchor, CurveGesture, CurveModel, CurvePath, GestureHandler, interactive_curve_box,
    },
};
use elevate::{
    ParameterId,
    primitives::filter::{Filter, MODE_BELL},
    scripting::audio_components::{AudioComponent, AudioControl},
};
use gpui::{App, IntoElement, Window};
use std::{collections::BTreeMap, rc::Rc};

pub(super) type ParameterEditFn = Rc<dyn Fn(&(u32, f32), &mut Window, &mut App)>;

pub(super) fn plot(
    ds: &DesignSystem,
    descriptor: &AudioComponent,
    parameters: &BTreeMap<u32, f32>,
    node: usize,
    rate: f64,
    spectrum: Vec<CurvePath>,
    on_edit: ParameterEditFn,
) -> impl IntoElement + use<> {
    let mut values = [
        descriptor.nodes[node]
            .values
            .first()
            .copied()
            .unwrap_or(1000.0),
        descriptor.nodes[node]
            .values
            .get(1)
            .copied()
            .unwrap_or(0.707),
        descriptor.nodes[node].values.get(2).copied().unwrap_or(0.0),
        descriptor.nodes[node].values.get(3).copied().unwrap_or(0.0),
    ];
    let mut x_control: Option<AudioControl> = None;
    let mut q_control: Option<AudioControl> = None;
    let mut gain_control: Option<AudioControl> = None;
    for control in &descriptor.controls {
        for &(target, parameter) in &control.targets {
            if target == node {
                let idx = parameter.0 as usize;
                if idx < values.len() {
                    values[idx] = parameters
                        .get(&control.id)
                        .copied()
                        .unwrap_or(control.default);
                }
                match parameter {
                    ParameterId(0) => x_control = Some(control.clone()),
                    ParameterId(1) => q_control = Some(control.clone()),
                    ParameterId(3) => gain_control = Some(control.clone()),
                    _ => {}
                }
            }
        }
    }

    let mode = values[2] as usize;
    let is_eq_band = mode >= MODE_BELL && gain_control.is_some();

    let (min_db, max_db) = if is_eq_band {
        (-24.0_f32, 24.0_f32)
    } else {
        (-60.0_f32, 24.0_f32)
    };
    let db_span = max_db - min_db;
    let db_to_y = move |db: f32| ((db - min_db) / db_span).clamp(0.0, 1.0);

    let points: Vec<(f32, f32)> = Filter::response_eq(rate, values)
        .into_iter()
        .map(|(x, db)| (x, db_to_y(db)))
        .collect();

    let nx = x_control
        .as_ref()
        .map_or((values[0] / 20.0).ln() / 1000.0_f32.ln(), |c| {
            c.fraction(values[0])
        })
        .clamp(0.0, 1.0);
    let ny = if is_eq_band {
        db_to_y(values[3])
    } else {
        q_control
            .as_ref()
            .map_or((values[1] - 0.5) / 9.5, |c| c.fraction(values[1]))
            .clamp(0.0, 1.0)
    };
    let q_frac = q_control
        .as_ref()
        .map_or((values[1] - 0.5) / 9.5, |c| c.fraction(values[1]))
        .clamp(0.0, 1.0);

    let mode_label = match mode {
        1 => "HP",
        2 => "BP",
        3 => "Notch",
        4 => "Bell",
        5 => "Low Shelf",
        6 => "High Shelf",
        _ => "LP",
    };
    let hz_text = if values[0] >= 1000.0 {
        format!("{:.2} kHz", values[0] / 1000.0)
    } else {
        format!("{:.0} Hz", values[0])
    };
    let readout = if is_eq_band {
        format!(
            "{mode_label} · {hz_text} · {:+.1} dB · Q {:.2}",
            values[3], values[1]
        )
    } else {
        format!("{mode_label} · {hz_text} · Q {:.2}", values[1])
    };
    let header = if is_eq_band {
        "Arrastra Hz / dB · Rueda: Q · Doble clic: reset"
    } else {
        "Arrastra Hz / Q · Rueda: Q · Doble clic: reset"
    };

    let grid_x = [100_f32, 1000., 10000.]
        .into_iter()
        .map(|hz| (hz / 20.0).ln() / 1000.0_f32.ln())
        .collect();
    let grid_y = if is_eq_band {
        [12.0_f32, 0.0, -12.0].into_iter().map(db_to_y).collect()
    } else {
        [0.0_f32, -24., -48.].into_iter().map(db_to_y).collect()
    };

    let model = CurveModel {
        wave: Vec::new(),
        wave_dim_until: 0.0,
        spectrum,
        grid_x,
        grid_y,
        baseline: Some(db_to_y(0.0)),
        curves: vec![CurvePath {
            points,
            color: ds.accent,
            stroke_px: 2.2,
            fill_alpha: 0x2c,
        }],
        anchors: vec![CurveAnchor {
            x: nx,
            y: ny,
            stem: is_eq_band,
            crosshair: true,
            color: ds.cyan,
        }],
        header: header.into(),
        readout: readout.into(),
        x_labels: ["20 Hz", "200", "2k", "20k"].map(Into::into).to_vec(),
        height_px: 156.0,
    };

    let handler: GestureHandler = Rc::new(move |gesture, window, cx| match *gesture {
        CurveGesture::Move { x, y, .. } => {
            if let Some(ctrl) = &x_control {
                on_edit(&(ctrl.id, ctrl.from_fraction(x)), window, cx);
            }
            if is_eq_band {
                if let Some(ctrl) = &gain_control {
                    let db = (min_db + y * db_span).clamp(ctrl.min, ctrl.max);
                    on_edit(&(ctrl.id, db), window, cx);
                }
            } else if let Some(ctrl) = &q_control {
                on_edit(&(ctrl.id, ctrl.from_fraction(y)), window, cx);
            }
        }
        CurveGesture::Wheel { delta, fine, .. } => {
            if let Some(ctrl) = &q_control {
                let step = if fine { 0.015 } else { 0.06 };
                let next = (q_frac + delta * step).clamp(0.0, 1.0);
                on_edit(&(ctrl.id, ctrl.from_fraction(next)), window, cx);
            }
        }
        CurveGesture::Reset => {
            if let Some(ctrl) = &x_control {
                on_edit(&(ctrl.id, ctrl.default), window, cx);
            }
            if let Some(ctrl) = &q_control {
                on_edit(&(ctrl.id, ctrl.default), window, cx);
            }
            if let Some(ctrl) = &gain_control {
                on_edit(&(ctrl.id, ctrl.default), window, cx);
            }
        }
        CurveGesture::Commit => {}
    });

    interactive_curve_box(("lua-response-plot", node).into(), ds, model, handler)
}
