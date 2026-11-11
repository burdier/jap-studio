#!/usr/bin/env bash
# Compila y ejecuta Hay. Desde el host entra solo a distrobox; dentro del contenedor corre directo.
# Uso: bash scripts/run.sh [--release] [argumentos de cargo run]
set -euo pipefail
cd "$(dirname "$0")/.."
CONTAINER="${HAY_CONTAINER:-gpui-dev}"
if [ -z "${CONTAINER_ID:-}" ] && command -v distrobox >/dev/null 2>&1; then
    # Sin esta comprobación, distrobox ofrece crear un contenedor vacío (sin Rust ni librerías).
    if ! timeout 15 podman container exists "$CONTAINER"; then
        echo "No existe el contenedor '$CONTAINER' o podman no responde (HAY_CONTAINER cambia el nombre)." >&2
        echo "Revisa con: podman ps -a" >&2
        exit 1
    fi
    echo "Entrando a '$CONTAINER'… (la primera compilación tarda; luego se abre la ventana)" >&2
    exec distrobox enter "$CONTAINER" -- bash scripts/run.sh "$@"
fi
export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"
if [[ -z "${WINELOADER:-}" ]] && ! command -v wine >/dev/null 2>&1 \
    && command -v distrobox-host-exec >/dev/null 2>&1 && [[ -e /run/host/usr/bin/wine || -L /run/host/usr/bin/wine ]]; then
    export WINELOADER="$PWD/scripts/wine-host.sh"
    # Host y Wine viven en namespaces de PID distintos; los sockets sí son compartidos.
    export YABRIDGE_NO_WATCHDOG=1
fi
exec cargo run --bin daw-gpui "$@"
