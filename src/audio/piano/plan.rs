//! Ejecución del patrón por frames, compartida por la voz live y la exportación.
use super::SynthBackend;
use elevate::musical::{
    scheduler::{EventKind, PreparedPattern},
    time::FrameWindow,
};
pub(super) fn render_plan_segment(
    synthesizer: &mut SynthBackend,
    plan: &PreparedPattern,
    position: &mut u64,
    channel: u8,
    cycle: u64,
    left: &mut [f32],
    right: &mut [f32],
) -> bool {
    let total = left.len();
    let planned = plan
        .length_frames()
        .saturating_sub(*position)
        .min(total as u64) as usize;
    let Ok(window) = FrameWindow::new(*position, planned as u32) else {
        synthesizer.render(left, right);
        return true;
    };
    let mut cursor = 0;
    for event in plan.events_in(window) {
        let offset = (event.sample_offset as usize).min(planned);
        if offset > cursor {
            synthesizer.render(&mut left[cursor..offset], &mut right[cursor..offset]);
            cursor = offset;
        }
        play_event(synthesizer, event, channel, cycle);
    }
    if cursor < planned {
        synthesizer.render(&mut left[cursor..planned], &mut right[cursor..planned]);
    }
    *position = position.saturating_add(total as u64);
    if *position >= plan.length_frames() {
        // Los Off exactamente en el final quedan fuera de la ventana semiabierta.
        if let Ok(end) = FrameWindow::new(plan.length_frames(), 1) {
            for event in plan
                .events_in(end)
                .filter(|event| matches!(event.kind, EventKind::NoteOff))
            {
                play_event(synthesizer, event, channel, cycle);
            }
        }
        synthesizer.midi(0xb0 | channel, 64, 0);
        synthesizer.note_off_all();
        if planned < total {
            synthesizer.render(&mut left[planned..], &mut right[planned..]);
        }
        return true;
    }
    false
}

/// Divide el bloque en los límites del ciclo. Una vuelta consume al menos un frame;
/// el trabajo queda acotado por el tamaño del bloque, incluso con patrones diminutos.
pub(super) fn render_plan_block(
    synthesizer: &mut SynthBackend,
    plan: &PreparedPattern,
    position: &mut u64,
    looped: bool,
    channel: u8,
    cycle: &mut u64,
    left: &mut [f32],
    right: &mut [f32],
) -> bool {
    if !looped || plan.length_frames() == 0 {
        return render_plan_segment(synthesizer, plan, position, channel, *cycle, left, right);
    }
    let mut written = 0;
    while written < left.len() {
        *position %= plan.length_frames();
        let count = (plan.length_frames() - *position).min((left.len() - written) as u64) as usize;
        let end = written + count;
        render_plan_segment(
            synthesizer,
            plan,
            position,
            channel,
            *cycle,
            &mut left[written..end],
            &mut right[written..end],
        );
        written = end;
        if *position >= plan.length_frames() {
            *position = 0;
            *cycle = cycle.wrapping_add(1);
        }
    }
    false
}

fn play_event(
    synthesizer: &mut SynthBackend,
    event: elevate::musical::scheduler::ScheduledEvent,
    channel: u8,
    cycle: u64,
) {
    match event.kind {
        EventKind::Control { lane, value } => {
            let (command, a, b) = lane.midi(value);
            synthesizer.midi(command | channel, a, b);
        }
        EventKind::NoteOn { velocity } if event.expression.plays(event.note, cycle) => {
            synthesizer.note_on_expression(channel, event.pitch, velocity, event.expression)
        }
        EventKind::NoteOff if event.expression.plays(event.note, cycle) => synthesizer.midi(
            0x80 | channel,
            event.pitch,
            event.expression.release_velocity,
        ),
        _ => {}
    }
}
