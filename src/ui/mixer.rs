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
    Send(TrackId),
}

#[derive(Clone, Copy)]
pub(super) enum MixSwitch {
    Mute,
    Solo,
    Bypass,
}

#[derive(Clone)]
pub(super) struct MixerDrag {
    target: MixerTarget,
    param: MixParam,
    last_y: Pixels,
    start_fraction: f32,
    fraction: f32,
    height: f32,
    /// Pistas seleccionadas al iniciar el arrastre con su valor inicial, para moverlas en bloque.
    peers: Vec<(TrackId, f32)>,
}

impl DawApp {
    fn mixer_track_index(&self, id: TrackId) -> Option<usize> {
        self.tracks.iter().position(|track| track.id == id)
    }

    fn ganged_tracks(&self, target: MixerTarget) -> Option<Vec<TrackId>> {
        let MixerTarget::Track(id) = target else {
            return None;
        };
        if self.selected_tracks.len() > 1 && self.selected_tracks.contains(&id) {
            Some(self.selected_tracks.clone())
        } else {
            None
        }
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
                self.publish_drum_mix();
            }
        }
    }

    fn mixer_fraction(&self, target: MixerTarget, param: MixParam) -> Option<f32> {
        let read = |level, pan| match param {
            MixParam::Level => Some(level as f32 / 100.0),
            MixParam::Pan => Some(pan as f32 / 100.0),
            MixParam::Send(_) => None,
        };
        match target {
            MixerTarget::Track(id) => {
                let index = self.mixer_track_index(id)?;
                match param {
                    MixParam::Send(dest) => {
                        if self.tracks[index].is_return() || id == dest {
                            None
                        } else {
                            Some(self.tracks[index].send_amount(dest) as f32 / 100.0)
                        }
                    }
                    _ => read(self.track_levels[index], self.track_pans[index]),
                }
            }
            MixerTarget::Master => match param {
                MixParam::Level => Some(self.utility.state().gain / 2.0),
                MixParam::Pan | MixParam::Send(_) => None,
            },
            channel => self
                .drum_channel_mix(channel)
                .and_then(|mix| read(mix.fader.level, mix.fader.pan)),
        }
    }

    fn set_single_mixer_fraction(&mut self, target: MixerTarget, param: MixParam, fraction: f32) {
        let fraction = fraction.clamp(0.0, 1.0);
        let value = (fraction * 100.0).round() as i32;
        match target {
            MixerTarget::Track(id) => {
                if let Some(index) = self.mixer_track_index(id) {
                    match param {
                        MixParam::Level => self.set_track_level(index, value),
                        MixParam::Pan => self.set_track_pan(index, value),
                        MixParam::Send(dest) => {
                            self.set_track_send(index, dest, value.clamp(0, 100) as u8)
                        }
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
                MixParam::Send(_) => {}
            }),
        }
    }

    pub(super) fn set_mixer_fraction(
        &mut self,
        target: MixerTarget,
        param: MixParam,
        fraction: f32,
    ) {
        let fraction = fraction.clamp(0.0, 1.0);
        if let Some(peers) = self.ganged_tracks(target)
            && let Some(current) = self.mixer_fraction(target, param)
        {
            let delta = fraction - current;
            for peer in peers {
                if let Some(base) = self.mixer_fraction(MixerTarget::Track(peer), param) {
                    self.set_single_mixer_fraction(MixerTarget::Track(peer), param, base + delta);
                }
            }
            return;
        }
        self.set_single_mixer_fraction(target, param, fraction);
    }

    pub(super) fn reset_mixer_fraction(
        &mut self,
        target: MixerTarget,
        param: MixParam,
        fraction: f32,
    ) {
        if let Some(peers) = self.ganged_tracks(target) {
            for peer in peers {
                self.set_single_mixer_fraction(MixerTarget::Track(peer), param, fraction);
            }
            return;
        }
        self.set_single_mixer_fraction(target, param, fraction);
    }

    pub(super) fn step_mixer_control(&mut self, target: MixerTarget, param: MixParam, delta: f32) {
        if let Some(peers) = self.ganged_tracks(target) {
            for peer in peers {
                if let Some(fraction) = self.mixer_fraction(MixerTarget::Track(peer), param) {
                    self.set_single_mixer_fraction(
                        MixerTarget::Track(peer),
                        param,
                        fraction + delta,
                    );
                }
            }
            return;
        }
        if let Some(fraction) = self.mixer_fraction(target, param) {
            self.set_single_mixer_fraction(target, param, fraction + delta);
        }
    }

    pub(super) fn toggle_mixer_control(&mut self, target: MixerTarget, button: MixSwitch) {
        match target {
            MixerTarget::Track(id) => {
                if let Some(index) = self.mixer_track_index(id) {
                    if let Some(peers) = self.ganged_tracks(target) {
                        match button {
                            MixSwitch::Mute => {
                                let desired = !self.tracks[index].muted;
                                for peer in peers {
                                    if let Some(idx) = self.mixer_track_index(peer)
                                        && self.tracks[idx].muted != desired
                                    {
                                        self.toggle_track_mute(idx);
                                    }
                                }
                            }
                            MixSwitch::Solo => {
                                let desired = !self.tracks[index].solo;
                                for peer in peers {
                                    if let Some(idx) = self.mixer_track_index(peer)
                                        && self.tracks[idx].solo != desired
                                    {
                                        self.toggle_track_solo(idx);
                                    }
                                }
                            }
                            MixSwitch::Bypass => (),
                        }
                        return;
                    }
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
            let peers = self
                .ganged_tracks(target)
                .map(|ids| {
                    ids.into_iter()
                        .filter_map(|id| {
                            self.mixer_fraction(MixerTarget::Track(id), param)
                                .map(|val| (id, val))
                        })
                        .collect()
                })
                .unwrap_or_default();
            self.mixer_drag = Some(MixerDrag {
                target,
                param,
                last_y: y,
                start_fraction: fraction,
                fraction,
                height: height.max(1.0),
                peers,
            });
        }
    }

    pub(super) fn update_mixer_drag(&mut self, y: Pixels, dragging: bool, precise: bool) -> bool {
        let Some(mut drag) = self.mixer_drag.take() else {
            return false;
        };
        if !dragging {
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
        if drag.peers.len() > 1 {
            let offset = drag.fraction - drag.start_fraction;
            for &(peer, base) in &drag.peers {
                self.set_single_mixer_fraction(MixerTarget::Track(peer), drag.param, base + offset);
            }
        } else {
            self.set_single_mixer_fraction(drag.target, drag.param, drag.fraction);
        }
        self.mixer_drag = Some(drag);
        true
    }
}
