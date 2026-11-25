//! Registro de componentes Lua y sus instancias; el callback solo ve planes preparados.
use super::{DawApp, fx::FxTarget};
use crate::audio::effects::ComponentEffect;
use elevate::{
    ProcessSpec,
    devices::audio_component::AudioComponentDevice,
    musical::{
        fx::{FxEdit, FxId, FxKind, FxNode},
        track::TrackDevice,
    },
    scripting::audio_components::{AudioComponent, parse},
};
use gpui::{AppContext, Context, Task};
use std::{collections::BTreeMap, path::Path, sync::Arc};

pub(super) struct LuaComponents {
    pub registry: BTreeMap<String, Arc<AudioComponent>>,
    pub instances: BTreeMap<FxId, Arc<ComponentEffect>>,
    pub task: Option<Task<()>>,
}

impl LuaComponents {
    pub fn new() -> Self {
        let registry = [
            include_str!("../../assets/scripts/components/audio/double_volume.lua"),
            include_str!("../../assets/scripts/components/audio/filter.lua"),
            include_str!("../../assets/scripts/components/audio/compressor.lua"),
            include_str!("../../assets/scripts/components/audio/limiter.lua"),
            include_str!("../../assets/scripts/components/audio/echo.lua"),
            include_str!("../../assets/scripts/components/audio/reverb.lua"),
        ]
        .into_iter()
        .map(|source| {
            let descriptor = parse(source).expect("Componente Lua distribuido inválido");
            (descriptor.id.clone(), Arc::new(descriptor))
        })
        .collect();
        Self {
            registry,
            instances: BTreeMap::new(),
            task: None,
        }
    }
}

fn scan() -> Result<BTreeMap<String, Arc<AudioComponent>>, String> {
    let mut roots = vec![(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts/components/audio"),
        true,
    )];
    if let Some(root) = dirs::config_dir() {
        roots.push((root.join("hay-daw/components/audio"), false));
    }
    scan_roots(roots)
}

fn scan_roots(
    roots: Vec<(std::path::PathBuf, bool)>,
) -> Result<BTreeMap<String, Arc<AudioComponent>>, String> {
    let mut registry = BTreeMap::new();
    let mut total = 0usize;
    for (root, bundled) in roots {
        if !root.exists() {
            continue;
        }
        if std::fs::symlink_metadata(&root)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err(format!(
                "{}: directorio enlazado no admitido",
                root.display()
            ));
        }
        let mut paths = Vec::new();
        for item in std::fs::read_dir(&root)
            .map_err(|e| e.to_string())?
            .take(1025)
        {
            let item = item.map_err(|e| e.to_string())?;
            paths.push(item.path());
        }
        if paths.len() > 1024 {
            return Err(format!("{}: demasiadas entradas", root.display()));
        }
        paths.sort();
        for path in paths {
            if path.extension().is_none_or(|ext| ext != "lua") {
                continue;
            }
            let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if !metadata.file_type().is_file() {
                return Err(format!("{}: solo archivos regulares", path.display()));
            }
            if metadata.len() > 256 * 1024 {
                return Err(format!("{}: supera 256 KiB", path.display()));
            }
            let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            total += source.len();
            if total > 8 * 1024 * 1024 || registry.len() >= 128 {
                return Err("Demasiados componentes Lua".into());
            }
            let component = parse(&source).map_err(|e| format!("{}: {e}", path.display()))?;
            if !bundled && component.id.starts_with("hay.") {
                return Err(format!("{}: hay.* reservado", path.display()));
            }
            let component = Arc::new(component);
            let spec = ProcessSpec::new(48_000.0).map_err(|e| e.to_string())?;
            AudioComponentDevice::new(component.clone(), spec)
                .map_err(|e| format!("{}: preparación: {e}", path.display()))?;
            if registry.insert(component.id.clone(), component).is_some() {
                return Err(format!("{}: ID duplicado", path.display()));
            }
        }
    }
    Ok(registry)
}

impl DawApp {
    pub(super) fn reload_lua_components(&mut self, cx: &mut Context<Self>) {
        if self.lua_components.task.is_some() {
            self.status_message = "Recarga de componentes en curso".into();
            return;
        }
        self.status_message = "Recargando componentes Lua…".into();
        let work = cx.background_spawn(async { scan() });
        self.lua_components.task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.lua_components.task = None;
                match result {
                    Ok(registry) => {
                        this.palette.replace_components(
                            registry
                                .values()
                                .map(|c| (c.id.clone(), c.name.clone(), c.description.clone()))
                                .collect(),
                        );
                        this.status_message =
                            format!("{} componentes Lua registrados", registry.len());
                        this.lua_components.registry = registry;
                    }
                    Err(error) => {
                        this.status_message = format!("{error} · Registro anterior conservado")
                    }
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn insert_lua_component(
        &mut self,
        target: FxTarget,
        id: &str,
    ) -> Result<(), String> {
        let descriptor = self
            .lua_components
            .registry
            .get(id)
            .cloned()
            .ok_or("Componente Lua no registrado")?;
        let chain = self.fx_chain(target.track).ok_or("Pista inexistente")?;
        if target.node.is_some_and(|id| chain.find(id).is_none()) {
            return Err("Destino FX inexistente".into());
        }
        let placement = chain.insertion_after(target.node);
        let parameters = descriptor
            .controls
            .iter()
            .map(|c| (c.id, c.default))
            .collect();
        let node = FxNode::new(
            descriptor.name.clone(),
            FxKind::Effect {
                plugin_id: TrackDevice::Hay {
                    catalog_id: descriptor.id.clone(),
                },
                parameters,
            },
        );
        let fx_id = node.id;
        let rate = self.settings.audio.saved.sample_rate.unwrap_or(48_000);
        let effect = ComponentEffect::prepare(descriptor, rate)?;
        self.lua_components.instances.insert(fx_id, effect);
        if let Err(error) = self.edit_fx(target.track, FxEdit::Insert(node, placement)) {
            self.lua_components.instances.remove(&fx_id);
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_reload_keeps_the_previous_component_and_instance_descriptor() {
        let old = LuaComponents::new();
        let descriptor = old.registry["hay.fx.double_volume"].clone();
        let effect = ComponentEffect::prepare(descriptor.clone(), 48_000).unwrap();
        let root = std::env::temp_dir().join(format!(
            "hay-lua-components-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let updated = include_str!("../../assets/scripts/components/audio/double_volume.lua")
            .replace("Hay Double Volume", "Hay Double Volume v2");
        std::fs::write(root.join("component.lua"), updated).unwrap();
        let candidate = scan_roots(vec![(root.clone(), true)]).unwrap();
        assert_eq!(
            candidate["hay.fx.double_volume"].name,
            "Hay Double Volume v2"
        );
        assert_eq!(effect.descriptor().name, "Hay Double Volume");
        std::fs::remove_file(root.join("component.lua")).unwrap();
        std::fs::write(root.join("bad.lua"), "return { api_version = 1 }").unwrap();
        let result = scan_roots(vec![(root.clone(), false)]);
        std::fs::remove_dir_all(root).unwrap();
        assert!(result.is_err());
        assert!(old.registry.contains_key("hay.fx.double_volume"));
        assert!(Arc::ptr_eq(effect.descriptor(), &descriptor));
    }
}
