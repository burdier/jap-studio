# Acciones Lua VST3

`vst3.script '/ruta/accion.lua'` ejecuta **una acción** devuelta por el archivo.
También está en `[+]` → **Ejecutar acción Lua VST3…**. Lua recibe `tracks`,
la misma copia de pistas y nodos FX que `track.script`, y `track_revision`.
Usa `tracks[n].id` y los `fx[n].id` como handles estables: la selección visible puede
cambiar sin redirigir la acción. Si la sesión cambia entre lectura y aplicación,
la acción se descarta y se puede ejecutar de nuevo.

```lua
return {
  op = "instrument.load",
  track = tracks[6].id,
  path = "/ruta/Synth.vst3",
  -- class = "ID de clase" -- opcional en bundles con varias clases
}
```

Para insertar un efecto en esa pista, usa `op = "effect.load"` con `track` y
`path`; `after = fx_id` es opcional y conecta el efecto después de ese nodo.
Sin `after`, se añade en la raíz de Track FX. El bundle debe contener un efecto;
si contiene varios, se usa el primero que devuelve el escaneo.

Configuración admitida: `effect.toggle`, `effect.remove` (campos `track`, `fx`),
`effect.rename` (añade `name`), `track.level` y `track.pan` (añaden `value` entre
0 y 100). La configuración utiliza los mismos controladores que el árbol FX y
los controles de pista. Un script inválido no toca la sesión. La carga corre en
background y vuelve a validar la pista y la conexión antes de publicar el plugin;
un fallo de carga conserva el instrumento o la cadena anterior.

El script no recibe interfaces COM ni acceso a funciones de procesamiento. No
hay edición Lua de parámetros internos VST3, automatización sample accurate,
PDC, exportación VST3 ni guardado/restauración de estado del plugin. La carga de
plugins reales y la audición todavía requieren comprobación interactiva.
