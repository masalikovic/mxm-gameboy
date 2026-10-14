#!/usr/bin/env python3
"""Membuat ikon PWA MXM Game Boy (web/icons/*.png) dari kode, bukan gambar
pihak ketiga (ADR-0006). Gaya MXM (ADR-0008): latar tinta, layar oranye,
tulisan piksel "MXM" berwarna tinta, D-pad cream. Ikon "maskable": isi penting berada di dalam
lingkaran 80 % di tengah, karena Android bisa memotong ikon menjadi bulat.

Jalankan ulang hanya kalau desain ikon berubah:
    python3 tools/make-icons.py      (butuh Pillow)
"""
from pathlib import Path

from PIL import Image, ImageDraw

INK = (0x0E, 0x0E, 0x0E)
CREAM = (0xF5, 0xEB, 0xD7)
ORANGE = (0xFF, 0x8A, 0x3D)
BROWN = (0x7A, 0x3F, 0x17)

# Huruf piksel 5×7 untuk "MXM".
GLYPHS = {
    "M": ["1...1", "11.11", "1.1.1", "1.1.1", "1...1", "1...1", "1...1"],
    "X": ["1...1", "1...1", ".1.1.", "..1..", ".1.1.", "1...1", "1...1"],
}


def icon(size: int) -> Image.Image:
    image = Image.new("RGB", (size, size), INK)
    draw = ImageDraw.Draw(image)
    unit = size / 32  # kisi 32×32 supaya tajam di semua ukuran
    # Layar oranye dengan bingkai cokelat, di dalam zona aman maskable.
    left, top, right, bottom = 7 * unit, 8 * unit, 25 * unit, 22 * unit
    draw.rounded_rectangle((left - unit, top - unit, right + unit, bottom + unit), radius=unit * 2, fill=BROWN)
    draw.rectangle((left, top, right, bottom), fill=ORANGE)
    # "MXM" dalam piksel tinta: 3 huruf × 5 + 2 spasi = 17 kolom.
    pixel = unit * 0.95
    x0, y0 = 16 * unit - 17 * pixel / 2, 15 * unit - 7 * pixel / 2
    for index, letter in enumerate("MXM"):
        for row, line in enumerate(GLYPHS[letter]):
            for col, cell in enumerate(line):
                if cell == "1":
                    x = x0 + (index * 6 + col) * pixel
                    y = y0 + row * pixel
                    draw.rectangle((x, y, x + pixel - 1, y + pixel - 1), fill=INK)
    # Dua tombol oranye di bawah layar, seperti Game Boy.
    for cx in (19, 23):
        cy = 25.5
        draw.ellipse(((cx - 1.4) * unit, (cy - 1.4) * unit, (cx + 1.4) * unit, (cy + 1.4) * unit), fill=ORANGE)
    # D-pad cream di kiri bawah.
    draw.rectangle((9 * unit, 24.5 * unit, 13 * unit, 26.5 * unit), fill=CREAM)
    draw.rectangle((10 * unit, 23.5 * unit, 12 * unit, 27.5 * unit), fill=CREAM)
    return image


def main() -> None:
    out = Path(__file__).resolve().parent.parent / "web" / "icons"
    out.mkdir(parents=True, exist_ok=True)
    for size in (192, 512):
        icon(size).save(out / f"icon-{size}.png", optimize=True)
    print(f"Ikon ditulis ke {out}")


if __name__ == "__main__":
    main()
