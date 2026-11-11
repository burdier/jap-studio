-- Hay Acent · Clic 4/4: convierte cuatro notas en una guía de cuatro pulsos.
-- Empieza en el compás 4/4 que contiene la primera nota; conserva sus tonos.
local selected = {}
for _, note in ipairs(notes) do
    if note.selected then selected[#selected + 1] = note end
end
if #selected ~= 4 then
    error("Hay Acent · Clic 4/4: selecciona exactamente cuatro notas")
end
table.sort(selected, function(a, b)
    return a.start < b.start or (a.start == b.start and a.id < b.id)
end)

local bar = 4 * ppq
local start = math.floor(selected[1].start / bar) * bar
if start + bar > length then
    error("Hay Acent · Clic 4/4: falta espacio para un compás completo en el patrón")
end
local duration = math.max(1, math.floor(ppq / 8))
for index, note in ipairs(selected) do
    note.start = start + (index - 1) * ppq
    note.duration = duration
    note.velocity = index == 1 and 112 or 72
end
