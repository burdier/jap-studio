//! Flujo de exportación de audio encima del formulario y worker compartidos.
use crate::ui::{DawApp, components::parameter_field::ParameterField};
use elevate::export::Format;
use gpui::{
    Context, Div, FontWeight,
    component::{
        Sizable,
        button::{Button, ButtonVariants},
    },
    div,
    prelude::*,
    px, relative, rgb,
};
use std::sync::atomic::Ordering::Relaxed;

impl DawApp {
    pub(super) fn render_export_fields(&self, cx: &mut Context<Self>) -> Div {
        let form = self.action_form.as_ref().unwrap();
        let export = form.export.as_ref().unwrap();
        let ds = &self.ds;
        if let Some(report) = &export.completed {
            let loudness = &report.loudness;
            let has_metrics = loudness.integrated_lufs.is_some()
                || loudness.true_peak_dbtp.is_some()
                || loudness.lra_lu.is_some();
            let tp_hot = loudness.true_peak_dbtp.is_some_and(|tp| tp > 0.0);
            return div()
                .py_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(ds.success))
                        .child(if report.paths.len() == 1 {
                            "Archivo exportado".into()
                        } else {
                            format!("{} archivos exportados", report.paths.len())
                        }),
                )
                .when(has_metrics, |body| {
                    body.child(
                        div()
                            .p_3()
                            .rounded(px(ds.radius_md))
                            .bg(rgb(ds.panel))
                            .border_1()
                            .border_color(rgb(ds.border))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(ds.muted))
                                    .child("MEDICIÓN EBU R128"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap_4()
                                    .when_some(loudness.integrated_lufs, |row, lufs| {
                                        row.child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(rgb(ds.muted))
                                                        .child("Sonoridad"),
                                                )
                                                .child(
                                                    div()
                                                        .font_family("monospace")
                                                        .text_sm()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(rgb(ds.text))
                                                        .child(format!("{lufs:.1} LUFS")),
                                                ),
                                        )
                                    })
                                    .when_some(loudness.true_peak_dbtp, |row, tp| {
                                        row.child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(rgb(ds.muted))
                                                        .child("True Peak"),
                                                )
                                                .child(
                                                    div()
                                                        .font_family("monospace")
                                                        .text_sm()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(rgb(if tp_hot {
                                                            ds.warning
                                                        } else {
                                                            ds.success
                                                        }))
                                                        .child(format!("{tp:.1} dBTP")),
                                                ),
                                        )
                                    })
                                    .when_some(loudness.lra_lu, |row, lra| {
                                        row.child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(rgb(ds.muted))
                                                        .child("Dinámica (LRA)"),
                                                )
                                                .child(
                                                    div()
                                                        .font_family("monospace")
                                                        .text_sm()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(rgb(ds.text))
                                                        .child(format!("{lra:.1} LU")),
                                                ),
                                        )
                                    }),
                            )
                            .when(
                                loudness.applied_gain_db.abs() >= 0.05
                                    || loudness.max_gain_reduction_db >= 0.05,
                                |card| {
                                    let mut notes = Vec::new();
                                    if loudness.applied_gain_db.abs() >= 0.05 {
                                        notes.push(format!(
                                            "Normalización {:+.1} dB",
                                            loudness.applied_gain_db
                                        ));
                                    }
                                    if loudness.max_gain_reduction_db >= 0.05 {
                                        notes.push(format!(
                                            "Limitador -{:.1} dB máx.",
                                            loudness.max_gain_reduction_db
                                        ));
                                    }
                                    card.child(
                                        div()
                                            .font_family("monospace")
                                            .text_xs()
                                            .text_color(rgb(ds.cyan))
                                            .child(notes.join(" · ")),
                                    )
                                },
                            ),
                    )
                })
                .when_some(report.paths.first(), |body, path| {
                    let file = path.clone();
                    body.child(
                        div()
                            .text_sm()
                            .text_color(rgb(ds.muted))
                            .child(path.parent().unwrap_or(path).display().to_string()),
                    )
                    .child(
                        Button::new("show-export-file")
                            .small()
                            .outline()
                            .label("Mostrar en carpeta")
                            .on_click(move |_, _, cx| cx.reveal_path(&file)),
                    )
                })
                .when(report.overs > 0, |body| {
                    body.child(
                        div()
                            .text_sm()
                            .text_color(rgb(ds.warning))
                            .child("La señal superó 0 dBFS durante la exportación."),
                    )
                });
        }
        if export.running {
            let progress = self
                .export_job
                .as_ref()
                .map_or(0, |job| job.progress.load(Relaxed))
                .min(100);
            let canceling = self
                .export_job
                .as_ref()
                .is_some_and(|job| job.cancel.load(Relaxed));
            return div()
                .py_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(div().text_sm().child(if canceling {
                    "Cancelando…".into()
                } else {
                    format!("Exportando audio · {progress}%")
                }))
                .child(
                    div()
                        .h(px(5.))
                        .w_full()
                        .rounded(px(ds.radius_sm))
                        .bg(rgb(ds.panel))
                        .child(
                            div()
                                .h_full()
                                .w(relative(progress as f32 / 100.))
                                .rounded(px(ds.radius_sm))
                                .bg(rgb(ds.cyan)),
                        ),
                );
        }

        let individuals = form.checked("individuals");
        let formats = [
            (Format::Wav, "WAV"),
            (Format::Mp3, "MP3"),
            (Format::Flac, "FLAC"),
            (Format::Aiff, "AIFF"),
        ];
        let filename = form.text("name", cx);
        let sample_rate = form.text("sample_rate", cx).parse::<f64>().ok();
        let quality =
            sample_rate.map_or_else(String::new, |rate| format!(" · {} kHz", rate / 1000.));
        let channels = if form.checked("mono") {
            "Mono"
        } else {
            "Estéreo"
        };
        let full_range = form.text("start", cx) == "0"
            && form.text("length", cx).trim().is_empty()
            && form.text("tracks", cx) == "all";
        let range = if full_range {
            "Toda la canción"
        } else {
            "Rango o pistas personalizados"
        };
        let lufs_text = form.text("lufs", cx);
        let tp_text = form.text("true_peak", cx);
        let lufs_val = lufs_text.trim().parse::<f64>().ok();
        let tp_val = tp_text.trim().parse::<f64>().ok();
        let mut loudness_summary = String::new();
        if let Some(v) = lufs_val {
            loudness_summary.push_str(&format!(" · {v:.0} LUFS"));
        }
        if let Some(v) = tp_val {
            loudness_summary.push_str(&format!(" · {v:.1} dBTP"));
        }
        let loudness_presets: [(&str, &str, &str); 5] = [
            ("Sin normalizar", "", ""),
            ("Streaming (-14 / -1)", "-14", "-1"),
            ("Podcast (-16 / -1)", "-16", "-1"),
            ("Club (-9 / -0.3)", "-9", "-0.3"),
            ("Solo True Peak (-1)", "", "-1"),
        ];
        let name_input = self.render_parameter_control(form.field_index("name"), cx);
        let directory_input = self.render_parameter_control(form.field_index("directory"), cx);
        div()
            .py_2()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Qué exportar"),
                    )
                    .child(
                        div().flex().flex_wrap().gap_2().children(
                            [(false, "Mezcla completa"), (true, "Pistas separadas")]
                                .into_iter()
                                .map(|(value, label)| {
                                    let button = Button::new(("export-target", usize::from(value)))
                                        .small()
                                        .label(label);
                                    (if individuals == value {
                                        button.primary()
                                    } else {
                                        button.outline()
                                    })
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            if let Some(form) = &mut this.action_form {
                                                let index = form.field_index("individuals");
                                                if let ParameterField::Bool { checked, .. } =
                                                    &mut form.fields[index]
                                                {
                                                    *checked = value;
                                                }
                                                form.error = None;
                                            }
                                            cx.notify();
                                        },
                                    ))
                                }),
                        ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child(if individuals {
                                "Un archivo por pista con audio. Útil para enviar pistas a mezcla."
                            } else {
                                "Un solo archivo con la mezcla, listo para escuchar."
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Nombre del archivo"),
                    )
                    .child(name_input),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Formato"),
                    )
                    .child(
                        div().flex().flex_wrap().gap_2().children(
                            formats
                                .into_iter()
                                .enumerate()
                                .map(|(index, (format, label))| {
                                    let active = export.formats.contains(&format);
                                    let button =
                                        Button::new(("export-format", index)).small().label(label);
                                    (if active {
                                        button.primary()
                                    } else {
                                        button.outline()
                                    })
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            if let Some(form) = &mut this.action_form
                                                && let Some(export) = &mut form.export
                                            {
                                                if let Some(index) =
                                                    export.formats.iter().position(|f| *f == format)
                                                {
                                                    if export.formats.len() > 1 {
                                                        export.formats.remove(index);
                                                    }
                                                } else {
                                                    export.formats.push(format);
                                                }
                                                form.error = None;
                                            }
                                            cx.notify();
                                        },
                                    ))
                                }),
                        ),
                    )
                    .child(div().text_xs().text_color(rgb(ds.muted)).child(
                        if export.formats.contains(&Format::Mp3) && export.formats.len() == 1 {
                            "MP3 ocupa menos espacio y es fácil de compartir."
                        } else {
                            "WAV es una buena opción para conservar la calidad."
                        },
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Guardar en"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .child(div().flex_1().min_w(px(120.)).child(directory_input))
                            .child(
                                Button::new("choose-export-folder")
                                    .small()
                                    .outline()
                                    .label(if export.choosing_folder {
                                        "Abriendo…"
                                    } else {
                                        "Elegir carpeta…"
                                    })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.choose_export_folder(window, cx)
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Sonoridad y True Peak"),
                    )
                    .child(div().flex().flex_wrap().gap_2().children(
                        loudness_presets.into_iter().enumerate().map(|(index, (label, lufs, tp))| {
                            let active = lufs_text.trim() == lufs && tp_text.trim() == tp;
                            let button = Button::new(("export-loudness", index))
                                .small()
                                .label(label);
                            (if active {
                                button.primary()
                            } else {
                                button.outline()
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if let Some(form) = &mut this.action_form {
                                    let lufs_idx = form.field_index("lufs");
                                    if let ParameterField::Input(input) = &form.fields[lufs_idx] {
                                        input.update(cx, |input, cx| {
                                            input.replace_all(lufs.to_string(), window, cx);
                                        });
                                    }
                                    let tp_idx = form.field_index("true_peak");
                                    if let ParameterField::Input(input) = &form.fields[tp_idx] {
                                        input.update(cx, |input, cx| {
                                            input.replace_all(tp.to_string(), window, cx);
                                        });
                                    }
                                    form.error = None;
                                }
                                cx.notify();
                            }))
                        }),
                    ))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child("Ajusta LUFS integrado (EBU R128) y protege picos con limitador True Peak 4×."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .border_t_1()
                    .border_color(rgb(ds.border))
                    .pt_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                Button::new("export-options")
                                    .small()
                                    .ghost()
                                    .label(if export.advanced {
                                        "Ocultar opciones"
                                    } else {
                                        "Más opciones"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(form) = &mut this.action_form
                                            && let Some(export) = &mut form.export
                                        {
                                            export.advanced = !export.advanced;
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(ds.muted))
                                    .child(format!("{range} · {channels}{quality}{loudness_summary}")),
                            ),
                    )
                    .when(export.advanced, |panel| {
                        panel.children(
                            [
                                "lufs",
                                "true_peak",
                                "sample_rate",
                                "mono",
                                "tracks",
                                "start",
                                "length",
                                "tail",
                            ]
                            .map(|key| self.render_action_field(form.field_index(key), cx)),
                        )
                    }),
            )
            .when(!individuals && !filename.trim().is_empty(), |body| {
                let extensions = export
                    .formats
                    .iter()
                    .map(|format| format!("{filename}.{}", format.extension()))
                    .collect::<Vec<_>>()
                    .join(" · ");
                body.child(
                    div()
                        .text_xs()
                        .truncate()
                        .text_color(rgb(ds.muted))
                        .child(extensions),
                )
            })
    }
}
