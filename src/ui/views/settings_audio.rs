//! Preferencias de audio: filas compactas etiqueta/control y desplegables compartidos.
use crate::ui::{
    DawApp,
    audio_preferences::{AudioChange, AudioField as Field},
    components::button::chip_option,
};
use gpui::{Context, Div, IntoElement, MouseButton, div, prelude::*, px, rgb};
impl DawApp {
    pub(super) fn render_audio_settings(
        &self,
        window: &gpui::Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let ds = &self.ds;
        let audio = &self.settings.audio;
        let draft = &audio.draft;
        let inventory = &audio.inventory;
        let latency = match (draft.sample_rate, draft.buffer_frames) {
            (Some(rate), Some(frames)) => format!(
                "{:.2} ms por bloque · estimado; no incluye conversión ni latencia del driver",
                f64::from(frames) * 1000. / f64::from(rate)
            ),
            _ => "Auto · la prueba muestra el tamaño de bloque observado".into(),
        };
        div().flex_1().min_h_0().flex().flex_col()
            .child(div().id("audio-settings-scroll").flex_1().min_h_0().overflow_y_scroll().p_4().flex().flex_col().gap_3()
                .child(div().text_sm().text_color(rgb(ds.muted)).child("Audio de entrada / salida"))
                .child(self.audio_selector(Field::Driver, window, cx))
                .when(inventory.host.eq_ignore_ascii_case("ALSA") && inventory.hosts.iter().any(|h| h.eq_ignore_ascii_case("PipeWire")), |panel| panel.child(
                    chip_option(ds, "Compartir audio con Linux · usar PipeWire", false, ds.accent)
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.change_audio_setting(AudioChange::Driver(Some("PipeWire".into())), cx); cx.stop_propagation(); }))
                ))
                .when(inventory.host.eq_ignore_ascii_case("PipeWire"), |panel| panel.child(div().text_xs().text_color(rgb(ds.muted)).child("Audio compartido con Linux. Selecciona la salida por su nombre; la salida del sistema no cambia.")))
                .child(self.audio_selector(Field::Output, window, cx))
                .when_some(inventory.selected(draft, false).filter(|d| d.error(false).is_some()), |panel, d| panel.child(div().text_xs().text_color(rgb(ds.warning)).child(d.status(false))))
                .child(self.audio_selector(Field::OutputPair, window, cx))
                .child(self.audio_selector(Field::Input, window, cx))
                .when_some(inventory.selected(draft, true).filter(|d| d.error(true).is_some()), |panel, d| panel.child(div().text_xs().text_color(rgb(ds.warning)).child(d.status(true))))
                .child(self.audio_selector(Field::InputChannel, window, cx))
                .child(div().mt_2().border_t_1().border_color(rgb(ds.border)).pt_3().text_sm().text_color(rgb(ds.muted)).child("Frecuencia y latencia"))
                .child(self.audio_selector(Field::Rate, window, cx))
                .child(self.audio_selector(Field::Buffer, window, cx))
                .child(div().text_xs().text_color(rgb(ds.muted)).child(latency))
                .child(div().text_xs().text_color(rgb(ds.muted)).child("Se muestran capacidades reportadas por el driver. Si no informa tamaños de buffer, usa Auto."))
                .child(div().mt_2().border_t_1().border_color(rgb(ds.border)).pt_3().text_sm().text_color(rgb(ds.muted)).child("Prueba de señal"))
                .child(div().flex().gap_2()
                    .child(chip_option(ds, if audio.probe.as_ref().is_some_and(|p| !p.input) { "Detener tono" } else { "Tono de prueba · −24 dBFS" }, false, ds.accent).on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.toggle_audio_probe(false, cx); cx.stop_propagation(); })))
                    .child(chip_option(ds, if audio.probe.as_ref().is_some_and(|p| p.input) { "Detener entrada" } else { "Medir entrada" }, false, ds.accent).on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.toggle_audio_probe(true, cx); cx.stop_propagation(); }))))
                .when_some(audio.probe.as_ref().filter(|p| p.input), |panel, probe| {
                    let peak = probe.level();
                    panel.child(div().h(px(8.)).w_full().bg(rgb(ds.panel)).child(div().h_full().w(gpui::relative(peak)).bg(rgb(if peak >= 0.98 { ds.danger } else { ds.success }))))
                        .child(div().text_xs().text_color(rgb(ds.text)).child(format!("Entrada {} · {:.1} dBFS · {} Hz · bloque {} frames", draft.input_channel + 1, 20. * peak.max(0.000001).log10(), probe.rate, probe.frames())))
                })
                .child(div().text_xs().text_color(rgb(ds.muted)).child("El medidor no graba ni monitoriza por altavoces. Se detiene al salir de Audio. Las pruebas detienen la reproducción."))
                .child(div().mt_2().border_t_1().border_color(rgb(ds.border)).pt_3().text_sm().text_color(rgb(ds.muted)).child("Interfaces disponibles · :list device"))
                .children(inventory.devices.iter().enumerate().map(|(index, d)| {
                    div().py_1().border_b_1().border_color(rgb(ds.border)).flex().flex_col().gap_1()
                        .child(div().text_sm().text_color(rgb(ds.text)).child(format!("{} · {}", index + 1, d.name)))
                        .child(div().text_xs().text_color(rgb(ds.muted)).child(d.id.clone()))
                        .child(div().text_xs().text_color(rgb(ds.muted)).child(format!("IN: {} · OUT: {}", d.status(true), d.status(false))))
                        .children(d.output_pairs().filter(|_| !d.shared()).map(|n| div().text_xs().text_color(rgb(ds.text)).child(d.output_label(n))))
                }))
                .when(inventory.devices.is_empty() && !audio.scanning, |panel| panel.child(div().text_sm().text_color(rgb(ds.warning)).child("Sin interfaces disponibles. Conecta la interfaz y pulsa Actualizar."))))
            .child(div().p_4().border_t_1().border_color(rgb(ds.border)).flex_shrink_0().flex().flex_col().gap_2()
                .when_some(audio.message.as_ref(), |panel, (ok, text)| panel.child(div().text_sm().text_color(rgb(if *ok { ds.success } else { ds.danger })).child(text.clone())))
                .child(div().flex().gap_2()
                    .child(chip_option(ds, if audio.busy { "Validando…" } else { "Aplicar y guardar" }, audio.draft != audio.saved, ds.accent).on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.apply_audio_settings(cx); cx.stop_propagation(); })))
                    .child(chip_option(ds, "Revertir", false, ds.muted).on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.restore_audio_settings(); this.refresh_audio_devices(cx); cx.stop_propagation(); cx.notify(); })))
                    .child(chip_option(ds, if audio.scanning { "Buscando…" } else { "Actualizar dispositivos" }, false, ds.muted).on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.refresh_audio_devices(cx); cx.stop_propagation(); }))))
                .child(div().text_xs().text_color(rgb(ds.muted)).child("Aplicar detiene el transporte. Piano, sampler, percusión y clips usan esta salida al volver a reproducir.")))
            .into_any_element()
    }
    fn audio_selector(&self, field: Field, window: &gpui::Window, _cx: &Context<Self>) -> Div {
        let ds = &self.ds;
        let controls = &self.settings.audio.selectors[&window.window_handle().window_id()];
        let entry = controls
            .entries
            .iter()
            .find(|e| e.field == field)
            .expect("audio field");
        div()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .w(px(160.))
                    .flex_shrink_0()
                    .text_sm()
                    .text_color(rgb(ds.text))
                    .child(entry.label),
            )
            .child(div().flex_1().min_w_0().child(entry.control.render(
                entry.label,
                entry.placeholder.clone(),
                self.settings.audio.busy || self.settings.audio.scanning,
            )))
    }
}
