#!/usr/bin/env bash
# Descarga y descomprime los bancos de scripts/soundfonts.txt en assets/soundfonts/freepats/.
# Cada banco deja <Nombre>.sf2 y <Nombre>.readme.txt (autoría y licencia del paquete).
# Reanudable: omite los .sf2 ya presentes. Requiere curl y 7z (p7zip) y tar.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT=assets/soundfonts/freepats
CACHE="$OUT/.cache"
mkdir -p "$OUT" "$CACHE"
while read -r url; do
    case "$url" in ''|\#*) continue ;; esac
    archive="$CACHE/$(basename "$url")"
    stem="$(basename "$url" | sed -E 's/-SF2-.*//')"
    if ls "$OUT/$stem"*.sf2 >/dev/null 2>&1; then echo "= $stem (ya está)"; continue; fi
    echo "↓ $stem"
    curl -fL --retry 3 -sS -o "$archive" "$url"
    tmp="$(mktemp -d "$CACHE/x.XXXXXX")"
    case "$archive" in
        *.7z) 7z x -y -bso0 -bsp0 -o"$tmp" "$archive" ;;
        *.tar.*) tar -xf "$archive" -C "$tmp" ;;
        *) echo "formato desconocido: $archive" >&2; rm -rf "$tmp"; continue ;;
    esac
    sf2="$(find "$tmp" -iname '*.sf2' | head -1)"
    if [ -z "$sf2" ]; then echo "  sin .sf2 en $archive" >&2; rm -rf "$tmp"; continue; fi
    mv "$sf2" "$OUT/$(basename "$sf2")"
    readme="$(find "$tmp" -iname 'readme*' | head -1)"
    [ -n "$readme" ] && cp "$readme" "$OUT/$(basename "$sf2" .sf2).readme.txt"
    rm -rf "$tmp" "$archive"
done < scripts/soundfonts.txt
echo "Listo: $(ls "$OUT"/*.sf2 | wc -l) bancos en $OUT"
