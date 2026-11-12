-- Plantilla de PR-02: dos pistas de audio con fader, panorama y Hay Utility.
-- Los handles viven solo durante este lote; Rust asigna TrackId al confirmar.
return {
    { op = "insert_audio", handle = "principal", name = "Voz principal" },
    { op = "level", track = "principal", value = 75 },
    { op = "pan", track = "principal", value = 45 },
    { op = "fx.insert_utility", track = "principal", name = "Utility principal", gain = 0.9 },

    { op = "insert_audio", handle = "doble", name = "Voz doble", after = "principal" },
    { op = "level", track = "doble", value = 65 },
    { op = "pan", track = "doble", value = 70 },
    { op = "fx.insert_utility", track = "doble", name = "Utility doble", gain = 0.8 },
}
