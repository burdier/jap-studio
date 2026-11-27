#!/usr/bin/env bash
set -euo pipefail

APP_ID="dev.sevenpitch.jap"
VERSION="${VERSION:-0.9.0}"
RUNTIME="org.freedesktop.Platform"
SDK="org.freedesktop.Platform"
RUNTIME_VER="25.08"
ARCH="$(uname -m)"

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
BUILD_DIR="/tmp/jap-flatpak-build"
REPO_DIR="/tmp/jap-flatpak-repo"
BUNDLE_PATH="$DIST_DIR/jap-studio-${VERSION}-${ARCH}.flatpak"
FULL_SOUNDFONTS=0
BIN_SRC="$ROOT_DIR/target/release/daw-gpui"
for arg in "$@"; do
  if [[ "$arg" == "--full" ]]; then
    FULL_SOUNDFONTS=1
  else
    BIN_SRC="$arg"
  fi
done

if [[ ! -x "$BIN_SRC" ]]; then
  echo "Error: no se encontró el binario release en $BIN_SRC" >&2
  echo "Compílalo primero con: cargo build --release --features ui,p2p" >&2
  exit 1
fi

mkdir -p "$DIST_DIR"
rm -rf "$BUILD_DIR" "$REPO_DIR" "$BUNDLE_PATH"

echo "==> Inicializando sandbox Flatpak ($APP_ID · $RUNTIME//$RUNTIME_VER)..."
flatpak build-init "$BUILD_DIR" "$APP_ID" "$SDK" "$RUNTIME" "$RUNTIME_VER"

FILES="$BUILD_DIR/files"
APP_SHARE="$FILES/share/jap-studio"
mkdir -p \
  "$FILES/bin" \
  "$FILES/lib" \
  "$APP_SHARE/assets/soundfonts" \
  "$FILES/share/applications" \
  "$FILES/share/metainfo" \
  "$FILES/share/icons/hicolor/scalable/apps"

echo "==> Copiando binario (stripped), libsfizz y assets..."
cp "$BIN_SRC" "$APP_SHARE/hay-daw"
chmod 755 "$APP_SHARE/hay-daw"
if command -v strip >/dev/null 2>&1; then
  strip --strip-unneeded "$APP_SHARE/hay-daw" || true
fi

if [[ -d "$ROOT_DIR/.local/sfizz/lib" ]]; then
  cp -a "$ROOT_DIR/.local/sfizz/lib/"libsfizz.so* "$FILES/lib/"
elif [[ -d "$ROOT_DIR/.local/sfizz/lib64" ]]; then
  cp -a "$ROOT_DIR/.local/sfizz/lib64/"libsfizz.so* "$FILES/lib/"
fi

cp -a "$ROOT_DIR/assets/icons" "$APP_SHARE/assets/"
cp -a "$ROOT_DIR/assets/scripts" "$APP_SHARE/assets/"
cp -a "$ROOT_DIR/assets/textures" "$APP_SHARE/assets/"
cp -a "$ROOT_DIR/assets/themes" "$APP_SHARE/assets/"
# Incluir el piano principal CC0 y el pack esencial de FreePats (pianos, bajos,
# órganos, acústica, percusión GM, arpa, kalimba, vientos, pads), omitiendo solo
# los bancos masivos (>150 MB c/u: EGuitarFSBS-* y MuldjordKit) salvo con --full.
cp -a "$ROOT_DIR/assets/soundfonts/UprightPianoKW-20220221.sf2" "$APP_SHARE/assets/soundfonts/"
cp -a "$ROOT_DIR/assets/soundfonts/cc0.txt" "$APP_SHARE/assets/soundfonts/"
cp -a "$ROOT_DIR/assets/soundfonts/readme.txt" "$APP_SHARE/assets/soundfonts/"
mkdir -p "$APP_SHARE/assets/soundfonts/freepats"
if [[ -d "$ROOT_DIR/assets/soundfonts/freepats" ]]; then
  shopt -s nullglob
  if [[ "$FULL_SOUNDFONTS" == "1" ]]; then
    cp -a "$ROOT_DIR/assets/soundfonts/freepats/." "$APP_SHARE/assets/soundfonts/freepats/"
  else
    for f in "$ROOT_DIR/assets/soundfonts/freepats/"*; do
      base="$(basename "$f")"
      case "$base" in
        EGuitarFSBS-*|MuldjordKit*) continue ;;
      esac
      cp -a "$f" "$APP_SHARE/assets/soundfonts/freepats/"
    done
  fi
  shopt -u nullglob
fi

install -m 755 "$ROOT_DIR/packaging/flatpak/jap-studio-launcher.sh" "$FILES/bin/jap-studio"
install -m 644 "$ROOT_DIR/packaging/flatpak/${APP_ID}.desktop" "$FILES/share/applications/${APP_ID}.desktop"
install -m 644 "$ROOT_DIR/packaging/flatpak/${APP_ID}.metainfo.xml" "$FILES/share/metainfo/${APP_ID}.metainfo.xml"
install -m 644 "$ROOT_DIR/assets/icons/hay-app-icon-release.svg" "$FILES/share/icons/hicolor/scalable/apps/${APP_ID}.svg"

echo "==> Finalizando permisos del sandbox..."
flatpak build-finish "$BUILD_DIR" \
  --command=jap-studio \
  --share=ipc \
  --share=network \
  --socket=wayland \
  --socket=fallback-x11 \
  --socket=pulseaudio \
  --device=dri \
  --filesystem=xdg-run/pipewire-0 \
  --filesystem=xdg-music \
  --filesystem=xdg-documents \
  --filesystem=~/.vst3:ro \
  --filesystem=~/.clap:ro

echo "==> Exportando repositorio OSTree y generando bundle único..."
flatpak build-export "$REPO_DIR" "$BUILD_DIR"
flatpak build-bundle "$REPO_DIR" "$BUNDLE_PATH" "$APP_ID" \
  --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo

rm -rf "$BUILD_DIR" "$REPO_DIR"
ls -lh "$BUNDLE_PATH"
echo "✅ Bundle Flatpak listo: $BUNDLE_PATH"
