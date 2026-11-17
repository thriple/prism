# Make Prism.icns from the owner's square image, in Apple's macOS app icon
# template (Big Sur to Sequoia): a 1024 canvas, the shape 824 px square at
# 100 px in, its corners continuous (a superellipse), and a soft shadow
# under it, so it matches the other icons in the Dock.
# Usage: python3 make-icon.py IMAGE Prism.icns PREVIEW.png (needs Pillow).
# Run by hand when the image changes; the build only copies Prism.icns.
import sys
from PIL import Image, ImageDraw, ImageFilter
src, out, preview = sys.argv[1], sys.argv[2], sys.argv[3]
S, BODY, OFF = 1024, 824, 100
im = Image.open(src).convert("RGB")
w, h = im.size
side = min(w, h)
im = im.crop(((w - side) // 2, (h - side) // 2, (w + side) // 2, (h + side) // 2))
art = im.resize((BODY, BODY), Image.LANCZOS)

# The shape: a superellipse |x|^n + |y|^n = 1 near Apple's corner, drawn 4x
# and scaled down for smooth edges.
k = 4
n = 5.0
big = BODY * k
pts = []
import math
for i in range(2000):
    t = 2 * math.pi * i / 2000
    c, s = math.cos(t), math.sin(t)
    x = math.copysign(abs(c) ** (2 / n), c)
    y = math.copysign(abs(s) ** (2 / n), s)
    pts.append(((x + 1) / 2 * (big - 1), (y + 1) / 2 * (big - 1)))
mask = Image.new("L", (big, big), 0)
ImageDraw.Draw(mask).polygon(pts, fill=255)
mask = mask.resize((BODY, BODY), Image.LANCZOS)

canvas = Image.new("RGBA", (S, S), (0, 0, 0, 0))
# Shadow: the shape, black at 30%, 10 px down, blurred.
shadow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
sh = Image.new("RGBA", (BODY, BODY), (0, 0, 0, 77))
shadow.paste(sh, (OFF, OFF + 10), mask)
shadow = shadow.filter(ImageFilter.GaussianBlur(14))
canvas = Image.alpha_composite(canvas, shadow)
body = art.convert("RGBA")
body.putalpha(mask)
canvas.alpha_composite(body, (OFF, OFF))
canvas.save(out, format="ICNS", sizes=[(16, 16), (32, 32), (64, 64), (128, 128), (256, 256), (512, 512), (1024, 1024)])
# A preview on a light Dock-like strip beside a plain grey squircle.
bg = Image.new("RGBA", (S, S), (236, 236, 236, 255))
bg.alpha_composite(canvas)
bg.convert("RGB").resize((256, 256), Image.LANCZOS).save(preview)
back = Image.open(out)
print(back.format, back.size, back.mode)
