//! Formulario de preferencias de exportación. Guardar valida el conjunto completo.
use crate::ui::{DawApp, components::button::chip_option, export_preferences::ExportField};
use elevate::export::{BitDepth, ExportOptions, Format, Mp3Mode, Target};
use gpui::{Context, Div, IntoElement, MouseButton, div, prelude::*, px, rgb};

impl DawApp {
    pub(super) fn render_export_settings(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let ds = &self.ds;
        let preferences = &self.settings.export;
        let draft = &preferences.draft;
        let validation = preferences.options();
        div().flex_1().min_h_0().flex().flex_col()
            .child(div().id("export-preferences-scroll").flex_1().min_h_0().overflow_y_scroll()
                .p_4().flex().flex_col().gap_4()
                .child(div().text_sm().text_color(rgb(ds.muted)).child("«Exportar audio» y el comando :export usan los valores guardados. Las opciones escritas en un comando tienen prioridad."))
                .child(self.export_text_field(ExportField::Name, cx))
                .child(self.export_text_field(ExportField::Directory, cx))
                .child(self.export_choices("Salida", draft.target, vec![("Mezcla master".into(), Target::Master), ("Archivos por pista".into(), Target::Individuals)], |o, v| o.target = v, cx))
                .child(div().flex().flex_col().gap_2().child(self.export_label("Formatos · puedes elegir varios"))
                    .child(div().flex().flex_wrap().gap_2().children([Format::Wav, Format::Aiff, Format::Flac, Format::Mp3].into_iter().map(|format| {
                        chip_option(ds, format.extension().to_uppercase(), draft.formats.contains(&format), ds.accent)
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                let settings = &mut this.settings.export;
                                if settings.draft.formats.contains(&format) { settings.draft.formats.retain(|f| *f != format); }
                                else { settings.draft.formats.push(format); }
                                settings.message = None; settings.editing = None; cx.stop_propagation(); cx.notify();
                            }))
                    }))))
                .child(self.export_choices("Frecuencia de muestreo", draft.sample_rate, [44_100, 48_000, 88_200, 96_000, 192_000].map(|rate| (format!("{} kHz", rate as f64 / 1000.0), rate)).to_vec(), |o, v| o.sample_rate = v, cx))
                .when(draft.formats.iter().any(|f| *f != Format::Mp3), |panel| panel.child(self.export_choices("Profundidad PCM", draft.bits,
                    vec![("16 bits".into(), BitDepth::Int16), ("24 bits".into(), BitDepth::Int24), ("32 bits".into(), BitDepth::Int32), ("32 bits float".into(), BitDepth::Float32)], |o, v| o.bits = v, cx)))
                .when(draft.formats.contains(&Format::Mp3), |panel| panel
                    .child(self.export_choices("MP3 · modo", matches!(draft.mp3, Mp3Mode::Vbr(_)), vec![("CBR · bitrate fijo".into(), false), ("VBR · calidad variable".into(), true)], |o, v| o.mp3 = if v { Mp3Mode::Vbr(2) } else { Mp3Mode::Cbr(320) }, cx))
                    .child(self.export_choices("MP3 · calidad", draft.mp3, if matches!(draft.mp3, Mp3Mode::Cbr(_)) {
                        [128, 192, 256, 320].map(|rate| (format!("{rate} kbps"), Mp3Mode::Cbr(rate))).to_vec()
                    } else { (0..=9).map(|q| (if q == 0 { "0 · mejor".into() } else { q.to_string() }, Mp3Mode::Vbr(q))).collect() }, |o, v| o.mp3 = v, cx)))
                .child(self.export_choices("Canales", draft.mono, vec![("Estéreo".into(), false), ("Mono".into(), true)], |o, v| o.mono = v, cx))
                .child(self.export_choices("Normalizar sonoridad (LUFS)", draft.normalize_lufs, vec![
                    ("Desactivado".into(), None),
                    ("-14 LUFS · Streaming".into(), Some(-14.0)),
                    ("-16 LUFS · Podcast".into(), Some(-16.0)),
                    ("-11 LUFS · CD".into(), Some(-11.0)),
                    ("-9 LUFS · Club".into(), Some(-9.0)),
                ], |o, v| o.normalize_lufs = v, cx))
                .child(self.export_choices("Limitador True Peak", draft.true_peak_limit_db, vec![
                    ("Desactivado".into(), None),
                    ("-2.0 dBTP".into(), Some(-2.0)),
                    ("-1.0 dBTP · Streaming".into(), Some(-1.0)),
                    ("-0.3 dBTP · Mastering".into(), Some(-0.3)),
                ], |o, v| o.true_peak_limit_db = v, cx))
                .child(self.export_choices("Utility de salida", draft.master_fx, vec![("Auto · solo master".into(), None), ("Incluir".into(), Some(true)), ("Excluir".into(), Some(false))], |o, v| o.master_fx = v, cx))
                .child(div().flex().flex_col().gap_2().child(self.export_label("Pistas incluidas"))
                    .child(div().flex().flex_wrap().gap_2()
                        .child(chip_option(ds, "Todas", draft.tracks.is_empty(), ds.accent).on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.settings.export.draft.tracks.clear(); this.settings.export.message = None; cx.stop_propagation(); cx.notify(); })))
                        .children(self.tracks.iter().enumerate().map(|(index, track)| {
                            chip_option(ds, format!("{} · {}", index + 1, track.name), draft.tracks.contains(&index), ds.accent)
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                    let tracks = &mut this.settings.export.draft.tracks;
                                    if tracks.contains(&index) { tracks.retain(|t| *t != index); } else { tracks.push(index); }
                                    this.settings.export.message = None; cx.stop_propagation(); cx.notify();
                                }))
                        }))))
                .child(self.export_text_field(ExportField::Start, cx))
                .child(self.export_text_field(ExportField::Length, cx))
                .child(self.export_text_field(ExportField::Tail, cx))
                .child(div().text_xs().text_color(rgb(ds.muted)).child("Los archivos existentes no se sobrescriben. Para repetir una exportación, cambia el nombre o la carpeta.")))
            .child(div().flex_shrink_0().p_4().border_t_1().border_color(rgb(ds.border)).flex().flex_col().gap_2()
                .when_some(validation.err(), |footer, error| footer.child(div().text_sm().text_color(rgb(ds.danger)).child(error)))
                .when_some(preferences.message.as_ref(), |footer, (success, message)| footer.child(div().text_sm().text_color(rgb(if *success { ds.success } else { ds.danger })).child(message.clone())))
                .child(div().flex().flex_wrap().gap_2()
                    .child(chip_option(ds, "Guardar valores predeterminados", true, ds.accent).on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.save_export_preferences(); cx.stop_propagation(); cx.notify(); })))
                    .child(chip_option(ds, "Deshacer cambios", false, ds.muted).on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.settings.export.restore_saved(); cx.stop_propagation(); cx.notify(); })))
                    .child(chip_option(ds, "Valores iniciales", false, ds.muted).on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.settings.export.reset_draft(); cx.stop_propagation(); cx.notify(); })))))
            .into_any_element()
    }

    fn export_label(&self, label: &'static str) -> Div {
        div().text_sm().text_color(rgb(self.ds.text)).child(label)
    }

    fn export_text_field(&self, field: ExportField, cx: &mut Context<Self>) -> Div {
        let ds = &self.ds;
        let active = self.settings.export.editing == Some(field);
        div().flex().flex_col().gap_1().child(self.export_label(field.label()))
            .child(div().px_3().py_2().border_1().rounded(px(ds.radius_sm))
                .border_color(rgb(if active { ds.accent } else { ds.border })).bg(rgb(ds.panel))
                .text_sm().text_color(rgb(if active && self.settings.export.select_all { ds.accent } else { ds.text }))
                .cursor_text().min_h(px(34.0)).overflow_hidden()
                .child(self.settings.export.text(field).to_string())
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                    this.settings.export.editing = Some(field); this.settings.export.select_all = true;
                    window.focus(&this.focus_handle, cx); cx.stop_propagation(); cx.notify();
                })))
            .when(active, |row| row.child(div().text_xs().text_color(rgb(ds.muted)).child("Escribe para sustituir · Ctrl+A seleccionar · Ctrl+V pegar · Enter terminar · Ctrl+S guardar")))
    }

    fn export_choices<T: Clone + PartialEq + 'static>(
        &self,
        label: &'static str,
        active: T,
        choices: Vec<(String, T)>,
        apply: fn(&mut ExportOptions, T),
        cx: &mut Context<Self>,
    ) -> Div {
        let ds = &self.ds;
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.export_label(label))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .children(choices.into_iter().map(|(label, value)| {
                        chip_option(ds, label, active == value, ds.accent).on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                apply(&mut this.settings.export.draft, value.clone());
                                this.settings.export.message = None;
                                this.settings.export.editing = None;
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                    })),
            )
    }
}
