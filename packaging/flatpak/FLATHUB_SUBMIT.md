# Publicar Jap Studio en Flathub (y Bazzite Store)

Una vez aceptado en **Flathub**, `dev.sevenpitch.jap` aparece automáticamente en **Bazzite Store (Bazaar)**, **Discover (KDE)**, **GNOME Software** y **flathub.org**.

## 1. Requisitos previos (una sola vez)

1. **Hacer público el repositorio `burdier/jap-studio`** (en GitHub → Settings → Danger Zone → Change visibility → Public) y añadir el archivo `LICENSE` (GPL-3.0-or-later) en la raíz del repo para que coincida con `<project_license>` en `dev.sevenpitch.jap.metainfo.xml`.
2. **Asegurar el tag `v0.8.0`** apuntando al commit final y actualizar `commit:` dentro de `packaging/flatpak/dev.sevenpitch.jap.yml`.
3. **Regenerar `cargo-sources.json`** desde el último `Cargo.lock`:
   ```bash
   bash packaging/flatpak/generate-cargo-sources.sh
   ```

## 2. Validar metadatos localmente antes del PR

```bash
desktop-file-validate packaging/flatpak/dev.sevenpitch.jap.desktop
appstreamcli validate --strict packaging/flatpak/dev.sevenpitch.jap.metainfo.xml
```

Para probar la compilación completa en sandbox Flatpak (opcional pero recomendado):

```bash
flatpak install -y flathub org.freedesktop.Platform//25.08 org.freedesktop.Sdk//25.08 org.freedesktop.Sdk.Extension.rust-stable//25.08
flatpak run org.flatpak.Builder --force-clean --sandbox --user --install --install-deps-from=flathub --ccache --mirror-screenshots-url=https://dl.flathub.org/media/ builddir packaging/flatpak/dev.sevenpitch.jap.yml
flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest packaging/flatpak/dev.sevenpitch.jap.yml
```

## 3. Enviar el Pull Request a Flathub

1. Haz fork de https://github.com/flathub/flathub (marca **desactivar** "Copy the master branch only" para incluir la rama `new-pr`).
2. Crea una rama nueva desde `new-pr` (por ejemplo `dev.sevenpitch.jap`).
3. Copia en la raíz del fork estos archivos:
   - `packaging/flatpak/dev.sevenpitch.jap.yml` → `dev.sevenpitch.jap.yml`
   - `packaging/flatpak/cargo-sources.json` → `cargo-sources.json`
   - `packaging/flatpak/flathub.json` → `flathub.json`
4. Abre el PR contra la rama **`new-pr`** de `flathub/flathub` con título `Add dev.sevenpitch.jap`.
   - En la descripción del PR, incluye la justificación de permisos (los revisores de Flathub la piden siempre):
     - `--filesystem=xdg-music` y `--filesystem=xdg-documents`: leer bibliotecas de samples y guardar/exportar proyectos de audio del DAW.
     - `--filesystem=~/.vst3:ro` y `--filesystem=~/.clap:ro`: cargar los plugins de audio VST3/CLAP instalados por el usuario en modo solo lectura.
     - `--share=network`: colaboración en vivo P2P sobre Iroh QUIC / Loro CRDT.
5. El bot `@flathubbot` compilará automáticamente el paquete y revisará `flatpak-builder-lint`.

## 4. Verificar tu dominio (`jap.sevenpitch.dev`) para la insignia Verificado

Al aprobarse el PR, entra en https://flathub.org/developer-portal, solicita la verificación de **`dev.sevenpitch.jap`** y publica el token que te entrega Flathub en:

- `https://jap.sevenpitch.dev/.well-known/org.flathub.VerifiedApps.txt` (solo agregándolo a `landing/.well-known/org.flathub.VerifiedApps.txt` y desplegando a Cloudflare).
