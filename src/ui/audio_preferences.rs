//! Configuración común a controles y comandos. Descubrimiento/validación en workers.
mod commands;
mod probe;
mod profile;
mod selector_models;
mod selectors;
use super::DawApp;
use crate::audio::device::{self, AudioConfig, DeviceInventory};
use gpui::{AppContext, Context, Task};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AudioField {
    Driver,
    Output,
    Input,
    Rate,
    Buffer,
    OutputPair,
    InputChannel,
}
#[derive(Clone, PartialEq, Eq)]
pub(super) enum AudioChange {
    Driver(Option<String>),
    Output(Option<String>),
    Input(Option<String>),
    Rate(Option<u32>),
    Buffer(Option<u32>),
    OutputPair(u16),
    OutputRoute(String, u16),
    InputChannel(u16),
}
#[derive(Default)]
pub(super) struct AudioPreferences {
    pub saved: AudioConfig,
    pub draft: AudioConfig,
    pub inventory: DeviceInventory,
    pub focus_request: Option<AudioField>,
    pub selectors: std::collections::HashMap<gpui::WindowId, selectors::AudioSelectors>,
    pub message: Option<(bool, String)>,
    pub busy: bool,
    pub scanning: bool,
    pub task: Option<Task<()>>,
    pub scan_task: Option<Task<()>>,
    pending_use: Option<(bool, String)>,
    pub probe: Option<crate::audio::device_probe::DeviceProbe>,
}
impl DawApp {
    pub(in crate::ui) fn open_audio_settings(&mut self, cx: &mut Context<Self>) {
        self.open_settings();
        self.settings.category = super::settings::SettingsCategory::Audio;
        self.refresh_audio_devices(cx);
    }
    pub(in crate::ui) fn refresh_audio_devices(&mut self, cx: &mut Context<Self>) {
        if self.settings.audio.scanning || self.settings.audio.busy {
            return;
        }
        self.settings.audio.scanning = true;
        let config = self.settings.audio.draft.clone();
        let host = config.host.clone();
        let work = cx.background_spawn(async move { device::scan(&config) });
        self.settings.audio.scan_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.settings.audio.scanning = false;
                this.settings.audio.scan_task = None;
                if this.settings.audio.draft.host != host {
                    this.refresh_audio_devices(cx);
                    return;
                }
                match result {
                    Ok(inventory) => {
                        this.settings.audio.message = Some((
                            true,
                            format!(
                                "{} interfaces encontradas · {}",
                                inventory.devices.len(),
                                inventory.host
                            ),
                        ));
                        this.settings.audio.inventory = inventory;
                        if let Some((input, value)) = this.settings.audio.pending_use.take() {
                            this.use_audio_device(input, &value, cx);
                        }
                    }
                    Err(error) => {
                        this.settings.audio.pending_use = None;
                        this.audio_message(false, error);
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    pub(in crate::ui) fn change_audio_setting(
        &mut self,
        change: AudioChange,
        cx: &mut Context<Self>,
    ) {
        if self.settings.audio.busy {
            return;
        }
        let audio = &mut self.settings.audio;
        audio.focus_request = None;
        audio.message = None;
        audio.probe = None;
        let driver = matches!(change, AudioChange::Driver(_));
        match change {
            AudioChange::Driver(value) => {
                audio.draft.host = value;
                audio.draft.output = None;
                audio.draft.input = None;
                audio.draft.output_pair = 0;
                audio.draft.input_channel = 0;
                audio.draft.sample_rate = None;
                audio.draft.buffer_frames = None;
                audio.inventory = Default::default();
            }
            AudioChange::Output(value) => {
                audio.draft.output = value;
                audio.draft.output_pair = 0;
            }
            AudioChange::Input(value) => {
                audio.draft.input = value;
                audio.draft.input_channel = 0;
            }
            AudioChange::Rate(value) => audio.draft.sample_rate = value,
            AudioChange::Buffer(value) => audio.draft.buffer_frames = value,
            AudioChange::OutputPair(value) => audio.draft.output_pair = value,
            AudioChange::OutputRoute(id, first) => {
                audio.draft.output = Some(id);
                audio.draft.output_pair = first;
            }
            AudioChange::InputChannel(value) => audio.draft.input_channel = value,
        }
        if driver {
            self.refresh_audio_devices(cx);
        }
        cx.notify();
    }
    pub(in crate::ui) fn apply_audio_settings(&mut self, cx: &mut Context<Self>) {
        if self.settings.audio.busy || self.settings.audio.scanning {
            self.audio_message(false, "Espera a que termine la consulta de audio".into());
            return;
        }
        self.settings.audio.probe = None;
        self.settings.audio.busy = true;
        let config = self.settings.audio.draft.clone();
        let candidate = config.clone();
        let work = cx.background_spawn(async move { candidate.validate() });
        self.settings.audio.task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.settings.audio.task = None;
                this.settings.audio.busy = false;
                match result {
                    Ok(description) if this.settings.audio.draft == config => {
                        this.stop_all();
                        this.instrument.configure_audio(config.clone());
                        this.sampler.configure_audio(config.clone());
                        this.vst.unload();
                        this.settings.audio.saved = config.clone();
                        if let Some(target) = this.vst_track().map(|i| this.tracks[i].id) {
                            if let Some(bundle) = this.vst.bundle.clone() {
                                this.load_vst(bundle, Some(target), cx);
                            }
                        }
                        this.prepare_sampler(cx);
                        match profile::save(&config) {
                            Ok(()) => this.audio_message(
                                true,
                                format!("{description} · guardado. Pulsa Play para usarlo."),
                            ),
                            Err(error) => this.audio_message(
                                false,
                                format!(
                                    "Aplicado solo en esta sesión; no se pudo guardar: {error}"
                                ),
                            ),
                        }
                    }
                    Ok(_) => this.audio_message(
                        false,
                        "La configuración cambió durante la validación; aplica de nuevo".into(),
                    ),
                    Err(error) => this.audio_message(false, error),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    pub(in crate::ui) fn restore_audio_settings(&mut self) {
        if self.settings.audio.busy {
            return;
        }
        self.settings.audio.probe = None;
        self.settings.audio.draft = self.settings.audio.saved.clone();
        self.settings.audio.focus_request = None;
        self.settings.audio.message = None;
    }
    pub(in crate::ui) fn audio_message(&mut self, success: bool, message: String) {
        self.status_message = message.clone();
        self.settings.audio.message = Some((success, message));
    }
    pub(in crate::ui) fn use_audio_device(
        &mut self,
        input: bool,
        value: &str,
        cx: &mut Context<Self>,
    ) {
        if self.settings.audio.scanning || self.settings.audio.inventory.host.is_empty() {
            self.settings.audio.pending_use = Some((input, value.to_string()));
            self.open_audio_settings(cx);
            return;
        }
        match self.settings.audio.inventory.resolve(value, input) {
            Ok(id) => {
                self.change_audio_setting(
                    if input {
                        AudioChange::Input(id)
                    } else {
                        AudioChange::Output(id)
                    },
                    cx,
                );
                self.apply_audio_settings(cx);
            }
            Err(error) => self.audio_message(false, error),
        }
    }
}
