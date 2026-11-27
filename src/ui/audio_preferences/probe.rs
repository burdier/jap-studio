//! Las pruebas usan el borrador, sin guardar ni alterar archivos de audio.
use crate::{audio::device_probe::DeviceProbe, ui::DawApp};
use gpui::{AppContext, Context};
impl DawApp {
    pub(in crate::ui) fn toggle_audio_probe(&mut self, input: bool, cx: &mut Context<Self>) {
        if self.settings.audio.probe.take().is_some() {
            cx.notify();
            return;
        }
        if self.settings.audio.busy || self.settings.audio.scanning {
            return;
        }
        self.stop_all();
        self.instrument.stop();
        for piano in self.piano_instances.values_mut() {
            piano.stop();
        }
        self.sampler.stop();
        self.synth.stop();
        for synth in self.synth_instances.values_mut() {
            synth.stop();
        }
        self.settings.audio.busy = true;
        let config = self.settings.audio.draft.clone();
        let work = cx.background_spawn(async move { DeviceProbe::start(&config, input) });
        self.settings.audio.task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.settings.audio.busy = false;
                this.settings.audio.task = None;
                if this.settings.is_visible() && this.settings.category == super::super::settings::SettingsCategory::Audio {
                    match result {
                        Ok(probe) => { this.settings.audio.probe = Some(probe); this.audio_message(true, if input { "Entrada activa · solo medidor; no graba ni envía audio a la salida" } else { "Tono 440 Hz · −24 dBFS · 2 segundos" }.into()); }
                        Err(error) => this.audio_message(false, error),
                    }
                }
                this.prepare_sampler(cx);
                cx.notify();
            });
        }));
    }
    pub(in crate::ui) fn poll_audio_probe(&mut self) {
        if !self.settings.is_visible()
            || self.settings.category != super::super::settings::SettingsCategory::Audio
        {
            self.settings.audio.probe = None;
            return;
        }
        if self
            .settings
            .audio
            .probe
            .as_ref()
            .is_some_and(|p| p.failed() || p.finished())
        {
            let probe = self.settings.audio.probe.take().unwrap();
            let message = if probe.failed() {
                "La interfaz dejó de responder; vuelve a consultar dispositivos".to_string()
            } else {
                format!(
                    "Prueba terminada · buffer observado: {} frames · {:.2} ms por bloque",
                    probe.frames(),
                    1000. * f64::from(probe.frames()) / f64::from(probe.rate)
                )
            };
            self.audio_message(!probe.failed(), message);
        }
    }
}
