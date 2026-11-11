-- Hay Acent · Creciente: velocity de 48 a 112 en orden temporal.
-- Los acordes cuentan como un ataque. Un único ataque recibe 112.
local selected = {}
for _, note in ipairs(notes) do
    if note.selected then selected[#selected + 1] = note end
end
table.sort(selected, function(a, b)
    return a.start < b.start or (a.start == b.start and a.id < b.id)
end)

local previous, count = nil, 0
for _, note in ipairs(selected) do
    if note.start ~= previous then
        count = count + 1
        previous = note.start
    end
end

local pulse = -1
previous = nil
for _, note in ipairs(selected) do
    if note.start ~= previous then
        pulse = pulse + 1
        previous = note.start
    end
    note.velocity = count == 1 and 112 or math.floor(48 + 64 * pulse / (count - 1) + 0.5)
end
