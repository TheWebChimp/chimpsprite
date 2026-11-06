-- Genera outline_aseprite.txt: los resultados de Edit > FX > Outline de Aseprite (1.3.18) para
-- comparar píxel a píxel con cs_raster::outline (tests/outline_aseprite.rs).
--
-- Uso (desde la raíz del repo):
--   Aseprite.exe -b --script-param out=crates/cs-raster/tests/fixtures/outline_aseprite.txt \
--     --script crates/cs-raster/tests/fixtures/outline_aseprite.lua
--
-- Formato de cada caso:
--   == nombre
--   place=outside|inside matrix=<0..511> tiled=none|x|y|both bg=none|b
--   <filas de entrada>
--   --
--   <filas de salida>
-- Colores: '.' transparente, '#' rojo, 'g' verde, 'b' azul (el fondo cuando bg=b) y 'k' negro
-- (el color del outline).
local OUT = app.params["out"] or "outline_aseprite.txt"
local out = io.open(OUT, "wb")

local palette = {
  ["."] = Color{ r = 0, g = 0, b = 0, a = 0 },
  ["#"] = Color{ r = 255, g = 0, b = 0, a = 255 },
  ["g"] = Color{ r = 0, g = 255, b = 0, a = 255 },
  ["b"] = Color{ r = 0, g = 0, b = 255, a = 255 },
  ["k"] = Color{ r = 0, g = 0, b = 0, a = 255 },
}

local function key(c)
  local px = app.pixelColor
  local r, g, b, a = px.rgbaR(c), px.rgbaG(c), px.rgbaB(c), px.rgbaA(c)
  if a == 0 then return "." end
  for k, v in pairs(palette) do
    if v.red == r and v.green == g and v.blue == b and v.alpha == a then return k end
  end
  error(string.format("unexpected color %02x%02x%02x%02x", r, g, b, a))
end

local function run(name, rows, place, matrix, tiled, bg)
  local h = #rows
  local w = #rows[1]
  local spr = Sprite(w, h, ColorMode.RGB)
  local img = Image(w, h, ColorMode.RGB)
  for y = 1, h do
    for x = 1, w do
      img:drawPixel(x - 1, y - 1, palette[rows[y]:sub(x, x)])
    end
  end
  spr.cels[1].image = img
  spr.cels[1].position = Point(0, 0)
  local params = {
    ui = false,
    color = palette["k"],
    matrix = matrix,
    place = place,
    tiledMode = tiled,
  }
  if bg then params.bgColor = palette["b"] end
  app.command.Outline(params)
  local flat = Image(spr.spec)
  flat:drawSprite(spr, 1)
  out:write("== " .. name .. "\n")
  out:write(string.format("place=%s matrix=%d tiled=%s bg=%s\n", place, matrix, tiled, bg and "b" or "none"))
  for _, row in ipairs(rows) do out:write(row .. "\n") end
  out:write("--\n")
  for y = 0, h - 1 do
    local line = ""
    for x = 0, w - 1 do line = line .. key(flat:getPixel(x, y)) end
    out:write(line .. "\n")
  end
  spr:close()
end

-- Los presets de Aseprite (octal): Circle 0252, Square 0757, Horizontal 0050, Vertical 0202.
local CIRCLE, SQUARE, HORIZONTAL, VERTICAL = 170, 495, 40, 130

local blob = {
  ".......",
  ".......",
  "..##...",
  "..###..",
  "...#...",
  ".......",
  ".......",
}
for _, place in ipairs({ "outside", "inside" }) do
  run(place .. "_circle", blob, place, CIRCLE, "none", false)
  run(place .. "_square", blob, place, SQUARE, "none", false)
  run(place .. "_horizontal", blob, place, HORIZONTAL, "none", false)
  run(place .. "_vertical", blob, place, VERTICAL, "none", false)
  for bit = 0, 8 do
    run(place .. "_bit" .. bit, blob, place, 1 << bit, "none", false)
  end
end

local onbg = {
  "bbbbbbb",
  "bbbbbbb",
  "bb##bbb",
  "bb#g#bb",
  "bbb#bbb",
  "bbbbbbb",
  "bbbbbbb",
}
run("bg_outside_circle", onbg, "outside", CIRCLE, "none", true)
run("bg_inside_square", onbg, "inside", SQUARE, "none", true)
run("no_bg_outside_circle", onbg, "outside", CIRCLE, "none", false)

-- En los bordes del lienzo (sin Tiled, lo de afuera es el píxel del borde más cercano).
local edge = {
  "##....",
  "#.....",
  "......",
  "......",
  ".....#",
}
run("edge_outside_circle", edge, "outside", CIRCLE, "none", false)
run("edge_inside_square", edge, "inside", SQUARE, "none", false)
run("edge_outside_top_left_only", edge, "outside", 1, "none", false)
run("edge_outside_bottom_right_only", edge, "outside", 256, "none", false)
run("tiled_outside_circle", edge, "outside", CIRCLE, "both", false)
run("tiled_x_outside_square", edge, "outside", SQUARE, "x", false)
run("tiled_y_outside_square", edge, "outside", SQUARE, "y", false)
local full = { "####", "####", "####" }
run("full_inside_circle", full, "inside", CIRCLE, "none", false)
run("full_inside_circle_tiled", full, "inside", CIRCLE, "both", false)

-- Casos al azar (siempre los mismos: LCG con semilla fija).
local seed = 12345
local function rand(n)
  seed = (seed * 1103515245 + 12345) % 2147483648
  return (seed // 65536) % n
end
local tiles = { "none", "x", "y", "both" }
local inks = { ".", ".", ".", "#", "#", "g", "b" }
for i = 1, 40 do
  local w, h = 4 + rand(9), 3 + rand(8)
  local rows = {}
  for y = 1, h do
    local row = ""
    for x = 1, w do row = row .. inks[1 + rand(#inks)] end
    rows[y] = row
  end
  local place = (rand(2) == 0) and "outside" or "inside"
  run("random_" .. i, rows, place, rand(512), tiles[1 + rand(4)], rand(2) == 0)
end

out:close()
