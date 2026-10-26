"""Verify the Windows release OCR subprocess using only original fixed pixels.

python tests/ocr-worker-smoke.py src-tauri/target/release/starshot.exe
Does not install or launch the GUI. Uses the app's internal stdin/stdout worker.
"""
import base64
import json
import os
from pathlib import Path
import subprocess
import sys

if os.name != "nt":
    raise SystemExit("This check runs only on Windows.")
executable = Path(sys.argv[1]).resolve(strict=True)
fixture = Path(__file__).resolve().parent / "fixtures" / "ocr-latin.png"
payload = b"data:image/png;base64," + base64.b64encode(fixture.read_bytes())
result = subprocess.run(
    [str(executable), "--starshot-ocr-worker"],
    input=payload,
    stdout=subprocess.PIPE,
    stderr=subprocess.DEVNULL,
    creationflags=subprocess.CREATE_NO_WINDOW,
    timeout=95,
    check=True,
    env={**os.environ, "RTEN_NUM_THREADS": "2", "RAYON_NUM_THREADS": "2"},
)
assert len(result.stdout) <= 2 * 1024 * 1024, "OCR result exceeds its IPC limit"
response = json.loads(result.stdout)
assert "Ok" in response, "OCR worker returned an error"
lines = response["Ok"]
assert any("1234567890" in line["text"] for line in lines), "OCR missed the fixed digits"
assert any("demo@example.test" in line["text"] for line in lines), "OCR missed the fixed email"
assert all(
    0 <= line["x"] < 1200 and 0 <= line["y"] < 500
    and 0 < line["width"] <= 1200 and 0 < line["height"] <= 500
    and line["x"] + line["width"] <= 1200
    and line["y"] + line["height"] <= 500
    for line in lines
), "OCR returned out-of-image coordinates"
print("Windows OCR worker: fixed text and image coordinates passed.")
