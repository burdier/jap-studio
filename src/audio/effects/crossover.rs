use super::Plan;
use crate::audio::vst3::TransportContext;
use elevate::musical::fx::FxId;

// El historial pertenece al stream. Publicar un rack nuevo conserva la memoria de
// los cruces por ID. El bounce y la monitorización tienen historiales independientes.
struct CrossoverState {
    filter: [f64; 2],
    mix: [f32; 2],
}
pub(crate) struct CrossoverHistory {
    filters: Vec<(FxId, CrossoverState)>,
    branch_levels: Vec<(FxId, f32)>,
}
impl CrossoverHistory {
    pub fn new() -> Self {
        Self {
            filters: Vec::with_capacity(256),
            branch_levels: Vec::with_capacity(256),
        }
    }
    pub(super) fn apply_branch_gain(
        &mut self,
        id: FxId,
        target: f32,
        frames: &mut [[f32; 2]],
        rate: u32,
    ) {
        let index = self
            .branch_levels
            .iter()
            .position(|(key, _)| *key == id)
            .unwrap_or_else(|| {
                if self.branch_levels.len() == 256 {
                    self.branch_levels.remove(0);
                }
                self.branch_levels.push((id, target));
                self.branch_levels.len() - 1
            });
        let gain = &mut self.branch_levels[index].1;
        let step = 1. / (rate.max(1) as f32 * 0.005).max(1.);
        for sample in frames {
            *gain += (target - *gain).clamp(-step, step);
            sample[0] *= *gain;
            sample[1] *= *gain;
        }
    }
    fn filter(&mut self, id: FxId) -> &mut CrossoverState {
        let i = if let Some(i) = self.filters.iter().position(|s| s.0 == id) {
            i
        } else {
            // Una cadena tiene hasta 256 nodos. Retira un ID histórico al rotar proyectos.
            if self.filters.len() == 256 {
                self.filters.remove(0);
            }
            self.filters.push((
                id,
                CrossoverState {
                    filter: [0.; 2],
                    mix: [1.; 2],
                },
            ));
            self.filters.len() - 1
        };
        &mut self.filters[i].1
    }
    pub(super) fn process(
        &mut self,
        id: FxId,
        hz: u16,
        side: u32,
        low_fx: &Plan,
        high_fx: &Plan,
        frames: &mut [[f32; 2]],
        rate: u32,
        time: Option<TransportContext>,
        delays: &super::convolver::StreamDsp,
    ) {
        let g = (std::f64::consts::PI * (f64::from(hz) / f64::from(rate.max(1))).min(0.49)).tan();
        let a = g / (1. + g);
        let target = [
            if side == 2 { 0. } else { 1. },
            if side == 1 { 0. } else { 1. },
        ];
        let step = 1. / (rate.max(1) as f32 * 0.005).max(1.);
        for (n, block) in frames.chunks_mut(128).enumerate() {
            let mut low = [[0.; 2]; 128];
            let mut high = low;
            let state = self.filter(id);
            let mut mix = state.mix;
            let state = &mut state.filter;
            for (i, sample) in block.iter().enumerate() {
                for ch in 0..2 {
                    let delta = (f64::from(sample[ch]) - state[ch]) * a;
                    low[i][ch] = (state[ch] + delta) as f32;
                    state[ch] += 2. * delta;
                    high[i][ch] = sample[ch] - low[i][ch];
                }
            }
            let original_low = low;
            let original_high = high;
            let time = time.map(|t| t.offset(n * 128));
            low_fx.process_timed(&mut low[..block.len()], rate, time, self, delays);
            high_fx.process_timed(&mut high[..block.len()], rate, time, self, delays);
            // Sumar solo los cambios conserva exactamente el transitorio seco con FX neutros.
            for (i, frame) in block.iter_mut().enumerate() {
                for (gain, target) in mix.iter_mut().zip(target) {
                    *gain += (target - *gain).clamp(-step, step);
                }
                for ch in 0..2 {
                    frame[ch] += (mix[0] * low[i][ch] - original_low[i][ch])
                        + (mix[1] * high[i][ch] - original_high[i][ch]);
                }
            }
            self.filter(id).mix = mix;
        }
    }
}
