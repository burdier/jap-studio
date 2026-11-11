//! Adaptación de entradas instaladas al mismo buscador y sus interacciones.
use super::{DawApp, Palette};
use elevate::catalog::plugins::InstalledInstrument;
use gpui::{AppContext, Context};

impl Palette {
    pub(super) fn replace_plugins(&mut self, plugins: Vec<InstalledInstrument>) {
        let selected = self
            .results
            .get(self.selected)
            .map(|entry| entry.id.clone());
        self.plugins = plugins;
        self.refresh();
        if let Some(index) = self
            .results
            .iter()
            .position(|entry| Some(&entry.id) == selected.as_ref())
        {
            self.selected = index;
        }
    }
}

impl DawApp {
    pub(in crate::ui) fn scan_palette_plugins(&mut self, cx: &mut Context<Self>) {
        if !self.palette.is_open() || !self.palette.scan_needed || self.palette.scan_task.is_some()
        {
            return;
        }
        self.palette.scan_needed = false;
        let work = cx.background_spawn(async { crate::audio::vst3::scan::discover() });
        self.palette.scan_task = Some(cx.spawn(async move |this, cx| {
            let plugins = work.await;
            let _ = this.update(cx, |this, cx| {
                this.palette.scan_task = None;
                this.palette.replace_plugins(plugins);
                cx.notify();
            });
        }));
    }
}
