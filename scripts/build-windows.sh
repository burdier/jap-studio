#!/usr/bin/env bash
set -euo pipefail

# Cross-compila Jap Studio para Windows x64 MSVC desde Linux usando cargo-xwin
# y genera dist/JapStudio-Windows-x64.zip listo para copiar y abrir en Windows.

export XWIN_ACCEPT_LICENSE=yes
cargo xwin build --profile windows-cross --target x86_64-pc-windows-msvc --features ui,p2p

EXE="target/x86_64-pc-windows-msvc/windows-cross/daw-gpui.exe"
ls -lh "$EXE"
command -v file >/dev/null 2>&1 && file "$EXE" || true

rm -rf dist/JapStudio-Windows-x64 dist/JapStudio-Windows-x64.zip
mkdir -p dist/JapStudio-Windows-x64
cp "$EXE" dist/JapStudio-Windows-x64/jap-studio.exe

if [ -d assets ]; then
  cp -a assets dist/JapStudio-Windows-x64/assets
  rm -rf dist/JapStudio-Windows-x64/assets/soundfonts/freepats
fi
[ -f README.md ] && cp README.md dist/JapStudio-Windows-x64/README.md

(cd dist && python3 -c '
import os, zipfile
root = "JapStudio-Windows-x64"
with zipfile.ZipFile("JapStudio-Windows-x64.zip", "w", zipfile.ZIP_DEFLATED) as z:
    for dirpath, _, filenames in os.walk(root):
        for f in sorted(filenames):
            full = os.path.join(dirpath, f)
            z.write(full, full)
')

ls -lh dist/JapStudio-Windows-x64.zip
echo "=== WINDOWS PACKAGE READY: dist/JapStudio-Windows-x64.zip ==="
