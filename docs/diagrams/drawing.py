"""Small Pillow renderer for the checked-in documentation diagrams."""

from math import atan2, cos, sin, pi
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

INK = "#233b30"
MUTED = "#52665a"
GREEN = "#e9f1e1"
BLUE = "#e8eff6"
AMBER = "#fff1d8"
LINE = "#627b6b"
PAPER = "#f7f9f4"


class Diagram:
    def __init__(self, filename, title, subtitle, width=1200, height=1000):
        self.filename = filename
        self.width, self.height = width, height
        self.scale = 2
        self.image = Image.new("RGB", (width * 2, height * 2), PAPER)
        self.draw = ImageDraw.Draw(self.image)
        self.text(40, 30, "MY RIVIAN DATA  /  OFFLINE PROTOTYPE", size=14, color=MUTED)
        self.text(40, 63, title, size=32, bold=True)
        self.text(40, 111, subtitle, size=18, color=MUTED)

    def font(self, size, bold=False):
        candidates = [
            "DejaVuSans-Bold.ttf" if bold else "DejaVuSans.ttf",
            "arialbd.ttf" if bold else "arial.ttf",
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf" if bold
            else "/System/Library/Fonts/Supplemental/Arial.ttf",
        ]
        for name in candidates:
            try:
                return ImageFont.truetype(name, round(size * self.scale))
            except OSError:
                pass
        raise RuntimeError("Install DejaVu Sans or Arial to render documentation diagrams.")

    def text(self, x, y, value, size=18, color=INK, bold=False, center=False):
        font = self.font(size, bold)
        lines = value.split("\n")
        for i, line in enumerate(lines):
            px, py = x * 2, (y + i * (size + 8)) * 2
            if center:
                px -= self.draw.textlength(line, font=font) / 2
            self.draw.text((px, py), line, font=font, fill=color)

    def rect(self, x, y, width, height, fill="white", outline=LINE, radius=14, dashed=False):
        coords = (x * 2, y * 2, (x + width) * 2, (y + height) * 2)
        self.draw.rounded_rectangle(coords, radius=radius * 2, fill=fill,
                                    outline=None if dashed else outline, width=2)
        if dashed:
            for x0 in range(int(x), int(x + width), 16):
                for yy in (y, y + height):
                    self.draw.line((x0 * 2, yy * 2, min(x0 + 8, x + width) * 2, yy * 2), fill=outline, width=2)
            for y0 in range(int(y), int(y + height), 16):
                for xx in (x, x + width):
                    self.draw.line((xx * 2, y0 * 2, xx * 2, min(y0 + 8, y + height) * 2), fill=outline, width=2)

    def box(self, x, y, width, height, title, body="", fill="white", dashed=False):
        self.rect(x, y, width, height, fill=fill, dashed=dashed)
        self.text(x + 20, y + 17, title, size=20, bold=True)
        if body:
            self.text(x + 20, y + 52, body, size=17, color=MUTED)

    def arrow(self, points, color=LINE, dashed=False):
        points = [(x * 2, y * 2) for x, y in points]
        for (x0, y0), (x1, y1) in zip(points, points[1:]):
            if dashed:
                distance = ((x1 - x0) ** 2 + (y1 - y0) ** 2) ** 0.5
                for start in range(0, int(distance), 24):
                    end = min(start + 12, distance)
                    self.draw.line((x0 + (x1 - x0) * start / distance,
                                    y0 + (y1 - y0) * start / distance,
                                    x0 + (x1 - x0) * end / distance,
                                    y0 + (y1 - y0) * end / distance), fill=color, width=4)
            else:
                self.draw.line((x0, y0, x1, y1), fill=color, width=4)
        (x0, y0), (x1, y1) = points[-2:]
        angle = atan2(y1 - y0, x1 - x0)
        head = [(x1, y1)] + [(x1 - 19 * cos(angle + turn), y1 - 19 * sin(angle + turn))
                             for turn in (-pi / 6, pi / 6)]
        self.draw.polygon(head, fill=color)

    def footer(self, value):
        self.text(40, self.height - 55, value, size=15, color=MUTED)

    def save(self):
        destination = Path(__file__).parent / self.filename
        self.image.save(destination, optimize=True)
        return destination
