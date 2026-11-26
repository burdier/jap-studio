#!/bin/sh
set -eu
export HAY_SFIZZ="/app/lib/libsfizz.so"
export LD_LIBRARY_PATH="/app/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
# Para modo gráfico asegura acceso directo a assets/ relativos si el cwd no es la app;
# en modo --mcp / --collab preserva el directorio del usuario.
case "${1:-}" in
  --mcp|--collab)
    exec /app/share/jap-studio/hay-daw "$@"
    ;;
  *)
    cd /app/share/jap-studio
    exec /app/share/jap-studio/hay-daw "$@"
    ;;
esac
