//! Traducción MIDI y render por segmentos del scheduler existente.
use super::{VstEngine, events};
use std::ptr;
use vst3::Steinberg::{Vst::*, kResultOk};
impl VstEngine {
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
    fn cc(&self, channel: usize, cc: usize, value: f64) {
        if let Some(id) = self.mapping[channel].get(cc).copied().flatten() {
            self.changes.set(id, value);
        }
    }
    pub fn panic(&mut self) {
        self.events.clear();
        for channel in 0..16 {
            self.cc(channel, 64, 0.);
            for pitch in 0..128 {
                if self.held[channel][pitch] {
                    self.events
                        .push(events::note(channel as u8, pitch as u8, 0, false));
                    self.held[channel][pitch] = false;
                }
            }
        }
    }
    pub fn process_effect(&mut self, frames: &mut [[f32; 2]]) {
        for block in frames.chunks_mut(512) {
            self.buffers.input(block);
            let (mut left, mut right) = ([0.; 512], [0.; 512]);
            self.render(&mut left[..block.len()], &mut right[..block.len()]);
            for (i, frame) in block.iter_mut().enumerate() {
                *frame = [left[i], right[i]];
            }
        }
    }
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        left.fill(0.);
        right.fill(0.);
        if self.failed || left.len() != right.len() || left.len() > 512 {
            return;
        }
        for _ in 0..256 {
            let Ok((id, value)) = self.edits.pop() else {
                break;
            };
            self.changes.set(id, value);
        }
        self.buffers.clear();
        let mut context: ProcessContext = unsafe { std::mem::zeroed() };
        context.sampleRate = self.rate;
        context.projectTimeSamples = self.frame;
        let mut data = ProcessData {
            processMode: 0,
            symbolicSampleSize: 0,
            numSamples: left.len() as i32,
            numInputs: self.buffers.inputs.len() as i32,
            numOutputs: self.buffers.outputs.len() as i32,
            inputs: self.buffers.inputs.as_mut_ptr(),
            outputs: self.buffers.outputs.as_mut_ptr(),
            inputParameterChanges: self.changes_interface.as_ptr(),
            outputParameterChanges: ptr::null_mut(),
            inputEvents: self.events_interface.as_ptr(),
            outputEvents: ptr::null_mut(),
            processContext: &mut context,
        };
        if unsafe { self.processor.as_ref().unwrap().process(&mut data) } != kResultOk {
            self.failed = true;
            left.fill(0.);
            right.fill(0.);
        } else {
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
        self.events.clear();
        self.changes.clear();
        self.frame += left.len() as i64;
    }
}
