# Performance research: processing pipelines for a CleanShot X clone on Omarchy (Arch + Hyprland)
Date: 2026-10-06. Four parallel research agents plus my own local checks.

**Evidence labels used throughout**
- **[measured]**: run on this machine. i5-13600KF (6P+8E, 20 threads), RTX 4080 (nvidia-open 610.57), 32 GB RAM, Hyprland 0.56.2, DP-2 3440×1440 at scale 1.25 (logical 2752×1152, XRGB8888, cm preset srgb). Single-threaded unless noted.
- **[published]**: taken from the cited source.
- **[estimate]**: my reasoning, not measured.

Most local benchmarks used synthetic, screenshot-like images and clips, which compress better and OCR more easily than real desktops. Trust the ratios between options more than the absolute numbers. Benchmark scripts are throwaway, in `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/` (`bench_tess.py`, `bench2.py`, `bench3.py`, `bench_qr.py`, `bench_enc.c`, `bench_cv.cpp`, `rbench/`, `bench/`, `mem/`). Nothing was installed system-wide.

---

## 0. Decisions in one page

| Area | Default | Optional / alternative | Expected cost |
|---|---|---|---|
| OCR | **Tesseract 5.5 LSTM, linked in-process**, `tessdata_fast` models. Run 1 thread per engine (`OMP_THREAD_LIMIT=1`) and parallelise by tiles yourself. 2× upscale at scale ≤1.25, invert dark regions, rebuild layout from word boxes. | "High accuracy": **PaddleOCR PP-OCRv5 mobile (or PP-OCRv6) ONNX on ONNX Runtime CPU**. Georgian is always routed to Tesseract `kat`. | 1000×600: 0.1–0.26 s [measured]. Full 3440×1440: 0.31 s with 6 tiles, 0.73 s on 1 thread [measured]. |
| QR/barcode | **zxing-cpp** (Arch `extra`, Rust crate `zxing-cpp`). Enable only 2D formats by default. | 1D formats opt-in | 35–50 ms per full 3440×1440 frame [measured] |
| Scrolling capture | Per-row signature cost curve, then full-res SAD ±2 px with parabolic subpixel fit, an exact run-hash fast path, sticky-band detection, and a tiled canvas | Study `snow-stitch-images` (Rust, Apache-2.0) | ~2–3 ms/frame at 1500×1200 on desktop [measured]; 5–10 ms on a 15 W laptop [estimate]. Capture readback dominates. |
| Hot-path PNG | **Rust `png` crate + fdeflate (`Compression::Fast`)**, encoded immediately on a worker thread | Background re-encode with libdeflate/zlib-ng L6 for disk; oxipng optional | 3440×1440: 6.5–8.6 ms; 4K: 11–16 ms [measured] |
| Clipboard | Offer **`image/png`** (pre-encoded bytes), plus `text/uri-list` once the file is saved | — | Never encode lazily inside the paste request (KDE bug 519651) |
| Redaction | **Solid fill** is the default "Redact". Pixelate/blur visuals are generated from pixels outside the region, or flagged "not secure". | OCR+regex auto-redaction that *suggests* boxes only | Box blur / stackBlur: <3 ms per 1000×600 region [measured] |
| GIF | ffmpeg `palettegen=stats_mode=diff` + `paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle`, 15 fps, ≤1280 px wide | gifski CLI as optional "HQ" (AGPL, so do not link unless the app is GPL/AGPL). **Offer MP4 first.** | 10 s 720p: ~0.8–1.4 s, 5–11 MB [measured]. Same clip as H.264 MP4: 0.3–0.9 MB. |
| Video editing | Stream-copy trim (MP4 edit lists). Smart-cut only when the recording has no B-frames, re-encoding the head with the same encoder and settings. **ffmpeg CLI for exports**; persistent in-process decoder for scrubbing. | GStreamer (gstreamer-rs) or libmpv for preview | Trim ~50 ms. Keyframe thumbnail strip 140 ms per 20 s of 1440p60 [measured]. |
| Beautify | GPU: SDF rounded rect + erf analytic shadow + gradient for preview. CPU export: 9-slice cached shadow. No gradient dithering by default. | tiny-skia CPU path | GPU <1 ms render + 2–4 ms readback [estimate]. CPU ~60 ms single-threaded at 4K [measured/derived]. |
| Memory | Raw frames only for "hot" items under a byte budget. **The Fast-PNG bytes become the canonical cold form.** Large buffers come from `mmap`/memfd, not malloc, and are marked `MADV_DONTDUMP`. Do not rely on swap. | lz4/zstd-1 for frozen scroll-canvas tiles | 4K raw = 31.6 MiB; as Fast PNG ≈ 2–3.5 MB on UI content |

**Licence decision needed early.** gifski is AGPL-3.0 and libimagequant is GPL-3.0. Arch's ffmpeg is built with `--enable-gpl --enable-version3`, so dynamically linking libav* puts a distributed binary in GPLv3 territory. For an MIT/Apache app: shell out to `ffmpeg`/`gifski` and link only permissive libraries (Tesseract and zxing-cpp are Apache-2.0, ONNX Runtime is MIT, Paddle models are Apache-2.0). For a GPL-3.0-or-later app, linking all of them is fine. (Engineering guidance, not legal advice.)

---

## Local environment facts and prior art on this machine

**Installed tools**
- tesseract 5.5.3 with only `eng` + `osd`.
- zbar 0.23.93, zxing-cpp 3.1.1, opencv 5.0.0, libvips 8.18.6, pixman 0.46.4, libjxl 0.12.0, libwebp 1.6.0, libavif 1.4.2, libdeflate 1.26, zlib-ng 2.3.3.
- ffmpeg 9.0.1, GStreamer 1.28.6, svt-av1 4.2.0, gpu-screen-recorder 6.1.0, libvpl, mpv 0.41.

**Available in Arch `extra` but not installed:** `tesseract-data-kat`/`-rus` (4.1.0 "tessdata" set), `gifski` 1.34.0, `gifsicle` 1.96, `oxipng` 10.2.1, `onnxruntime-cpu` 1.29.0, `onnxruntime-opt-cuda` 1.29.0, `openvino` 2026.3.1, `intel-media-driver`, `vpl-gpu-rt`, `gst-plugin-va`, `gst-plugin-gtk4`, `gst-editing-services` 1.28.6.

**AUR:** `python-rapidocr` 3.9.2, `ocrs` 0.13.1, `normcap` 0.6.0, `chalkak` 0.5.3, `python-surya-ocr` 0.22.1.

**What Omarchy already ships** (`/usr/share/omarchy/bin`). This is your baseline and your competition.
- `omarchy-capture-text`: `grim | tesseract --oem 1 --psm 6 --dpi 300 -c preserve_interword_spaces=1`, then `wl-copy`. It freezes the screen with `hyprpicker -r -z` before `slurp`.
- `omarchy-capture-qr`: `zbarimg -Sdisable -Sqrcode.enable`. Its comment says that leaving 1D symbologies on "lets dense screen content false-positive as an EAN or Code 39 barcode". It copies with `wl-copy --sensitive` so OTP secrets stay out of clipboard history. **Adopt both behaviours.**
- `omarchy-capture-screenshot`: grim, then save, then `wl-copy --type image/png`. It forces hardware cursors during capture so software cursors are not baked into the image.
- Default editor: **tensaku** 0.29.0 (Rust/GTK4/OpenGL, MPL-2.0, a Satty fork). It already has scrolling capture and an "irreversible Secure Blur / Black Out".
- **Clipboard history runs `wl-paste --type image/png --watch .../clipboard/capture.sh`.** Every image/png you offer is read *immediately* and written to `~/.local/state/omarchy/clipboard-images/<sha256>.png`. Entries carrying `x-kde-passwordManagerHint` are skipped. Consequences:
  - Lazy encoding saves nothing on Omarchy.
  - Unredacted captures land in history the moment you auto-copy (see §4).
- **Memory environment:** zram (zstd, size = RAM, priority 100) plus a disk swapfile. `systemd-oomd` is active and monitors `app.slice` with a 90% swap-used limit and 50% pressure for 20 s. THP is `always`. glibc 2.44.

**Direct competitors found**
- **OmniShot** (github.com/joshdaws/omnishot): Python, MIT, v0.2.0, explicitly CleanShot-inspired, targets Omarchy 4.0.2 / Hyprland 0.56.2. It has scrolling capture with auto-scroll, OCR/QR, blur/pixelate, and H.264/GIF recording with timeline editing.
- **ChalKak** (AUR): a Hyprland screenshot editor using PaddleOCR v5 via MNN.
- **snow-shot** (5.5k stars): Windows/macOS only. Its stitching crate is Apache-2.0.

---

## 1. OCR ("Capture Text") and QR

### 1.1 Recommendation
- **Default: Tesseract 5.5 LSTM, linked in-process** (Rust: `tesseract-plumbing`, `kreuzberg-tesseract` or `tesseract-rs`; `leptess` has had no release since 2023).
  - Ship or download your own **tessdata_fast** models for eng/rus/ukr/kat. Arch packages only the larger "tessdata" set (4.1.0, integerised best LSTM + legacy).
  - It is the only CPU-friendly engine that covers Latin + Cyrillic + **Georgian** + CJK, at 2.5–4 MB per language.
- **High accuracy (optional): PP-OCRv5 mobile** (det 4.7 MB; rec 7.5–16 MB) or **PP-OCRv6 tiny/small** (June 2026; det 1.9/9.6 MB; rec 4.4/20.4 MB). Run as ONNX on ONNX Runtime CPU (`onnxruntime-cpu` 1.29 in extra; Rust `oar-ocr` 0.10 or `paddle-ocr-rs` via `ort`).
  - **Paddle has no Georgian model.** Trap: `ka_*` Paddle models are **Kannada**; oar-ocr's docs mislabel them "Georgian".
  - **PP-OCRv6's dictionary has no Cyrillic either.** Use the PP-OCRv5 `cyrillic`/`eslav` recognisers for Russian.
  - Route each detected line to the recogniser for its script; Georgian lines go to Tesseract `kat` with psm 7.
- **No Linux equivalent of Apple Live Text exists.** Every Linux OCR tool surveyed uses Tesseract (normcap, Frog, TextSnatcher, gImageReader, dpscreenocr, omarchy-cmd-ocr), except ChalKak (PaddleOCR v5). PP-OCRv5/v6 is the closest CPU option.

### 1.2 Engine comparison

| Engine | Latin / Cyrillic / Georgian / CJK | Latency, 1000×600 / 3440×1440 | Size | Boxes | Arch | Notes |
|---|---|---|---|---|---|---|
| Tesseract 5.5 fast | ✔ / ✔ / ✔ / ✔ | 0.10–0.26 s / 0.31 s with 6 tiles, 0.73 s on 1 thread [measured] | eng 4.1 MB, kat 2.5 MB | block, para, line, word + confidence | extra | Apache-2.0 |
| Tesseract std / best | same | std ~1.4×, best ~2.4× slower [measured] | eng 23.5 / 15.4 MB | same | extra / AUR | **No accuracy gain over fast** in tests |
| PP-OCRv5 mobile | ✔ / ✔ / ✘ / ✔ | 0.1–0.25 s / 0.4–1.2 s [estimate]. Official: det 57.8 ms + rec 21.2 ms, Xeon 8 threads [published] | ~20 MB | line polygons | ORT in extra; models not packaged | Accuracy on Paddle's internal set: mobile 81.3%, server 86.4%, en 85.3%, cyrillic 80.3% [published] |
| PP-OCRv6 tiny/small | ✔ (46 Latin-script languages) / **✘** / ✘ / ✔ | whole image 0.20 / 0.59 s, Xeon 8350C with OpenVINO [published] | 6–30 MB | same | `python-rapidocr` defaults to v6 small | Apache-2.0 |
| ocrs 0.13 | **Latin only** | no numbers | 12 MB | word, line | AUR | "early preview" |
| EasyOCR 1.7 | ✔ / ✔ / ✘ / ✔ | needs PyTorch | large | line | AUR | Not recommended |
| Surya OCR 2 (0.69B VLM) | 91 languages | ~9 s/page on Apple Silicon via llama.cpp; 5 pages/s on an RTX 5090 [published] | ~1.4 GB | line + reading order | AUR | Weights under modified OpenRAIL-M, free only below $5M revenue |
| Small VLM OCR (Florence-2 0.23B, Granite-Docling 0.26B, GOT-OCR2 0.56B, PaddleOCR-VL 0.96B, MinerU2.5 1.16B (AGPL), Qwen3-VL-2B, DeepSeek-OCR-2 3.4B, olmOCR-2 8.3B, Chandra 2) | broad | token-by-token decoding: **seconds on CPU** for a dense screenshot [estimate] | 0.5–16 GB | varies | — | **GPU-only** for interactive use. Possible later opt-in "document mode" on the 4080, not viable on iGPUs. |
| docTR | — | not verified | — | — | — | Search budget ran out |

### 1.3 Measured Tesseract behaviour

Synthetic 13 px UI, code and Georgian text, so real subpixel screenshots will score worse.

- **fast models were quickest and at least as accurate as std/best.** Each extra language adds ~40–90% to run time.
- **OpenMP hurts.** Arch's build uses OpenMP; `OMP_THREAD_LIMIT=1` was faster almost everywhere (code at 2×: 880 → 412 ms). Parallelise yourself:
  - split the frame at blank horizontal bands into ~6 tiles, or
  - detect lines, then run psm 7 per line.
- **Upscaling:** a 9 px font scored 9.0% CER at 1× and 0.5% at 2×. ≥13 px fonts are ~0% at 1×. 3× and 4× give no further gain. The interpolation filter did not matter.
  - Rule: 2× at scale ≤1.25, 1.5× at 1.5, 1× at ≥2.
  - Better: choose the factor from the median connected-component height, targeting an x-height of ~10–12 px (capital height ~30 px per willus.com).
- **Binarisation:** feed grayscale and let Tesseract run its default Otsu. Pre-binarising was equal or worse, and Leptonica adaptive Otsu was much worse (7.0% vs 0.0%).
- **Dark mode:** Tesseract 5.5 retries a line inverted when confidence < `invert_threshold` (0.7). Inverting first still improved results (0.6% → 0.0%) and avoids the second pass. Decide per region, since panels mix light and dark.
- **Code indentation:** Tesseract drops all leading whitespace even with `preserve_interword_spaces=1`; whitespace-inclusive CER was 14.2%.
  - Fix: read TSV word boxes and set `indent = round((left − min_left) / char_width)`, with char width = median of word width ÷ chars. **CER fell to 2.9%.**
  - Remaining glyph errors: `->`→`—`, `}`→`¥`, `Ok(())`→`0k(Q)`, `i32`→`132`.
  - Disabling the dictionaries changed nothing.
- **Init cost:** about 60 ms per `TessBaseAPI`. Keep one per worker in the daemon. Lazy-load and unload after idle; an instance is about 50–150 MB [estimate].

### 1.4 Preprocessing pipeline
1. Luminance grayscale. This also kills subpixel colour fringes.
2. Per-region inversion when the background is dark (mean < ~125, or the Otsu majority class).
3. Upscale as in §1.3.
4. Pad ≥10 px with the dominant edge colour (normcap pads 80 px).
5. No pre-binarisation.
6. psm 6 for one block, psm 3 for mixed layout, psm 7 for detector line crops.
7. Optional CLAHE for low-contrast tokens such as grey comments on dark backgrounds [estimate].

**normcap's approach (worth copying):**
- Run zxing-cpp first and skip OCR if a code is found.
- 2× upscale, then pad 80 px with the most frequent edge colour.
- psm 3 with TSV output, then scored "transformers" that choose single-line, multi-line, paragraph, email or URL output.
- Strip spaces in CJK output (works around Tesseract #2702).

### 1.5 QR/barcode, full 3440×1440 frame [measured]

| Case | zxing-cpp default | zbar 0.23.93 |
|---|---|---|
| QR, 6 px modules | ✔ ~40 ms | ✔ ~66 ms |
| QR, 1 px modules | ✔ | ✘ |
| Inverted QR on dark background | ✔ | ✘ (✔ with `-Stest-inverted`, 128 ms) |
| Rotated 30° | ✔ | ✔ |
| 7 mixed codes (QR, Code128, DataMatrix, PDF417, Aztec, EAN-13) | **7/7, 47 ms** | 4/7 |
| No code | 0 false positives | 0 |

- zxing-cpp's `-fast -noscale` mode found only 3/7, so keep its default "try harder" settings.
- Published (zxing-bench, author-biased), BoofCV set: zxing-cpp 1124 codes in 5.1 s; rxing 992 in 34 s; zbar 446 in 42 s; rqrr and bardecoder crashed.
- `rxing` (pure Rust) is ~10× slower. `rqrr`/`quircs` handle QR only with low recall. OpenCV `wechat_qrcode` takes ~0.76 s/image.
- **Recommendation:**
  - zxing-cpp on a worker thread after every capture (cheap), restricted to QR, MicroQR, rMQR, DataMatrix, Aztec and PDF417; 1D codes opt-in, because of Omarchy's false-positive finding.
  - Copy decoded values with the `x-kde-passwordManagerHint` (sensitive) flag. OTP URIs are secrets.

### 1.6 High-accuracy pipeline details
- Run detection once at native resolution. Do **not** let RapidOCR's default `limit_type: min, 736` shrink screenshots.
- Rotate and crop lines, resize to height 48, sort by width, and batch.
- ORT settings:
  - `intra_op_num_threads` = number of P-cores;
  - **`allow_spinning=0`**, so an idle daemon does not spin and burn battery;
  - one warm-up inference at load;
  - bucket input widths so allocations are reused.
- Ship CPU-only:
  - Arch's `onnxruntime-cpu` has no OpenVINO execution provider;
  - OpenVINO 2026.3 (GPU/NPU plugins) is a possible separate backend for Intel later;
  - ORT's ROCm provider was removed in 1.23 in favour of MIGraphX;
  - the CUDA build (619 MiB) is not worth it for 5–20 MB models.
- Int8 quantisation is untested; benchmark it before shipping.
- `ort` 2.0.0-rc.13 targets ORT 1.28 while Arch has 1.29; check with `load-dynamic`.

### 1.7 Edge cases
- Programming-font ligatures (`->` drawn as one glyph).
- Tabs cannot be recovered; emit spaces.
- Emoji and Nerd Font icons come out as junk; drop isolated low-confidence symbol tokens.
- Mixed-script lines: cost grows linearly with the language count.
- Paddle may split lines at wide gaps or merge columns; regroup by y-overlap.
- Lines longer than ~25:1: chunk them before recognition.
- Rotation beyond 0/180° is mostly unsupported.
- Text over images is better handled by detector pipelines.
- **Build a real screenshot test corpus** (subpixel AA, coloured backgrounds, 1.25× scale, Georgian/Russian UI) before freezing defaults.

---

## 2. Scrolling capture stitching

### 2.1 Recommended algorithm
This is a 1-D sequence-alignment problem. Do not use a panorama stitcher.

```
per frame F (cursor excluded, columns x0..x1 exclude scrollbar/margins):
  h = rowhash64(F); settle: in auto mode wait until 2 identical frames or 300 ms after injection
  G = gray(F); S = rowsig(G, 32 horizontal bins); X = row texture
  static bands: rows equal at same position but not when shifted → T(top), B(bottom); running max, cap H/3
  coarse: d_anchor = vote over unique 16-row run-hashes (exact fast path)
          C[d] = mean L1(S_prev[y+d], S[y]) over textured rows outside bands, all d
          d1 = best local min, d2 = next min with |d−d1|>3
  ambiguous if C[d1] > τ(noise) or C[d1]/C[d2] > 0.7  → choose the minimum closest to prior; if not unique → HOLD (yellow)
  refine: E[d] = 60th-percentile(|ΔI| + |Δ∇|/2) over textured px, d ∈ d1±2; parabolic subpixel
  reject if E > k·noise (red: "scroll back"); warn if <75% of textured tiles agree (dynamic content)
  commit: y += d (float accumulator); seam at lowest-gradient row in overlap; write rows [max(T,seam), H−B)
          into a tiled canvas (256–1024 rows per tile); the tail still on screen stays mutable (lazy images)
  end: auto mode, d==0 with identical frames for 2–3 injections
finish: header(first frame rows 0..T) + canvas + footer(last frame rows H−B..H)
```
Thresholds (0.7, 1.15, 0.75) are starting estimates to tune.

**Per-frame cost at 1500×1200** [measured, synthetic page, 1 thread]:

| Stage | Time |
|---|---|
| gray | 0.28 ms |
| row hash | 0.34 ms |
| signatures | 0.10 ms |
| cost curve, both directions | ~1.0 ms |
| run-hash anchors | 0.15 ms |
| ±2 px refine | ~0.6 ms |
| append | ≤0.5 ms |
| **Total** | **≈2–3 ms** |

- Laptop at 15 W: 5–10 ms [estimate].
- xxh3 runs at 31–59 GB/s [published], so hashing is memory-trivial. Capture readback (shm copy) dominates.
- Use an adaptive capture rate of 1–30 fps, like snow-shot.

### 2.2 Methods compared

| Method | Verdict |
|---|---|
| Exact row-hash equality (ShareX) | Fast, but **fails under resampling**. At a 0.4 px shift with ±1 noise, 0/1063 rows matched exactly, while SAD+parabola estimated 137.32 vs a true 137.4 [measured]. Keep it only as a fast path, and vote on 16-row runs because single rows repeat. |
| 1-D projection profiles | Coarse candidate generator only (0.05–0.25 ms). Similar ink density aliases. |
| NCC (`matchTemplate`) | Robust. Skip flat strips (std < 4). Keep top-k peaks. A few ms per strip. |
| Phase correlation | Poor fit: static headers produce a zero-shift peak, and the circular-shift assumption fails. 5–33 ms. |
| SSIM | Use as a verification score only (~2–4 ms per candidate [estimate]). |
| ORB/AKAZE + RANSAC | Handles big jumps. 5–20 ms [estimate]. Fails on blank areas. wayscrollshot and snow-shot use feature methods. |
| OpenCV `Stitcher::SCANS` | Wrong tool: affine degrees of freedom, registration at 0.6 Mpx, multiband blending that blurs text, batch-only, no static-band handling. |

### 2.3 How others do it
- **ShareX** (C#, rewritten in v16):
  - ignores max(50, W/20) px on both sides;
  - matches with exact `SequenceEqual` row runs against the bottom of the **stitched result**;
  - ignores a bottom band of max(50, H/10), plus "auto ignore bottom edge" in v17;
  - stops when two captures are identical;
  - on no match, reuses the previous offset and shows yellow;
  - 300 ms scroll delay.
- **snow-shot / `snow-stitch-images`** (Rust, Apache-2.0). The best design found:
  - binary features with dy voting;
  - a score combining inlier share, residual gain and tile coverage;
  - a 60th-percentile robust refine;
  - per-tile fixed/scrolling/dynamic classification;
  - a 256-row tiled canvas;
  - adaptive cadence;
  - horizontal and prepend support.
- **Wayland tools:** wayscrollshot (ORB, grim per frame), scrollshot (row SAD + seam), wl-longshot, **OmniShot** (strip-anchor NCC consensus; "reject uncertain matches rather than lose rows").
- **Flameshot** closed the feature as not planned. Spectacle and ksnip have open requests only.
- **Commercial:**
  - CleanShot X has scrolling since 2.6, Auto-Scroll since 4.4 and horizontal since 4.8; the algorithm is undocumented.
  - Shottr caps output at 400,000 px.
  - Snagit advises scrolling with the scrollbar and excluding scrollbars.
- **Cooperative approaches** (Firefox `drawSnapshot` in tiles, max 32,766 px; Chrome CDP `captureBeyondViewport`; Android `ScrollCaptureCallback`, max 12,000 px; iOS PDF) re-render instead of stitching. They are not available to a Wayland screenshot tool.

### 2.4 Edge cases → handling

| Case | Handling |
|---|---|
| Sticky header/footer | Static-band detection. Overestimating a band is safe; underestimating duplicates content. |
| Floating buttons, banners | Per-tile fixed/dynamic classification. Last writer wins only on the mutable tail. |
| Scrollbar | Exclude ~24 physical px on the right (overlay scrollbars widen on hover), or detect columns that do not follow the shift. |
| Smooth-scroll settling | Firefox's wheel animation lasts 50–200 ms; wait for 2 identical frames. **Hyprland's ext-image-copy-capture currently sends full-buffer damage** (source TODO), so compare frames yourself and treat damage as a hint only. |
| Lazy-loaded images | Keep rows that are still on screen mutable. |
| Video, spinners, cursor | Capture without the cursor. Down-weight dynamic tiles; warn above 25% disagreement. Park the virtual pointer in the scrollbar gutter. |
| Overscroll / kinetic scrolling | Inject `zwlr_virtual_pointer_v1` axis events with **`axis_source=wheel` + discrete steps**, not finger source. |
| Scrolling too fast | Keep matching against the last committed tail. Show red "scroll back". |
| Repeated rows or blank overlap | Unsolvable from pixels alone. Use the motion prior (calibrated px per notch; keep steps ≤1/3 of the viewport), otherwise HOLD. |
| Fractional scale 1.25 | Native fractional-scale apps (Chromium) round offsets to device pixels, but the per-notch offset varies (71/72 px). XWayland and legacy clients are rendered at 2× and downsampled ×0.625, so every frame gets new resampling noise. Hence tolerant metrics and a float accumulator. |
| Horizontal | Same pipeline, transposed. |
| Very tall output | See §8. Tiled canvas. GPU `maxImageDimension2D` is 16384 on Intel/AMD and 32768 on NVIDIA (gpuinfo), so preview must be tiled. Export limits: WebP 16,383 px; JPEG 65,500; libpng's default reader limit 1,000,000; Rust `image` crate default `max_alloc` 512 MiB (a 3440×40,000 image fails to decode). Default cap plus a warning; split or refuse WebP. |

**UX:**
- Live green/yellow/red status.
- Seam preview with ±1 px nudge.
- Undo last segment.
- Header/footer crop handles.
- Length-cap warning.
- Capture the **toplevel source** (ext-image-copy-capture) so our own UI is never captured.

---

## 3. Encoding, decoding, clipboard, colour, HiDPI

### 3.1 Lossless encode at 3440×1440 [measured, 1 thread]
"ss" = mixed screenshot with a photo region; "ui" = pure UI.

| Encoder | ss ms / KiB | ui ms / KiB |
|---|---|---|
| **Rust `png` 0.18 `Fastest`** (fdeflate + Up) | **6.1 / 1948** | **4.0 / 1218** |
| **Rust `png` `Fast`** (fdeflate + adaptive) | **8.6 / 1685** | **6.5 / 1054** |
| Rust `png` `Balanced` (default) | 165 / 1314 | 65 / 621 |
| libpng + zlib L6 (libpng default) | 276 / 1365 | 110 / 690 |
| libpng L9 | 918 / 1349 | 460 / 678 |
| zlib-ng L1 + Up | 18.6 / 2103 | 11.2 / 906 |
| libdeflate L1 + Up | 24.3 / 1490 | 18.9 / 679 |
| libdeflate L6 + Paeth | 76 / 1329 | 50 / 665 |
| QOI | 7.1 / 1508 | 5.0 / 755 |
| **JPEG XL lossless e1** (1 thread / 20 threads) | 12.1 / 7.3 ms, 1203 KiB | 9.9 / 4.4 ms, 667 KiB |
| JXL e7 (default; 20 threads) | 257 / 673 | 225 / 374 |
| WebP lossless z0 / z6 | 45 / 1131; 906 / 640 | 38 / 627; 101 / 162 |
| AVIF lossless (aom speed 6, all threads) | ~750 / 1193 | ~537 / 731 (poor) |
| libjpeg-turbo q90 4:4:4 / 4:2:0 | 11 ms / 813 KiB; 7 ms / 674 KiB | — |

**At 4K:** `png` Fast 11–16 ms (1.8–3.1 MB); `Balanced` 130–369 ms; JXL e1 8–23 ms.

**Published:**
- fpnge (AVX2): 180–800 MP/s per core.
- fpng: 110 MP/s.
- QOIR "screenshot_web" set: fpnge ratio 0.101 at 360 MP/s vs libpng 0.076 at 13.9 MP/s.
- mtpng: dual-4K in 301 ms on 1 thread, 25 ms on 32 threads.
- mozjpeg's default mode is 44–59× slower than libjpeg-turbo for ~20% smaller files.

**Takeaways**
- fdeflate PNG runs at ~1 GP/s, about 10× libpng's fastest level, and outputs standard PNG at 1.3–1.9× the size of L6.
- fdeflate **needs a filter**: NoFilter produced 16 MB.
- JXL e1 is about as fast and ~30–40% smaller, but cannot go on the clipboard.
- Use JPEG 4:4:4 for screenshots; 4:2:0 smears coloured text.
- Lossless AVIF is not worth offering.
- `zune-png` has no fast encoder.
- oxipng is a slow background "Optimize" pass (`-o4` ≈ 6× `-o2` for 5–10%).

### 3.2 Pipeline
1. Capture into a wl_shm memfd buffer, which becomes the raw frame.
2. A worker encodes `png::Compression::Fast` immediately (6–16 ms). These bytes serve the clipboard, drag-and-drop and the first save.
3. Save via temp file + `fsync` + `rename(2)`.
4. A background re-encode (libdeflate L6–9 + Paeth, or `Balanced`) atomically replaces the file.
5. Optional oxipng.
6. Exports: JXL (e1 fast / e7 small), WebP z6, JPEG q90 4:4:4.

### 3.3 Clipboard
- **Chromium/Electron on Linux (Chrome, Slack, Discord, VS Code) reads only `image/png`**, plus `text/uri-list` for files. Firefox accepts png, jpeg and gif. The web Async Clipboard API mandates only `image/png`. **Offer `image/png`, plus `text/uri-list` after save.**
- **Never encode inside the paste request.** KDE bug 519651 (fixed Aug 2026): Klipper gives up if the first byte does not arrive within 1 s, and PNG re-encoding of large screenshots took more than 1 s. Serve pre-encoded bytes from a non-blocking writer thread.
- `wl-copy` offers only one MIME type per copy, so the daemon should own its own data source.
  - Right after a capture, the overlay interaction provides an input serial for `wl_data_device.set_selection`.
  - Without one (e.g. copying from a notification action), use `zwlr_data_control_manager_v1` (what wl-clipboard uses). Hyprland's support for `ext-data-control-v1` is unverified.
- The daemon must keep the bytes for as long as it owns the selection.
- Size caveat: cliphist (common on non-Omarchy setups) has a default `max-store-size` of 5 MB, so large 4K/5K PNGs may be silently skipped. Omarchy's own history script has no such cap.

### 3.4 Colour depth and colour management
- Hyprland warns that "some applications do not support screen capture with 10 bit enabled". Handle `XRGB2101010`/`XBGR2101010` shm formats:
  - 8-bit: `v8 = (v10*255 + 511)/1023`, with optional ordered dither for gradients;
  - optional 16-bit PNG: `v16 = v10<<6 | v10>>4`.
- Tag sRGB output with the `sRGB` chunk; embed `iCCP` for wide gamut.
- For HDR (Hyprland cm `hdr`/`wide`, since 0.47), PNG 3rd Edition (W3C Rec, June 2025) adds `cICP`/`mDCV`/`cLLI`; precedence is cICP > iCCP > sRGB.
- **Put a tone-mapped SDR PNG on the clipboard.**
- What screencopy returns in `wide`/`hdr` modes must be tested on real hardware (unverified).

### 3.5 HiDPI
- Default to **physical pixels** (CleanShot's Retina default), with an option to downscale to 1×.
- Tag `pHYs` = **90 DPI (3543 px/m) for 1.25×** and 144 DPI for 2×. Gitea/Forgejo compute `dppx = DPI/72`, and macOS writes 144 DPI.
- Optionally add an `eXIf` chunk with logical dimensions, as macOS does. Browser support for this in PNG is unverified.
- The 0.8× downscale is non-integer (3440×1440 → 2752×1152) [measured]:

| Resizer | Time |
|---|---|
| OpenCV INTER_AREA | 22.7 ms on 1 thread, 4.0 ms on 8 |
| OpenCV LANCZOS4 | 57 / 11 ms |
| **pixman separable lanczos3** | **574 ms**; avoid, and note cairo GOOD/BEST downscales hit this path |
| swscale lanczos | ~30 ms |

- Published, fast_image_resize (AVX2) Lanczos3 RGBA 4928×3279 → 852×567: 24.5 ms, vs the `resize` crate 217 ms.
- **For Rust, use fast_image_resize** (rayon-capable).
- Thumbnails: area/box reduce, 1–4 ms.

---

## 4. Blur, pixelate, redaction

### 4.1 Security findings
- **Hill et al., PoPETs 2016:** HMM attacks recovered text with **94–96% accuracy when the mosaic block equals the font size**. Gaussian blur stayed attackable up to a 45 px radius on 24 px text. Their conclusion: mosaicing and blurring "are not viable approaches for text redaction".
- **McPherson et al. 2016:** neural nets beat mosaic and blur for faces, objects and digits.
- **Depix** reverses linear box-filter pixelation of known fonts. **Bishop Fox's Unredacter (2022)** brute-forces block offsets and per-character search; its advice is "use black bars… No pixelization, no blurring".
- **aCropalypse** (CVE-2023-21036 Pixel Markup; CVE-2023-28303 Windows Snipping Tool): files were overwritten without truncation, so the original data remained recoverable.
- **Flameshot 13** made pixelate sample only pixels *outside* the selection. **Spectacle's blur** was flagged insecure (bug 474444).
- Newer (2024–25) ML depixelation papers were not verified; the 2016 results already justify the defaults below.

### 4.2 Safe defaults
1. **"Redact" = opaque solid fill, flattened into the pixels.** Make it the default redaction tool.
2. "Pixelate" and "Blur" as *secure styles* render a look generated from **non-content sources**: pixels outside the region, or seeded random tiles.
   - Content-derived pixelation or blur may exist only as an "aesthetic, not secure" style.
   - Adding noise to a content mosaic is weak, because noise averages out across repeated publications.
3. **Exports contain flattened pixels only.** No layers, history or original in private chunks. Project files that keep originals must say so.
4. **Write with temp + fsync + rename**, never an in-place overwrite without `O_TRUNC`.
5. **Clipboard-history leak:** auto-copy at capture puts the *unredacted* image into Omarchy's history (`~/.local/state/omarchy/clipboard-images`) instantly.
   - After redaction, replace the selection.
   - Consider delaying auto-copy while the editor is open, or marking captures sensitive (`x-kde-passwordManagerHint`, which Omarchy skips).
6. Mark pixel buffers `MADV_DONTDUMP` so a daemon crash does not write every held screenshot into systemd-coredump storage (see §8).

### 4.3 Fast implementations [measured]

| Operation | 1000×600 | 3440×1440 | 4K |
|---|---|---|---|
| OpenCV stackBlur (1T / 8T) | 2.96 / 0.77 ms | 25.9 / 5.5 ms | 43 ms (1T) |
| OpenCV 3× box blur (≈ Gaussian; Wells 1986 / Kutskir box widths) | 2.05 ms | 17.3 ms | 28.9 ms |
| OpenCV GaussianBlur σ=10 / σ=30 (1T) | 17.4 / 94.8 ms | — | — |
| OpenCV GaussianBlur σ=20 (1T) | — | 234 ms | — |
| Pixelate, 16 px block | 0.96 ms | — | — |

- GPU dual-Kawase (Hyprland's `blur1.glsl` pattern) at 4K: <0.5 ms on the 4080, 1–3 ms on an iGPU, plus 1–3 ms transfer [estimate].
- CPU is fast enough for edits; use the GPU for live preview.

### 4.4 Auto-redaction (OCR + regex)
Feasible as **suggestions only, never auto-applied.**

**Detectors**
- Email.
- Card numbers: 13–19 digits plus Luhn check.
- IBAN: mod-97 check.
- gitleaks rules:
  - AWS `(AKIA|ASIA|ABIA|ACCA|A3T…)[A-Z2-7]{16}`
  - GitHub `ghp_…{36}`, `github_pat_…`
  - Slack `xox[bpe]-`
  - JWT `ey….ey….…`
  - PEM private-key headers
  - Stripe `(sk|rk)_(live|test)_`
  - OpenAI `sk-(proj|svcacct|admin)-…`
  - Anthropic `sk-ant-api03-`
  - GCP `AIza…`
- Entropy thresholds: gitleaks 3.5 for generic keys; detect-secrets base64 4.5 and hex 3.0.
- Reference pipeline: Microsoft Presidio image redactor.

**OCR failure modes and mitigations**
- Confusables O/0, l/I/1, S/5, B/8: normalise before prefix matching.
- For checksum IDs, try substitutions until the checksum passes.
- Join tokens wrapped across lines.
- High-entropy strings stay high-entropy under OCR noise, which helps recall.
- False positives: git SHAs, UUIDs, hex colours, base64 blobs.
- Pad boxes by ~20% of line height.

**Cost:** full-screen Tesseract 0.3–0.7 s [measured, §1]. Run off the hot path.

---

## 5. GIF creation

### 5.1 Measured: 10 s 1280×720@30 synthetic screen clip
"Clip B" is a lanczos-downscaled 3440×1440 recording.

| Variant | Time | Size | Notes |
|---|---|---|---|
| Naive `ffmpeg -i x.mp4 x.gif` | 2.9 s | **110 MB** | SSIM 0.41. Never do this. |
| Palette, sierra2_4a (ffmpeg default) | 4.6 s | 11.1 MB | |
| **diff + rectangle, bayer scale 5** | 1.35 s | 11.4 MB (clip B 9.5 MB) | Temporally stable |
| diff + rectangle, bayer 3, **15 fps** | 0.79 s | 6.7 MB | 30 → 15 fps ≈ −44% |
| Per-frame palette `new=1` | 6.1 s | **22.9–30.6 MB** | Breaks transdiff; avoid |
| ImageMagick `-layers Optimize` | 22.8 s | 5.7 MB | 20–30× slower |
| Animated WebP lossy q75 | 3.7 s | 5.0 MB | Always 4:2:0, which blurs text |
| APNG | 7.6 s | 26.9 MB | |
| **x264 veryfast crf23 MP4** | **0.39 s** | **0.31 MB** (clip B 0.93 MB) | 10–35× smaller than GIF |
| SVT-AV1 p8 crf35 (`scm=1`) | 1.2 s | 0.24 MB | |

**Published:**
- pkh.me (2015): `stats_mode=diff` + `diff_mode=rectangle` for static backgrounds.
- bit-101: gifski 19.3 s / 16 MB vs ffmpeg 5.8 s / 27 MB on 300 frames.
- Selig 2025: `gifsicle -O3 --lossy=65` cut a further 48%.
- gifski README: `--fast` is ~50% faster for ~10% worse quality.
- gifski and gifsicle were not measured locally. Estimate: gifski is ~3–4× slower than ffmpeg at ~40% smaller size.

### 5.2 Defaults
- Offer **"Video (MP4)" first** and GIF second.
- GIF filtergraph:
  `fps=15,scale='min(1280,iw)':-2:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle`
  - Use `dither=none` for pure-UI clips.
  - Over the size target (GitHub 10 MB, X 15 MB, Discord 20 MB): step to 960 px, then 10 fps, then `gifsicle -O3 --lossy=40–80` if installed.
  - "HQ (gifski)" when the CLI is present (`extra/gifski`).
- **Licensing:** the gifski crate is AGPL-3.0 and libimagequant is GPL-3.0. Permissive alternatives: `quantette` (MIT/Apache, Wu/k-means in Oklab) + the `gif` crate; you then implement frame diffing yourself.

### 5.3 Edge cases
- Delays are in centiseconds: 30 fps becomes 3/3/4 cs (ffmpeg 9 alternates these with no drift [measured]). Exact rates are 50/25/20/10 fps.
- **Browsers clamp delays of 0–1 cs to 10 cs** (Chromium `kShortFrameDurationThreshold`), so 50 fps is the maximum.
- ImageMagick's integer `-delay` drifts.
- Looping uses NETSCAPE2.0 (`-loop 0`).
- X/Twitter: GIF ≤15 MB, ≤1280×1080, **≤350 frames and ≤300 Mpx total**.
- GitHub accepts .mp4 up to 10 MB (100 MB on paid plans) and **not .webp**.
- Discord animates WebP/AVIF; its free upload limit has been 20 MB since 2026-08-13.
- gpu-screen-recorder writes **VFR** by default, so always put `fps=N` first in the filtergraph.
- MP4 export: `yuv420p`, even dimensions, `+faststart`.

---

## 6. Video post-processing (trimmer/editor)

### 6.1 gpu-screen-recorder 6.1 output (from source)
- `-keyint` default **2 s**.
- **No B-frames** by default; 2 with `-tune quality` on NVIDIA.
- NVENC `tune=ll`, `forced-idr=1`.
- **VFR** with a 1 µs timebase.
- Fragmented "hybrid" MP4 with edit lists.

### 6.2 Trimming [measured]
- `ffmpeg -ss 5.5 -i rec.mp4 -t 5 -c copy` took 46 ms. MP4 gets an `elst` box, and ffprobe decodes **exactly 300 frames from 5.5 s**. MKV, or `-avoid_negative_ts make_zero`, starts at the previous keyframe (+1.5 s lead-in).
  - **Use MP4 edit lists for exact-start copy trims.** Players that ignore edit lists show the lead-in.
- **Smart cut:**
  1. re-encode head 5.5→6.0 s with **h264_nvenc and the recording's settings**;
  2. stream-copy the rest;
  3. join with the concat demuxer.
  - Result: byte-identical avcC (SPS/PPS), 0 decode errors, untouched part bit-exact. An x264 head produced different extradata.
  - Pitfalls: browsers may stop on parameter-set changes under `avc1` (GStreamer's smartencoder uses `avc3`/`hev1`); HEVC with mismatched parameters produces garbage (trac #6494); B-frames and open GOP need reorder-aware cuts; LosslessCut's smart cut is still experimental.
  - **Rule:** smart cut only for H.264/HEVC without B-frames, using the same encoder family. Compare extradata after encoding and fall back to a full re-encode on mismatch.
- GOP trade-off: 0.5 s GOP = +30% size and saves only ~30 ms median seek. **Keep 1–2 s.**

### 6.3 Re-encode speed, fps [measured, from raw]

| Encoder | 1080p | 3440×1440 |
|---|---|---|
| h264_nvenc p1 / p4 / p7 | 499 / 365 / 229 | 274 / 182 / 110 |
| hevc_nvenc p4 / av1_nvenc p4 | 340 / 361 | 180 / 187 |
| h264_vulkan | 427 | 272 |
| x264 ultrafast / veryfast / medium | 1034 / 668 / 427 | 499 / 337 / 199 |
| SVT-AV1 p8 / p10 / p12 | 253 / 432 / 474 | 146 / 278 / 294 |

- Full-GPU NVDEC → (`scale_cuda`) → NVENC: 199–328 fps. These runs are decode-bound; PCIe copies barely matter.
- **h264_vaapi on NVIDIA fails** (nvidia-vaapi-driver is decode-only); use NVENC or Vulkan.
- NVIDIA's published Ada 1080p H.264 P1/P7: 885/211 fps.

**Quality, 1080p60 screen content:**
- NVENC `cq23` is ~3× the size of x264 `crf23`; map NVENC CQ ≈ x264 CRF + 8–10.
- At about 1 MB, Ada NVENC matched or beat x264 veryfast (VMAF 96.2 vs 95.4).
- `-tune stillimage` did not help.
- SVT-AV1 `scm=1` cut size 24% on flat UI.

**Share defaults:**
- CPU: x264 `veryfast -crf 23`, High profile, yuv420p.
- NVIDIA: `h264_nvenc -preset p4 -tune hq -rc vbr -cq 30–33`.

**iGPU (not measured):**
- Intel: `intel-media-driver` VA-API; QSV needs `libvpl` + `vpl-gpu-rt`.
- AMD: radeonsi VA-API or RADV Vulkan Video.
- **AMD VCN4 pads AV1/HEVC (1080 → 1082/1088)**; always verify output dimensions.

**Operations:**
- Mute: `-an -c:v copy`, 52 ms.
- Speed: `setpts` + `atempo` (0.5–100 per instance); always a re-encode; 2.9 s for 20 s of 1440p60.
- Resize/crop: re-encode, ideally with GPU scalers.

### 6.4 Libraries

| Option | Verdict |
|---|---|
| **ffmpeg CLI + `-progress pipe:1`** | **Default for exports.** Robust, isolates crashes and the GPL. ~100 ms startup, plus 150–250 ms with CUDA. |
| ffmpeg-next / ffmpeg-sys-next 9.0.0 | Maintenance-only, but CI covers FFmpeg 6.1–9.0. Fine for an in-process decoder. |
| rsmpeg 0.18 (FFmpeg 8.0) | No FFmpeg 9 release seen. |
| gstreamer-rs 0.25.4 + GStreamer 1.28 | First-class, LGPL. `gtk4paintablesink` (`gst-plugin-gtk4`) does DMABuf with GTK ≥4.14. GES 1.28 is used by Pitivi. `va` plugin; nvcodec `nvh264enc`/`nvautogpuh264enc`. |
| libmpv (`libmpv2` crate) | Good for preview: `hr-seek` is exact. Render API is GL/software only. `frame-back-step` is slow and **unreliable on VFR**. |

### 6.5 Scrubbing [measured, 20 s 3440×1440@60]

| Operation | Result |
|---|---|
| CLI accurate seek, CPU, 2 s GOP | median 190 ms |
| CLI accurate seek, NVDEC, 2 s GOP | 462 ms, dominated by ~250–310 ms process + CUDA init |
| CLI accurate seek, 10 s GOP | up to 1.5 s |
| **All-intra 860 px proxy** (MJPEG / x264 `-g 1`) | **26–38 ms per seek**; built in ~2.8 s; 24–37 MB |
| Decode, H.264 | CPU **1170 fps** vs NVDEC 431 |
| Decode, HEVC | CPU 727 vs NVDEC 819 |
| Decode, AV1 | dav1d 620 vs NVDEC 758 |
| Thumbnail strip, `-discard nokey` | **140 ms** |
| Thumbnail strip, full decode + `fps=1` | 1.03 s |
| Thumbnail strip, 20 separate `-ss` processes | 3.6 s |

- Prefer CPU decode for H.264 on desktops. Laptop results may invert, and the Arch wiki notes nvidia-vaapi can draw more power.

**Editor design:**
- Persistent in-process decoder.
- Instant keyframe thumbnail strip, refined in the background.
- Low-res all-intra proxy, or a decoded-frame cache around the playhead.
- Use PTS, not frame indices (VFR).

---

## 7. Beautify rendering (gradient, padding, rounded corners, shadow)

### 7.1 Approaches
- **GPU (wgpu/GL):**
  - SDF rounded-rect coverage `clamp(0.5 − d, 0, 1)`;
  - **Evan Wallace's erf-based analytic rounded-rect shadow** (closed form in x, 4 samples in y; Raph Levien's 2020 variant is "nearly perfect");
  - gradient in the same pass.
  - Cost: <1 ms render plus ~2–4 ms upload/readback on the 4080, ~3–8 ms on an iGPU [estimate].
  - Keep the device warm in the daemon; device creation costs 50–200 ms [estimate].
- **CPU export:** precompute a **9-slice shadow**. Rasterise and blur one corner mask once (336×336: **1.15 ms** [measured]), then stretch the edges.
  - The analytic erf shadow on CPU costs 300 ms on 1 thread and 53 ms on 8 threads at 4K [measured]; it is a GPU technique.
- **Libraries:** tiny-skia (Rust) has **no blur/filters** and runs 20–100% slower than Skia on x86. Cairo has no blur, and its GOOD/BEST downscale is the slow pixman path (§3.5).

### 7.2 Measured composite, tiny-skia, 1 thread
4K canvas, 3440×1440 screenshot:

| Step | Time |
|---|---|
| Gradient | 29 ms |
| Shadow: rasterise + full-size 3× box blur + tint | 83 ms |
| Shadow composite | 19 ms |
| Rounded-clip draw, bicubic 0.85× | 68 ms (9 ms at 1:1 nearest) |
| **Total** | **208 ms** |

- With a 9-slice shadow and a 1:1 draw: **~60 ms**.
- Tiled multithreading: ~10–20 ms [estimate].

### 7.3 Quality
- **Blend in sRGB-encoded space** to match CSS and design tools (CleanShot-like look). Linear-light blending gives thinner dark-on-light edges and lighter shadow falloff. Use the same choice in preview and export.
- **Gradient dithering inflates PNGs** [measured]:
  - gradient-only 4K: none 144 KiB, Bayer 918 KiB, TPDF noise 6081 KiB;
  - realistic composite: Bayer +30%, TPDF +164%, and encode time 253 → 1210 ms.
  - Default: no dither. Use ordered dither only for very shallow gradients (step < 1 LSB per ~30 px).
- **Transparent export:** PNG alpha is straight, while tiny-skia stores premultiplied values.
  - Write the shadow as RGB 0 with A = shadow, which is exact.
  - Un-premultiply other pixels with rounding: `(c*255 + a/2)/a`.
- **macOS-like shadow:** no documented CleanShot or macOS values were found. Approximately `0 20px 60–70px rgba(0,0,0,0.4–0.55)` [estimate]; make it configurable.

---

## 8. Memory budget (daemon)

### 8.1 Sizes (4 B/px for XRGB8888 and 2101010; 8 B/px for FP16)

| Item | Raw | Fast-PNG (UI content) | Notes |
|---|---|---|---|
| 1920×1080 | 7.9 MiB | ~0.4–1 MB | |
| 3440×1440 | 18.9 MiB | 1.0–1.7 MB [measured, synthetic] | |
| 3840×2160 | 31.6 MiB | 1.8–3.1 MB [measured]; photo-heavy up to ~8–10 MB [measured, mixed image] | |
| 5120×2880 | 56.3 MiB | ~3–6 MB [estimate] | |
| **3 × 4K "all screens"** (freeze overlay) | **94.9 MiB** | — | Transient: the frozen overlay needs these frames, plus the same again in GPU textures |
| 4K FP16 HDR | 63.3 MiB | — | If HDR capture is ever supported |
| Scroll 1500 × 20,000 | 114 MiB | ~6–12 MB [estimate] | |
| Scroll 3440 × 20,000 | 262 MiB | ~15–30 MB [estimate] | Exceeds GPU max texture height (16384 Intel/AMD) |
| History thumbnail, 512 px wide | ~0.4 MiB raw | ~20–60 KB WebP/PNG [estimate] | |

**In-memory compressors, measured on raw RGBA** (zstd 1.5.7 and lz4 1.10, single thread, in-memory benchmark mode):

| Data | zstd -1 | lz4 |
|---|---|---|
| Synthetic UI 4K | 52× at 4.3 GB/s (≈8 ms per frame) | 34× at 6.3 GB/s |
| Synthetic UI + 35% photo, 4K | 3.2× at 1.7 GB/s | 2.7× at 3.3 GB/s |

Three real grim region screenshots, compression ratios:

| Format | Ratio |
|---|---|
| grim PNG | 11.5–25× |
| zstd -1 on raw | 8.0–21.7× |
| lz4 on raw | 5.2–11.6× |

So zstd-1 on raw is within ~15–40% of PNG size at multi-GB/s. Published comparison: QOI's "screenshot_web" set compresses to 8.3% vs libpng's 7.6%, at 310 vs 14.8 MP/s.

### 8.2 Strategy

1. **Zero-copy capture.** Capture into memfd-backed wl_shm buffers and keep that memfd as the raw frame.
   - If the editor is a separate process, pass the fd over `SCM_RIGHTS` with `F_SEAL_WRITE|F_SEAL_GROW|F_SEAL_SHRINK` (`MFD_ALLOW_SEALING`) for safe zero-copy sharing.
   - memfd pages count as shmem and are swappable.
2. **Hot/cold tiers with a byte budget** (e.g. a 256–512 MiB default, configurable):
   - **Hot (raw):** captures open in the editor/overlay, plus the last 1–2.
   - **Warm:** Fast-PNG bytes, which already exist for the clipboard. Decode back costs ~6.6 ms at 3440×1440 [measured].
   - **Cold:** a file path only.
   - Evict LRU by bytes.
   - Example: 10 "all screens" 3×4K captures would be 949 MiB raw. Under this policy: 2 hot (190 MiB) + 8 warm (~25–80 MB), or disk only.
   - No separate QOI or zstd tier is needed for single screenshots, because Fast-PNG matches their speed.
3. **Scroll capture:**
   - Keep only the previous frame, the current frame and the 1 B/px gray anchor plus signatures (~7 MB + 2 MB at 1500×1200) alongside the canvas.
   - Canvas in 256–1024-row tiles. Rows still on screen stay mutable.
   - **Compress frozen tiles with lz4 or zstd-1** (<1 ms per 3 MB tile [measured throughput]).
   - At export, stream rows into the PNG encoder (png crate `StreamWriter` / libpng row API), decompressing one tile at a time. Peak memory ≈ one tile + encoder state.
   - Export time for 1500×20,000: ~30 ms with fdeflate, ~0.5 s with libdeflate L6 [estimate from measured MP/s].
   - Default height cap with a warning. Shottr allows 400k px at 4 B/px (3072×80,000 ≈ 1 GB).
4. **Editor display of tall images:**
   - Tile textures, because GPU max 2D dimension is 16384 on Intel/AMD and 32768 on NVIDIA. GTK4's GSK slices oversized textures automatically (much improved in 4.12), but avoid uploading 262 MB.
   - Use a downscaled overview (mip) plus full-resolution visible tiles. On iGPUs, VRAM is system RAM.
5. **Allocator trap (glibc).**
   - `M_MMAP_THRESHOLD` starts at 128 KiB but rises dynamically to the size of any freed mmapped block, up to `DEFAULT_MMAP_THRESHOLD_MAX = 4*1024*1024*sizeof(long)` = **32 MiB on 64-bit**.
   - 4K frames (31.6 MiB) and all smaller frames sit under that cap. After the first free, later frame buffers come from malloc arenas and may never return to the OS, so RSS creeps (the documented GNOME/Nautilus image-buffer problem).
   - Fix options:
     - allocate pixel buffers with `mmap`/memfd directly (e.g. `memmap2` anon maps), so `munmap` frees deterministically;
     - or call `mallopt(M_MMAP_THRESHOLD, 1 MiB)` at startup (setting any of these parameters disables the dynamic adjustment);
     - or use mimalloc/jemalloc with decay settings.
   - Rust's default allocator on Linux is glibc malloc.
6. **`MADV_DONTDUMP`** (Linux 3.4+) on pixel buffers keeps core dumps small and **stops screenshots (possibly secrets) from landing in systemd-coredump storage** after a crash.
7. **Do not use swap as a store.**
   - `MADV_COLD`/`MADV_PAGEOUT` (5.4+) would push frames into Omarchy's zram (zstd), but **systemd-oomd is active and, for swap-based kills, picks cgroups "with the highest swap usage" first**. It monitors `app.slice` at a 90% swap limit and 50% pressure for 20 s. A daemon that parks frames in swap becomes the top kill candidate.
   - Compress explicitly and drop raw frames instead.
   - Run as a systemd user service with `MemoryHigh=` as a soft cap. `ManagedOOMPreference=avoid` exists, but whether it is honoured for user-manager units depends on the systemd version; verify on systemd 261.
8. **Models:**
   - Tesseract ~50–150 MB per instance and ORT sessions ~100–200 MB [estimate].
   - Lazy-load on first use and unload after idle (e.g. 60 s), so an idle daemon stays well under ~50 MB RSS [target, not measured].
9. **Thumbnails:**
   - Generate at capture (area/box reduce, 1–4 ms) and store on disk.
   - Optionally also write them into the freedesktop thumbnail cache (`~/.cache/thumbnails/{normal 128, large 256, x-large 512, xx-large 1024}`, PNG with `Thumb::URI`/`Thumb::MTime`) so file managers reuse them.
   - Keep in RAM only the thumbnails currently visible.
10. **Clipboard:** as the selection owner, the daemon must keep the PNG bytes alive until the selection is replaced. Omarchy's history copies them to disk immediately anyway.
11. **Decode limits:** reading back very tall PNGs with the Rust `image` crate hits the default `max_alloc` of 512 MiB, and libpng readers default to 1,000,000 px per dimension. Set explicit limits.

---

## 9. Open items to verify before committing
- A real screenshot corpus for OCR and stitching (subpixel AA, 1.25× and XWayland clients, Georgian/Russian UIs, dark themes).
- Real numbers on Intel Xe and AMD 780M class iGPUs: VA-API/QSV encode, iGPU shader paths, laptop-CPU OCR and stitching.
- gifski and gifsicle measured on screen content.
- PP-OCRv5/v6 through ORT on this CPU, including int8.
- Screencopy pixel format and transfer function under Hyprland cm `wide`/`hdr` and 10-bit.
- `ext-data-control-v1` support in Hyprland.
- Whether `ManagedOOMPreference` works for user units on systemd 261.
- Whether browsers honour PNG `eXIf` density.
- docTR numbers.
- Slack animated-GIF limits.

## Sources (grouped)

**Local files read:**
- `/usr/share/omarchy/bin/omarchy-capture-text`, `omarchy-capture-qr`, `omarchy-capture-screenshot`, `omarchy-capture-region`
- `/usr/share/omarchy/shell/plugins/clipboard/capture.sh`
- `/usr/share/omarchy/default/systemd/zram-generator.conf.d/90-omarchy.conf`
- `oomctl` output; `hyprctl monitors`

**OCR/QR:**
- Tesseract: https://tesseract-ocr.github.io/tessdoc/ImproveQuality.html · https://tesseract-ocr.github.io/tessdoc/Data-Files.html · https://github.com/tesseract-ocr/tessdata_fast · https://willus.com/blog.shtml?tesseract_accuracy
- normcap: https://github.com/dynobo/normcap
- PaddleOCR: https://www.paddleocr.ai/main/en/version3.x/algorithm/PP-OCRv5/PP-OCRv5.html · https://www.paddleocr.ai/main/en/version3.x/algorithm/PP-OCRv5/PP-OCRv5_multi_languages.html · https://www.paddleocr.ai/main/version3.x/algorithm/PP-OCRv6/PP-OCRv6.html · https://arxiv.org/abs/2606.13108 · https://github.com/PaddlePaddle/PaddleOCR/blob/main/ppocr/utils/dict/ka_dict.txt
- Paddle runners and Rust crates: https://github.com/RapidAI/RapidOCR · https://github.com/greatv/oar-ocr · https://github.com/mg-chao/paddle-ocr-rs · https://github.com/siska-tech/pure-onnx-ocr
- Other engines: https://github.com/robertknight/ocrs · https://github.com/datalab-to/surya · https://github.com/datalab-to/chandra · https://huggingface.co/IrakliJani/ka-ocr-v1-line
- Other tools: https://github.com/bityoungjae/chalkak · https://github.com/TenderOwl/Frog
- QR/barcode: https://github.com/zxing-cpp/zxing-cpp · https://github.com/axxel/zxing-bench · https://github.com/rxing-core/rxing · https://github.com/WanzenBug/rqrr · https://boofcv.org/index.php?title=Performance%3AQrCode
- ONNX Runtime: https://onnxruntime.ai/docs/performance/tune-performance/threading.html

**Stitching:**
- ShareX: https://github.com/ShareX/ShareX/blob/develop/ShareX.ScreenCaptureLib/ScrollingCaptureManager.cs
- Open-source stitchers: https://github.com/mg-chao/snow-shot · https://github.com/jswysnemc/wayscrollshot · https://github.com/joshdaws/omnishot · https://github.com/jbonney/scrollshot · https://github.com/Brkgng/ScrollSnap
- Linux tool issues: https://github.com/flameshot-org/flameshot/issues/4548 · https://bugs.kde.org/show_bug.cgi?id=193201 · https://github.com/ksnip/ksnip/issues/398
- Commercial tools: https://cleanshot.com/changelog · https://shottr.cc/kb/scrollingcapture
- Browser full-page capture: https://hg.mozilla.org/mozilla-central/raw-file/tip/browser/components/screenshots/ScreenshotsUtils.sys.mjs
- Android: https://developer.android.com/reference/android/view/ScrollCaptureCallback
- Wayland protocols: https://wayland.app/protocols/ext-image-copy-capture-v1 · https://gitlab.freedesktop.org/wlroots/wlr-protocols/-/blob/master/unstable/wlr-virtual-pointer-unstable-v1.xml
- GPU limits: https://vulkan.gpuinfo.org/displaydevicelimit.php?name=maxImageDimension2D&platform=linux
- xxHash: https://github.com/Cyan4973/xxHash

**Encoding and clipboard:**
- PNG encoders: https://github.com/veluca93/fpnge · https://github.com/richgel999/fpng · https://github.com/nigeltao/qoir · https://github.com/image-rs/fdeflate · https://github.com/brion/mtpng · https://github.com/shssoichiro/oxipng · https://libjpeg-turbo.org/About/Mozjpeg
- QOI: https://qoiformat.org/benchmark/
- Clipboard: https://chromium.googlesource.com/chromium/src/+/main/ui/base/clipboard/clipboard_ozone.cc · https://searchfox.org/mozilla-central/source/widget/gtk/nsClipboard.cpp · https://bugs.kde.org/show_bug.cgi?id=519651 · https://man.archlinux.org/man/wl-clipboard.1.en · https://github.com/sentriz/cliphist
- Colour and HiDPI: https://wiki.hypr.land/configuring/core/monitors/colors/ · https://www.w3.org/TR/png-3/ · https://raw.githubusercontent.com/go-gitea/gitea/main/web_src/js/utils/image.ts · https://github.com/Cykooz/fast_image_resize
- JPEG XL in browsers: https://www.phoronix.com/news/Chrome-145-Released · https://groups.google.com/a/mozilla.org/g/dev-platform/c/3YMV4MS34KA

**Redaction:**
- Research and attack tools: https://petsymposium.org/popets/2016/popets-2016-0047.php · https://arxiv.org/abs/1609.00408 · https://github.com/beurtschipper/Depix · https://bishopfox.com/blog/unredacter-tool-never-pixelation
- aCropalypse: https://www.da.vidbuchanan.co.uk/blog/exploiting-acropalypse.html
- Tool behaviour: https://flameshot.org/docs/advanced/protecting-your-privacy/ · https://discuss.kde.org/t/spectacles-blur-tool-is-not-secure-should-this-be-considered-a-bug/43368
- Secret detection: https://github.com/gitleaks/gitleaks/blob/master/config/gitleaks.toml · https://github.com/Yelp/detect-secrets · https://github.com/microsoft/presidio/tree/main/presidio-image-redactor
- Blur algorithms: https://blog.ivank.net/fastest-gaussian-blur.html · https://raw.githubusercontent.com/hyprwm/Hyprland/main/src/render/shaders/glsl/blur1.glsl

**GIF/video:**
- GIF: https://gif.ski/ · https://github.com/ImageOptim/gifski · https://blog.pkh.me/p/21-high-quality-gif-with-ffmpeg.html · https://www.bit-101.com/2017/2021/09/more-gif-making-tips-and-tools · https://christianselig.com/2025/08/high-quality-low-filesize-gifs/ · https://github.com/IanManske/quantette · https://learn.microsoft.com/en-us/archive/blogs/ieinternals/trivia-animated-gif-timing
- Platform limits: https://docs.github.com/en/get-started/writing-on-github/working-with-advanced-formatting/attaching-files · https://docs.x.com/x-api/media/quickstart/best-practices · https://discord.com/blog/modern-image-formats-at-discord-supporting-webp-and-avif
- Encoders and hardware: https://docs.nvidia.com/video-technologies/video-codec-sdk/13.0/nvenc-application-note/index.html · https://gitlab.com/AOMediaCodec/SVT-AV1/-/blob/master/Docs/Parameters.md · https://gitlab.freedesktop.org/mesa/mesa/-/issues/9185 · https://wiki.archlinux.org/title/Hardware_video_acceleration
- Seeking and smart cut: https://trac.ffmpeg.org/wiki/Seeking · https://github.com/mifi/lossless-cut/issues/126 · https://gitlab.freedesktop.org/gstreamer/gst-plugins-base/-/merge_requests/1249
- Libraries and players: https://github.com/zmwangx/rust-ffmpeg · https://gstreamer.freedesktop.org/releases/1.28/ · https://gstreamer.freedesktop.org/documentation/gtk4/index.html · https://mpv.io/manual/stable/
- gpu-screen-recorder source: https://repo.dec05eba.com/gpu-screen-recorder

**Beautify:** https://madebyevan.com/shaders/fast-rounded-rectangle-shadows/ · https://raphlinus.github.io/graphics/2020/04/21/blurred-rounded-rects.html · https://github.com/linebender/tiny-skia

**Memory:**
- Kernel and allocator: https://man7.org/linux/man-pages/man3/mallopt.3.html · https://man7.org/linux/man-pages/man2/madvise.2.html · https://man7.org/linux/man-pages/man2/memfd_create.2.html · https://blogs.gnome.org/alexl/2009/02/
- Compressors: https://github.com/facebook/zstd
- systemd-oomd: https://man.archlinux.org/man/systemd-oomd.service.8.en · https://www.fedoraproject.org/wiki/Changes/EnableSystemdOomd
- GTK texture slicing: https://blog.gtk.org/2023/04/05/gtk-4-11-1/ · https://gitlab.gnome.org/GNOME/gtk/-/merge_requests/5656
- Thumbnails: https://specifications.freedesktop.org/thumbnail/latest/
- Decode limits: https://docs.rs/image/latest/image/struct.Limits.html · https://libpng.sourceforge.io/decompression_bombs.html
