from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter


ROOT = Path(__file__).resolve().parents[1]
ICON_DIR = ROOT / "src-tauri" / "icons"


def rounded_rectangle(draw, xy, radius, fill):
    draw.rounded_rectangle(xy, radius=radius, fill=fill)


CANVAS_SIZE = 1024


def make_source_icon():
    scale = CANVAS_SIZE / 256
    size = CANVAS_SIZE
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(canvas)

    def s(value):
        return int(round(value * scale))

    # Soft but deterministic vector-like gradient.
    for y in range(size):
      t = y / max(size - 1, 1)
      r = int(255 * (1 - t) + 246 * t)
      g = int(139 * (1 - t) + 47 * t)
      b = int(187 * (1 - t) + 126 * t)
      draw.line([(0, y), (size, y)], fill=(r, g, b, 255))

    mask = Image.new("L", (size, size), 0)
    mask_draw = ImageDraw.Draw(mask)
    mask_draw.rounded_rectangle((s(12), s(12), s(244), s(244)), radius=s(48), fill=255)
    canvas.putalpha(mask)
    draw = ImageDraw.Draw(canvas)

    # Main white arch as clean mask subtraction.
    arch = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    arch_mask = Image.new("L", (size, size), 0)
    am = ImageDraw.Draw(arch_mask)
    am.rounded_rectangle((s(48), s(48), s(208), s(214)), radius=s(80), fill=255)
    am.rounded_rectangle((s(82), s(82), s(174), s(215)), radius=s(46), fill=0)
    shadow = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    shadow.paste((126, 12, 60, 72), mask=arch_mask.filter(ImageFilter.GaussianBlur(s(2.2))))
    canvas.alpha_composite(shadow)
    arch.paste((255, 255, 255, 255), mask=arch_mask)
    canvas.alpha_composite(arch)

    # Inner tunnel silhouette.
    draw.rounded_rectangle((s(86), s(82), s(170), s(216)), radius=s(42), fill=(153, 15, 70, 255))
    draw.rectangle((s(86), s(143), s(170), s(216)), fill=(128, 12, 58, 255))

    # Floor plane.
    draw.polygon([(s(78), s(188)), (s(178), s(188)), (s(212), s(224)), (s(44), s(224))], fill=(255, 112, 166, 210))
    draw.line((s(83), s(188), s(173), s(188)), fill=(255, 170, 204, 190), width=s(3))

    # Reference-inspired key, drawn directly on the final diagonal to avoid rotation distortion.
    kd = ImageDraw.Draw(canvas)
    white = (255, 255, 255, 255)
    rose = (141, 15, 67, 255)
    shaft_width = s(15)
    shaft_start = (s(102), s(153))
    shaft_end = (s(147), s(108))
    kd.line((shaft_start, shaft_end), fill=white, width=shaft_width)
    r = shaft_width // 2
    kd.ellipse((shaft_start[0] - r, shaft_start[1] - r, shaft_start[0] + r, shaft_start[1] + r), fill=white)
    kd.ellipse((shaft_end[0] - r, shaft_end[1] - r, shaft_end[0] + r, shaft_end[1] + r), fill=white)
    kd.ellipse((s(132), s(90), s(172), s(130)), fill=white)
    kd.ellipse((s(144), s(102), s(160), s(118)), fill=rose)
    for start, end, width in [
        ((s(108), s(146)), (s(124), s(162)), s(12)),
        ((s(119), s(135)), (s(134), s(150)), s(11)),
    ]:
        kd.line((start, end), fill=white, width=width)
        rr = width // 2
        kd.ellipse((start[0] - rr, start[1] - rr, start[0] + rr, start[1] + rr), fill=white)
        kd.ellipse((end[0] - rr, end[1] - rr, end[0] + rr, end[1] + rr), fill=white)

    return canvas


def make_icon(size):
    source = make_source_icon()
    if size == CANVAS_SIZE:
        return source
    return source.resize((size, size), Image.Resampling.LANCZOS)


def main():
    ICON_DIR.mkdir(parents=True, exist_ok=True)
    base = make_icon(CANVAS_SIZE)
    base.save(ICON_DIR / "icon.png")
    images = [make_icon(size) for size in (256, 128, 64, 48, 32, 16)]
    images[0].save(ICON_DIR / "icon.ico", sizes=[(img.width, img.height) for img in images], append_images=images[1:])


if __name__ == "__main__":
    main()
