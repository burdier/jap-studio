//! Consulta de fábricas fuera del proceso de UI. El catálogo solo enumera archivos.
use super::module::Module;
use elevate::catalog::plugins::{InstalledInstrument, discover_instruments};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

pub(crate) fn worker() -> bool {
    let mut args = std::env::args_os();
    if args.nth(1).as_deref() != Some(std::ffi::OsStr::new("--vst3-inspect")) {
        return false;
    }
    let result = args
        .next()
        .ok_or("Falta bundle".to_owned())
        .and_then(|path| {
            let module = Module::open(std::path::Path::new(&path))?;
            for class in module.classes()? {
                println!(
                    "HAY_VST3\t{}\t{}\t{}",
                    class.id(),
                    u8::from(class.instrument),
                    class.name.replace(['\t', '\n', '\r'], " ")
                );
            }
            Ok(())
        });
    if let Err(error) = result {
        eprintln!("VST3: {error}");
    }
    true
}

pub(crate) fn discover() -> Vec<InstalledInstrument> {
    // La caché evita arrancar Wine cada vez que se abre [+]. Se invalida por mtime.
    type Cache = std::collections::HashMap<
        PathBuf,
        (Option<std::time::SystemTime>, Vec<InstalledInstrument>),
    >;
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    let mut result = Vec::new();
    for candidate in discover_instruments() {
        let modified = std::fs::metadata(&candidate.bundle)
            .and_then(|m| m.modified())
            .ok();
        let cached = CACHE
            .get_or_init(Default::default)
            .lock()
            .ok()
            .and_then(|cache| {
                cache
                    .get(&candidate.bundle)
                    .filter(|(time, _)| *time == modified)
                    .map(|(_, entries)| entries.clone())
            });
        if let Some(entries) = cached {
            result.extend(entries);
            continue;
        }
        match inspect(&candidate) {
            Ok(entries) => {
                if let Ok(mut cache) = CACHE.get().unwrap().lock() {
                    cache.insert(candidate.bundle.clone(), (modified, entries.clone()));
                }
                result.extend(entries);
            }
            Err(error) => eprintln!("Escaneo {}: {error}", candidate.bundle.display()),
        }
    }
    result
}
pub(crate) fn inspect(candidate: &InstalledInstrument) -> Result<Vec<InstalledInstrument>, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "hay-scan-{}-{}.log",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let run = (|| {
        let mut child = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
            .arg("--vst3-inspect")
            .arg(&candidate.bundle)
            .stdin(Stdio::null())
            .stdout(file.try_clone().map_err(|e| e.to_string())?)
            .stderr(file)
            .spawn()
            .map_err(|e| e.to_string())?;
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                if !status.success() {
                    return Err(format!("Proceso de escaneo terminó con {status}"));
                }
                break;
            }
            if start.elapsed() > Duration::from_secs(30) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Tiempo de escaneo agotado".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        Ok(content
            .lines()
            .filter_map(|line| {
                let mut fields = line.strip_prefix("HAY_VST3\t")?.splitn(3, '\t');
                let mut entry = candidate.clone();
                entry.class_id = fields.next()?.into();
                entry.instrument = fields.next()? == "1";
                entry.name = fields.next()?.into();
                Some(entry)
            })
            .collect())
    })();
    let _ = std::fs::remove_file(path);
    run
}
