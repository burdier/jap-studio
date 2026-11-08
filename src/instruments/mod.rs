//! Instrumentos independientes de GPUI. La UI solo envía notas y consulta estado.
//! Hoy solo existe el piano; el catálogo ya permite que otros se registren después.

pub(crate) mod piano;

pub(crate) use piano::Piano;
