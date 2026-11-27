#!/usr/bin/env bash
# Compila el binario release dentro de gpui-dev y mantiene sincronizado el lanzador del escritorio.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
RELEASE_BIN="$ROOT_DIR/target/release/daw-gpui"
LOCAL_BIN="$ROOT_DIR/.local/bin/hay-daw"

echo "==> Compilando Jap Studio (release + ui + p2p) en gpui-dev..."
distrobox enter gpui-dev -- bash -lc "cd '$ROOT_DIR' && cargo build --release --features ui,p2p --bin daw-gpui"

mkdir -p "$ROOT_DIR/.local/bin"
# Enlace simbólico atómico: evita copias obsoletas y asegura que el icono del escritorio
# y el servidor MCP usen siempre el último binario compilado (aun si MCP está en ejecución).
ln -sfn "$RELEASE_BIN" "${LOCAL_BIN}.tmp"
mv -Tf "${LOCAL_BIN}.tmp" "$LOCAL_BIN"

if command -v update-desktop-database >/dev/null 2>&1 && [ -d "$HOME/.local/share/applications" ]; then
  update-desktop-database "$HOME/.local/share/applications" >/dev/null 2>&1 || true
fi

ls -lh "$RELEASE_BIN" "$LOCAL_BIN"
echo "✅ Release listo y sincronizado con el icono de escritorio."
