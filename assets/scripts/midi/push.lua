-- Política de contacto del gesto: geometría en ticks, sin GPUI ni audio.
-- Devuelve una función reutilizable; las notas originales nunca se modifican.
local selected = ...

local cached_pitch, contacts
local function frontier(values, boundary, right)
    local lo, hi = 1, #values
    local found
    while lo <= hi do
        local mid = (lo + hi) // 2
        local value = values[mid]
        if right then
            if value <= boundary then found = value; lo = mid + 1 else hi = mid - 1 end
        else
            if value >= boundary then found = value; hi = mid - 1 else lo = mid + 1 end
        end
    end
    return found
end

local function prepare(pitch_delta, neighbors)
    local lanes = {}
    for _, row in ipairs(selected) do
        local pitch = row[4] + pitch_delta
        local lane = lanes[pitch] or {starts = {}, ends = {}}
        lanes[pitch] = lane
        table.insert(lane.starts, row[2])
        table.insert(lane.ends, row[3])
    end
    for _, lane in pairs(lanes) do
        table.sort(lane.starts)
        table.sort(lane.ends)
    end
    contacts = {}
    for _, row in ipairs(neighbors) do
        local lane = lanes[row[4]]
        if lane then
            -- Los solapamientos que ya existían no son contactos nuevos.
            local right = frontier(lane.ends, row[2], true)
            local left = frontier(lane.starts, row[3], false)
            if right or left then
                table.insert(contacts, {row = row, right = right, left = left})
            end
        end
    end
    cached_pitch = pitch_delta
end

return function(delta, pitch_delta, resize, neighbors)
    if cached_pitch ~= pitch_delta then prepare(pitch_delta, neighbors) end
    local right = delta > 0
    if resize and not right then return delta, {} end
    -- La vecina conserva al menos un tick: al agotarse, el avance se detiene.
    for _, contact in ipairs(contacts) do
        if right and contact.right then
            delta = math.min(delta, contact.row[3] - 1 - contact.right)
        elseif not right and contact.left then
            delta = math.max(delta, contact.row[2] + 1 - contact.left)
        end
    end
    local changes = {}
    for _, contact in ipairs(contacts) do
        local row = contact.row
        local first, last = row[2], row[3]
        if right and contact.right then
            first = math.max(first, contact.right + delta)
        elseif not right and contact.left then
            last = math.min(last, contact.left + delta)
        end
        if first ~= row[2] or last ~= row[3] then
            changes[#changes + 1] = {row[1], first, last}
        end
    end
    return delta, changes
end
