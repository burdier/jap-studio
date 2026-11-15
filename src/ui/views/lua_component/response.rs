//! Respuesta de una primitiva indicada por Lua sobre `CurveOverlay`; no es un analizador de señal.
use crate::ui::{
    DesignSystem,
    components::curve_overlay::{
        CurveAnchor, CurveGesture, CurveModel, CurvePath, GestureHandler, interactive_curve_box,
    },
};
use elevate::{
    ParameterId,
    primitives::filter::Filter,
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
    on_edit: ParameterEditFn,
) -> impl IntoElement + use<> {
    let mut values = [
        descriptor.nodes[node].values[0],
        descriptor.nodes[node].values[1],
        descriptor.nodes[node].values[2],
    ];
    let mut x_control: Option<AudioControl> = None;
    let mut y_control: Option<AudioControl> = None;
    for control in &descriptor.controls {
        for &(target, parameter) in &control.targets {
            if target == node {
                values[parameter.0 as usize] = parameters
                    .get(&control.id)
                    .copied()
                    .unwrap_or(control.default);
                match parameter {
                    ParameterId(0) => x_control = Some(control.clone()),
                    ParameterId(1) => y_control = Some(control.clone()),
                    _ => {}
                }
            }
        }
    }

    let points: Vec<(f32, f32)> = Filter::response(rate, values)
        .into_iter()
        .map(|(x, db)| (x, ((db + 60.0) / 84.0).clamp(0.0, 1.0)))
        .collect();

    let nx = x_control
        .as_ref()
        .map_or((values[0] / 20.0).ln() / 1000.0_f32.ln(), |c| {
            c.fraction(values[0])
        })
        .clamp(0.0, 1.0);
    let ny = y_control
        .as_ref()
        .map_or((values[1] - 0.5) / 9.5, |c| c.fraction(values[1]))
        .clamp(0.0, 1.0);

    let mode_label = match values[2] as u32 {
        1 => "HP",
        2 => "BP",
        _ => "LP",
    };
    let hz_text = if values[0] >= 1000.0 {
        format!("{:.2} kHz", values[0] / 1000.0)
    } else {
        format!("{:.0} Hz", values[0])
    };
    let readout = format!("{mode_label} · {hz_text} · Q {:.2}", values[1]);

    let grid_x = [100_f32, 1000., 10000.]
        .into_iter()
        .map(|hz| (hz / 20.0).ln() / 1000.0_f32.ln())
        .collect();
    let grid_y = [0.0_f32, -24., -48.]
        .into_iter()
        .map(|db| (db + 60.0) / 84.0)
        .collect();

    let model = CurveModel {
        grid_x,
        grid_y,
        baseline: Some(60.0 / 84.0),
        curves: vec![CurvePath {
            points,
            color: ds.accent,
            stroke_px: 2.0,
            fill_alpha: 0x24,
        }],
        anchors: vec![CurveAnchor {
            x: nx,
            y: ny,
            stem: false,
            crosshair: true,
            color: ds.cyan,
        }],
        header: "Respuesta del filtro · arrastra Hz/Q · Shift: fino · doble clic: reset".into(),
        readout: readout.into(),
        x_labels: ["20 Hz", "200", "2k", "20k"].map(Into::into).to_vec(),
        height_px: 136.0,
    };

    let handler: GestureHandler = Rc::new(move |gesture, window, cx| match *gesture {
        CurveGesture::Move { x, y, .. } => {
            if let Some(ctrl) = &x_control {
                on_edit(&(ctrl.id, ctrl.from_fraction(x)), window, cx);
            }
            if let Some(ctrl) = &y_control {
                on_edit(&(ctrl.id, ctrl.from_fraction(y)), window, cx);
            }
        }
        CurveGesture::Wheel { delta, fine, .. } => {
            if let Some(ctrl) = &y_control {
                let step = if fine { 0.01 } else { 0.05 };
                let next = (ny + delta * step).clamp(0.0, 1.0);
                on_edit(&(ctrl.id, ctrl.from_fraction(next)), window, cx);
            }
        }
        CurveGesture::Reset => {
            if let Some(ctrl) = &x_control {
                on_edit(&(ctrl.id, ctrl.default), window, cx);
            }
            if let Some(ctrl) = &y_control {
                on_edit(&(ctrl.id, ctrl.default), window, cx);
            }
        }
        CurveGesture::Commit => {}
    });

    interactive_curve_box(("lua-response-plot", node).into(), ds, model, handler)
}
