-- Usa la rejilla capturada al invocar la acción; conserva duración e identidad.
for _, note in ipairs(notes) do
    if note.selected then
        local snapped = math.min(((note.start + grid_ticks // 2) // grid_ticks) * grid_ticks,
            length - note.duration)
        note.start = math.max(0, math.min(length - note.duration,
            note.start + math.floor((snapped - note.start) * strength / 100)))
    end
end
