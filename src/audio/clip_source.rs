//! Fuente inmutable de clip preparada: sin stream, reloj propio, E/S ni reservas al renderizar.
use super::{AudioClip, AudioFile};
use elevate::StereoFrame;
use std::sync::Arc;

pub(super) struct ClipSource {
    file: Arc<AudioFile>,
    converted: Option<Arc<Vec<StereoFrame>>>,
    start: u64,
    offset: u64,
    end: u64,
}
impl ClipSource {
    /// Los recursos ya están decodificados/remuestreados; preparar fuera del callback.
    pub fn new(
        clip: &AudioClip,
        converted: Option<Arc<Vec<StereoFrame>>>,
        rate: u32,
        beats_per_second: f64,
    ) -> Self {
        let rate = f64::from(rate);
        let start = (clip.start_beat / beats_per_second * rate).round() as u64;
        Self {
            file: clip.file.clone(),
            converted,
            start,
            offset: (clip.trim_start * rate).round() as u64,
            end: start.saturating_add((clip.length_seconds() * rate).round() as u64),
        }
    }
    pub fn end_frame(&self) -> u64 {
        self.end
    }
    /// Sobrescribe el bloque; silencio fuera del intervalo visible o de las muestras disponibles.
    /// La posición pertenece al llamador, para poder compartirla entre varias fuentes.
    pub fn render(&self, position: u64, output: &mut [StereoFrame]) {
        let samples = self.converted.as_deref().unwrap_or(&self.file.frames);
        for (index, frame) in output.iter_mut().enumerate() {
            let absolute = position.saturating_add(index as u64);
            *frame = absolute
                .checked_sub(self.start)
                .filter(|_| absolute < self.end)
                .and_then(|relative| relative.checked_add(self.offset))
                .and_then(|source| usize::try_from(source).ok())
                .and_then(|source| samples.get(source))
                .copied()
                .unwrap_or([0.; 2]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elevate::musical::clip::ClipProps;
    fn clip() -> AudioClip {
        AudioClip {
            id: elevate::musical::clip::ClipId::fresh(),
            file: Arc::new(AudioFile {
                name: "fixture".into(),
                source_path: None,
                sample_rate: 4,
                frames: (0..12).map(|i| [i as f32, -(i as f32)]).collect(),
                peaks: vec![],
                max_peak: 11.,
            }),
            name: "recorte".into(),
            track: 0,
            start_beat: 2.,
            trim_start: 0.5,
            trim_len: Some(1.),
            props: ClipProps::default(),
        }
    }
    #[test]
    fn trimmed_source_is_aligned_and_independent_of_block_size_and_seek_order() {
        let source = ClipSource::new(&clip(), None, 4, 2.);
        let mut whole = [[99.; 2]; 12];
        source.render(0, &mut whole);
        assert_eq!(&whole[..4], &[[0.; 2]; 4]);
        assert_eq!(&whole[4..8], &[[2., -2.], [3., -3.], [4., -4.], [5., -5.]]);
        assert_eq!(&whole[8..], &[[0.; 2]; 4]);
        let mut parts = [[99.; 2]; 12];
        for (i, block) in parts.chunks_mut(3).enumerate() {
            source.render((i * 3) as u64, block);
        }
        assert_eq!(parts, whole);
        let mut seek = [[0.; 2]; 2];
        source.render(6, &mut seek);
        assert_eq!(seek, whole[6..8]);
        source.render(4, &mut seek);
        assert_eq!(seek, whole[4..6]);
        assert_eq!(source.end_frame(), 8);
    }
    #[test]
    fn converted_samples_use_output_rate_and_missing_tail_is_silent() {
        let converted = Arc::new((0..10).map(|i| [100. + i as f32, 0.]).collect());
        let source = ClipSource::new(&clip(), Some(converted), 8, 2.);
        let mut output = [[1.; 2]; 8];
        source.render(8, &mut output);
        assert_eq!(output[0], [104., 0.]);
        assert_eq!(output[5], [109., 0.]);
        assert_eq!(&output[6..], &[[0.; 2]; 2]);
        assert_eq!(source.end_frame(), 16);
        source.render(u64::MAX, &mut output);
        assert_eq!(output, [[0.; 2]; 8]);
    }
}
