-- Genera sprites con capas tilemap para probar la compatibilidad de ChimpSprite. Uso:
--   aseprite -b --script-param out=tilemap_rgba.aseprite --script-param mode=rgb --script tilemaps.lua
--   aseprite -b --script-param out=tilemap_indexed.aseprite --script-param mode=indexed --script tilemaps.lua
-- Dos capas tilemap comparten un tileset de 8×8 ("Terrain") con user data en el tileset y en el
-- tile 2; el mapa usa los 8 flips posibles, está vinculado en los 2 frames y la otra capa se mueve.
local out = app.params["out"]
local indexed = app.params["mode"] == "indexed"

local spr = Sprite(32, 24, ColorMode.RGB)
app.activeSprite = spr
spr.gridBounds = Rectangle(0, 0, 8, 8)

local function rgba(r, g, b, a) return Color{ r = r, g = g, b = b, a = a or 255 } end

-- Fondo normal, para que los tilemaps se compongan sobre algo.
local back = spr.layers[1]
back.name = "Back"
local bg = Image(32, 24, ColorMode.RGB)
for y = 0, 23 do
  for x = 0, 31 do bg:drawPixel(x, y, rgba(20 + x * 4, 40, 60 + y * 4)) end
end
spr:newCel(back, 1, bg, Point(0, 0))
spr:newEmptyFrame(2)
spr:newCel(back, 2, bg, Point(0, 0))

-- Dos capas tilemap; la segunda pasa a usar el tileset de la primera.
app.command.NewLayer{ name = "Ground", tilemap = true, gridBounds = Rectangle(0, 0, 8, 8) }
local ground = app.activeLayer
app.command.NewLayer{ name = "Deco", tilemap = true, gridBounds = Rectangle(0, 0, 8, 8) }
local deco = app.activeLayer
local ts = ground.tileset
local unused = deco.tileset
deco.tileset = ts
spr:deleteTileset(unused)
ts.name = "Terrain"
ts.data = "biome=grass"
ts.color = rgba(10, 200, 30)

-- Tiles 1..3: asimétricos (borde izquierdo e inferior, triángulo y dos puntos) para que cada
-- flip se distinga.
local function tile_image(seed)
  local img = Image(8, 8, ColorMode.RGB)
  for y = 0, 7 do
    for x = 0, 7 do
      if x == 0 or y == 7 then
        img:drawPixel(x, y, rgba(255, 60 * seed, 0))
      elseif x > y then
        img:drawPixel(x, y, rgba(0, 120, 40 * seed + 80, 200))
      end
    end
  end
  img:drawPixel(1, 1, rgba(255, 255, 255))
  img:drawPixel(6, 2, rgba(0, 0, 0))
  return img
end
for i = 1, 3 do
  local tile = spr:newTile(ts)
  tile.image = tile_image(i)
end
ts:tile(2).data = "solid"
ts:tile(2).color = rgba(200, 0, 0)

-- Bits de flip de Aseprite (los mismos que declara en la cabecera del cel tilemap).
local FLIP_X, FLIP_Y, FLIP_D = 0x80000000, 0x40000000, 0x20000000
local function tile(i, fx, fy, fd)
  local flags = 0
  if fx then flags = flags | FLIP_X end
  if fy then flags = flags | FLIP_Y end
  if fd then flags = flags | FLIP_D end
  return app.pixelColor.tile(i, flags)
end

local function tilemap(w, h, f)
  local img = Image(ImageSpec{ width = w, height = h, colorMode = ColorMode.TILEMAP })
  for y = 0, h - 1 do
    for x = 0, w - 1 do img:drawPixel(x, y, f(x, y)) end
  end
  return img
end

-- Ground: 4×3 celdas; las 8 primeras usan las 8 combinaciones de flips, la última fila vacía.
local ground_map = tilemap(4, 3, function(x, y)
  if y == 2 then return tile(0, false, false, false) end
  local combo = y * 4 + x
  return tile(1 + combo % 3, (combo & 1) ~= 0, (combo & 2) ~= 0, (combo & 4) ~= 0)
end)
spr:newCel(ground, 1, ground_map, Point(0, 0))
spr:newCel(ground, 2, ground_map, Point(0, 0))
app.range.layers = { ground }
app.range.frames = { spr.frames[1], spr.frames[2] }
app.command.LinkCels()
app.range:clear()

-- Deco: 2×1 celdas que se mueven y se aclaran en el frame 2.
local deco_map = tilemap(2, 1, function(x, y)
  if x == 0 then return tile(3, true, false, false) end
  return tile(2, false, false, true)
end)
spr:newCel(deco, 1, deco_map, Point(8, 16))
local moved = spr:newCel(deco, 2, deco_map, Point(16, 8))
moved.opacity = 160

if indexed then
  -- En modo batch la paleta inicial es negra entera: se genera una real con los colores del sprite
  -- (incluidos los tiles) antes de convertir.
  app.command.ColorQuantization{ ui = false, withAlpha = true, maxColors = 256 }
  app.command.ChangePixelFormat{ format = "indexed" }
end
spr:saveAs(out)
