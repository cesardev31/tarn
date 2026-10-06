#!/usr/bin/env python3
"""Generate the Tarn logo: one peak and its reflection in still water.

Minimal by design: two shapes, two tones, readable at 16px, so the same
geometry serves as extension icon, file icon and README logo.

Outputs (SVG + PNG via Pillow, the marketplace requires PNG):
  editors/vscode/icons/tarn.svg, tarn.png, tarn-file.svg
  docs/assets/icon.svg, icon.png, logo.svg

Run: python3 editors/vscode/icons/make_icons.py
"""

from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]

SIZE = 128
RADIUS = 28
BG = "#0E2A3B"
PEAK = "#3FB0CF"
REFLECTION = "#1F5F78"

# Peak above the waterline, a shorter mirrored reflection below, a thin gap
# between them reads as the water surface.
PEAK_POLY = [(24, 76), (64, 26), (104, 76)]
REFL_POLY = [(24, 84), (64, 108), (104, 84)]
SHAPES = [(PEAK_POLY, PEAK), (REFL_POLY, REFLECTION)]


def svg_shapes() -> str:
    out = [f'<rect width="{SIZE}" height="{SIZE}" rx="{RADIUS}" fill="{BG}"/>']
    for poly, color in SHAPES:
        pts = " ".join(f"{x:g},{y:g}" for x, y in poly)
        out.append(f'<polygon points="{pts}" fill="{color}"/>')
    return "".join(out)


def icon_svg() -> str:
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {SIZE} {SIZE}">{svg_shapes()}</svg>\n'


def file_icon_svg() -> str:
    """A bold geometric T for the editor's 16px file icons."""
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">'
        f'<path fill="{PEAK}" d="M2 2h12v3h-4.5v9h-3V5H2z"/>'
        '</svg>\n'
    )


def artifact_icon_svg(kind: str) -> str:
    """Distinct 16px symbols for compiler snapshots."""
    shapes = {
        "diag": ('#E5AD52', '<path d="M8 1 15 14H1z"/><path fill="#0E2A3B" d="M7 5h2v5H7zm0 6h2v2H7z"/>'),
        "res": ('#B294E8', '<path d="M1 7h9L7 4l2-2 6 6-6 6-2-2 3-3H1z"/>'),
        "ast": ('#65C59C', '<path d="M6 1h4v4H9v2h4v4h2v4h-4v-4h1V8H4v3h1v4H1v-4h2V7h4V5H6z"/>'),
    }
    color, shape = shapes[kind]
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="{color}">{shape}</svg>\n'


def logo_svg() -> str:
    """Icon + wordmark, for the README."""
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 380 128">'
        f"{svg_shapes()}"
        f'<text x="150" y="88" font-family="Inter, Helvetica, Arial, sans-serif" font-size="72" '
        f'font-weight="600" letter-spacing="-1" fill="{PEAK}">tarn</text>'
        "</svg>\n"
    )


def png(path: Path, size: int) -> None:
    from PIL import Image, ImageDraw

    ss = 8  # supersampling for anti-aliasing
    k = size * ss / SIZE
    img = Image.new("RGBA", (size * ss, size * ss), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle([0, 0, img.size[0] - 1, img.size[1] - 1], radius=RADIUS * k, fill=BG)
    for poly, color in SHAPES:
        d.polygon([(x * k, y * k) for x, y in poly], fill=color)
    img.resize((size, size), Image.LANCZOS).save(path)


if __name__ == "__main__":
    (HERE / "tarn.svg").write_text(icon_svg())
    (HERE / "tarn-file.svg").write_text(file_icon_svg())
    for kind in ("diag", "res", "ast"):
        (HERE / f"tarn-{kind}.svg").write_text(artifact_icon_svg(kind))
    png(HERE / "tarn.png", 256)
    assets = ROOT / "docs" / "assets"
    assets.mkdir(exist_ok=True)
    (assets / "icon.svg").write_text(icon_svg())
    (assets / "logo.svg").write_text(logo_svg())
    png(assets / "icon.png", 512)
