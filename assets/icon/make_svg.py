import math, make_icon as m

def squircle_path(half, n, steps=288):
    pts = []
    for i in range(steps):
        t = 2*math.pi*i/steps
        c, s = math.cos(t), math.sin(t)
        x = 512 + half*math.copysign(abs(c)**(2/n), c)
        y = 512 + half*math.copysign(abs(s)**(2/n), s)
        pts.append(f"{x:.1f},{y:.1f}")
    return "M" + " L".join(pts) + " Z"

px, py = m.PIVOT
hx = lambda c: "#%02X%02X%02X" % c
gap_top = py - m.GAP_UP
gap_bot = gap_top + m.GAP_H
cut_y = py + m.CUT
bar_y = gap_top + (m.GAP_H - m.BAR_H)/2
strips = "\n".join(
    f'      <rect x="{px - m.STRIP_W/2}" y="{py - L}" width="{m.STRIP_W}" height="{L + m.TAIL}" rx="{m.STRIP_W/2}" transform="rotate({a} {px} {py})"/>'
    for a, L in zip(m.ANGLES, m.LENGTHS))
d = squircle_path(m.TILE_HALF, m.SQUIRCLE_N)
svg = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">
  <title>Sheaf</title>
  <defs>
    <linearGradient id="bg" x1="0" y1="100" x2="0" y2="924" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="{hx(m.BG_TOP)}"/><stop offset="1" stop-color="{hx(m.BG_BOT)}"/>
    </linearGradient>
    <radialGradient id="bloom" cx="512" cy="40" r="470" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#FFF4E1" stop-opacity="0.14"/><stop offset="1" stop-color="#FFF4E1" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="rim" x1="0" y1="100" x2="0" y2="512" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.18"/><stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/>
    </linearGradient>
    <linearGradient id="ivory" x1="0" y1="{py - 500}" x2="0" y2="{py + m.TAIL}" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="{hx(m.IVORY_TOP)}"/><stop offset="1" stop-color="{hx(m.IVORY_BOT)}"/>
    </linearGradient>
    <linearGradient id="gold" x1="0" y1="{bar_y}" x2="0" y2="{bar_y + m.BAR_H}" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="{hx(m.GOLD_TOP)}"/><stop offset="1" stop-color="{hx(m.GOLD_BOT)}"/>
    </linearGradient>
    <mask id="stalks" maskUnits="userSpaceOnUse" x="0" y="0" width="1024" height="1024">
      <g fill="#fff">
{strips}
      </g>
    </mask>
    <clipPath id="cuts">
      <rect x="0" y="0" width="1024" height="{gap_top}"/>
      <rect x="0" y="{gap_bot}" width="1024" height="{cut_y - gap_bot}"/>
    </clipPath>
    <filter id="tileShadow" x="-10%" y="-10%" width="120%" height="125%"><feGaussianBlur stdDeviation="14"/></filter>
    <filter id="markShadow" x="-20%" y="-20%" width="140%" height="150%"><feGaussianBlur stdDeviation="10"/></filter>
  </defs>
  <!-- tile shadow -->
  <path d="{d}" transform="translate(0 12)" fill="#000" fill-opacity="0.42" filter="url(#tileShadow)"/>
  <!-- tile -->
  <path d="{d}" fill="url(#bg)"/>
  <path d="{d}" fill="url(#bloom)"/>
  <path d="{d}" fill="none" stroke="url(#rim)" stroke-width="2"/>
  <!-- mark shadow -->
  <g transform="translate(0 14)" opacity="0.47" filter="url(#markShadow)">
    <g clip-path="url(#cuts)"><rect width="1024" height="1024" fill="#000" mask="url(#stalks)"/></g>
    <rect x="{px - m.BAR_HALF}" y="{bar_y}" width="{2*m.BAR_HALF}" height="{m.BAR_H}" rx="{m.BAR_H/2}" fill="#000"/>
  </g>
  <!-- mark: stalks (ivory), tie is a cut in the bundle with a gold bar -->
  <g clip-path="url(#cuts)"><rect width="1024" height="1024" fill="url(#ivory)" mask="url(#stalks)"/></g>
  <rect x="{px - m.BAR_HALF}" y="{bar_y}" width="{2*m.BAR_HALF}" height="{m.BAR_H}" rx="{m.BAR_H/2}" fill="url(#gold)"/>
</svg>
'''
open('icon.svg', 'w').write(svg)
print(len(svg), 'bytes')
