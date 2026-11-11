-- Desplaza las subdivisiones impares hasta media rejilla; amount=100 es swing máximo.
for _, note in ipairs(notes) do
    if note.selected then
        local cell = note.start // grid_ticks
        if cell % 2 == 1 then
            local shift = math.floor(grid_ticks * amount / 200)
            note.start = math.min(length - note.duration, note.start + shift)
        end
    end
end
