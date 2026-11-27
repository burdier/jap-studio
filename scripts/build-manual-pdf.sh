#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT_DIR/docs/manual/manual.html"
RAW="/tmp/jap-manual-raw.pdf"
OUT="$ROOT_DIR/landing/manual-jap-studio.pdf"

if flatpak info com.google.Chrome >/dev/null 2>&1; then
  CHROME_CMD=(flatpak run "--filesystem=$ROOT_DIR:ro" "--filesystem=/tmp:rw" com.google.Chrome)
elif command -v google-chrome >/dev/null 2>&1; then
  CHROME_CMD=(google-chrome)
elif command -v chromium >/dev/null 2>&1; then
  CHROME_CMD=(chromium)
else
  echo "Error: se requiere Google Chrome o Chromium para generar el PDF." >&2
  exit 1
fi

echo "==> Generando PDF desde $SRC..."
"${CHROME_CMD[@]}" \
  --headless=new \
  --disable-gpu \
  --no-pdf-header-footer \
  --print-to-pdf="$RAW" \
  "file://$SRC" 2>/dev/null

if command -v gs >/dev/null 2>&1; then
  echo "==> Optimizando PDF con Ghostscript (180 DPI)..."
  gs -sDEVICE=pdfwrite -dCompatibilityLevel=1.5 -dPDFSETTINGS=/ebook \
    -dColorImageResolution=180 -dGrayImageResolution=180 \
    -dNOPAUSE -dQUIET -dBATCH -sOutputFile="$OUT" "$RAW"
  rm -f "$RAW"
else
  mv "$RAW" "$OUT"
fi

ls -lh "$OUT"
echo "✅ Manual PDF listo: $OUT"
