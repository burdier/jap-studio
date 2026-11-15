#!/usr/bin/env bash
# Ejecutar desde el proyecto: distrobox enter gpui-dev -- bash scripts/check.sh
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo test --no-default-features --lib --tests --examples --locked
# Una prueba de exportación MIDI falló en paralelo y pasó aislada y en serie;
# ejecutar el binario en serie mantiene estable esta verificación de audio.
cargo test --bin daw-gpui --locked -- --test-threads=1
cargo check --bin daw-gpui --locked
