#!/usr/bin/env bash
# Ejecutar en un entorno con git, CMake, make y compilador C/C++. No instala globalmente.
set -euo pipefail
sampler_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
sampler_prefix="$sampler_root/.local/sfizz"
sampler_source="$sampler_root/.local/sfizz-source"
sampler_build="$sampler_root/.local/sfizz-build"
sampler_revision=4e70dc0bef53b41f2853ed46e26f5911114c92d0 # sfizz 1.2.3, BSD-2-Clause
if [[ ! -d "$sampler_source/.git" ]]; then
    git clone --depth 1 --branch 1.2.3 https://github.com/sfztools/sfizz.git "$sampler_source"
fi
if [[ "$(git -C "$sampler_source" rev-parse HEAD)" != "$sampler_revision" ]]; then
    echo 'La copia de sfizz no corresponde a la revisión fijada; usa una carpeta limpia.' >&2
    exit 1
fi
git -C "$sampler_source" submodule update --init --recursive --depth 1
cmake -S "$sampler_source" -B "$sampler_build" -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$sampler_prefix" -DSFIZZ_JACK=OFF -DSFIZZ_RENDER=OFF \
    -DSFIZZ_TESTS=OFF -DSFIZZ_SHARED=ON -DSFIZZ_GIT_SUBMODULE_CHECK=OFF
cmake --build "$sampler_build" --target sfizz_shared -j "${HAY_BUILD_JOBS:-2}"
mkdir -p "$sampler_prefix/lib" "$sampler_prefix/licenses"
cp -a "$sampler_build"/library/lib/libsfizz.so* "$sampler_prefix/lib/"
cp "$sampler_source/LICENSE" "$sampler_prefix/licenses/sfizz.txt"
cp "$sampler_source/README.md" "$sampler_prefix/licenses/README.md"
echo "sfizz instalado para Hay en $sampler_prefix"
