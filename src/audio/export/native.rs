//! Carga dinámica local al worker, como FluidSynth. Ningún puntero sale de su adaptador.
use crate::audio::dynlib::DynLib;
use std::ffi::CStr;

pub(super) struct Library(DynLib);

impl Library {
    pub fn open(name: &CStr) -> Result<Self, String> {
        DynLib::open(name).map(Self)
    }

    /// El llamador debe usar la firma C exacta y no usar el símbolo después de Library.
    pub unsafe fn symbol<T: Copy>(&self, name: &CStr) -> Result<T, String> {
        unsafe { self.0.symbol(name) }
    }
}
