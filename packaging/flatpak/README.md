# Empaquetado Flatpak y Despliegue en Flathub (Bazzite Store) para Elevate

Este directorio contiene todos los metadatos y manifiestos necesarios para compilar **Elevate** en Flatpak y desplegarlo en **Flathub** (la tienda oficial utilizada por Bazzite y SteamOS).

---

## Estructura de archivos

- `io.github.YIAN6557.Elevate.desktop`: Archivo de integración de escritorio (menú de aplicaciones).
- `io.github.YIAN6557.Elevate.metainfo.xml`: Metadatos AppStream (título, descripción, categorías, lanzamientos para la tienda).
- `io.github.YIAN6557.Elevate.svg`: Icono Release, con la zanahoria coral sobre fondo transparente.
- `../../assets/icons/hay-app-icon-dev.svg`: Icono Dev, con marco y fondo de desarrollo.
- `../../assets/icons/hay-app-icon-release.svg`: Icono Release independiente del paquete.
- `io.github.YIAN6557.Elevate.yml`: Manifiesto Flatpak principal con la configuración de permisos (Wayland, Vulkan, Audio PipeWire/PulseAudio) y módulos (`sfizz` + `daw-gpui`).
- `flatpak-cargo-generator.py`: Herramienta oficial de Flatpak para generar dependencias Rust para el sandbox offline.
- `generate-cargo-sources.sh`: Script automatizado para generar `cargo-sources.json` sin alterar el sistema.

---

## 1. Requisitos previos en Bazzite

En el sistema host Bazzite, instala el SDK y la extensión de Rust para Flatpak:

```bash
# 1. Instalar runtime y SDK Freedesktop 26.08
flatpak install flathub org.freedesktop.Platform//26.08 org.freedesktop.Sdk//26.08

# 2. Instalar la extensión Rust estable
flatpak install flathub org.freedesktop.Sdk.Extension.rust-stable//26.08
```

Para la herramienta `flatpak-builder`, puedes instalarla en tu contenedor de desarrollo Distrobox (`gpui-dev`):
```bash
distrobox enter gpui-dev -- sudo dnf install -y flatpak-builder
```
O directamente vía Flatpak en el host:
```bash
flatpak install flathub org.flatpak.Builder
```

---

## 2. Compilación y Prueba Local

1. **Generar el manifiesto de dependencias offline:**
   ```bash
   cd packaging/flatpak
   ./generate-cargo-sources.sh
   ```
   Esto creará `cargo-sources.json` a partir del `Cargo.lock` actual.

2. **Compilar e instalar la app para el usuario actual:**
   ```bash
   flatpak-builder --user --install --force-clean build-dir io.github.YIAN6557.Elevate.yml
   ```

3. **Ejecutar Elevate desde Flatpak:**
   ```bash
   flatpak run io.github.YIAN6557.Elevate
   ```

---

## 3. Publicación en Flathub (Disponible en la Store de Bazzite)

Cuando el repositorio esté publicado en GitHub:

1. **Ajustar el Application ID (si cambia el usuario o nombre):**
   - Si tu usuario de GitHub es diferente a `YIAN6557`, renombra los archivos cambiando el prefijo:
     `io.github.<tu-usuario>.Elevate.*`
2. **Hacer Fork de Flathub:**
   - Ve a [github.com/flathub/flathub](https://github.com/flathub/flathub) y haz clic en **Fork**.
3. **Crear rama con el App ID:**
   ```bash
   git checkout -b new-pr/io.github.<tu-usuario>.Elevate
   ```
4. **Subir los archivos:**
   Copia el archivo `.yml`, el `.metainfo.xml` y el `cargo-sources.json` a la rama y haz commit.
5. **Abrir Pull Request:**
   Envía el PR a `flathub/flathub`. El bot de Flathub compilará y validará la app automáticamente en x86_64 y ARM64. Una vez aprobado y fusionado, ¡aparecerá en la tienda de Bazzite!
