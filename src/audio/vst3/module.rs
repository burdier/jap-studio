//! Carga dinámica multiplataforma de módulos VST3 (Linux .so / yabridge, Windows .vst3, macOS).
//! Nunca desde el callback RT.
use crate::audio::dynlib::{DynLib, close_raw, symbol_raw};
use std::{
    collections::HashMap,
    ffi::{CString, c_void},
    path::{Path, PathBuf},
    ptr,
    sync::{Mutex, OnceLock},
};
use vst3::{ComPtr, Interface, Steinberg::*};

pub(super) struct Module {
    pub factory: Option<ComPtr<IPluginFactory>>,
    path: PathBuf,
}
struct Loaded {
    handle: usize,
    users: usize,
    factory: unsafe extern "C" fn() -> *mut IPluginFactory,
    exit: Option<unsafe extern "C" fn() -> bool>,
}
// ModuleEntry/Exit pertenecen al bundle. Un bounce puede solaparse con Play y con otros editores.
fn libraries() -> &'static Mutex<HashMap<PathBuf, Loaded>> {
    static TABLE: OnceLock<Mutex<HashMap<PathBuf, Loaded>>> = OnceLock::new();
    TABLE.get_or_init(Default::default)
}

fn resolve_vst3_binary(bundle: &Path) -> Result<PathBuf, String> {
    #[cfg(target_os = "linux")]
    {
        if bundle
            .components()
            .any(|part| part.as_os_str() == "yabridge")
            && std::env::var_os("WINELOADER").is_none()
            && !std::env::var_os("PATH").is_some_and(|path| {
                std::env::split_paths(&path).any(|dir| dir.join("wine").is_file())
            })
        {
            return Err("yabridge necesita Wine. En gpui-dev inicia Hay con bash scripts/run.sh para usar el Wine del sistema".into());
        }
        if bundle.is_file() {
            return Err(
                "Este archivo VST3 es de Windows; selecciona su bundle Linux en ~/.vst3/yabridge"
                    .into(),
            );
        }
        let name = bundle.file_stem().ok_or("Bundle VST3 inválido")?;
        Ok(bundle
            .join("Contents")
            .join(format!("{}-linux", std::env::consts::ARCH))
            .join(format!("{}.so", name.to_string_lossy())))
    }
    #[cfg(target_os = "windows")]
    {
        if bundle.is_file() {
            return Ok(bundle.to_path_buf());
        }
        let name = bundle.file_stem().ok_or("Bundle VST3 inválido")?;
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x86_64-win",
            "aarch64" => "arm64-win",
            other => other,
        };
        Ok(bundle
            .join("Contents")
            .join(arch)
            .join(format!("{}.vst3", name.to_string_lossy())))
    }
    #[cfg(target_os = "macos")]
    {
        let name = bundle.file_stem().ok_or("Bundle VST3 inválido")?;
        Ok(bundle
            .join("Contents")
            .join("MacOS")
            .join(name.to_string_lossy().as_ref()))
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = bundle;
        Err("Carga de módulos VST3 no soportada en esta plataforma".into())
    }
}

impl Module {
    pub fn open(bundle: &Path) -> Result<Self, String> {
        let binary = resolve_vst3_binary(bundle)?;
        let key = binary
            .canonicalize()
            .map_err(|e| format!("{}: {e}", binary.display()))?;
        let mut libraries = libraries().lock().map_err(|e| e.to_string())?;
        if let Some(lib) = libraries.get_mut(&key) {
            let factory =
                unsafe { ComPtr::from_raw((lib.factory)()) }.ok_or("VST3: fábrica vacía")?;
            lib.users += 1;
            return Ok(Self {
                factory: Some(factory),
                path: key,
            });
        }
        let path = CString::new(binary.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
        let lib = DynLib::open(&path)?;
        let handle = lib.raw();
        unsafe {
            let factory: unsafe extern "C" fn() -> *mut IPluginFactory =
                symbol_raw(handle, c"GetPluginFactory")?;

            #[cfg(target_os = "linux")]
            let exit = {
                let entry: unsafe extern "C" fn(*mut c_void) -> bool =
                    symbol_raw(handle, c"ModuleEntry")?;
                let exit: unsafe extern "C" fn() -> bool = symbol_raw(handle, c"ModuleExit")?;
                if !entry(handle) {
                    return Err("VST3: ModuleEntry falló".into());
                }
                Some(exit)
            };
            #[cfg(target_os = "windows")]
            let exit = {
                if let Ok(init_dll) = symbol_raw::<unsafe extern "C" fn() -> bool>(handle, c"InitDll") {
                    if !init_dll() {
                        return Err("VST3: InitDll falló".into());
                    }
                }
                symbol_raw::<unsafe extern "C" fn() -> bool>(handle, c"ExitDll").ok()
            };
            #[cfg(target_os = "macos")]
            let exit = {
                let _ = symbol_raw::<unsafe extern "C" fn(*mut c_void) -> bool>(handle, c"bundleEntry")
                    .map(|entry| entry(handle));
                symbol_raw::<unsafe extern "C" fn() -> bool>(handle, c"bundleExit").ok()
            };
            #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
            let exit: Option<unsafe extern "C" fn() -> bool> = None;

            let Some(interface) = ComPtr::from_raw(factory()) else {
                if let Some(exit_fn) = exit {
                    exit_fn();
                }
                return Err("VST3: fábrica vacía".into());
            };
            let raw_handle = lib.into_raw();
            libraries.insert(
                key.clone(),
                Loaded {
                    handle: raw_handle as usize,
                    users: 1,
                    factory,
                    exit,
                },
            );
            Ok(Self {
                factory: Some(interface),
                path: key,
            })
        }
    }
    pub fn create<I: Interface>(&self, cid: &TUID) -> Result<ComPtr<I>, String> {
        let mut raw = ptr::null_mut();
        let factory = self.factory.as_ref().unwrap();
        unsafe {
            let result = factory.createInstance(cid.as_ptr(), I::IID.as_ptr().cast(), &mut raw);
            if result != kResultOk {
                return Err(format!("VST3: createInstance ({result})"));
            }
            ComPtr::from_raw(raw.cast()).ok_or("VST3: instancia vacía".into())
        }
    }
    pub fn classes(&self) -> Result<Vec<ClassInfo>, String> {
        let factory = self.factory.as_ref().unwrap();
        let extended = factory
            .cast::<IPluginFactory2>()
            .ok_or("VST3: fábrica sin metadatos de categoría")?;
        let mut result = Vec::new();
        unsafe {
            for index in 0..factory.countClasses().clamp(0, 4096) {
                let mut info: PClassInfo2 = std::mem::zeroed();
                if extended.getClassInfo2(index, &mut info) != kResultOk
                    || text(&info.category) != "Audio Module Class"
                {
                    continue;
                }
                let categories = text(&info.subCategories);
                result.push(ClassInfo {
                    cid: info.cid,
                    name: text(&info.name),
                    instrument: categories.split('|').any(|s| s == "Instrument"),
                });
            }
        }
        Ok(result)
    }
    pub fn audio_class(&self, id: Option<&str>, instrument: bool) -> Result<ClassInfo, String> {
        self.classes()?
            .into_iter()
            .find(|c| c.instrument == instrument && id.is_none_or(|id| c.id() == id))
            .ok_or("VST3: no contiene la clase solicitada o su tipo no coincide".into())
    }
}
impl Drop for Module {
    fn drop(&mut self) {
        self.factory.take();
        let mut libraries = libraries().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(lib) = libraries.get_mut(&self.path) {
            lib.users -= 1;
            if lib.users == 0 {
                unsafe {
                    if let Some(exit_fn) = lib.exit {
                        exit_fn();
                    }
                    close_raw(lib.handle as *mut c_void);
                }
                libraries.remove(&self.path);
            }
        }
    }
}

pub(super) struct ClassInfo {
    pub cid: TUID,
    pub name: String,
    pub instrument: bool,
}
impl ClassInfo {
    pub fn id(&self) -> String {
        self.cid
            .iter()
            .map(|b| format!("{:02X}", *b as u8))
            .collect()
    }
}
fn text(bytes: &[i8]) -> String {
    String::from_utf8_lossy(
        &bytes
            .iter()
            .take_while(|b| **b != 0)
            .map(|b| *b as u8)
            .collect::<Vec<_>>(),
    )
    .into_owned()
}
