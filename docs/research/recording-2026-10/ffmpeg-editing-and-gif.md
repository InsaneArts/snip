# Verified ffmpeg 9 recipes for Snip's video editor and GIF export (recording without Studio Mode)

All results come from runs on this machine. Every command below ran on synthetic recordings built to match gpu-screen-recorder 6.1.0 output. Each run was timed, checked with ffprobe, and checked for A/V sync with timing markers (a white box flash and audio beeps). I made no edits under `/home/tgomareli/Development/OpenShotX` or `~/.config`. Nothing was shown on screen, nothing was recorded from the microphone, and gsr itself was never run.

`$R` = `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research`. It holds the scripts (`gen.sh`, `lib.py`, `recipes.py`, `s1…s7*.py`, `mp4dump.py`, `fixfrag.py`) and the raw logs (`logs/*.json`, `logs/*.txt`).

Confidence labels used below:
- **[V]**: verified locally.
- **[D]**: documented, or read in source code.
- **[I]**: inferred.

---

## 0. Findings that change or add to the existing docs

1. **The `concat` filter assumes constant frame rate (CFR).** It sets a segment's video length to `last_pts × n/(n−1)` (FFmpeg 9.0.1 `libavfilter/avf_concat.c` `push_frame`) **[D]**.
   - gsr records variable frame rate (VFR) by default.
   - With VFR input, cuts drifted 58.6 ms with frame-aligned cuts and the video was 480 ms out of sync with unaligned cuts **[V]**.
   - **Fix:** put `fps=<recording fps>` before `trim`. The result is then exact (0.0 ms error) **[V]**.
2. **Exact cuts need trim/atrim/concat in one filter graph, with cut points snapped to `k/fps`.** That gives 0.0 ms error on video, audio and A/V sync **[V]**. The other methods are not exact **[V]**:
   - `select`/`aselect`: about 2 extra frames and up to 43 ms A/V error. `between()` is inclusive at both ends, and `aselect` works on whole 20 ms Opus frames.
   - concat demuxer with `inpoint`/`outpoint`: 6–7 extra frames before each cut, or up to 20 ms error.
3. **A single-pass GIF (split + palettegen + paletteuse) holds every frame in RAM.** That was 1.55 GB RSS for a 60 s 1080p source. It also prints **no progress at all** until the palette is done **[V]**.
   - Use two passes instead: about 200 MB RSS and real progress in pass 2.
   - Building pass 1's palette from keyframes only (`-skip_frame nokey`) cut pass 1 from 4.9 s to 0.46 s on 60 s of 1080p, with equal SSIM (a 0–1 image-similarity score; higher is better) **[V]**.
4. **`bayer:bayer_scale=5` (the dither in the architecture doc) is the wrong default for UI content.**
   - On a UI clip it was 4–11% larger and had lower SSIM (0.9756) than `sierra2_4a:diff_mode=rectangle` (0.9853) or `dither=none` (0.9855) **[V]**.
   - The architecture doc's figure of "7–11 MB for a 10 s 720p GIF" is for worst-case moving content. Realistic UI clips came to 1.3–1.5 MB at 15 fps and 800 px **[V]**.
5. **gsr 6.1.0 writes `movflags=+hybrid_fragmented` with `use_editlist=1`** when libavformat ≥ 62.6.101 (`gsrsrc/src/recorder/muxer.c:86-101`) **[D]**.
   - A clean stop produces a normal MP4 with the moov box at the end: `ftyp, free, mdat, moov` **[V]**.
   - After a crash, the file is a fragmented MP4. **ffmpeg 9 recovers only 1 frame unless you pass `-ignore_editlist 1`** **[V]**. Cause: the initial moov's edit list has duration 0, which `mov_fix_index` treats as an empty window (`mov.c:4545`) **[D]**.
   - With gsr defaults, a crash loses about one GOP (up to about 2–3 s, and everything before the first 2 s flush). Fragments of `frag_duration=500000` cut that to about 0.5–1 s **[V]**.
6. **Audio traps** **[V]**:
   - `-ac 1` makes correlated stereo 3.1 dB louder. `pan=mono|c0=0.5*c0+0.5*c1` keeps the level.
   - `amix` halves each input by default (−6 dB). Use `normalize=0`.
   - `alimiter` delays audio 5 ms unless `latency=1`.
   - `-shortest` with a shorter replacement track cut the video from 10 s to 7 s.
7. **ffmpeg 9 `-progress` facts** **[V]**:
   - `out_time_ms` is still in microseconds (same value as `out_time_us`).
   - The first report waits until every output is initialised (`fftools/ffmpeg.c:595-597`) **[D]**.
   - SIGTERM or SIGINT: graceful stop in 0.3–1.2 s, exit code 255, and a valid truncated MP4.
   - SIGKILL: stops in about 10 ms and leaves no moov.
   - `q` on stdin: exit code **0**, so it looks like success.

---

## 1. Setup

**Machine:** i5-13600KF (20 threads), RTX 4080 (driver 610.57.04), ffmpeg n9.0.1 (lavc/lavf 63.1.101, lavfi 12.1.101). /tmp is **tmpfs**, so copy and remux timings will be somewhat slower on an SSD.

**Encoders present:** `h264_nvenc`, `hevc_nvenc`, `av1_nvenc`, `h264/hevc/av1_vulkan`, `libx264`, `libx265`, `libsvtav1`, `aac`, `libopus`, `gif`, `libwebp_anim` **[V]**.

**Encoder probe:** `ffmpeg … -f lavfi -i color=…:d=0.1 -c:v h264_nvenc -f null -` takes 175–200 ms. With CUDA hidden it returns rc 171 and `CUDA_ERROR_NO_DEVICE`, so fall back to libx264 (that probe takes 29 ms) **[V]**.

**What gsr 6.1.0 writes** (source snapshot `dec05eba.com/snapshot/gpu-screen-recorder.git.6.1.0.tar.gz`, unpacked in `$R/gsrsrc`) **[D]**:

| Property | Value | Source |
|---|---|---|
| Rate control | NVENC `rc=constqp`, `qp=25` ("very_high") | `video_codec.c:10-18, 273-283, 307-311` |
| Encoder options | `tune=ll`, `forced-idr=1` | `video_codec.c:349-350` |
| B-frames | `max_b_frames=0`; 2 only with `-tune quality` | `video_codec.c:73, 185` |
| GOP | `fps×keyint` **frames** (keyint default 2 s) | `video_codec.c:68-71` |
| Video timebase | 1/fps for CFR, 1/1,000,000 for VFR | `video_codec.c:57-58` |
| Audio | 48 kHz stereo; Opus 128k by default for mp4/mkv | `audio_codec.c:126-142`; man page `-ac` |
| Audio tracks | One track per `-a`; `a\|b` merges sources into one track | man page |
| Track titles | Device name, stored as MP4 `udta/name`; ffprobe shows it as tag **`name`**, not `title` **[V]** | `muxer.c:174-175` |
| Colour tags | SDR is tagged BT.709. ffmpeg 9 honours the tag when converting to RGB; the 601 matrix would give a visibly different result **[V]** | `video_codec.c:81-83` |

In VFR mode with KMS capture (anything other than `-fm content`), every tick counts as damaged (`recorder.c:524-526`). So gsr "VFR" on Hyprland is near-60 fps with microsecond timestamps **[I]**.

**Synthetic inputs** (`$R/gen.sh`, about 15 s to build). All use the gsr options above (`-use_editlist 1 -movflags +hybrid_fragmented`):

| File | Content |
|---|---|
| `in720.mp4` | testsrc2 1280×720@60 CFR, 10 s, 1 Opus track |
| `in2trk.mp4` | Same, plus a 2nd track. Track `system` beeps at k+0.5 s over a −30 dBFS tone; track `mic` beeps at k+0.25 s |
| `invfr.mp4` | VFR with a µs timebase; 3–6 s keeps only 2 fps. Keyframes fall at 0, 2.0, **6.9**, 8.9 s, because the GOP is counted in frames **[V]** |
| `inblack.mp4` | First 0.4 s black |
| `inui.mp4` | Realistic UI clip from Snip e2e screenshots: cursor, scroll, static, cut |
| `inui1080.mp4` | Same at 1080p |
| `inlong.mp4` | 1080p60, 60 s |
| `inuw.mp4` | 3440×1440@60 |
| `inuw2trk.mp4` | 3440×1440@60, 2 tracks |

Every file has a white 160×160 box at (20,20) for 6 frames from each k+0.5 s.

**Checks used:** `signalstats` YAVG on the box finds video events, `silencedetect` (−20 dB) finds beep onsets, then ffprobe gives durations, stream counts and decoded frame counts. On the source, the A/V offset measured 0.0 ms.

---

## 2. Probe, poster frame, thumbnail strip

**Probe** (29–31 ms) **[V]**:

```
ffprobe -v error -of json -show_entries format=duration,size,bit_rate:stream=index,codec_type,codec_name,width,height,pix_fmt,r_frame_rate,avg_frame_rate,time_base,start_time,duration,nb_frames,channels,sample_rate:stream_tags=name,title SRC
```

- Has audio = any stream with `codec_type=="audio"`.
- For VFR, use `r_frame_rate` as the export fps (60/1), not `avg_frame_rate` (213/5 on `invfr`).
- Frame and keyframe index for frame stepping and snapping: `ffprobe -v error -select_streams v:0 -show_entries packet=pts_time,flags -of csv=p=0 SRC` takes 55 ms for 3,600 frames **[V]**.

**Poster: first non-black frame at or after 0.5 s.** One run, 84–91 ms **[V]**:

```
ffmpeg -hide_banner -nostdin -y -loglevel error -ss 0.5 -t 5 -i SRC -vf "blackframe=amount=0:threshold=20,metadata=select:key=lavfi.blackframe.pblack:value=99:function=less" -frames:v 1 -update 1 poster.png
```

- `amount=0` tags every frame with `pblack`, and `function=less` keeps non-black frames. On `inblack` (scanned from 0) it picked the frame at exactly 0.4 s.
- **Use threshold 20, not 32.** With 32, a #121212 dark-terminal frame counts as 99% black (dark Omarchy themes); with 20 it counts as 0% **[V]**.
- On an all-black input ffmpeg exits **0 and writes no file**. Check the file exists, then fall back to `-ss 0.5 -i SRC -frames:v 1 -update 1 poster.png` (84 ms).

**Thumbnail strip, N=12, one ffmpeg run** **[V]**:

| Method | 10 s 720p | 60 s 1080p | Notes |
|---|---|---|---|
| A: full decode, `-ss D/(2N) -i SRC -an -vf "fps=N/D,scale=-2:H:flags=area,tile=Nx1" -frames:v 1 -update 1 strip.png` | 379 ms | 3,278 ms | Exact slot centres (0.42, 1.25 … 9.58 s). Cost grows with duration. Works on VFR (667 ms). |
| B: `-skip_frame nokey` + same `fps`/`tile` | 70 ms | 292 ms | Short clips get only D/2 keyframes, so the last 2 tiles were blank (Y=16). |
| C: N accurate seeks, one process (`-ss t_k -i SRC` ×N, `hstack=inputs=N`) | 397 ms | 770 ms | |
| C2: N keyframe seeks, `-noaccurate_seek -skip_frame nokey -ss t_k -i SRC` ×N + `hstack` | 284 ms | 503 ms | Takes the keyframe at or before t_k (2.5→2, 7.5→6). Plain accurate seek plus `nokey` takes the next keyframe instead. |

**Recommendation:** use A for clips up to about 30 s and C2 for longer ones (its cost grows with N, not duration). Use H = 64 × output scale, so 80 at 1.25. The PNG sprite is 1368×64 and 14–42 KB.

---

## 3. Trim and cuts

**Fast trim (stream copy)**, 37–40 ms **[V]**:

```
ffmpeg … -ss A -to B -i SRC -map 0 -c copy -movflags +faststart DST
```

- **Exact in every player when A is a keyframe.**
- When A is not a keyframe (A=2.4, keyframe at 2.0), the file stores 318 frames from the keyframe and an edit list (video media_time 6144/15360 = 0.4 s). ffmpeg-based players then show exactly 294 frames with 0.0 ms error.
- Players that ignore edit lists (simulated with `-ignore_editlist 1`) show the 0.4 s lead-in, with all events +0.4 s.
- QtMultimedia uses libavformat, so it should honour the edit list **[I]**.
- `-avoid_negative_ts make_zero` starts at the previous keyframe (+0.53 s error). Do not use it.
- The out-point is frame-exact for video (gsr has no B-frames). Audio ends on a packet boundary (+13.5 ms).

**Exact trim (re-encode):** `-ss A -to B -i SRC` (input seek, accurate by default) with NVENC took 699 ms and libx264 485 ms for 4.9 s of 720p, both with 0.0 ms error **[V]**.

**Cuts: keep segments [(a,b)…], 10 s 720p in, 6.65 s out** **[V]**:

| Method | Time | Frame-aligned cuts | Unaligned cuts | VFR source |
|---|---|---|---|---|
| **F: one input, trim/atrim + concat** | ~1.0 s | exact (0.0 ms) | ≤1 frame (video 3.0, audio 6.7, A/V 3.7 ms) | **+67 ms / 480 ms A/V without `fps`; exact with `fps=60` first** |
| S: `-ss a -to b -i SRC` per segment + concat | 0.84 s | exact | +1 frame, 23 ms | needs `fps` per input; unaligned fails |
| select/aselect + `setpts=N/FRAME_RATE/TB` | 0.9 s | +2 frames, A/V −27…−43 ms | wrong | breaks VFR timing |
| concat demuxer `inpoint`/`outpoint`, copy | 48 ms | +6 lead-in frames | +7 | — |
| concat demuxer, re-encode | 0.83 s | +16.7 ms, A/V −10 ms | 19.7 ms | — |

**60 s 1080p source with 3 cuts (17.15 s out)** **[V]**:

| Method | Time | RSS |
|---|---|---|
| F + fps, NVENC p5 | 5.87 s | 557 MB |
| S, NVENC p5 | 3.76 s | 637 MB |
| F, x264 | 4.06 s | 1,146 MB |
| Decode only | 3.73 s | — |

All were exact with aligned cut points. S is faster when the kept part is small, but only use it with snapped cut points and `fps` on each input.

**Full export in one run** (2-track 3440×1440 → mix + cuts + 1080p cap): NVENC p5 cq28 took 2.44 s (704 MB); x264 veryfast crf20 took 1.79 s (1.47 GB). Video events, all 13 merged-audio events and the 6.650 s duration were exact **[V]**.

---

## 4. Audio

Measured on the 10 s 2-track file **[V]**:

| Operation | Command | Time | Check |
|---|---|---|---|
| Mute | `-map 0:v -c:v copy -an` | 38 ms | no audio stream |
| Keep system only | `-map 0:v -map 0:a:0 -c copy` | 41 ms | system beeps only |
| Keep mic only | same with `0:a:1` | 41 ms | mic beeps only |
| Mono | `-af "pan=mono\|c0=0.5*c0+0.5*c1" -c:a aac` | 188 ms | level kept (max −5.3 dB). `-ac 1` gives −2.3 dB, which is +3.1 dB louder |
| Gain | `-af volume=6dB` | 0.2–0.4 s | already clips at 0 dBFS on loud peaks |
| Gain with limiter | `volume=12dB,alimiter=limit=0.891:level=disabled:latency=1` | 195 ms | peak −0.8 dBFS, onsets unchanged. Without `latency=1` the audio is 5.1 ms late |
| Merge two tracks with volumes | `[0:a:0]volume=1.0[s];[0:a:1]volume=0.8[m];[s][m]amix=inputs=2:duration=first:normalize=0` | 359 ms | all 20 beeps; matches `amix weights='1 0.8':normalize=0` and `amerge,pan=stereo\|c0=c0+0.8*c2\|c1=c1+0.8*c3` |
| Merge with default `amix` | `normalize=1` | — | −6 dB per input |
| Replace audio | `-map 0:v -map 1:a -c:v copy -af "apad=whole_dur=D,atrim=end=D" -c:a aac` | 181 ms | exactly 10.000 s for 7 s or 15 s replacements. `-shortest` cut the video to 7 s |

**Audio codec speed for 60 s stereo:** native AAC 1.9 s, `-aac_coder fast` 0.7 s, libopus 0.32 s.

**Duration trap:** a re-encode without atrim gives Opus a 13.5 ms tail. Always bound the audio with `atrim` (the graph does this) or `-t`.

**Opus in MP4 on Apple:** a 2022 user report says Opus in MP4 does not play on Apple (Safari/QuickTime) ([Apple forum](https://developer.apple.com/forums/thread/722399)) **[D, unverified]**. So exports should re-encode audio to AAC; copy Opus only on untouched fast paths.

---

## 5. Export options

**NVENC `-cq` vs x264 `-crf` at equal SSIM** (NVENC p5 vbr; `-b:v 0` made **identical** output, so it is not needed in ffmpeg 9) **[V]**:

| NVENC | UI 1080p | testsrc2 720p | 3440×1440 | → x264 crf at equal SSIM |
|---|---|---|---|---|
| cq28 | 358 KiB, .99897 | 6.5 MiB, .9946 | 36.7 MiB, .9956 | 18.6–19.8 |
| cq32 | 335 KiB, .99812 | 3.9 MiB, .9889 | 21.3 MiB, .9909 | 22.5–24.0 |
| cq36 | 268 KiB, .99673 | 2.4 MiB, .9831 | 12.9 MiB, .9871 | 25.4–27.8 |

So **NVENC CQ ≈ x264 CRF + 9 (±1.5)**.

**Proposed presets** (output size relative to the gsr source, measured with p4):

| Preset | NVENC | x264 | UI / busy content |
|---|---|---|---|
| High | cq24 | crf ~16 (extrapolated) | 0.83 / 0.93–0.98× |
| Medium (default) | cq28 | crf 19 | 0.70 / 0.61–0.65× |
| Low | cq32 | crf 23 | 0.62 / 0.36–0.39× |
| Very low | cq36 | crf 27 | 0.51 / 0.22–0.24× |

Use stream copy for "Original" when nothing forces a re-encode.

**Encoder speed for 10 s** **[V]**:

| Encoder | UI 1080p | 3440×1440 |
|---|---|---|
| NVENC p1 | 1.05 s | 2.29 s |
| NVENC p4 | 1.46 s | 3.02 s |
| NVENC p5 | 2.35 s | 5.0 s |
| NVENC p7 | 2.39 s | 5.4 s |
| x264 veryfast | 0.9–1.1 s | 2.4–3.1 s |

- x264's RSS is 1.4 GB at 2580×1080. With `-threads 8` it drops to 773 MB but takes +60% time.
- Recommend NVENC **p4**.
- At about the same SSIM, `hevc_nvenc` cq28 was 8% smaller and `av1_nvenc` cq36 28% smaller on UI content.

**Resolution** (10 s 3440×1440, NVENC p4) **[V]**:

| Target | Filter | Output | Time |
|---|---|---|---|
| 1080p cap | `scale=-2:'min(1080,ih)':flags=lanczos` | 2580×1080 | 1.96 s |
| 720p cap | same with 720 | 1720×720 | 1.86 s |
| 1× of a 1.25 HiDPI capture | `scale='trunc(iw/1.25/2)*2':'trunc(ih/1.25/2)*2':flags=lanczos` | 2752×1152 | 2.18 s |
| GPU path | `-hwaccel cuda -hwaccel_output_format cuda` + `scale_cuda=-2:'min(1080,ih)':interp_algo=lanczos` | same dimensions | 1.87 / 2.03 s |

The GPU path supports `-2` and `min()`. Its SSIM against the CPU result is 0.995. Its speed gain was negligible here, so the CPU path is enough. Scale **after** concat, so cut-away frames are never scaled.

**Estimated size** **[V]**:

- **Stream-copy paths:** exact. Sum the packet sizes in the range (ffprobe `packet=pts_time,size`).
- **Re-encode, instant estimate:** kept source bytes × preset ratio (table above) × (output pixels / input pixels)^e, plus audio kbps × D/8.
  - Measured e: 0.67–0.79 on UI, 1.2 on busy content.
  - Error: ±40% with one fixed ratio.
- **Re-encode, refined estimate:** encode 5 one-second samples in one process (`-ss … -t 1 -i SRC` ×5, then concat), 0.6–2 s.
  - Estimate = source non-keyframe bytes × (sample P-bytes / source P-bytes in the windows) + output keyframes × mean sample I-frame size. NVENC's default GOP is 250 frames.
  - Error ≤10% on busy content; +10–21% (an overestimate) on UI content.
  - Plain extrapolation of the samples overestimated UI content by 30–60%.

---

## 6. GIF

**Recipe: two passes** (bounded memory, real progress in pass 2) **[V]**:

```
# pass 1 (use -skip_frame nokey and drop fps= when the segment is ≥10 s)
ffmpeg … -ss A -t D -i SRC -an -vf "fps=F,scale='min(W,iw)':-2:flags=lanczos,palettegen=stats_mode=diff" -update 1 pal.png
# pass 2
ffmpeg … -ss A -t D -i SRC -i pal.png -an -lavfi "fps=F,scale='min(W,iw)':-2:flags=lanczos[x];[x][1:v]paletteuse=dither=sierra2_4a:diff_mode=rectangle" -loop 0 DST.gif
```

**10 s clips, 800 px wide, total of both passes** (KiB) **[V]**:

| fps | UI 720p sierra / none | UI 1080p→800 sierra / none | testsrc2 720p sierra / none |
|---|---|---|---|
| 10 | 0.56 s 1,055 / 0.45 s 987 | 0.64 s 988 / 0.51 s 922 | 1.66 s 3,765 / 1.24 s 3,251 |
| 15 | 0.71 s 1,495 / 0.48 s 1,409 | 0.83 s 1,421 / 0.67 s 1,324 | 2.16 s 5,237 / 1.59 s 4,564 |
| 20 | 0.81 s 1,893 / 0.57 s 1,761 | 1.05 s 1,791 / 0.76 s 1,655 | 2.73 s 6,886 / 1.58 s 5,946 |
| 30 | 1.14 s 2,705 / 0.84 s 2,542 | 1.40 s 2,501 / 1.03 s 2,331 | 3.61 s 9,873 / 2.01 s 8,584 |

**Dither comparison** (UI 1080p, 15 fps, frame-aligned SSIM) **[V]**:

| Dither | Size (KiB) | SSIM |
|---|---|---|
| none | 1,324 | .9855 |
| floyd_steinberg | 1,367 | .9856 |
| sierra2_4a | 1,421 | .9853 |
| bayer:5 | 1,476 | .9756 |
| bayer:3 | 1,696 | .909 |
| bayer:2 | 1,726 | .788 |
| none, `max_colors=128` | 1,098 | .9847 |
| none, `max_colors=64` | 860 | .9835 |

Other GIF notes **[V]**:
- `diff_mode=rectangle` halves sierra's time (1.21 → 0.64 s) at the same size.
- `mpdecimate` gave no gain.
- `stats_mode=full` was mixed: smaller on UI, larger on testsrc2.

**Suggested quality mapping:**
- High: sierra2_4a, 256 colours.
- Medium: sierra2_4a or none.
- Low: none with 128 or 64 colours. That costs a lot on busy content (testsrc2 SSIM .81 → .68).

**Memory: one pass vs two passes** **[V]**:

| Clip | Single pass | Two passes |
|---|---|---|
| UI 10 s | 393 MB RSS | 176 + 182 MB |
| 30 fps | 613 MB | — |
| 1280 px | 714 MB | — |
| 60 s 1080p | **1,549 MB**, 6.2 s | 231 + 203 MB, 4.3 + 3.6 s |
| 60 s 1080p, keyframe-only palette | — | pass 1 0.46 s |

Keyframe-only palette quality: UI SSIM .9850 vs .9853; testsrc2 .826 vs .812; size +0–10%.

**Behaviour checks** **[V]**:
- A trimmed segment (`-ss 2 -t 3`) gives 45 frames and 3.000 s.
- Delays at 15 fps are 7/6/7 cs with no drift.
- Without `fps` first, a 60 fps VFR source made 426 frames with 2/1/2 cs delays, which browsers clamp. Always put `fps` first.
- `-loop 0` writes NETSCAPE count 0 (loop forever). `-loop -1` writes no NETSCAPE block (play once). `-loop 3` writes count 3.

**Optimizers:** both are in Arch `extra`, neither is installed **[V]**:
- `gifsicle` 1.96-1, GPL-2.0-only, depends on libx11.
- `gifski` 1.34.0-3, AGPL-3.0-or-later, links ffmpeg.

I downloaded both, checked their sha256 against `extra.db`, and ran them from `$R/pkgs/root`.

| Tool | UI 1080p (from 1,421 KiB sierra) | testsrc2 720p |
|---|---|---|
| `gifsicle -O3` | 1,369, 0.39 s | 4,989, 1.6 s |
| `gifsicle -O3 --lossy=30` | **854, 1.2 s, SSIM .9802** | 4,881, **6.2 s** |
| `gifsicle --lossy=80` | 794, .9733 | 4,749 |
| `gifski --fps 15 --width 800 -Q 90` | **1,093, 1.09 s, .9857** | **3,984, 1.31 s, .883** (ffmpeg: .812) |
| `gifski -Q 70` | 669, .950 | 2,929, .860 |

- gifski works on a trimmed segment through a pipe: `ffmpeg … -f yuv4mpegpipe -pix_fmt yuv444p - | gifski --fps F -Q 90 --quiet -o DST.gif -`. A 3 s clip took 427 ms and came out at 789 KB.
- gifski's colour-matrix handling in the pipe is ambiguous; it showed no measurable colour shift here **[V/I]**.

**Recommendation:** "Optimize" = `gifsicle -O3 --lossy=30` when installed (UI −40%; slow on busy content, so cap it with a timeout). "HQ" = gifski when installed, run as a separate CLI process (it is AGPL).

---

## 7. Progress and cancellation

**Progress output.** Add `-progress pipe:1 -nostats -stats_period 0.5`. Each block arrives every 0.5 s and ends with `progress=continue` or `progress=end` **[V]**:

```
frame=31 fps=0.00 stream_0_0_q=28.0 bitrate=8674.4kbits/s total_size=786480 out_time_us=725333 out_time_ms=725333 out_time=00:00:00.725333 dup_frames=0 drop_frames=0 speed=1.45x progress=continue
```

- `speed` can carry leading spaces.
- `total_size` can be `N/A`.

**Percent** = clamp(`out_time_us` / expected µs). Expected = Σ(b−a) for exports, D for GIF pass 2.
- During cut-away regions the value plateaus. In the export run it held at 42% for about 1 s.
- Success = rc 0 **and** `progress=end` seen.

**Phases with no progress output** **[V]**:
- Single-pass GIF: no blocks for 4.5 s, then 12→100% in 1.2 s.
- GIF pass 1: only the final block. Adding a null side output did not help, because the first report waits for all outputs to initialise (`ffmpeg.c:595-597`) **[D]**.
- So show pass 1 as indeterminate and make it cheap with the keyframe palette.
- The gifski pipe reports fine with `-progress pipe:2`.

**Cancellation** (x264 slow export on the 60 s source) **[V]**:

| Signal | Stop time | rc | Output |
|---|---|---|---|
| SIGINT / SIGTERM | 0.3–1.2 s | 255 | valid truncated MP4, moov written (faststart included) |
| SIGKILL | 10 ms | −9 | `moov atom not found` |
| `q` on stdin | 1.2 s | **0** | valid but truncated |

**Rust rules:**
- Spawn with `-nostdin`, stdin null, and `process_group(0)`.
- Write to a temp file in the destination directory, then rename on success.
- Cancel = `killpg(SIGKILL)` and unlink the temp file. Use SIGTERM only to "stop and keep partial".
- Set `PR_SET_PDEATHSIG` so ffmpeg dies with snipd **[I]**.

---

## 8. Crash recovery (gsr hybrid MP4)

I simulated this with an ffmpeg writer using gsr's exact mux settings, killed with SIGKILL while writing in real time (`$R/s7.py`) **[V]**.

**Detecting the file's state:**
- Finalized file: `ftyp, free, mdat, moov`, with the moov at the end. For sharing, remux with `+faststart` (100 ms for 137 MB on tmpfs).
- Crashed file: `ftyp, free(8), free(44), moov, mdat, moof, mdat, …`. **Any top-level `moof` means the file was not finalized.**

**Recovery command** (30–42 ms):

```
ffmpeg -hide_banner -nostdin -y -loglevel warning -ignore_editlist 1 -fflags +discardcorrupt -i CRASHED.mp4 -map 0 -c copy -movflags +faststart RECOVERED.mp4
```

**What was recovered** (media time ≈ wall time + 0.5 s burst):

| Writer, kill time | Default options | `-ignore_editlist 1` |
|---|---|---|
| hybrid, kill at ~5 s | 3.0 s video, 0.0135 s audio | **3.03 s video / 2.02 s audio**, A/V +6.5 ms (Opus pre-skip not applied) |
| hybrid, kill at 1.2 s | 88 bytes: nothing to recover | — |
| hybrid, kill at ~9 s | — | 7.03 s / 6.02 s |
| hybrid + `frag_duration=500000`, kill at 1.2 s | — | 1.4 s / 1.0 s |
| hybrid + `frag_duration=500000`, kill at ~5 s | — | 4.37 s / 4.0 s |
| plain mp4 | unrecoverable ("moov atom not found") | — |
| mkv | remux with defaults: 3.9 s of ~5 s, A/V 0.5 ms | — |

- gsr's `-ffmpeg-opts` dict is passed to `avformat_write_header` (`muxer.c:117-118`), so `-ffmpeg-opts "frag_duration=500000"` should take effect **[I]**. A graceful stop with `frag_duration` still finalizes to a normal file **[V]**.
- `hybrid_fragmented` together with `faststart` is rejected: "Setting both hybrid_fragmented and faststart is not supported." ([FFmpeg commit](https://ffmpeg.org/pipermail/ffmpeg-cvslog/2025-May/148737.html)) **[V/D]**.
- ffmpeg writes the hiding mdat size only after the complete final moov (`movenc.c:9103-9109`) **[D]**.
- If a *finalized* file loses its tail later, rewriting the box at offset 44 (`mdat` → `free`, size 44) brings back the fragments. Remuxing with `-ignore_editlist 1` then recovered all 10 s with exact sync **[V]**. This depends on the muxer's internal layout, so check the bytes first.

---

## 9. Command templates for Rust

Prefix every ffmpeg run with: `ffmpeg -hide_banner -nostdin -y -loglevel error -progress pipe:1 -nostats -stats_period 0.5`. Pass each argument as its own argv element; the quotes inside filter strings belong to the filtergraph syntax.

| Job | Template | Measured |
|---|---|---|
| Probe | `ffprobe -v error -of json -show_entries format=duration,size,bit_rate:stream=index,codec_type,codec_name,width,height,r_frame_rate,avg_frame_rate,time_base,duration,nb_frames,channels,sample_rate:stream_tags=name,title {SRC}` | 30 ms |
| Frame/keyframe index | `ffprobe -v error -select_streams v:0 -show_entries packet=pts_time,flags -of csv=p=0 {SRC}` | 55 ms / 3,600 frames |
| Poster | `-ss 0.5 -t 5 -i {SRC} -vf "blackframe=amount=0:threshold=20,metadata=select:key=lavfi.blackframe.pblack:value=99:function=less" -frames:v 1 -update 1 {OUT}.png`; if no file: `-ss 0.5 -i {SRC} -frames:v 1 -update 1 {OUT}.png` | 84–91 ms |
| Strip (D ≤ 30 s) | `-ss {D/(2N)} -i {SRC} -an -vf "fps={N}/{D},scale=-2:{H}:flags=area,tile={N}x1" -frames:v 1 -update 1 {OUT}.png` | 0.38 s / 10 s 720p |
| Strip (long) | `(-noaccurate_seek -skip_frame nokey -ss {t_k} -i {SRC})×N -filter_complex "[k:v]scale=-2:{H}:flags=area[tk];…;[t0]…hstack=inputs={N}" -frames:v 1 -update 1 {OUT}.png` | 0.50 s / 60 s 1080p |
| Fast trim (copy) | `-ss {A} -to {B} -i {SRC} -map 0 -c copy -movflags +faststart {TMP}` | 37–40 ms |
| Mute | `-i {SRC} -map 0:v -c:v copy -an -movflags +faststart {TMP}` | 38 ms |
| Keep one track | `-i {SRC} -map 0:v -map 0:a:{i} -c copy -movflags +faststart {TMP}` | 41 ms |
| Replace audio | `-i {SRC} -i {AUD} -map 0:v -map 1:a -c:v copy -af "apad=whole_dur={D},atrim=end={D}" -c:a aac -b:a 160k -movflags +faststart {TMP}` | 0.18 s |
| Export (cuts/mix/gain/mono/scale) | `-i {SRC} -filter_complex "{GRAPH}" -map "[v]" -map "[a]" -c:v h264_nvenc -preset p4 -tune hq -rc vbr -cq {CQ} -profile:v high -pix_fmt yuv420p -c:a aac -b:a 160k -movflags +faststart {TMP}`; fallback `-c:v libx264 -preset veryfast -crf {CQ-9}` | 1.0 s (10 s 720p); 2.4 s (3440×1440 → 1080p) |
| Size sample | `(-ss {s_k} -t 1 -i {SRC})×5 -filter_complex "[0:v:0]…concat=n=5:v=1:a=0[v]" -map "[v]" {VENC} {TMP}` | 0.6–2 s |
| GIF pass 1 | `[-skip_frame nokey] -ss {A} -t {D} -i {SRC} -an -vf "fps={F},scale='min({W},iw)':-2:flags=lanczos,palettegen=stats_mode=diff" -update 1 {PAL}.png` | 0.09–0.46 s (keyframes); 0.3–4.9 s (all frames) |
| GIF pass 2 | `-ss {A} -t {D} -i {SRC} -i {PAL}.png -an -lavfi "fps={F},scale='min({W},iw)':-2:flags=lanczos[x];[x][1:v]paletteuse=dither={sierra2_4a\|none}:diff_mode=rectangle" -loop {0\|-1} {TMP}.gif` | 0.2–0.6 s (UI 10 s) |
| GIF optimize | `gifsicle -O3 --lossy=30 {IN}.gif -o {TMP}.gif` | 0.4–1.2 s UI; 6 s busy |
| GIF HQ | `ffmpeg … -ss {A} -t {D} -i {SRC} -an -vf "fps={F},scale='min({W},iw)':-2:flags=lanczos" -f yuv4mpegpipe -pix_fmt yuv444p - \| gifski --fps {F} -Q 90 --quiet -o {TMP}.gif -` | 0.43 s / 3 s clip |
| Recover crashed gsr file | `-ignore_editlist 1 -fflags +discardcorrupt -i {SRC} -map 0 -c copy -movflags +faststart {TMP}` | 30–42 ms |
| Faststart for sharing | `-i {SRC} -map 0 -c copy -movflags +faststart {TMP}` | 0.1 s / 137 MB |

**`{GRAPH}` builder** (verified exact; `$R/recipes.py:export_graph`). Snap every cut point to k/FPS first. With K segments:

```
[0:a:0]volume=SV[as];[0:a:1]volume=MV[am];[as][am]amix=inputs=2:duration=first:normalize=0[,alimiter=limit=0.891:level=disabled:latency=1][,pan=mono|c0=0.5*c0+0.5*c1],asplit=K[ain0]…;
[0:v]fps=FPS,split=K[vin0]…;
[vin{i}]trim=start=A_i:end=B_i,setpts=PTS-STARTPTS[v{i}];[ain{i}]atrim=start=A_i:end=B_i,asetpts=PTS-STARTPTS[a{i}];…
[v0][a0]…concat=n=K:v=1:a=1[vc][a];[vc]scale=-2:'min(MAXH,ih)':flags=lanczos[v]
```

- One audio track: start the audio chain with `[0:a:0]volume=SV`.
- No audio or muted: drop the audio parts, use `concat=n=K:v=1:a=0`, and pass `-an`.
- Build the graph even for one segment; it keeps audio duration exact.

---

## 10. Checklist for the implementation

- Always `-update 1` for single images, and check the output exists (exit code 0 does not mean a file was written).
- Use `r_frame_rate` as the export fps, and put `fps=` before `trim` and in GIF graphs.
- Use `normalize=0` on `amix`, `latency=1` on `alimiter`, and pan-mono instead of `-ac 1`.
- Never use `-shortest` for replacement audio; use `apad` + `atrim`.
- Never use the `q` key to cancel.
- Show pass 1 as indeterminate.
- Recover crashed files with `-ignore_editlist 1`.
- Consider passing `frag_duration=500000` to gsr through `-ffmpeg-opts`.

**Sources:**
- FFmpeg n9.0.1 sources (`avf_concat.c`, `ffmpeg.c`, `mov.c`, `movenc.c`, `vf_paletteuse.c`): https://raw.githubusercontent.com/FFmpeg/FFmpeg/n9.0.1/ (copies in `$R/ffsrc`).
- [hybrid_fragmented patch](https://ffmpeg.org/pipermail/ffmpeg-devel/2024-June/329979.html), [hybrid+faststart commit](https://ffmpeg.org/pipermail/ffmpeg-cvslog/2025-May/148737.html).
- gsr 6.1.0 snapshot (`$R/gsrsrc`) and the gsr man page (`…/scratchpad/gsr-man.txt`).
- `pacman -Si gifski gifsicle`.
- [Apple forum on Opus in MP4](https://developer.apple.com/forums/thread/722399).
