//! Instrumentos independientes de GPUI. La UI solo envía notas y consulta estado.
//! Piano SoundFont y sampler sfizz; ambos comparten el reproductor de eventos.

pub(crate) mod piano;
pub(crate) mod piano_params;
pub(crate) mod sampler;
pub(crate) mod vst;

pub(crate) use piano::Piano;
