-- Hay Chord Utility · Motor armónico y de voicings embebido en Lua (inspirado en music.lua + Ableton Chord).
-- Soporta armonización diatónica según la escala activa del Piano Roll y diccionario cromático de cifrado,
-- con inversiones (-2..3), disposiciones (close, open, drop2, drop3), doblaje de bajo (-1 octava) y rasgueo (strum).

return {
    api_version = 1,
    id = "hay.midi.chord_utility",
    name = "Chord Utility · generador de acordes y voicings",
    description = "Construye acordes diatónicos o por cifrado con inversiones, voicings (close/open/drop2/drop3), bajo y strum.",
    parameters = {
        {
            key = "quality",
            label = "Calidad / modo",
            hint = "scale, scale7, scale9, scale_sus2, scale_sus4, scale_add9, maj, min, 7, maj7, m7, m7b5, dim, dim7, aug, sus2, sus4, 6, m6, add9, madd9, 9, maj9, m9, power",
            kind = "text",
            default = "scale",
        },
        {
            key = "inversion",
            label = "Inversión",
            hint = "0 = fundamental; 1..3 = sube voces graves +12 st; -1..-2 = baja voces agudas -12 st",
            kind = "number",
            default = 0,
            min = -2,
            max = 3,
            step = 1,
            whole = true,
            unit = "voces",
        },
        {
            key = "voicing",
            label = "Voicing / disposición",
            hint = "close (cerrado), open (abre la 2ª voz +12 st), drop2 (baja 2ª voz aguda -12 st), drop3 (baja 3ª voz aguda -12 st)",
            kind = "text",
            default = "close",
        },
        {
            key = "bass_octave",
            label = "Bajo (-1 octava)",
            hint = "Añade la fundamental una octava abajo (-12 semitonos)",
            kind = "bool",
            default = false,
        },
        {
            key = "strum_ticks",
            label = "Strum (rasgueo)",
            hint = "Desfase en ticks entre voces (>0 grave a agudo, <0 agudo a grave)",
            kind = "number",
            default = 0,
            min = -240,
            max = 240,
            step = 5,
            whole = true,
            unit = "ticks",
        },
    },
    run = function(context, params)
        local CHROMATIC_SPECS = {
            maj = { 0, 4, 7 },
            major = { 0, 4, 7 },
            min = { 0, 3, 7 },
            minor = { 0, 3, 7 },
            m = { 0, 3, 7 },
            dim = { 0, 3, 6 },
            aug = { 0, 4, 8 },
            sus2 = { 0, 2, 7 },
            sus4 = { 0, 5, 7 },
            power = { 0, 7 },
            ["5"] = { 0, 7 },
            ["7"] = { 0, 4, 7, 10 },
            dom7 = { 0, 4, 7, 10 },
            maj7 = { 0, 4, 7, 11 },
            m7 = { 0, 3, 7, 10 },
            min7 = { 0, 3, 7, 10 },
            m7b5 = { 0, 3, 6, 10 },
            half_dim = { 0, 3, 6, 10 },
            dim7 = { 0, 3, 6, 9 },
            m_maj7 = { 0, 3, 7, 11 },
            ["6"] = { 0, 4, 7, 9 },
            maj6 = { 0, 4, 7, 9 },
            m6 = { 0, 3, 7, 9 },
            min6 = { 0, 3, 7, 9 },
            add9 = { 0, 4, 7, 14 },
            madd9 = { 0, 3, 7, 14 },
            ["9"] = { 0, 4, 7, 10, 14 },
            dom9 = { 0, 4, 7, 10, 14 },
            maj9 = { 0, 4, 7, 11, 14 },
            m9 = { 0, 3, 7, 10, 14 },
            min9 = { 0, 3, 7, 10, 14 },
        }

        local DIATONIC_DEGREES = {
            scale = { 0, 2, 4 },
            diatonic = { 0, 2, 4 },
            triad = { 0, 2, 4 },
            scale7 = { 0, 2, 4, 6 },
            diatonic7 = { 0, 2, 4, 6 },
            scale9 = { 0, 2, 4, 6, 8 },
            diatonic9 = { 0, 2, 4, 6, 8 },
            scale_sus2 = { 0, 1, 4 },
            scale_sus4 = { 0, 3, 4 },
            scale_add9 = { 0, 2, 4, 8 },
        }

        local quality = string.lower((string.gsub(params.quality, "^%s*(.-)%s*$", "%1")))
        local voicing = string.lower((string.gsub(params.voicing, "^%s*(.-)%s*$", "%1")))

        if not CHROMATIC_SPECS[quality] and not DIATONIC_DEGREES[quality] then
            error(
                "Calidad desconocida '"
                    .. params.quality
                    .. "'. Usa scale, scale7, scale9, maj, min, 7, maj7, m7, m7b5, dim, dim7, aug, sus2, sus4, 6, m6, add9, 9, maj9, m9 o power"
            )
        end

        if voicing ~= "close" and voicing ~= "open" and voicing ~= "drop2" and voicing ~= "drop3" then
            error("Voicing desconocido '" .. params.voicing .. "'. Usa close, open, drop2 o drop3")
        end

        local function in_scale(pitch)
            local interval = (pitch - context.scale.root) % 12
            return math.floor(context.scale.intervals / (2 ^ interval)) % 2 == 1
        end

        local function snap_to_scale(pitch)
            if in_scale(pitch) then
                return pitch
            end
            for offset = 1, 11 do
                if pitch - offset >= 0 and in_scale(pitch - offset) then
                    return pitch - offset
                end
                if pitch + offset <= 127 and in_scale(pitch + offset) then
                    return pitch + offset
                end
            end
            return pitch
        end

        local function scale_step_above(pitch, steps)
            if steps == 0 then
                return pitch
            end
            local found = 0
            for candidate = pitch + 1, 127 do
                if in_scale(candidate) then
                    found = found + 1
                    if found == steps then
                        return candidate
                    end
                end
            end
            error("El acorde diatónico supera el límite MIDI superior (127)")
        end

        local function build_base_pitches(root_pitch)
            local pitches = {}
            local degrees = DIATONIC_DEGREES[quality]
            if degrees then
                local base_pitch = snap_to_scale(root_pitch)
                for _, step in ipairs(degrees) do
                    pitches[#pitches + 1] = scale_step_above(base_pitch, step)
                end
            else
                local intervals = CHROMATIC_SPECS[quality]
                for _, semitones in ipairs(intervals) do
                    local target = root_pitch + semitones
                    if target < 0 or target > 127 then
                        error("El acorde generado sale del rango MIDI (0–127)")
                    end
                    pitches[#pitches + 1] = target
                end
            end
            return pitches
        end

        local function apply_inversion(pitches, inv)
            local out = {}
            for i = 1, #pitches do
                out[i] = pitches[i]
            end
            if inv > 0 then
                for _ = 1, inv do
                    local lowest = table.remove(out, 1)
                    if lowest + 12 > 127 then
                        error("La inversión supera el límite MIDI superior (127)")
                    end
                    out[#out + 1] = lowest + 12
                    table.sort(out)
                end
            elseif inv < 0 then
                for _ = 1, -inv do
                    local highest = table.remove(out, #out)
                    if highest - 12 < 0 then
                        error("La inversión baja del límite MIDI inferior (0)")
                    end
                    table.insert(out, 1, highest - 12)
                    table.sort(out)
                end
            end
            return out
        end

        local function apply_voicing(pitches, mode)
            local out = {}
            for i = 1, #pitches do
                out[i] = pitches[i]
            end
            if mode == "open" and #out >= 3 then
                if out[2] + 12 <= 127 then
                    out[2] = out[2] + 12
                end
            elseif mode == "drop2" and #out >= 3 then
                local idx = #out - 1
                if out[idx] - 12 >= 0 then
                    out[idx] = out[idx] - 12
                end
            elseif mode == "drop3" and #out >= 4 then
                local idx = #out - 2
                if out[idx] - 12 >= 0 then
                    out[idx] = out[idx] - 12
                end
            end
            table.sort(out)
            return out
        end

        local function deduplicate_sorted(pitches)
            local unique = {}
            for _, p in ipairs(pitches) do
                if #unique == 0 or unique[#unique] ~= p then
                    unique[#unique + 1] = p
                end
            end
            return unique
        end

        local function note_key(instrument, start, duration, pitch)
            return instrument .. ":" .. start .. ":" .. duration .. ":" .. pitch
        end

        local existing = {}
        for _, note in ipairs(context.notes) do
            existing[note_key(note.instrument, note.start, note.duration, note.pitch)] = true
        end

        for _, root in ipairs(context.notes) do
            if root.selected then
                local origin_pitch = root.pitch
                local origin_start = root.start
                local origin_duration = root.duration

                local voices = build_base_pitches(origin_pitch)
                local chord_root = voices[1]
                voices = apply_inversion(voices, params.inversion)
                voices = apply_voicing(voices, voicing)

                if params.bass_octave and chord_root - 12 >= 0 then
                    table.insert(voices, 1, chord_root - 12)
                    table.sort(voices)
                end
                voices = deduplicate_sorted(voices)

                local anchor_index = 1
                for i, p in ipairs(voices) do
                    if p == origin_pitch then
                        anchor_index = i
                        break
                    end
                end

                local voice_count = #voices
                for i, pitch in ipairs(voices) do
                    local order = (params.strum_ticks >= 0) and (i - 1) or (voice_count - i)
                    local delay = math.abs(params.strum_ticks) * order
                    local max_delay = math.max(0, origin_duration - 1)
                    if delay > max_delay then
                        delay = max_delay
                    end
                    local voice_start = origin_start + delay
                    local voice_duration = origin_duration - delay

                    if i == anchor_index then
                        existing[note_key(root.instrument, root.start, root.duration, root.pitch)] = nil
                        root.pitch = pitch
                        root.start = voice_start
                        root.duration = voice_duration
                        existing[note_key(root.instrument, root.start, root.duration, root.pitch)] = true
                    else
                        local key = note_key(root.instrument, voice_start, voice_duration, pitch)
                        if not existing[key] then
                            context.new_notes[#context.new_notes + 1] = {
                                source_id = root.id,
                                start = voice_start,
                                duration = voice_duration,
                                pitch = pitch,
                                selected = true,
                            }
                            existing[key] = true
                        end
                    end
                end
            end
        end
    end,
}
