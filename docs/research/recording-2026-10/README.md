# Recording research (2026-10-09)

Six research passes made before and while building M4 (recording without Studio Mode). Each one tags its claims as verified on the reference machine, read in source or docs, or inferred. Paths under `/tmp/claude-1000/…` point to scratch files (harnesses, logs, source checkouts) that no longer exist; the results are in these reports.

| Report | What it answers |
|---|---|
| [cleanshot-x-4-recording.md](cleanshot-x-4-recording.md) | What CleanShot X 3.x–4.8.11 (before Studio Mode) did: every recording setting, the toolbar, keys, controls, camera, GIF, the 4.x video editor, the safety alerts. Read from the vendor's app files. **The parity checklist for M4.** |
| [gpu-screen-recorder.md](gpu-screen-recorder.md) | gsr 6.1.0 on this stack: the region rule at fractional scale, the IPC socket, the first-frame file, audio tracks, crash recovery, exit codes, failure modes |
| [omarchy-recording.md](omarchy-recording.md) | Omarchy's own recorder: its bar indicator, keys, webcam overlay, folder and post-processing, and how Snip stays out of its way |
| [hyprland-clicks-keys-windows.md](hyprland-clicks-keys-windows.md) | Hyprland 0.56.2's Lua key hook and click binds for REC-09/10, how events reach Snip, lock and privacy, and why window recording records the window's rectangle |
| [qt-camera-and-player.md](qt-camera-and-player.md) | Qt 6.11 Multimedia: the webcam bubble (V4L2 only, formats, CPU, hot-plug), the trimmer's player (seeking, frame steps, scrubbing), GIF playback |
| [brief.md](brief.md) | The six merged into one, with a parity list, slices and the contradictions between them (05:16, before most of M4 was built) |
| [review.md](review.md) | A completeness review of the brief against the code at 06:05, with five gaps closed by measurement: gsr's start edge, the idle inhibitor (it didn't work), the stop edge, encoder failure, error messages |
| [ffmpeg-editing-and-gif.md](ffmpeg-editing-and-gif.md) | ffmpeg 9 recipes, timed and checked for A/V sync: exact cuts, audio mixing, export presets, two-pass GIFs, progress, recovery |
