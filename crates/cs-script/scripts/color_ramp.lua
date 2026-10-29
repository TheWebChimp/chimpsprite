-- Una rampa de 7 tonos a partir del color primario: las sombras se van hacia azul y las luces
-- hacia amarillo (como se hace a mano en pixel art). Se agregan al final de la paleta.
local function channel(hex, i) return tonumber(hex:sub(2 + i * 2, 3 + i * 2), 16) / 255 end

local function to_hsv(r, g, b)
  local mx, mn = math.max(r, g, b), math.min(r, g, b)
  local d = mx - mn
  local h = 0
  if d > 0 then
    if mx == r then h = 60 * math.fmod((g - b) / d, 6)
    elseif mx == g then h = 60 * ((b - r) / d + 2)
    else h = 60 * ((r - g) / d + 4) end
  end
  if h < 0 then h = h + 360 end
  return h, (mx > 0) and d / mx or 0, mx
end

local function to_hex(h, s, v)
  local c = v * s
  local x = c * (1 - math.abs(math.fmod(h / 60, 2) - 1))
  local m = v - c
  local r, g, b
  if h < 60 then r, g, b = c, x, 0
  elseif h < 120 then r, g, b = x, c, 0
  elseif h < 180 then r, g, b = 0, c, x
  elseif h < 240 then r, g, b = 0, x, c
  elseif h < 300 then r, g, b = x, 0, c
  else r, g, b = c, 0, x end
  local function byte(k)
    local n = math.floor((k + m) * 255 + 0.5)
    return math.max(0, math.min(255, n))
  end
  return string.format("#%02x%02x%02x", byte(r), byte(g), byte(b))
end

-- Gira el tono `h` hacia `target` hasta `step` grados.
local function toward(h, target, step)
  local diff = math.fmod(target - h + 540, 360) - 180
  local turn = (math.abs(diff) < step) and diff or ((diff > 0) and step or -step)
  return math.fmod(h + turn + 360, 360)
end

local base = primary()
if base == "transparent" then error("Pick an opaque primary color first") end
local h, s, v = to_hsv(channel(base, 0), channel(base, 1), channel(base, 2))
local ramp = {}
for _, i in ipairs{3, 2, 1} do
  ramp[#ramp + 1] = to_hex(toward(h, 240, 12 * i), math.min(1, s + 0.08 * i), math.max(0, v - 0.16 * i))
end
ramp[#ramp + 1] = to_hex(h, s, v)
for _, i in ipairs{1, 2, 3} do
  ramp[#ramp + 1] = to_hex(toward(h, 60, 10 * i), math.max(0, s - 0.12 * i), math.min(1, v + 0.12 * i))
end
local palette = {}
for _, a in ipairs(describe().artboards) do
  if a.id == current_artboard() then palette = a.palette end
end
for _, c in ipairs(ramp) do palette[#palette + 1] = c end
set_palette{ colors = palette }
print("Ramp: [" .. table.concat(ramp, ", ") .. "]")
