-- Hay Acent · Alternado: 112–72, por ataque seleccionado.
-- Las notas simultáneas reciben el mismo acento. Solo cambia velocity.
local selected = {}
for _, note in ipairs(notes) do
    if note.selected then selected[#selected + 1] = note end
end
table.sort(selected, function(a, b)
    return a.start < b.start or (a.start == b.start and a.id < b.id)
end)

local previous, pulse = nil, -1
for _, note in ipairs(selected) do
    if note.start ~= previous then
        pulse = pulse + 1
        previous = note.start
    end
    note.velocity = pulse % 2 == 0 and 112 or 72
end
