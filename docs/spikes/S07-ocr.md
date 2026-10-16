# Spike S07: OCR bake-off (covers S12)

Date: 2026-10-06. Code: [`spikes/s01-foundation/qao-test/ocr.html`](https://github.com/InsaneArts/snip/blob/1b9ab47a31056383be0a7b2a2d4506ceb580fa36/spikes/s01-foundation/qao-test/ocr.html), `qao-test/s12_ocr.py`.

## Method
- Seven text samples with **known ground truth** are rendered in Chromium and **grabbed through Snip's capture path** at physical resolution. The page renders at 1.68 device px per CSS px on this machine.
  - UI text, light and dark (13 px)
  - small low-contrast text (11 px grey)
  - indented code (JetBrains Mono)
  - **Georgian**
  - Russian
  - mixed Georgian and English
- Scored by **character error rate (CER)**, whitespace-normalised. For code, leading indentation counts.
- Engines:
  - Tesseract 5.5 with Arch's `tessdata` (int "best" LSTM) and with `tessdata_fast`;
  - variants with 2× Lanczos upscale, per-region inversion of dark samples, and `OMP_THREAD_LIMIT=1`;
  - **TSV clean-up**: word boxes, drop words with confidence < 35, rebuild lines and code indentation from box positions;
  - **RapidOCR** (PP-OCR on ONNX Runtime 1.30, default models).

## Results: CER %, and ms per region excluding model init

| Sample | best | fast | fast + 2× + invert | **+ TSV clean-up (chosen)** | RapidOCR |
|---|---|---|---|---|---|
| UI light | 0.0 (65) | 0.0 (60) | 0.0 (57) | **0.0** | 0.0 (1181) |
| UI dark | 2.0 (118) | 2.0 (82) | 2.0 (60) | **2.0** | 2.7 (1196) |
| Small 11 px grey | 0.0 | 0.0 | 0.0 (51) | **0.0** | 0.0 (767) |
| Code (indent counted) | 24.6 | 30.5 | 27.8 (66) | **8.0** (indentation rebuilt) | 24.1 (646) |
| **Georgian** | 0.0 | 0.0 | 0.0 (62) | **0.0** | n/a (no model) |
| Russian | 2.5 | 2.5 | 2.5 (37) | **2.5** | 92.6 (default models are Latin/CJK) |
| Mixed Georgian + English | 142 | 142 | 128 (203) | **6.6** | n/a |

Model and process init is about 36 ms (fast) to 48 ms (best) per call. A daemon pays it once per language set and keeps a warm `TessBaseAPI`.

## Decisions
1. **Default engine: Tesseract 5.5 + `tessdata_fast`, linked in-process.**
   - Pipeline: 2× upscale (at scale ≤ 1.25), per-region invert when dark, `OMP_THREAD_LIMIT=1`, PSM 6, then **TSV clean-up**.
   - Accuracy: 0–2.5 % CER on UI text, small text, Georgian and Russian; about 50–65 ms per region warm.
   - Ship our own `tessdata_fast` models: Arch packages only the larger `tessdata` set, which was no more accurate here and up to 1.5× slower.
2. **The TSV clean-up is required, not optional.**
   - It cut code CER from 28 % to 8 % by rebuilding indentation from word boxes.
   - It cut mixed-script CER from 128 % to 6.6 % by dropping a garbage line that a selection edge produced.
3. **RapidOCR / PP-OCR is not the default.** No accuracy gain on these samples, **10–20× slower** on CPU, and no Georgian or Cyrillic in the default models. Keep it as a possible future option for CJK, where Tesseract is weaker. CJK was not tested: `tesseract-data-chi_sim`/`jpn` are not installed.

## Known limitations and follow-ups
- **Code glyph confusions:** `i32` → `i132`, `->` → `—`, `%` and `==` mangled. Fix with code-aware post-correction (a token dictionary: `i32`, `u8`, `->`, `=>`, `==`, `!=`), or by recognising monospace regions (fixed advance) and running a second pass with a character whitelist.
- **Mixed-script words:** a Georgian suffix glued to a Latin word ("Omarchy-სთვის") is read as Latin look-alikes. Per-word script re-recognition (re-run low-confidence tokens with `kat` only) is the fix.
- Capture edges produce garbage lines. The confidence filter handles them; padding the crop with the edge colour (normcap's 80 px) helps further.
- Not yet tested: CJK, real-world screenshots with subpixel antialiasing (Chromium here used greyscale AA), QR detection (zxing-cpp; research measured 35–50 ms per full frame), auto language detection.
