# Ocrs model attribution

The unmodified text detection and text recognition models bundled in the Windows
Starshot executable are by **Robert Knight**, from the author's Ocrs model host,
as referenced by the Ocrs 0.12.2 download script at commit
`2dbc1f840e47d45630ef6060499138bf597a9f65`:

https://github.com/robertknight/ocrs/blob/2dbc1f840e47d45630ef6060499138bf597a9f65/ocrs/examples/download-models.sh

- https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten
- https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten

Retrieved 2026-10-26; the original files report a 2024-01-01 Last-Modified date.
The exact redistributed copies are pinned by SHA-256 below, not fetched at runtime.
Run `npm run models:prepare` before building from source. The build helper downloads
from the project's unchanged mirror and verifies both byte count and SHA-256
before replacing a cached file. The original host returned HTTP 403 on 2026-10-28;
the mirror preserves the exact copies retrieved and verified on 2026-10-26:
https://github.com/cpython666/starshot/releases/tag/ocr-models-v1
Model binaries are ignored by Git and embedded in the Windows executable at
compile time. This build-dependency release is not an application update.

The author explicitly identifies the model license as **Creative Commons
Attribution-ShareAlike 4.0 International (CC BY-SA 4.0)** in:
https://github.com/robertknight/ocrs-models/issues/34#issuecomment-5085587667
The author's model card also declares the same license:
https://huggingface.co/robertknight/ocrs/blob/df0edd170279ab971b53e094c627255a87e1a503/README.md
See the accompanying
`LICENSE-CC-BY-SA-4.0.txt` and https://creativecommons.org/licenses/by-sa/4.0/.
Starshot has not modified the weights. The models retain their own license;
Starshot's MIT license does not replace the license on these assets.

| Included filename | Original filename | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| text-detection.rten | text-detection.rten | 2510284 | f15cfb56bd02c4bf478a20343986504a1f01e1665c2b3a0ad66340f054b1b5ca |
| text-recognition.rten | text-recognition.rten | 9716568 | e484866d4cce403175bd8d00b128feb08ab42e208de30e42cd9889d8f1735a6e |

The model card describes training on HierText and synthetic data:
https://github.com/google-research-datasets/hiertext

The OCR engine uses Ocrs 0.12.2 and RTen 0.24.0, under their MIT licenses (included
in `LICENSE-OCR-CODE.txt`):
https://github.com/robertknight/ocrs
https://github.com/robertknight/rten

Windows OCR recognizes Latin text and digits; these models do not recognize
Chinese. macOS uses the operating system's Vision framework for Chinese and
English, without loading these model files. OCR is invoked only for an explicit
user scan. There is no runtime model download or image upload.
