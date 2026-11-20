//! Adaptador de control de las tiras. Los gestos guardan el ID del canal, no su posición.
use super::DawApp;
use elevate::{
    devices::utility_slot::UtilityAction,
    musical::{drums::DrumMix, model::InstrumentId, track::TrackId},
};
use gpui::Pixels;

#[derive(Clone, Copy)]
pub(super) enum MixerTarget {
    Track(TrackId),
    Piece(InstrumentId),
    Drums,
    Master,
}

#[derive(Clone, Copy)]
pub(super) enum MixParam {
    Level,
    Pan,
}

#[derive(Clone, Copy)]
pub(super) enum MixSwitch {
    Mute,
    Solo,
    Bypass,
}

#[derive(Clone, Copy)]
pub(super) struct MixerDrag {
    target: MixerTarget,
    param: MixParam,
    last_y: Pixels,
    fraction: f32,
    height: f32,
}

impl DawApp {
    fn mixer_track_index(&self, id: TrackId) -> Option<usize> {
        self.tracks.iter().position(|track| track.id == id)
    }

    fn drum_channel_mix(&self, target: MixerTarget) -> Option<&DrumMix> {
        let kit = &self.pattern_editor.session.kit;
        match target {
            MixerTarget::Drums => Some(&kit.bus),
            MixerTarget::Piece(id) => kit
                .pieces
                .iter()
                .find(|piece| piece.id == id)
                .map(|piece| &piece.mix),
            _ => None,
        }
    }

    fn edit_drum_channel(&mut self, target: MixerTarget, update: impl FnOnce(&mut DrumMix)) {
        let kit = &mut self.pattern_editor.session.kit;
        let mix = match target {
            MixerTarget::Drums => Some(&mut kit.bus),
            MixerTarget::Piece(id) => kit
                .pieces
                .iter_mut()
                .find(|piece| piece.id == id)
                .map(|piece| &mut piece.mix),
            _ => None,
        };
        if let Some(mix) = mix {
            let previous = *mix;
            update(mix);
            if *mix != previous {
                self.publish_drum_edit();
            }
        }
    }

    fn mixer_fraction(&self, target: MixerTarget, param: MixParam) -> Option<f32> {
        let read = |level, pan| match param {
            MixParam::Level => level as f32 / 100.0,
            MixParam::Pan => pan as f32 / 100.0,
        };
        match target {
            MixerTarget::Track(id) => {
                let index = self.mixer_track_index(id)?;
                Some(read(self.track_levels[index], self.track_pans[index]))
            }
            MixerTarget::Master => match param {
                MixParam::Level => Some(self.utility.state().gain / 2.0),
                MixParam::Pan => None,
            },
            channel => self
                .drum_channel_mix(channel)
                .map(|mix| read(mix.fader.level, mix.fader.pan)),
        }
    }

    pub(super) fn set_mixer_fraction(
        &mut self,
        target: MixerTarget,
        param: MixParam,
        fraction: f32,
    ) {
        let fraction = fraction.clamp(0.0, 1.0);
        let value = (fraction * 100.0).round() as i32;
        match target {
            MixerTarget::Track(id) => {
                if let Some(index) = self.mixer_track_index(id) {
                    match param {
                        MixParam::Level => self.set_track_level(index, value),
                        MixParam::Pan => self.set_track_pan(index, value),
                    }
                }
            }
            MixerTarget::Master => {
                let gain = fraction * 2.0;
                if matches!(param, MixParam::Level)
                    && (self.utility.state().gain - gain).abs() > f32::EPSILON
                {
                    self.apply_utility_action(UtilityAction::SetGain(gain));
                }
            }
            channel => self.edit_drum_channel(channel, |mix| match param {
                MixParam::Level => mix.fader.level = value,
                MixParam::Pan => mix.fader.pan = value,
            }),
        }
    }

    pub(super) fn step_mixer_control(&mut self, target: MixerTarget, param: MixParam, delta: f32) {
        if let Some(fraction) = self.mixer_fraction(target, param) {
            self.set_mixer_fraction(target, param, fraction + delta);
        }
    }

    pub(super) fn toggle_mixer_control(&mut self, target: MixerTarget, button: MixSwitch) {
        match target {
            MixerTarget::Track(id) => {
                if let Some(index) = self.mixer_track_index(id) {
                    match button {
                        MixSwitch::Mute => self.toggle_track_mute(index),
                        MixSwitch::Solo => self.toggle_track_solo(index),
                        MixSwitch::Bypass => (),
                    }
                }
            }
            MixerTarget::Master => {
                if matches!(button, MixSwitch::Bypass) {
                    self.apply_utility_action(UtilityAction::ToggleBypass);
                }
            }
            channel => self.edit_drum_channel(channel, |mix| match button {
                MixSwitch::Mute => mix.muted = !mix.muted,
                MixSwitch::Solo => mix.solo = !mix.solo,
                MixSwitch::Bypass => (),
            }),
        }
    }

    pub(super) fn begin_mixer_drag(
        &mut self,
        target: MixerTarget,
        param: MixParam,
        y: Pixels,
        height: f32,
    ) {
        if let Some(fraction) = self.mixer_fraction(target, param) {
            self.mixer_drag = Some(MixerDrag {
                target,
                param,
                last_y: y,
                fraction,
                height: height.max(1.0),
            });
        }
    }

    pub(super) fn update_mixer_drag(&mut self, y: Pixels, dragging: bool, precise: bool) -> bool {
        let Some(mut drag) = self.mixer_drag else {
            return false;
        };
        if !dragging {
            self.mixer_drag = None;
            return true;
        }
        let divisor = if precise {
            drag.height * 6.0
        } else {
            drag.height
        };
        let delta = f32::from(drag.last_y - y) / divisor;
        drag.last_y = y;
        drag.fraction = (drag.fraction + delta).clamp(0.0, 1.0);
        self.mixer_drag = Some(drag);
        self.set_mixer_fraction(drag.target, drag.param, drag.fraction);
        true
    }
}
