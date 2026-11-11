#!/usr/bin/env bash
# Genera el archivo cargo-sources.json a partir de Cargo.lock
# Necesario para que Flatpak descargue todas las dependencias de Rust antes del sandbox offline.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "==> Verificando dependencias para Flatpak Cargo Generator..."

if [ ! -f "$PROJECT_ROOT/Cargo.lock" ]; then
    echo "Error: No se encontró Cargo.lock en $PROJECT_ROOT" >&2
    exit 1
fi

VENV_DIR="$SCRIPT_DIR/.generator-venv"

if ! python3 -c "import aiohttp" 2>/dev/null; then
    echo "==> Creando entorno virtual aislado para herramientas de empaquetado..."
    python3 -m venv "$VENV_DIR"
    "$VENV_DIR/bin/pip" install --quiet --upgrade pip
    "$VENV_DIR/bin/pip" install --quiet aiohttp tomlkit
    PYTHON_EXEC="$VENV_DIR/bin/python3"
else
    PYTHON_EXEC="python3"
fi

echo "==> Generando $SCRIPT_DIR/cargo-sources.json a partir de Cargo.lock..."
"$PYTHON_EXEC" "$SCRIPT_DIR/flatpak-cargo-generator.py" \
    "$PROJECT_ROOT/Cargo.lock" \
    -o "$SCRIPT_DIR/cargo-sources.json"

echo "==> ¡Listo! cargo-sources.json generado exitosamente."
