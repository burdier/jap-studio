//! Adaptador de datos DSP al componente visual independiente.
use super::{
    DesignSystem,
    components::spectrum::{SpectrumData, SpectrumMode},
};
use crate::audio::analyzer::SpectrumAnalyzer;
pub(in crate::ui) fn data(
    analyzer: Option<&SpectrumAnalyzer>,
    mode: SpectrumMode,
    ds: &DesignSystem,
) -> SpectrumData {
    let mut data = SpectrumData {
        color: ds.cyan,
        hot: ds.accent,
        background: ds.panel_dark,
        border: ds.border,
        muted: ds.muted,
        height: 110.,
        upper_hz: 20000.,
        history_capacity: 128,
        ..Default::default()
    };
    if let Some(analyzer) = analyzer {
        data.upper_hz = analyzer.upper_hz();
        data.seconds = analyzer.history_seconds();
        data.history_capacity = analyzer.history_capacity();
        (data.peak, data.rms) = analyzer.levels();
        match mode {
            SpectrumMode::Spectrum => data.spectrum = analyzer.bands().to_vec(),
            SpectrumMode::Visualizer => {
                let (min, max) = analyzer.waveform();
                data.wave_min = min.to_vec();
                data.wave_max = max.to_vec();
            }
            SpectrumMode::SpectrogramLog | SpectrumMode::SpectrogramLinear => {
                data.history_bins = analyzer.bands().len();
                data.history = analyzer
                    .history(mode == SpectrumMode::SpectrogramLinear)
                    .flatten()
                    .copied()
                    .collect();
            }
            SpectrumMode::Level => {}
        }
    }
    data
}
