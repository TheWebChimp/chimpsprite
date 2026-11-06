-- Corre en Aseprite los escenarios de tile_pixels.json (píxeles sobre tilemaps con los modos
-- Manual/Auto/Stack) y escribe el resultado, que tile_pixels_aseprite.rs compara con ChimpSprite:
--   aseprite -b --script-param dir=<esta carpeta> --script tile_pixels.lua
-- Escribe tile_pixels.expected. Los flips permitidos del tileset no tienen API en Lua: el sprite se
-- guarda, se parchean los flags del chunk de tileset (8 = X, 16 = Y, 32 = D) y se vuelve a abrir.
local dir = app.params["dir"]
local file = io.open(dir .. "/tile_pixels.json", "r")
local spec = json.decode(file:read("a"))
file:close()

local FLIPS = { x = 0x80000000, y = 0x40000000, d = 0x20000000 }

local function color(hex)
  return Color{
    r = tonumber(hex:sub(1, 2), 16),
    g = tonumber(hex:sub(3, 4), 16),
    b = tonumber(hex:sub(5, 6), 16),
    a = 255,
  }
end

local function tile_value(v)
  if type(v) == "number" then return math.tointeger(v) end
  local index = tonumber(v:match("^%d+"))
  local flags = 0
  for c in v:gmatch("[xyd]") do flags = flags | FLIPS[c] end
  return app.pixelColor.tile(index, flags)
end

-- Pone los flags de flips permitidos en el chunk 0x2023 del archivo.
local function patch_flips(path, flips)
  local extra = 0
  if flips:find("x") then extra = extra | 8 end
  if flips:find("y") then extra = extra | 16 end
  if flips:find("d") then extra = extra | 32 end
  local f = io.open(path, "rb")
  local data = f:read("a")
  f:close()
  local frames = string.unpack("<I2", data, 7)
  local pos = 129
  for _ = 1, frames do
    local fsize, _, old = string.unpack("<I4I2I2", data, pos)
    local nchunks = string.unpack("<I4", data, pos + 12)
    if nchunks == 0 then nchunks = old end
    local c = pos + 16
    for _ = 1, nchunks do
      local csize, ctype = string.unpack("<I4I2", data, c)
      if ctype == 0x2023 then
        local flags = string.unpack("<I4", data, c + 10)
        data = data:sub(1, c + 9) .. string.pack("<I4", flags | extra) .. data:sub(c + 14)
      end
      c = c + csize
    end
    pos = pos + fsize
  end
  f = io.open(path, "wb")
  f:write(data)
  f:close()
end

local function setup(sc)
  local spr = Sprite(16, 8, ColorMode.RGB)
  spr.gridBounds = Rectangle(0, 0, 4, 4)
  for _ = 2, sc.frames do spr:newEmptyFrame() end
  local image_layer = spr.layers[1]
  local ts
  for li, frames in ipairs(sc.layers) do
    app.command.NewLayer{ name = "Map" .. li, tilemap = true, gridBounds = Rectangle(0, 0, 4, 4) }
    local layer = app.activeLayer
    if ts == nil then
      ts = layer.tileset
      for _, pixels in ipairs(sc.tiles) do
        local tile = spr:newTile(ts)
        local img = Image(4, 4, ColorMode.RGB)
        for _, p in ipairs(pixels) do img:drawPixel(p[1], p[2], color(p[3])) end
        tile.image = img
      end
    else
      local unused = layer.tileset
      layer.tileset = ts
      spr:deleteTileset(unused)
    end
    for fi, rows in ipairs(frames) do
      if rows ~= nil then
        local map = Image(ImageSpec{ width = #rows[1], height = #rows, colorMode = ColorMode.TILEMAP })
        for y, row in ipairs(rows) do
          for x, v in ipairs(row) do map:drawPixel(x - 1, y - 1, tile_value(v)) end
        end
        local pos = Point(0, 0)
        if li == 1 and fi == 1 and sc.cel then pos = Point(sc.cel[1], sc.cel[2]) end
        spr:newCel(layer, fi, map, pos)
      end
    end
  end
  spr:deleteLayer(image_layer)
  if sc.linked then
    app.range.layers = { spr.layers[1] }
    app.range.frames = { 1, 2 }
    app.command.LinkCels()
  end
  if sc.flips then
    local path = app.fs.joinPath(app.fs.tempPath, "tile_pixels_flips.aseprite")
    spr:saveAs(path)
    spr:close()
    patch_flips(path, sc.flips)
    spr = app.open(path)
  end
  return spr
end

local MODES = { manual = TilesetMode.MANUAL, auto = TilesetMode.AUTO, stack = TilesetMode.STACK }

local function stroke(spr, s)
  local points = {}
  for _, p in ipairs(s.points) do points[#points + 1] = Point(p[1], p[2]) end
  app.useTool{
    tool = s.tool or "pencil",
    color = color(s.color or "000000"),
    brush = Brush(1),
    points = points,
    layer = spr.layers[s.layer or 1],
    frame = spr.frames[s.frame or 1],
    tilemapMode = TilemapMode.PIXELS,
    tilesetMode = MODES[s.mode],
  }
end

local function hex(c)
  return string.format("%02x%02x%02x%02x", app.pixelColor.rgbaR(c), app.pixelColor.rgbaG(c),
    app.pixelColor.rgbaB(c), app.pixelColor.rgbaA(c))
end

local out = {}
local function dump(name, spr)
  out[#out + 1] = "== " .. name
  local ts = spr.layers[1].tileset
  for i = 1, #ts - 1 do
    local img = ts:getTile(i)
    local px = {}
    for y = 0, img.height - 1 do
      for x = 0, img.width - 1 do
        local c = img:getPixel(x, y)
        if app.pixelColor.rgbaA(c) ~= 0 then px[#px + 1] = x .. "," .. y .. "=" .. hex(c) end
      end
    end
    out[#out + 1] = "tile " .. i .. ":" .. (#px > 0 and " " or "") .. table.concat(px, " ")
  end
  for li, layer in ipairs(spr.layers) do
    for _, cel in ipairs(layer.cels) do
      local img = cel.image
      local cells = {}
      for y = 0, img.height - 1 do
        if y > 0 then cells[#cells + 1] = "|" end
        for x = 0, img.width - 1 do
          local t = img:getPixel(x, y)
          local s = tostring(app.pixelColor.tileI(t))
          local f = app.pixelColor.tileF(t)
          if f & FLIPS.x ~= 0 then s = s .. "x" end
          if f & FLIPS.y ~= 0 then s = s .. "y" end
          if f & FLIPS.d ~= 0 then s = s .. "d" end
          cells[#cells + 1] = s
        end
      end
      out[#out + 1] = string.format("L%d F%d (%d,%d) %s", li, cel.frameNumber, cel.position.x,
        cel.position.y, table.concat(cells, " "))
    end
  end
end

for _, sc in ipairs(spec.scenarios) do
  local spr = setup(sc)
  for _, s in ipairs(sc.strokes) do stroke(spr, s) end
  dump(sc.name, spr)
  spr:close()
end

file = io.open(dir .. "/tile_pixels.expected", "w")
file:write(table.concat(out, "\n") .. "\n")
file:close()
