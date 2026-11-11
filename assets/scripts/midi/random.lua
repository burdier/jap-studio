-- Variación determinista y acotada de tiempo/velocity basada en el ID de nota.
for _, note in ipairs(notes) do
    if note.selected then
        local seed = note.id * 0x9e3779b97f4a7c15
        local time = ((seed >> 24) % 25) - 12
        local velocity = ((seed >> 40) % 17) - 8
        note.start = math.max(0, math.min(length - note.duration,
            note.start + math.floor(time * amount / 100)))
        note.velocity = math.max(1, math.min(127,
            note.velocity + math.floor(velocity * amount / 100)))
    end
end
