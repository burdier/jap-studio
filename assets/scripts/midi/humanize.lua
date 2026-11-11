-- Jitter reproducible por ID, compatible con la herramienta Rust original.
for _, note in ipairs(notes) do
    if note.selected then
        local seed = note.id * 0x9e3779b97f4a7c15
        local function scale(value)
            local result = value * amount / 100
            return result < 0 and math.ceil(result) or math.floor(result)
        end
        local jitter = scale(((seed >> 24) % 25) - 12)
        local velocity = scale(((seed >> 40) % 17) - 8)
        note.start = math.max(0, math.min(length - note.duration, note.start + jitter))
        note.velocity = math.max(1, math.min(127, note.velocity + velocity))
    end
end
