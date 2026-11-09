//! Núcleo musical headless. No depende de GPUI, CPAL ni de un instrumento.
//! Modelo editable → plan preparado → eventos por ventana de frames.
pub mod clip;
pub mod controller;
pub mod fx;
pub mod keyboard;
pub mod materialize;
pub mod midi;
pub mod midi_map;
pub mod model;
pub mod pitch;
pub mod scheduler;
pub mod smf;
pub mod time;
pub mod track;
