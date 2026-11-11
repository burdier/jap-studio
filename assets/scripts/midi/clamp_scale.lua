-- Ajusta cada nota a la nota más cercana de la escala/tónica del piano roll.
local function allowed(pitch)
    local interval = (pitch - scale_root) % 12
    return ((scale_intervals >> interval) & 1) == 1
end
for _, note in ipairs(notes) do
    if note.selected then
        local original = note.pitch
        for distance = 0, 6 do
            local lower = original - distance
            local upper = original + distance
            if lower >= 0 and allowed(lower) then
                note.pitch = lower
                break
            end
            if upper <= 127 and allowed(upper) then
                note.pitch = upper
                break
            end
        end
    end
end
