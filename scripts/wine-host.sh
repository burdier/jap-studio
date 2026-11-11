#!/usr/bin/env bash
# WINELOADER para yabridge dentro de distrobox: conserva el prefijo elegido por el puente.
set -euo pipefail
wine_env=(-u LD_LIBRARY_PATH -u LD_PRELOAD -u WINELOADER YABRIDGE_NO_WATCHDOG=1)
if [[ -n "${WINEPREFIX:-}" ]]; then wine_env+=("WINEPREFIX=$WINEPREFIX"); fi
if [[ -n "${WINEDEBUG:-}" ]]; then wine_env+=("WINEDEBUG=$WINEDEBUG"); fi
exec distrobox-host-exec env "${wine_env[@]}" /usr/bin/wine "$@"
