-- Genera archivos .aseprite "ricos" con Aseprite para probar la compatibilidad de ChimpSprite.
local out = app.params["out"]
local mode = app.params["mode"] == "indexed" and ColorMode.INDEXED or ColorMode.RGB

local spr = Sprite(32, 24, ColorMode.RGB)
app.activeSprite = spr

local function rgba(r, g, b, a) return Color{ r = r, g = g, b = b, a = a or 255 } end

local function fill(img, x0, y0, x1, y1, c)
  for y = y0, y1 do for x = x0, x1 do img:drawPixel(x, y, c) end end
end

-- Capa base: un rectángulo con borde.
local base = spr.layers[1]
base.name = "Base"
local img = Image(20, 14, ColorMode.RGB)
fill(img, 0, 0, 19, 13, rgba(40, 90, 200))
fill(img, 2, 2, 17, 11, rgba(250, 200, 60))
spr:newCel(base, 1, img, Point(6, 5))

-- Frames 2..4 con duraciones distintas.
for i = 2, 4 do spr:newEmptyFrame(i) end
spr.frames[1].duration = 0.1
spr.frames[2].duration = 0.15
spr.frames[3].duration = 0.2
spr.frames[4].duration = 0.05

-- Base vinculada en los frames 1-3 y movida en el 4.
spr:newCel(base, 2, img, Point(6, 5))
spr:newCel(base, 3, img, Point(6, 5))
app.range.layers = { base }
app.range.frames = { spr.frames[1], spr.frames[2], spr.frames[3] }
app.command.LinkCels()
app.range:clear()
local moved = spr:newCel(base, 4, img, Point(9, 7))
moved.opacity = 160

-- Grupo con una capa en multiply semitransparente y otra normal.
local group = spr:newGroup()
group.name = "FX"
local shade = spr:newLayer()
shade.name = "Shade"
shade.parent = group
shade.blendMode = BlendMode.MULTIPLY
shade.opacity = 180
local s = Image(12, 24, ColorMode.RGB)
fill(s, 0, 0, 11, 23, rgba(120, 40, 160))
for f = 1, 4 do spr:newCel(shade, f, s, Point(14 + f, 0)) end

local spark = spr:newLayer()
spark.name = "Spark"
spark.parent = group
local p = Image(3, 3, ColorMode.RGB)
fill(p, 0, 0, 2, 2, rgba(255, 255, 255, 200))
p:drawPixel(1, 1, rgba(255, 40, 40))
for f = 1, 4 do spr:newCel(spark, f, p, Point(2 + f * 4, 1 + f)) end

-- Capa oculta (no se ve) y capa en screen.
local hidden = spr:newLayer()
hidden.name = "Hidden"
hidden.isVisible = false
spr:newCel(hidden, 1, img, Point(0, 0))
local glow = spr:newLayer()
glow.name = "Glow"
glow.blendMode = BlendMode.SCREEN
local g = Image(8, 8, ColorMode.RGB)
fill(g, 0, 0, 7, 7, rgba(60, 120, 60))
spr:newCel(glow, 2, g, Point(1, 14))

-- Tags con colores y direcciones.
local walk = spr:newTag(1, 3)
walk.name = "walk"
walk.aniDir = AniDir.PING_PONG
walk.color = rgba(200, 50, 50)
local hit = spr:newTag(4, 4)
hit.name = "hit"
hit.aniDir = AniDir.REVERSE
hit.color = rgba(50, 200, 50)

-- Slices: hitbox con pivote y datos; panel 9-slice.
local hb = spr:newSlice(Rectangle(6, 5, 20, 14))
hb.name = "hitbox"
hb.pivot = Point(10, 14)
hb.color = rgba(255, 0, 128)
hb.data = "damage=3"
local panel = spr:newSlice(Rectangle(0, 0, 12, 12))
panel.name = "panel"
panel.center = Rectangle(3, 3, 6, 6)

if mode == ColorMode.INDEXED then
  app.command.ChangePixelFormat{ format = "indexed" }
end
spr:saveAs(out)
