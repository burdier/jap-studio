//! Inspección offline de eventos. No abre audio ni ventana.
use elevate::musical::{
    model::{InstrumentId, Pattern},
    scheduler::PreparedPattern,
    time::{PPQ, Tick, TimeBase, Transport},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let time = TimeBase::new(48000, 120.0)?;
    let mut pattern = Pattern::new(Tick(PPQ * 4))?;
    for beat in 0..4 {
        pattern.add_note(InstrumentId(1), Tick(beat * PPQ), Tick(PPQ / 4), 36, 100)?;
    }
    let prepared = PreparedPattern::compile(&pattern, time)?;
    let mut transport = Transport::default();
    transport.play();
    while transport.frame() <= time.frame_at(pattern.length())? {
        let window = transport.advance(256)?.expect("transporte en reproducción");
        for event in prepared.events_in(window) {
            println!(
                "frame={} offset={} {:?}",
                window.start() + u64::from(event.sample_offset),
                event.sample_offset,
                event.kind
            );
        }
    }
    Ok(())
}
