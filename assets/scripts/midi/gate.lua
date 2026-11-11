-- amount=100 conserva la duración; 50 reduce cada nota a la mitad.
for _, note in ipairs(notes) do
    if note.selected then
        note.duration = math.max(1, math.floor(note.duration * amount / 100))
    end
end
