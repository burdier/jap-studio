-- Panorama por nota; el host adapta la expresión al backend compatible.
for _, note in ipairs(notes) do
    if note.selected then note.pan = pan end
end
