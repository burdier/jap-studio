use elevate::scripting::packages::{PackageDirectories, PackageError, PackageRegistry};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

pub const NUMBER: &str = "{key='amount', label='Acento', kind='number', default=12, min=0, max=126, step=1, whole=true, unit='velocity'}";

pub fn source(id: &str, parameters: &str) -> String {
    format!(
        "return {{api_version=1, id='{id}', name='Acentuar', description='Edita notas', parameters={{{parameters}}}, run=function() error('run no se ejecuta al registrar') end}}"
    )
}

pub fn altered(body: &str) -> String {
    format!(
        "local p = {}; {body}; return p",
        source("user.midi.accent", NUMBER).trim_start_matches("return ")
    )
}

pub struct Fixture {
    root: PathBuf,
    pub directories: PackageDirectories,
}

impl Fixture {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "hay-lua-packages-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let directories = PackageDirectories {
            bundled: root.join("bundled"),
            user: Some(root.join("user")),
        };
        fs::create_dir_all(&directories.bundled).unwrap();
        fs::create_dir_all(directories.user.as_ref().unwrap()).unwrap();
        Self { root, directories }
    }

    pub fn write(&self, name: &str, source: impl AsRef<[u8]>) -> PathBuf {
        let path = self.directories.user.as_ref().unwrap().join(name);
        fs::write(&path, source).unwrap();
        path
    }

    pub fn load(&self) -> Result<PackageRegistry, PackageError> {
        let mut registry = PackageRegistry::default();
        registry.reload(&self.directories)?;
        Ok(registry)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
