//! Acción UI de transcripción Audio → MIDI polifónica (Basic Pitch en hilo en segundo plano).
use super::DawApp;
use elevate::musical::{
    clip::MidiClip,
    track::{Track, TrackKind},
};
use gpui::{AppContext, Context};

impl DawApp {
    /// Transcribe el clip de audio activo (o la selección activa en el Sample Editor)
    /// a un nuevo clip MIDI polifónico en una pista Synth equipada con Hay Piano.
    pub(in crate::ui) fn convert_audio_clip_to_midi(&mut self, cx: &mut Context<Self>) {
        if self.audio_to_midi_task.is_some() {
            self.status_message = "Ya hay una transcripción Audio → MIDI en curso…".into();
            return;
        }
        let Some(clip) = self.timeline.clip.clone() else {
            self.status_message = "Selecciona un clip de audio para convertir a MIDI".into();
            return;
        };

        let sample_rate = clip.file.sample_rate;
        let total = clip.file.frames.len();
        let (start_frame, end_frame, offset_beats) = if self.shown_sample().is_some()
            && let Some(sel) = self.timeline.sample_editor.selection
        {
            let (vis_a, vis_b) = self.timeline.sample_editor.visible(&clip);
            let a = sel.0.clamp(vis_a, vis_b);
            let b = sel.1.clamp(vis_a, vis_b);
            if b > a {
                let offset_s = (a.saturating_sub(vis_a)) as f64 / f64::from(sample_rate).max(1.0);
                (a, b, offset_s * self.timeline.tempo.bpm() / 60.0)
            } else {
                (vis_a, vis_b, 0.0)
            }
        } else {
            let start = ((clip.trim_start * f64::from(sample_rate)).round() as usize).min(total);
            let end = (((clip.trim_start + clip.length_seconds()) * f64::from(sample_rate)).round()
                as usize)
                .clamp(start, total);
            (start, end, 0.0)
        };

        if end_frame <= start_frame {
            self.status_message = "El clip de audio no contiene muestras en el rango activo".into();
            return;
        }

        // Respetar ganancia del clip (sin anularla si el clip estaba silenciado en la mezcla).
        let gain = 10.0_f32.powf(clip.props.gain_db / 20.0);
        let frames: Vec<[f32; 2]> = clip.file.frames[start_frame..end_frame]
            .iter()
            .map(|&[l, r]| [l * gain, r * gain])
            .collect();
        let bpm = self.timeline.tempo.bpm();
        let clip_name = clip.name.clone();
        let target_start_beat = (clip.start_beat + offset_beats).max(0.0);

        self.status_message = format!("♪ Transcribiendo «{clip_name}» a MIDI polifónico…");
        let work = cx.background_spawn(async move {
            crate::audio::audio_to_midi::transcribe_audio_to_midi(&frames, sample_rate, bpm)
        });

        self.audio_to_midi_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.audio_to_midi_task = None;
                match result {
                    Ok(report) => {
                        let note_count = report.note_count();
                        if note_count == 0 {
                            this.status_message = format!(
                                "Audio → MIDI: no se detectaron notas claras en «{clip_name}»"
                            );
                            cx.notify();
                            return;
                        }
                        let min_pitch = report.min_pitch;
                        let max_pitch = report.max_pitch;
                        let new_track_index = this.tracks.len();
                        let mut tracks = this.tracks.clone();
                        let mut sources: Vec<Option<usize>> =
                            (0..this.tracks.len()).map(Some).collect();
                        let mut levels = this.track_levels.clone();
                        let mut pans = this.track_pans.clone();
                        let midi_track =
                            Track::new(format!("{clip_name} (MIDI)"), TrackKind::Synth);
                        let midi_track_id = midi_track.id;
                        tracks.push(midi_track);
                        sources.push(None);
                        levels.push(78);
                        pans.push(50);
                        this.commit_track_plan(
                            tracks,
                            sources,
                            levels,
                            pans,
                            vec![midi_track_id],
                            true,
                            true,
                            None,
                        );

                        let piano_id = this.assign_piano_instance(new_track_index);
                        this.activate_piano(piano_id);
                        let pattern = match report.into_pattern(piano_id) {
                            Ok(pattern) => pattern,
                            Err(error) => {
                                this.status_message = format!("Audio → MIDI: {error}");
                                cx.notify();
                                return;
                            }
                        };

                        let midi_clip = MidiClip::new(
                            format!("{clip_name} (MIDI)"),
                            new_track_index,
                            target_start_beat,
                            pattern,
                        );
                        let midi_id = midi_clip.id;
                        this.timeline.insert_midi_clip(midi_clip);
                        this.timeline.selected_audio.clear();
                        this.timeline.focused_audio = None;
                        this.timeline.selected_midi = vec![midi_id];
                        this.select_track(new_track_index, false, false);
                        this.open_piano_roll();
                        this.publish_clip_edit();
                        this.refresh_project_dirty();
                        this.status_message = format!(
                            "♪ Audio → MIDI listo: {} notas ({}–{}) en «{clip_name}» · Hay Piano",
                            note_count,
                            midi_note_name(min_pitch),
                            midi_note_name(max_pitch),
                        );
                    }
                    Err(error) => {
                        this.status_message = format!("Audio → MIDI: {error}");
                    }
                }
                cx.notify();
            });
        }));
    }
}

fn midi_note_name(pitch: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = (i32::from(pitch) / 12) - 1;
    format!("{}{octave}", NAMES[(pitch % 12) as usize])
}
