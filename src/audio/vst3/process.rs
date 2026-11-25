//! Traducción MIDI y render por segmentos del scheduler existente.
use super::{TransportContext, VstEngine, events};
use std::ptr;
use vst3::Steinberg::{Vst::*, kResultOk};
impl VstEngine {
    pub(crate) fn fix_export_latency(&mut self) {
        self.export_latency = Some(self.editor.latency_samples());
    }
    /// El host acota colas infinitas y garantiza tiempo para releases de instrumentos.
    pub fn tail_frames(&self, rate: u32) -> u64 {
        self.effect_tail_frames(rate).max(u64::from(rate) * 2)
    }
    pub fn effect_tail_frames(&self, rate: u32) -> u64 {
        let reported = self
            .processor
            .as_ref()
            .map_or(0, |processor| unsafe { processor.getTailSamples() });
        let maximum = u64::from(rate).saturating_mul(10);
        if reported == vst3::Steinberg::Vst::kInfiniteTail {
            u64::from(rate).saturating_mul(5)
        } else {
            u64::from(reported).min(maximum)
        }
    }
    pub fn midi(&mut self, status: u8, a: u8, b: u8) {
        let channel = (status & 15) as usize;
        let on = status & 0xf0 == 0x90 && b != 0;
        match status & 0xf0 {
            0x80 | 0x90 if a < 128 => {
                if !self.events.push(events::note(channel as u8, a, b, on)) {
                    self.panic();
                    return;
                }
                self.held[channel][a as usize] = on;
            }
            0xb0 if a == 120 || a == 123 => self.panic(),
            0xb0 => self.cc(channel, a as usize, b as f64 / 127.),
            0xd0 => self.cc(channel, 128, a as f64 / 127.),
            0xe0 => self.cc(channel, 129, ((b as u16) << 7 | a as u16) as f64 / 16383.),
            _ => {}
        }
    }
    pub(super) fn cc(&self, channel: usize, cc: usize, value: f64) {
        if let Some(id) = self.mapping[channel].get(cc).copied().flatten() {
            self.changes.set(id, value);
        }
    }
    pub fn panic(&mut self) {
        self.events.retain_note_offs();
        self.mpe_voices.fill(None);
        for channel in 0..16 {
            self.cc(channel, 64, 0.);
            for pitch in 0..128 {
                if self.held[channel][pitch] {
                    if !self
                        .events
                        .push(events::note(channel as u8, pitch as u8, 0, false))
                    {
                        self.failed = true;
                        return;
                    }
                    self.held[channel][pitch] = false;
                }
            }
        }
    }
    pub fn process_effect(&mut self, frames: &mut [[f32; 2]]) {
        self.process_effect_timed(frames, None);
    }
    pub fn process_effect_timed(
        &mut self,
        frames: &mut [[f32; 2]],
        time: Option<TransportContext>,
    ) {
        self.process_effect_timed_params(frames, time, &[]);
    }
    pub fn process_effect_timed_params(
        &mut self,
        frames: &mut [[f32; 2]],
        time: Option<TransportContext>,
        params: &[(u32, f64)],
    ) {
        self.process_effect_timed_params_sidechain(frames, time, params, None);
    }
    pub fn process_effect_timed_params_sidechain(
        &mut self,
        frames: &mut [[f32; 2]],
        time: Option<TransportContext>,
        params: &[(u32, f64)],
        sidechain: Option<&[[f32; 2]]>,
    ) {
        if frames.is_empty() {
            return;
        }
        if self.failed {
            frames.fill([0.0; 2]);
            return;
        }
        for (index, block) in frames.chunks_mut(512).enumerate() {
            let offset = index * 512;
            let sc_slice = sidechain.and_then(|sc| {
                if offset < sc.len() {
                    Some(&sc[offset..(offset + block.len()).min(sc.len())])
                } else {
                    None
                }
            });
            self.buffers.input(block);
            self.buffers.sidechain_input(sc_slice, block.len());
            self.render_effect_slice(block, time.map(|time| time.offset(offset)), params);
            if self.failed {
                block.fill([0.0; 2]);
            }
        }
    }

    fn render_effect_slice(
        &mut self,
        block: &mut [[f32; 2]],
        time: Option<TransportContext>,
        params: &[(u32, f64)],
    ) {
        let _access = match self.editor.latency.enter(self.export_latency) {
            Ok(Some(guard)) => guard,
            result => {
                self.failed = result.is_err();
                block.fill([0.0; 2]);
                return;
            }
        };
        for &(id, value) in params {
            self.changes.set(id, value);
        }
        let mut last_edit = None;
        for _ in 0..256 {
            let Ok((seq, id, value)) = self.edits.pop() else {
                break;
            };
            self.changes.set(id, value);
            last_edit = Some(seq);
        }
        let len = block.len();
        self.buffers.clear_len(len);
        let mut context = if let Some(time) = time {
            time.vst()
        } else {
            let mut context: ProcessContext = unsafe { std::mem::zeroed() };
            context.sampleRate = self.rate;
            context.projectTimeSamples = self.frame;
            context
        };
        let frame = context.projectTimeSamples;
        let mut data = ProcessData {
            processMode: 0,
            symbolicSampleSize: 0,
            numSamples: len as i32,
            numInputs: self.buffers.inputs.len() as i32,
            numOutputs: self.buffers.outputs.len() as i32,
            inputs: self.buffers.inputs.as_mut_ptr(),
            outputs: self.buffers.outputs.as_mut_ptr(),
            inputParameterChanges: self.changes_interface.as_ptr(),
            outputParameterChanges: self.feedback_interface.as_ptr(),
            inputEvents: self.events_interface.as_ptr(),
            outputEvents: ptr::null_mut(),
            processContext: &mut context,
        };
        if unsafe { self.processor.as_ref().unwrap().process(&mut data) } != kResultOk {
            self.failed = true;
            block.fill([0.0; 2]);
        } else {
            if let Some(seq) = last_edit {
                self.applied_edit
                    .store(seq, std::sync::atomic::Ordering::Release);
            }
            self.feedback.send_latest(&mut self.to_controller);
            if !self.buffers.copy_output_interleaved(block) {
                self.failed = true;
                block.fill([0.0; 2]);
            }
        }
        self.feedback.clear();
        self.events.clear();
        self.changes.clear();
        self.frame = frame.saturating_add(len as i64);
    }
    #[allow(dead_code)] // El diagnóstico vst3_probe usa la ruta local sin transporte.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.render_timed(left, right, None);
    }
    pub fn render_timed(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        time: Option<TransportContext>,
    ) {
        self.render_timed_params(left, right, time, &[]);
    }
    pub fn render_timed_params(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        time: Option<TransportContext>,
        params: &[(u32, f64)],
    ) {
        if left.is_empty() {
            return;
        }
        if self.failed || left.len() != right.len() || left.len() > 512 {
            left.fill(0.);
            right.fill(0.);
            return;
        }
        let mut start = 0;
        while start < left.len() {
            let count = self.pitch_slice_len(left.len() - start);
            self.prepare_note_pitch(count);
            let end = start + count;
            self.render_slice(
                &mut left[start..end],
                &mut right[start..end],
                time.map(|time| time.offset(start)),
                params,
            );
            start = end;
            if self.failed {
                left[start..].fill(0.);
                right[start..].fill(0.);
                return;
            }
        }
    }

    fn render_slice(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        time: Option<TransportContext>,
        params: &[(u32, f64)],
    ) {
        let _access = match self.editor.latency.enter(self.export_latency) {
            Ok(Some(guard)) => guard,
            result => {
                self.failed = result.is_err();
                left.fill(0.);
                right.fill(0.);
                return;
            }
        };
        for &(id, value) in params {
            self.changes.set(id, value);
        }
        let mut last_edit = None;
        for _ in 0..256 {
            let Ok((seq, id, value)) = self.edits.pop() else {
                break;
            };
            self.changes.set(id, value);
            last_edit = Some(seq);
        }
        self.buffers.clear_len(left.len());
        let mut context = if let Some(time) = time {
            time.vst()
        } else {
            let mut context: ProcessContext = unsafe { std::mem::zeroed() };
            context.sampleRate = self.rate;
            context.projectTimeSamples = self.frame;
            context
        };
        let frame = context.projectTimeSamples;
        let mut data = ProcessData {
            processMode: 0,
            symbolicSampleSize: 0,
            numSamples: left.len() as i32,
            numInputs: self.buffers.inputs.len() as i32,
            numOutputs: self.buffers.outputs.len() as i32,
            inputs: self.buffers.inputs.as_mut_ptr(),
            outputs: self.buffers.outputs.as_mut_ptr(),
            inputParameterChanges: self.changes_interface.as_ptr(),
            outputParameterChanges: self.feedback_interface.as_ptr(),
            inputEvents: self.events_interface.as_ptr(),
            outputEvents: ptr::null_mut(),
            processContext: &mut context,
        };
        if unsafe { self.processor.as_ref().unwrap().process(&mut data) } != kResultOk {
            self.failed = true;
            left.fill(0.);
            right.fill(0.);
        } else {
            if let Some(seq) = last_edit {
                self.applied_edit
                    .store(seq, std::sync::atomic::Ordering::Release);
            }
            self.feedback.send_latest(&mut self.to_controller);
            self.buffers.copy_output(left, right);
            if left
                .iter()
                .chain(right.iter())
                .any(|sample| !sample.is_finite())
            {
                self.failed = true;
                left.fill(0.);
                right.fill(0.);
            }
        }
        self.feedback.clear();
        self.events.clear();
        self.changes.clear();
        self.frame = frame.saturating_add(left.len() as i64);
    }
}
