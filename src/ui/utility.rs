//! Acciones del slot Utility compartidas por catálogo, comandos, MIDI y controles.
use super::DawApp;
use elevate::devices::utility_slot::UtilityAction;

impl DawApp {
    pub(in crate::ui) fn apply_utility_action(&mut self, action: UtilityAction) {
        match self.utility.apply(action) {
            Ok(state) => {
                self.status_message = if !state.inserted {
                    "Hay Utility retirado · salida de audición".into()
                } else {
                    format!(
                        "Hay Utility · salida · {:.2}x · {}",
                        state.gain,
                        if state.bypassed { "BYPASS" } else { "ON" }
                    )
                };
            }
            Err(error) => self.status_message = error.to_string(),
        }
    }

    pub(in crate::ui) fn open_utility(&mut self) {
        self.apply_utility_action(UtilityAction::Insert);
        self.utility_open = true;
    }
}
