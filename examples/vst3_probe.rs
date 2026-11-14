//! Diagnóstico manual sin salida física: carga un VST3 y calcula el pico de una nota.
#[path = "../src/audio/vst3/mod.rs"]
mod vst3_host;
fn main() -> Result<(), String> {
    vst3_host::environment::configure()?;
    if vst3_host::scan::worker() {
        return Ok(());
    }
    let args: Vec<_> = std::env::args().collect();
    let effect = args
        .iter()
        .position(|a| a == "--effect")
        .and_then(|i| args.get(i + 1));
    let path = std::env::args_os().nth(1).ok_or("Indica el bundle .vst3")?;
    let mut engine = if let Some(class) = effect {
        vst3_host::VstEngine::load_effect(std::path::Path::new(&path), 48000, class)?
    } else {
        vst3_host::VstEngine::load(std::path::Path::new(&path), 48000)?
    };
    println!("Cargado: {}", engine.name);
    if args.iter().any(|arg| arg == "--shared") {
        if effect.is_some() {
            return Err("--shared requiere un instrumento VST3".into());
        }
        let editor = engine.editor.clone();
        let shared = vst3_host::SharedInstrument::new(engine, 48_000);
        let preview = vst3_host::InstrumentVoice::preview(shared.clone());
        let arrangement = vst3_host::InstrumentVoice::arrangement(shared.clone());
        let meter = elevate::musical::time::Meter::new(4, 4).unwrap();
        let mut peaks = [0.0_f32; 3];
        let (mut left, mut right) = ([0.; 512], [0.; 512]);
        preview.midi(0x90, 60, 100);
        for block in 0..192 {
            if block == 64 {
                shared.enter_arrangement()?;
                arrangement.midi(0x90, 64, 100);
                preview.render(&mut left, &mut right, None);
                if left.iter().chain(&right).any(|sample| *sample != 0.) {
                    return Err("La audición procesó el VST3 durante Play".into());
                }
            } else if block == 96 {
                if !shared.queue_live_midi(0x90, 72, 100) {
                    return Err("Cola MIDI en vivo saturada".into());
                }
            } else if block == 128 {
                arrangement.midi(0x80, 64, 64);
                shared.leave_arrangement();
                arrangement.render(&mut left, &mut right, None);
                if left.iter().chain(&right).any(|sample| *sample != 0.) {
                    return Err("Play procesó el VST3 después de Stop".into());
                }
                preview.midi(0x90, 67, 100);
            }
            if block < 64 || block >= 128 {
                preview.render(&mut left, &mut right, None);
            } else {
                let time =
                    vst3_host::TransportContext::new(block * 512, 48_000, 120., meter, None)?;
                arrangement.render(&mut left, &mut right, Some(time));
            }
            for value in left.iter().chain(&right) {
                if !value.is_finite() {
                    return Err("Salida no finita".into());
                }
                peaks[block as usize / 64] = peaks[block as usize / 64].max(value.abs());
            }
        }
        println!("Picos audición/Play/audición: {peaks:?}");
        if shared.failed() || peaks.iter().any(|peak| *peak == 0.) {
            return Err("El cambio de ruta VST3 no produjo audio en sus tres etapas".into());
        }
        drop(preview);
        drop(arrangement);
        drop(shared);
        editor.wait_for_shutdown();
        return Ok(());
    }
    engine.midi(0x90, 60, 100);
    let (mut left, mut right) = ([0.; 512], [0.; 512]);
    let mut peak = 0_f32;
    for block in 0..190 {
        if block == 95 {
            engine.midi(0x80, 60, 64);
        }
        if effect.is_some() {
            let mut frames = [[0.; 2]; 512];
            for (i, f) in frames.iter_mut().enumerate() {
                let v = (((block * 512 + i) as f32 * 440. * std::f32::consts::TAU / 48000.).sin())
                    * 0.1;
                *f = [v, v];
            }
            engine.process_effect(&mut frames);
            for (i, f) in frames.iter().enumerate() {
                left[i] = f[0];
                right[i] = f[1];
            }
        } else {
            engine.render(&mut left, &mut right);
        }
        for value in left.iter().chain(&right) {
            if !value.is_finite() {
                return Err("Salida no finita".into());
            }
            peak = peak.max(value.abs());
        }
    }
    println!("Pico: {peak:.6} · error: {}", engine.failed);
    if engine.failed || peak == 0. {
        return Err("El instrumento no produjo audio".into());
    }
    if std::env::args().any(|arg| arg == "--editor") {
        engine.editor.open();
        for _ in 0..6000 {
            engine.render(&mut left, &mut right);
            if let Some(message) = engine.editor.take_message() {
                println!("{message}");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        engine.editor.close();
    }
    let editor = engine.editor.clone();
    drop(engine);
    editor.wait_for_shutdown();
    Ok(())
}
