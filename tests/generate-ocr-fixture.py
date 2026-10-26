"""Generate original fixed text pixels for explicit local OCR verification.

Requires Pillow. Pass an installed font path; never capture a real screen.
Example on macOS:
python3 tests/generate-ocr-fixture.py /System/Library/Fonts/Supplemental/Arial.ttf
"""
from pathlib import Path
import sys
from PIL import Image, ImageDraw, ImageFont

font = ImageFont.truetype(sys.argv[1], 44)
image = Image.new("RGB", (1200, 500), "white")
draw = ImageDraw.Draw(image)
for y, text in [(50, "PHONE 1234567890"), (150, "Email demo@example.test"), (250, "Token abcdef0123456789")]:
    draw.text((60, y), text, fill="black", font=font)
destination = Path(__file__).resolve().parents[1] / "artifacts" / "ocr" / "generated-fixture.png"
destination.parent.mkdir(parents=True, exist_ok=True)
image.save(destination)
print(destination)
