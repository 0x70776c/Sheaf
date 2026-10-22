"""Sheaf icon generator: renders the PNG with PIL and writes a matching SVG (single source of parameters)."""
import math, sys
import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageChops

# ---------- parameters (1024 design space) ----------
CANVAS = 1024
TILE_HALF = 412          # 824 px tile as in Apple's macOS icon grid
SQUIRCLE_N = 5.0
BG_TOP, BG_BOT = (0x2E, 0x2C, 0x29), (0x14, 0x13, 0x12)
IVORY_TOP, IVORY_BOT = (0xFB, 0xF7, 0xEE), (0xE6, 0xDD, 0xCA)
GOLD_TOP, GOLD_BOT = (0xF2, 0xC5, 0x5C), (0xDE, 0xA2, 0x32)
ANGLES  = [-36, -18, 0, 18, 36]
LENGTHS = [412, 438, 450, 438, 412]   # above pivot
STRIP_W = 50
TAIL = 210                # below pivot (before the flat cut)
GAP_UP, GAP_H = 40, 80  # gap band centered on pivot (negative space)
BAR_H, BAR_HALF = 44, 120
PIVOT = (512, 600)
S = 3                    # supersampling
CUT = 126                # flat cut below pivot

def lerp(a, b, t): return tuple(int(round(a[i] + (b[i]-a[i])*t)) for i in range(3))

# bounding box of the mark to center it vertically
def mark_bbox():
    top = min(PIVOT[1] - L*math.cos(math.radians(a)) - (STRIP_W/2)*math.sin(math.radians(abs(a))) for a, L in zip(ANGLES, LENGTHS))
    bot = PIVOT[1] + CUT
    return top, bot
top, bot = mark_bbox()
dy = 512 - (top + bot)/2 + 6   # +6: optical centering (mark is top-heavy)
PIVOT = (PIVOT[0], PIVOT[1] + dy)

def squircle_mask(size, half, n, scale):
    N = size*scale
    c = N/2
    ys, xs = np.mgrid[0:N, 0:N].astype(np.float32)
    x = np.abs((xs + 0.5 - c) / (half*scale)); y = np.abs((ys + 0.5 - c) / (half*scale))
    m = (x**n + y**n) <= 1.0
    return Image.fromarray((m*255).astype(np.uint8), 'L')

def vgrad(size, top, bot, y0, y1):
    N = size
    arr = np.zeros((N, N, 3), np.uint8)
    t = np.clip((np.arange(N) - y0) / max(1, (y1 - y0)), 0, 1)[:, None]
    for i in range(3):
        arr[:, :, i] = (top[i] + (bot[i]-top[i]) * t).astype(np.uint8)
    return Image.fromarray(arr, 'RGB')

def render():
    N = CANVAS*S
    # --- tile ---
    tile_mask = squircle_mask(CANVAS, TILE_HALF, SQUIRCLE_N, S)
    ty0, ty1 = (CANVAS/2 - TILE_HALF)*S, (CANVAS/2 + TILE_HALF)*S
    bg = vgrad(N, BG_TOP, BG_BOT, ty0, ty1).convert('RGBA')
    # subtle top-left light bloom
    bloom = Image.new('L', (N, N), 0)
    ImageDraw.Draw(bloom).ellipse((N*0.12, -N*0.25, N*0.88, N*0.45), fill=34)
    bloom = bloom.filter(ImageFilter.GaussianBlur(N*0.06))
    bg.paste(Image.new('RGBA', (N, N), (255, 244, 225, 255)), (0, 0), bloom)
    tile = Image.new('RGBA', (N, N), (0, 0, 0, 0)); tile.paste(bg, (0, 0), tile_mask)
    # inner hairline highlight
    inner = tile_mask.filter(ImageFilter.MinFilter(2*S+1))
    rim = ImageChops.subtract(tile_mask, inner)
    rim_top = Image.new('L', (N, N), 0)
    ImageDraw.Draw(rim_top).rectangle((0, 0, N, N*0.5), fill=255)
    rim = ImageChops.multiply(rim, rim_top).point(lambda v: int(v*0.16))
    tile.paste(Image.new('RGBA', (N, N), (255, 255, 255, 255)), (0, 0), rim)

    # --- mark: strips ---
    px, py = PIVOT[0]*S, PIVOT[1]*S
    strips = Image.new('RGBA', (N, N), (0, 0, 0, 0))
    ivory = vgrad(N, IVORY_TOP, IVORY_BOT, (PIVOT[1]-500)*S, (PIVOT[1]+TAIL)*S).convert('RGBA')
    for a, L in zip(ANGLES, LENGTHS):
        layer = Image.new('L', (N, N), 0)
        d = ImageDraw.Draw(layer)
        w = STRIP_W*S
        d.rounded_rectangle((px - w/2, py - L*S, px + w/2, py + TAIL*S), radius=w/2, fill=255)
        layer = layer.rotate(-a, center=(px, py), resample=Image.BICUBIC)   # PIL rotates counter-clockwise
        strips.paste(ivory, (0, 0), layer)
    # flat cut of the stalk bases
    cut = Image.new('L', (N, N), 255)
    ImageDraw.Draw(cut).rectangle((0, (PIVOT[1]+CUT)*S, N, N), fill=0)
    strips.putalpha(ImageChops.multiply(strips.getchannel('A'), cut))
    # negative-space gap (the "tie" is a cut)
    gap = Image.new('L', (N, N), 255)
    ImageDraw.Draw(gap).rectangle((0, (PIVOT[1]-GAP_UP)*S, N, (PIVOT[1]-GAP_UP+GAP_H)*S), fill=0)
    alpha = ImageChops.multiply(strips.getchannel('A'), gap)
    strips.putalpha(alpha)
    # --- gold bar ---
    bar_mask = Image.new('L', (N, N), 0)
    by0 = (PIVOT[1]-GAP_UP + (GAP_H-BAR_H)/2)*S
    ImageDraw.Draw(bar_mask).rounded_rectangle((px - BAR_HALF*S, by0, px + BAR_HALF*S, by0 + BAR_H*S), radius=BAR_H*S/2, fill=255)
    gold = vgrad(N, GOLD_TOP, GOLD_BOT, by0, by0 + BAR_H*S).convert('RGBA')
    mark = Image.new('RGBA', (N, N), (0, 0, 0, 0))
    mark.alpha_composite(strips); mark.paste(gold, (0, 0), bar_mask)
    # soft shadow under mark for depth
    sh = Image.new('RGBA', (N, N), (0, 0, 0, 0))
    sh.paste(Image.new('RGBA', (N, N), (0, 0, 0, 120)), (0, 14*S), mark.getchannel('A'))
    sh = sh.filter(ImageFilter.GaussianBlur(10*S))
    tile.alpha_composite(sh)
    tile.alpha_composite(mark)

    # --- tile drop shadow on transparent canvas ---
    canvas = Image.new('RGBA', (N, N), (0, 0, 0, 0))
    shadow_a = tile_mask.point(lambda v: int(v*0.42))
    shadow = Image.new('RGBA', (N, N), (0, 0, 0, 0)); shadow.paste(Image.new('RGBA', (N, N), (0, 0, 0, 255)), (0, 12*S), shadow_a)
    shadow = shadow.filter(ImageFilter.GaussianBlur(14*S))
    canvas.alpha_composite(shadow); canvas.alpha_composite(tile)
    return canvas.resize((CANVAS, CANVAS), Image.LANCZOS)

if __name__ == '__main__':
    img = render()
    img.save('icon-1024.png')
    print('pivot', PIVOT)
