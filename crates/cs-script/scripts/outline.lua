-- Contorno de 1 px con el color primario alrededor de lo pintado en la capa actual
-- (con selección, solo dentro de ella).
local px = pixels()
local w, h = width(), height()
local color = primary()
local sel = selection()
local selected = {}
if sel then
  for _, p in ipairs(sel) do selected[p[1] .. "," .. p[2]] = true end
end
local function painted(x, y)
  return x >= 0 and y >= 0 and x < w and y < h and px[y + 1][x + 1] ~= "transparent"
end
local out = {}
for y = 0, h - 1 do
  for x = 0, w - 1 do
    if not painted(x, y) and (not sel or selected[x .. "," .. y]) then
      if painted(x + 1, y) or painted(x - 1, y) or painted(x, y + 1) or painted(x, y - 1) then
        out[#out + 1] = { x = x, y = y, color = color }
      end
    end
  end
end
if #out == 0 then
  print("Nothing to outline on this layer")
else
  draw_pixels{ pixels = out }
  print("Outline: " .. #out .. " pixels")
end
