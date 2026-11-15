-- Agrupa las dos primeras pistas raíz que ya tienen instrumento.
-- Las instancias siguen perteneciendo a sus pistas; este lote solo crea la ruta.
local members = {}
for _, track in ipairs(tracks) do
    if track.instrument and not track.parent then
        members[#members + 1] = track.id
        if #members == 2 then break end
    end
end
assert(#members == 2, "Se necesitan dos pistas raíz con instrumento")

return {
    { op = "group", handle = "hilo", name = "Hilo RR", members = members },
    { op = "hilo.mode", track = "hilo", mode = "round_robin" },
    { op = "fx.insert_utility", track = members[1], name = "Nivel rama A", gain = 1.0 },
    { op = "fx.insert_utility", track = members[2], name = "Nivel rama B", gain = 1.0 },
}
