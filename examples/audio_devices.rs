//! Inventario real del DAW. --connect-silent abre brevemente una salida sin señal.
#[cfg(feature = "ui")]
#[allow(dead_code)]
#[path = "../src/audio/device.rs"]
mod device;
#[cfg(feature = "ui")]
#[allow(dead_code)]
#[path = "../src/audio/output.rs"]
mod output;

#[cfg(feature = "ui")]
fn main() {
    let filter = std::env::args().nth(1).unwrap_or_default().to_lowercase();
    let config = device::AudioConfig {
        host: std::env::args().nth(2),
        ..Default::default()
    };
    if std::env::args().any(|arg| arg == "--connect-silent") {
        if let Err(error) = connect_silent(config, &filter) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    match device::scan(&config) {
        Ok(inventory) => {
            let inventory: device::DeviceInventory = inventory;
            println!(
                "{} · {} dispositivos",
                inventory.host,
                inventory.devices.len()
            );
            for d in inventory.devices.iter().filter(|d| {
                d.name.to_lowercase().contains(&filter) || d.id.to_lowercase().contains(&filter)
            }) {
                println!(
                    "{} · {}\n  IN: {}\n  OUT: {}\n  Hz: {:?}",
                    d.name,
                    d.id,
                    d.status(true),
                    d.status(false),
                    d.rates(false)
                );
                for n in d.output_pairs() {
                    println!("  {} · canal local {}", d.output_label(n), n + 1);
                }
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
#[cfg(not(feature = "ui"))]
fn main() {
    eprintln!("Requiere la feature ui (CPAL)");
}

#[cfg(feature = "ui")]
fn connect_silent(mut config: device::AudioConfig, filter: &str) -> Result<(), String> {
    use cpal::traits::{DeviceTrait, StreamTrait};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    };
    let inventory = device::scan(&config)?;
    let matches: Vec<_> = inventory
        .devices
        .iter()
        .filter(|d| d.supports(false) && d.name.to_lowercase().contains(filter))
        .collect();
    let [selected] = matches.as_slice() else {
        return Err("Indica un nombre de salida único".into());
    };
    config.output = Some(selected.id.clone());
    let device = config.device(false)?;
    let setup = output::choose_output_config(&device, &config)?;
    if setup.format != cpal::SampleFormat::F32 {
        return Err("La conexión silenciosa requiere F32".into());
    }
    let samples = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicBool::new(false));
    let count = samples.clone();
    let error = failed.clone();
    let (error_tx, error_rx) = std::sync::mpsc::sync_channel(1);
    let stream = device
        .build_output_stream(
            setup.config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                data.fill(0.);
                count.fetch_add(data.len() as u64, Ordering::Relaxed);
            },
            move |err| {
                error.store(true, Ordering::Relaxed);
                let _ = error_tx.try_send(err);
            },
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    println!("Conexión sin señal: {}", selected.name);
    std::thread::sleep(std::time::Duration::from_secs(8));
    // Capturar el resultado mientras el stream sigue vivo: el backend puede emitir
    // DeviceDisconnected al desmontar su propio core durante Drop.
    let count = samples.load(Ordering::Relaxed);
    let runtime_error = error_rx.try_recv().ok();
    let runtime_failed = failed.load(Ordering::Relaxed);
    drop(stream);
    if let Some(error) = runtime_error {
        return Err(format!("Stream: {error} · {count} muestras"));
    }
    if runtime_failed || count == 0 {
        return Err("El stream no procesó bloques o informó un error".into());
    }
    println!("Conexión cerrada · {count} muestras de silencio procesadas · sin errores");
    Ok(())
}
