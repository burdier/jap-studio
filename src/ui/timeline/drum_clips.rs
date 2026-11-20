//! Lleva cada ocurrencia de Patrón o Canción a carriles independientes del Arrangement.
use super::Timeline;
use crate::ui::DawApp;
use elevate::musical::{
    clip::MidiClip,
    drums::{
        DrumClipTarget, DrumPatternId, DrumPlaybackSource, DrumSession, DrumSongCell, piece_pattern,
    },
    time::{PPQ, Tick},
    track::{Track, TrackKind},
};

impl Timeline {
    pub(in crate::ui) fn refresh_drum_links(
        &mut self,
        session: &DrumSession,
        pattern: DrumPatternId,
    ) -> Result<(), String> {
        for clip in self.midi_clip.iter_mut().chain(&mut self.other_midi_clips) {
            if clip
                .drum_target
                .is_some_and(|target| target.linked && target.pattern == pattern)
            {
                clip.refresh_drum_link(session)?;
            }
        }
        Ok(())
    }
}

impl DawApp {
    pub(in crate::ui) fn send_drums_to_arrangement(&mut self) {
        match self.place_drum_clips() {
            Ok(count) => {
                self.status_message =
                    format!("{count} clips MIDI por pieza · Drums Latex escucha el Arrangement")
            }
            Err(error) => self.status_message = error,
        }
    }

    fn place_drum_clips(&mut self) -> Result<usize, String> {
        let session = &self.pattern_editor.session;
        let mut tracks = self.tracks.clone();
        let mut used = Vec::with_capacity(session.kit.pieces.len());
        for piece in &session.kit.pieces {
            let is_piece = |clip: &MidiClip| {
                clip.drum_target
                    .is_some_and(|target| target.kit == session.kit.id && target.piece == piece.id)
            };
            let existing = self
                .timeline
                .midi_clip
                .iter()
                .chain(&self.timeline.other_midi_clips)
                .filter(|clip| is_piece(clip))
                .map(|clip| clip.track)
                .find(|index| {
                    !used.contains(index)
                        && tracks
                            .get(*index)
                            .is_some_and(|track| track.kind == TrackKind::Drum)
                });
            let track = existing
                .or_else(|| {
                    tracks.iter().enumerate().position(|(index, track)| {
                        track.kind == TrackKind::Drum
                            && track.instrument.is_none()
                            && !used.contains(&index)
                            && track.name.eq_ignore_ascii_case(&piece.name)
                            && self
                                .timeline
                                .midi_clip
                                .iter()
                                .chain(&self.timeline.other_midi_clips)
                                .all(|clip| clip.track != index || is_piece(clip))
                    })
                })
                .unwrap_or_else(|| {
                    let index = tracks.len();
                    tracks.push(Track::new(piece.name.clone(), TrackKind::Drum));
                    index
                });
            used.push(track);
        }
        if tracks.len() > 256 {
            return Err("Arrangement admite hasta 256 pistas".into());
        }
        let occurrences = match session.source {
            DrumPlaybackSource::Song => {
                if session.song.is_empty() {
                    return Err("Añade un patrón a Canción antes de enviarla".into());
                }
                session.song.clone()
            }
            _ => vec![DrumSongCell {
                at: Tick(0),
                pattern: session.active,
            }],
        };
        let count = used
            .len()
            .checked_mul(occurrences.len())
            .ok_or("Demasiados clips MIDI")?;
        let existing_clips =
            self.timeline.other_midi_clips.len() + usize::from(self.timeline.midi_clip.is_some());
        if count.saturating_add(existing_clips) > 4096 {
            return Err("Arrangement admite hasta 4096 clips MIDI".into());
        }

        let origin = self.timeline.edit_beat();
        let mut clips = Vec::with_capacity(count);
        for cell in occurrences {
            let source = session
                .pattern(cell.pattern)
                .ok_or("Patrón de Canción inexistente")?;
            let beat = origin + cell.at.0 as f64 / PPQ as f64;
            for (&track, piece) in used.iter().zip(&session.kit.pieces) {
                let pattern = piece_pattern(&source.notes, piece.id)?;
                let mut clip = MidiClip::new(
                    format!("{} · {}", source.name, piece.name),
                    track,
                    beat,
                    pattern,
                );
                clip.drum_target = Some(DrumClipTarget {
                    kit: session.kit.id,
                    piece: piece.id,
                    pattern: source.id,
                    linked: true,
                });
                clips.push(clip);
            }
        }
        if tracks.len() != self.tracks.len() {
            let source_count = self.tracks.len();
            let sources = (0..tracks.len())
                .map(|index| (index < source_count).then_some(index))
                .collect();
            let mut levels = self.track_levels.clone();
            let mut pans = self.track_pans.clone();
            levels.resize(tracks.len(), 75);
            pans.resize(tracks.len(), 50);
            self.commit_track_plan(
                tracks,
                sources,
                levels,
                pans,
                self.selected_tracks.clone(),
                true,
                false,
                None,
            );
        }
        for clip in clips {
            self.timeline.insert_midi_clip(clip);
        }
        self.pattern_editor.session.source = DrumPlaybackSource::Arrangement;
        self.publish_drum_edit();
        Ok(count)
    }
}
