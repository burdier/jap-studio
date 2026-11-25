//! Estado portátil de un efecto. Se publica como un único valor binario Loro.
use std::{
    fmt,
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
};

#[derive(Clone, PartialEq, Eq)]
pub struct VstPreset {
    bytes: Arc<[u8]>,
    fingerprint: u64,
}
impl VstPreset {
    pub const LIMIT: usize = 16 * 1024 * 1024;
    pub fn new(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() > Self::LIMIT {
            return Err("Estado VST3 demasiado grande".into());
        }
        let mut hash = DefaultHasher::new();
        bytes.hash(&mut hash);
        Ok(Self {
            fingerprint: hash.finish(),
            bytes: bytes.into(),
        })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
impl fmt::Debug for VstPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "VstPreset({} bytes, {:016x})",
            self.bytes.len(),
            self.fingerprint
        )
    }
}
