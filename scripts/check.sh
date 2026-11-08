#!/usr/bin/env bash
# Ejecutar desde el proyecto: distrobox enter gpui-dev -- bash scripts/check.sh
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo test --no-default-features --lib --tests --examples --locked
cargo test --bin daw-gpui --locked
cargo check --bin daw-gpui --locked
