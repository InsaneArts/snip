# CleanShot X recording, GIF and Video Editor before Studio Mode (3.x to 4.8.11)

The pre-Studio recorder can now be described from vendor-primary evidence, not only from 5.0-era material. I analysed the last 4.7 build (4.7.6) and the last 4.x build (4.8.11) statically, without running them. Their recording settings, Video Editor layout and recorder strings are identical. So the pre-Studio recorder did not change from about 4.6 through 4.8.11, and everything below applies to that range unless a version is named. Nothing in the repo or `~/.config` was touched.

**Confidence levels:**
- **V** (verified locally): read from the 4.x app files or an official image.
- **D** (documented): stated in official text or a review.
- **I** (inferred).

**Evidence used:**
- **[BIN]** Official DMGs from `https://updates.getcleanshot.com/v3/CleanShot-X-4.7.6.dmg` (SHA-256 `677178b8…3307b55`) and `…/CleanShot-X-4.8.11.dmg` (SHA-256 `298afb3d…a1a81`).
  - I decoded each DMG to a raw APFS image (UDIF and LZFSE).
  - From that I parsed the compiled interface files (NIBs) for exact labels, menus, slider ranges and key equivalents.
  - I disassembled the Intel code to read the menus, alerts and settings defaults that are built in code.
  - Hex numbers given below (for example "at 0x5b9808") are file offsets inside `cs476.apfs`.
- **[CL]** https://cleanshot.com/changelog. Version tags such as [CL 4.6] refer to it.
- **[F24]** The archived features page of June 2024: http://web.archive.org/web/20240620164438/https://cleanshot.com/features
- **[IMG]** Official pre-5.0 images, downloaded and viewed locally:
  - `screen_recording_2x.797f6110.png` (recorder toolbar)
  - `video_editor_2x.28d97fdb.png` (editor)
  - `recording.baccd44b.jpg` (controls pill, December 2024)
  - `recording.7d27e588.png` (camera)
- **[API]** https://cleanshot.com/docs-api
- **Reviews:**
  - **[DS]** https://daveswift.com/cleanshot-x/ (18 March 2024)
  - **[TSS]** https://thesweetsetup.com/how-to-record-professional-screen-shares-with-cleanshot-x-or-loom/ (7 June 2022)
  - **[BT]** https://birchtree.me/blog/cleanshot-x-4-3-is-out-now/ (28 June 2022)
  - **[9to5]** https://9to5mac.com/2022/01/31/cleanshot-x-utility-for-mac-adds-new-quick-controls-blur-options-more/
  - **[SK]** https://www.screenkite.com/blog/reduce-cleanshot-x-screen-recording-file-size (May 2026)
- **[HK]** The history of soffes/HotKey `Key.swift`, used to decode the default keys.
- YouTube captions could not be fetched: yt-dlp got HTTP 429 and a bot check. Where needed I rely on the inventory's [V-x] captions.

---

## 0. Corrections to inventory §6 (it was written from 5.0.1)

1. **Window recording existed before 5.0.** The recorder had a recorder-only "Select Window" shortcut whose default is **Space**.
   - The hint reads "Drag to record a part of the screen." followed by " Press Space to select a window." (V: hint builder at 0x36ce50; default decoded from the shortcut table at 0x526945).
   - [DS] (2024): "snap to a specific window by pressing the spacebar".
   - So the sentence "window recording works since 5.0" should say: 5.0 added window capture with an automatic background, while 4.x snapped the recording area to the window's frame (I).
2. **The Vertical camera shape existed before 5.0.** The camera menu offers Circle, Square, Rectangle and Vertical (V, camera menu code at 0x5fd67c; [DS] "circular, square, or vertical").
3. **The FPS list is now verified:** 10, 15, 24, 25, 30, 48, 50, 60, default **60**. There is nothing above 60 (V).
4. **The Max resolution list is verified:** Original (default), 4k, 1440p, 1080p, 720p, 480p (V).
5. **The audio-track choice lives in a "Computer Audio Settings…" window.** Its labels are "Record on separate tracks" and "Record on a single track". After a recording, a separate merge dialog sets each track's volume (V).
6. **Low disk space only produced a warning in 4.x.** No text for an automatic stop exists in 4.7.6 or 4.8.11, so the auto-stop is 5.0-only or undocumented (V, by absence).
7. **The pre-5.0 editor trimmed only the start and end.**
   - It had no cuts in the middle ([TSS]: "you can't remove bits from the middle").
   - It had no GIF export and no timeline zoom (V, by absence).
   - Its quality and audio options have exact names, listed in §6.

---

## 1. Recording setup

### 1.1 Entry points

| Entry point | Detail | Evidence |
|---|---|---|
| Menu-bar menu item | "Record Screen" | V, menu NIB |
| Global hotkey | "Record Screen / Stop Recording"; the same key stops a running recording. No key is assigned by default | V |
| All-In-One | The Record mode | already in the inventory |
| URL | `cleanshot://record-screen[?x=&y=&width=&height=&display=]`. The parameters need 4.7. Without `display`, it uses the display the cursor is on | D, [API] |

### 1.2 What to record

**Area:**
- Drag a rectangle; it has 8 handles (V, [IMG]).
- The area snaps to edges ([CL 1.0.2], [CL 3.8]).
- Option draws a uniform selection ([CL 3.9.4]).
- Arrow keys with ⌘ or ⇧ move or resize it ([CL 3.1.2]).
- A typed **W × H** is in points, so on a Retina display 1920×1080 records 3840×2160 unless "Scale Retina videos to 1x" is on (V for the fields; D [DS] for the doubling).
- Esc cancels the overlay but never stops a running recording ([CL 3.8] fix).

**Aspect-ratio lock** (added in 4.1; [9to5] "16:9 for YouTube"):
- It uses a ratio button with a dropdown arrow, the same control All-In-One uses (V: the `recorderRatio` and `oneOverlayChevron` icons).
- The last ratio is remembered (V: `recorderLastSelectedRatio`).
- The binary's shared ratio list contains Freeform, "1 : 1 (Square)", "5 : 4 (10 : 8)", "7 : 5", "3 : 2 (6 : 4)", "16 : 10", "16 : 9", "2.35 : 1", portrait versions such as "2 : 3" and "9 : 16", and "Custom Ratio" (V for the strings; I that the recorder shows this exact list).

**Window:**
- Space switches to window picking; the area becomes the window's frame (V for Space; I for the frame).
- The overlay has a window-name label (V: `windowNameLabel`).

**Full screen:**
- The ⤢ button (tooltip "Toggle fullscreen") expands the area to the whole display and toggles back (V: `recorderFullscreen` and `recorderMinimize`).
- Return with no area drawn records the full display (D: [CL 3.5.1] and [CL 4.2.2] fixes, plus the inventory).

**Displays:**
- There is one overlay per display, and a recording covers one display (I: `displayId` on the overlay).
- Other displays are not dimmed while recording ([CL 3.3.1]).

**Remember last selection:** a setting, default **off**. It restores `lastRecordingArea` and works on external monitors since a [CL 3.8] fix (V).

### 1.3 Recorder toolbar: exact contents and order ([IMG], confirmed by the code order at 0x3c6000–0x3c8fca)

Two panels appear centred in the selection.

**Panel 1, row 1:**
- **Settings** (sliders icon, tooltip "Show preferences"; it opens Preferences › Recording).
- **W [field] x H [field]**.
- **⤢** "Toggle fullscreen".

**Panel 1, row 2.** These are toggles; a check mark shows under each active one.

| Button | What it does |
|---|---|
| **Microphone** (tooltip "Record Microphone") | Opens a menu: "Do Not Record Microphone" plus the input devices. The button shows a live mic level meter ([CL 3.9.4]; V: `vuMeter`) |
| **Computer audio** ("Record Computer Audio") | Toggle |
| **Camera** ("Record Camera") | Opens the camera menu (§3) |
| **Highlight Clicks** | Toggle |
| **Show Keystrokes** | Toggle |

**Panel 2:** two buttons that show their keys: **"Record GIF ⌥↩"** and **"Record Video ↩"**.

The toolbar toggles read and write the same settings as Preferences: `highlightClicks`, `showKeystrokes` and `recordComputerAudio` (V).

### 1.4 Recorder keys

These are defaults; each can be changed in Shortcuts › Screen Recording.
- **Return** = Start Video Recording.
- **⌥Return** = Start GIF Recording.
- **Space** = Select Window.

All three work only while the recorder is open.

How this was verified:
- The shortcut table at 0x526945–0x526b05 gives HotKey key indices 47 (Space) and 49 (Return), plus a modifier for the GIF entry.
- The HotKey `Key` enum had 113 cases before the "section" key was added in December 2024, so 113 means "no key".
- The image's ↩ and ⌥↩ match these defaults.

---

## 2. Preferences › Recording (4.x): every option, its values and its default

Defaults come from the code's fallback values when no setting is stored (V; the default for the two settings marked I is inferred from code order). The tab has three sub-tabs: **General | Video | GIF**.

### 2.1 General

| Section | Control | Default |
|---|---|---|
| Controls: | Show controls while recording | **on** |
| Menu bar: | Display recording time | off |
| Retina: | Scale Retina videos to 1x | off |
| Notifications: | "Do Not Disturb" while recording | off |
| Cursor: | Show cursor | **on** |
| | Highlight clicks [Options…] | off |
| Keyboard: | Show keystrokes [Options…] | off |
| Recording area: | Remember last selection | off |
| | Dim screen while recording | **on** |
| | Show countdown | off |

### 2.2 Video

**Max resolution:** Original (default), separator, 4k, 1440p, 1080p, 720p, 480p.
- Caption: "Set maximum resolution to reduce file size and upload time."
- Internally these are bounding boxes: 3840×2160, 2560×1440, 1920×1080, 1280×720 and 854×480 (V, table at 0x5ba6c8).

**Video FPS:** a popup with 10, 15, 24, 25, 30, 48, 50, 60. Default 60.
- [CL 3.4.5] added "lower FPS"; [CL 4.6] added "more FPS options". Mapping these to 10/15 and 24/25/48/50 is my inference.

**Audio:**
- A [Computer Audio Settings…] button (opens the window in §2.5).
- "Record audio in mono", default off.

**Video Encoder:** "Open Video Editor after recording". Caption: "Use Video Editor to change the recording quality, resolution and adjust audio settings."

**Encoding** (V: the only video codec the app links is H.264):
- MP4, H.264 Main profile at an automatic level. There is no HEVC or ProRes.
- Audio is AAC (I).
- Retina videos are tagged with `com.apple.quicktime.pixeldensity` so players show them at point size (V for the string; I for the behaviour).

### 2.3 GIF

| Control | Values | Default |
|---|---|---|
| GIF FPS: | A number field plus a slider from 5 to 60 | 15 |
| GIF quality: | A slider from 0 to 100 with 11 ticks, labelled Low to High | 80 |
| Optimize GIFs | Checkbox; runs gifsicle, a lossy pass ([CL 3.5] made it optional) | on |
| GIF size: | Original, 1200, 960, **"800 x auto (default)"**, 640, 500, 480, 320, 256 and 200 × auto | 800 |

The GIF quality slider's caption: "Setting the quality to maximum can speed up the processing time, but it will increase file size."

The GIF size caption: "Set maximum resolution of your GIFs. Changing it will affect file size and quality. CleanShot will only downscale the GIF if needed."

### 2.4 Options windows

**Highlight clicks › Options…**
- **Size:** slider from 25 to 75 (3 ticks), default 50.
- **Color:** "System Accent color" (default), Blue, Purple, Pink, Red, Orange, Yellow, Green, Gray.
- **Style:** Outline (default) or Filled.
- **Animation:** "Animate clicks", default on.
- A preview box, "Click here to preview", and an OK button.

**Keystrokes › Options…**
- **Size:** slider from 18 to 72 (7 ticks), default 45.
- **Position:** Top-Left, Top-Center, Top-Right, Bottom-Left, **Bottom-Center (default, I)**, Bottom-Right.
- **Appearance:** Dark (default) or Light.
- **Blur background:** default on.
- **Keystrokes:** "Show all keys" or "Show only command keys" (the default).
- Note: "CleanShot can't display any keystrokes you make within password fields."
- Buttons: [Preview] and [OK].
- Also: held modifiers stay visible ([CL 4.7]), and repeated presses are animated ([CL 4.2.2]).

### 2.5 Computer Audio Settings… window

- **Record Computer Audio** checkbox.
- On macOS older than 13 it adds a warning ("This is an experimental feature… update your system to macOS Ventura…") and an [Uninstall Driver] button for the old audio driver.
- **Audio merging:**
  - Intro: "Here you can adjust how CleanShot is saving your computer audio and microphone when recording both."
  - Option one: **"Record on separate tracks"**, described as "This allows you to edit the microphone audio and the computer audio separately in a video editing software."
  - Option two: **"Record on a single track"**, described as "Select this method if you don't want to edit the recorded video's audio later."
  - The default is single track (I: the `mergeAudioTracks` setting defaults to true).
- An OK button.

---

## 3. Camera overlay

**The camera menu**, built in code (V at 0x5fd67c–0x600804):
- **Devices:** the camera list, which includes virtual cameras ([CL 3.5.2]) and Continuity Camera ([DS]), plus "Don't Record Camera" (the default).
- **Size:** Small, **Medium (default, I)**, Large, Huge.
- **Full Screen:** an explanatory item, "Click on camera to toggle Full Screen".
- **Shape:** **Circle (default)**, Square, Rectangle, Vertical.
- **Options:** **Flip Camera**, on by default, so the camera is mirrored (V).

**Size** scales with the recording area ([CL 4.3]; V: the `cameraSizeAuto` setting).

**Position:**
- The camera is a floating window on screen, so it is recorded as it appears (I).
- It can be dragged anywhere, before and during recording ([TSS]: "move the video around as you record if it's suddenly in your way").
- Its position is kept (V: `cameraPositionX/Y`; I for the semantics).
- No snapping was found.
- The overlay is created while the recorder is open, so it can be placed before recording starts (I: `cameraOverlay` and `didInitCamera` belong to the recorder overlay).

**Full screen:**
- Clicking the camera, or the global hotkey "Toggle Camera Fullscreen" (unassigned), makes the camera fill the recording area. Doing it again restores it ([CL 4.3]; [BT]: "great for presentations").
- The cursor is hidden while the camera fills the area ([CL 4.6] fix).
- There was no separate camera-only mode; this toggle is the way to show only the camera (I).

**Camera off:** no overlay (V: the camera setting is empty by default).

---

## 4. While recording

### 4.1 Controls pill

The class is `RecorderControlsView` (V).

**Stop:**
- A red stop icon with the **elapsed time** inside the same button.
- The button also shows the **audio-level indicator** (`levelLayer`). It is driven by the mic and computer-audio level meters ([CL 4.6]: "audio indicator").
- Tooltip "Stop".

**Pause/Resume:** the icon swaps between the two states.

**Restart:** asks "Are you sure you want to cancel this recording and start a new one?" / "You can't undo this action.", with the buttons **[Restart Recording] [Cancel]** (V at 0x51375c).

**Delete (trash):** asks "Are you sure you want to delete this recording?" / "You can't undo this action.", with the buttons **[Delete] [Cancel]** (V at 0x5140ff).

**Drag grip:** the pill has a `recorderAnchor` icon.

**Placement:**
- The December 2024 official image shows the pill centred near the bottom of the area, with stop and timer, pause and trash.
- It can be moved ([CL 4.6]: "change recorder controls position").
- It also appears for full-screen recordings and is never captured ([CL 4.6]; V: `windowsIDToExclude`).

**With "Show controls while recording" off:** a one-time menu-bar tip says "Press to stop recording" (V: the `stopRecordingPopupDisplayed` flag; I for what triggers it).

### 4.2 Menu bar

- While recording, the icon becomes a stop icon; while paused it becomes a resume icon. Clicking it stops the recording.
- It optionally shows the elapsed time.
- During GIF conversion it shows a progress indicator.

Evidence: V for the `stopIcon`, `resumeIcon` and `gifProgressIndicator` outlets; I for exactly how they look.

### 4.3 Shortcuts › Screen Recording (the complete 4.x list, V)

| Shortcut | Scope | Default |
|---|---|---|
| Record Screen / Stop Recording | Global | none |
| Select Window | Recorder only | Space |
| Start Video Recording | Recorder only | Return |
| Start GIF Recording | Recorder only | ⌥Return |
| Pause/Resume Recording | Global | none |
| Restart Recording | Global | none |
| Toggle Camera Fullscreen | Global | none |

### 4.4 Sounds and system behaviour

- **Sounds** (V for the files; I for which plays on resume):
  - `startRecording.aif` at start, and also on resume.
  - `pauseRecording.aif` on pause.
  - `stopRecording.aif` on stop.
  - `countdown.aif` during the countdown.
- **Countdown:** 3-2-1 when "Show countdown" is on ([CL 4.6]; inventory [V-4.6]).
- **Do Not Disturb:** optional.
- **Display sleep:** prevented ([CL 3.8.1]).
- **Desktop clutter:** the desktop is hidden when the general "hide desktop icons while capturing" option is on ([F24]: "Hide clutter from your desktop when recording").
- **Outside the area:** it is dimmed by default, or shows a border when dimming is off ([CL 4.1]).
- **Paused recordings:** they dim only when dimming is on ([CL 4.7.5] fix).

---

## 5. After recording

### 5.1 Video

- When recording stops, the "Recording" column of After Capture runs (already in inventory §8.1).
  - The Video tab's "Open Video Editor after recording" checkbox is a second way to set the same thing.
- **Audio merge dialog** (V: strings and buttons at 0x4eb060–0x4ecbd1; I: it appears when both the mic and computer audio were recorded and single track is chosen):
  - It says "Merge audio to adjust the volume of each track separately:".
  - Two rows: **"Microphone volume:"** [slider] and **"Computer audio volume:"** [slider].
  - Buttons: **[Merge Audio] [Don't Merge]**, plus a "Don't ask again" checkbox.
  - While merging it shows "Merging audio...". Cancelling asks "Are you sure you want to stop the processing?" / "The audio tracks will not be merged.", with [Stop Processing] [Cancel].
  - Don't Merge asks "Do you want to continue?" / "Microphone and computer audio will remain on separate tracks."
  - Errors: "Can't merge audio tracks / Your free disk space is too low." and "The audio tracks cannot be merged. Please try again."
- **Quick Access card for a video** (V: the `ScreenRecordingPopup` class and its tooltips):
  - Thumbnail with a type badge, plus a **duration label**, a **file-size label** and a **sound indicator** ([CL 1.0.2]).
  - A preview button, "Open Quick Look (Space)".
  - An edit button, **"Trim & encode video (⌘E)"**.
  - Double-click opens the Video Editor.
  - The context menu adds **"Open Video Editor..."** for videos and **"Trim the GIF..."** for GIFs.
  - Tip texts: "Large videos can be compressed to reduce file size. Double-click on the overlay to open Video Editor." and "Double-click on the overlay to trim or compress the video."
  - Closing an unsaved recording asks "Are you sure you want to close this recording?" / "You may lose the file if it hasn't been saved.", with [Close] [Cancel] and "Don't ask for confirmation". It asks by default.
- No "convert this video to GIF" action exists in 4.x (V, by absence).

### 5.2 GIF

- The recorder captures video first and converts it to GIF after Stop (V: `recordingToGif`, `GIFUtility`, and the bundled gifsicle tool).
- **Conversion progress:** a processing card in the Quick Access stack with a progress bar and **"Cancel Processing..."**, plus progress in the menu-bar icon.
- **Cancelling** asks "Are you sure you want to stop the conversion?" / "You can delete the recording or stop the GIF conversion and save it as a video instead.", with **[Delete] [Save as a Video] [Cancel]** (V at 0x5c2055).
- **Trimming a GIF:** "Trim the GIF..." ([CL 3.3]) re-renders from the kept MP4 with gifsicle trim and optimize (V: `GIFTrimmer` keeps the source MP4 and runs gifsicle trim and optimize; I for the flow).

### 5.3 Files

- Output is MP4 (H.264) or GIF.
- Videos use the screenshot name template (inventory §8.9; I for videos).
- Any video file can be opened into the Quick Access card ([CL 3.6]), and an MP4 can be opened from the clipboard ([CL 4.6]).

---

## 6. Video Editor before 5.0

Internally this is "VideoConverter". It was added in 3.6, and its layout is the same through 4.8.11 (V: the editor NIB is identical in both builds).

**How it opens:**
- Double-click the Quick Access card, or use its edit button or ⌘E.
- Context menu: "Open Video Editor...".
- After Capture: "Open Video Editor", or the Recording › Video checkbox.
- Several editors can be open at once (V: `videoEditors`).

**Layout** (window 750×728, minimum 750×650):

**Top:** the video player.

**Trim bar** (moved under the video in [CL 3.7.1] and [CL 4.2]): a Play button, a thumbnail strip, yellow in and out handles and a red playhead (V, [IMG]).

**Left box:**
- **"Dimensions:"** popup.
  - The first entry is "W x H  (Original)". The other entries are "W x H  (~NN%)" (V: the label format).
  - The sizes come from a table of standard frames: 7680×4320, 4096², 3840×2160, 2560×1440, 2048², 1920×1080, 1280×720, 858×480, 480×360 and 352×240 (V at 0x5b7b21). Probably only sizes smaller than the original are offered (I).
  - **Width:** and **Height:** number fields. Typing in them switches the popup to "Custom" (I).
- **"Video quality:"** slider with 7 stops: **Poor, Very Low, Low, Medium, High (default), Very High, Ultra**. The current name shows to the right of the slider.
  - V: the slider runs 0 to 6 with default 4, the names come from a jump table at 0x5b9808, and the official image shows the thumb at stop 5 labelled "High".
  - [SK] corroborates the "Ultra" level.

**Right box:** **"Audio:"** radio buttons:
- **Don't change** (default)
- **Convert to mono**
- **Mute**
- **Change volume**, with a slider from 0% through 100% to 200% (ticks only at those three values; default 100%; added in [CL 3.9.1]).

**Bottom row:**
- [Cancel] (Esc).
- "Estimated file size: ~9,2 MB".
- **[Trim Only]**: cuts without re-encoding and shows "Trimming Video..." (I for the no-re-encode semantics).
- **[Trim & Convert]** (Return, the default button): re-encodes with the chosen dimensions, quality and audio, and shows "Converting Video...".

**Saving and processing:**
- The editor asks **"Do you want to replace the existing video?"**, with **[Replace Video] [Save as New Video] [Cancel]** (V at 0x44dcc3).
- The result updates the Quick Access card (V: `screenRecordingDidEdit`).
- A summary follows: "Video encoded successfully!" / "The final file size is X (Y% of the original).", with [OK] and "Don't show again". It is shown by default.
- Stopping the processing asks "Are you sure you want to stop the processing?" / "The video will not be encoded.", with [Stop Processing] [Cancel].
- The last-used encoder settings are remembered (V: `lastUsedEncoderSettings`; I for what is restored).

**Keys:**
- Space plays and pauses ([CL 4.5] fix).
- Return and Esc work as listed.
- The 4.x shortcut list has no Video Editor group (V).

**Not in the pre-5.0 editor:** cuts in the middle ([TSS]), GIF export, timeline zoom, backgrounds, zooms and cursor effects.

---

## 7. Other pre-Studio behaviour a parity clone needs

All alert texts below are exact (V); triggers are marked I where they are inferred.

**Crash recovery** ([CL 3.3], [CL 3.9.1]): a warning-emoji title, "CleanShot quit unexpectedly", then "Your screen recording has been successfully recovered."

**Low disk space:** "Your free disk space is low." / "The recording might stop unexpectedly and you may lose the file.", with [OK] and "Don't warn me again". It warns by default.
- The threshold is near 3000 (a comparison with 0xbb7 next to the alert); the unit is probably MB but unverified (I).
- There is no auto-stop in 4.x.

**Microphone muted** ([CL 4.6]): "Microphone is muted" / 'It appears that "<device>" might be muted. Would you like to continue recording?', with [Ignore] [Stop Recording] and "Don't warn me again".

**Microphone disconnected:** "Microphone Disconnected" / '"<device>" has been disconnected during the recording. Do you want to continue recording without audio or stop the recording?', with **[Continue Without Audio] [Stop Recording]**.

**Computer audio failed:** "Audio Recording Failed" / "Unable to capture computer audio. Do you want to continue…", with the same two buttons.

**Unexpected stop:** "Screen Recording stopped unexpectedly." / "An error occurred with screen recording." The title is "Recording Stopped", and it adds "It may have been caused by your microphone disconnecting during the recording."

**No microphone selected:** "To have sound in your video, you need to select a microphone first.", with [Don't Record Audio] [Cancel] (I: shown when the camera is on and no mic is selected).

**Laptop lid closed** ([CL 3.7.1]): "The built-in microphone cannot be used when the MacBook lid is closed.", with [Ignore] [Cancel] and "Don't ask again".

**Start errors:** a generic error and a DRM variant. Both are macOS-only, as are the permission texts for the microphone, the camera and Accessibility (for keystrokes).

---

## 8. Where Snip's spec differs from pre-5.0 CleanShot

- **REC-08 (webcam):** add the Vertical shape, the four sizes Small/Medium/Large/Huge scaled to the recording area, mirroring on by default, a remembered position, and the "Toggle Camera Fullscreen" hotkey.
- **REC-12 (video):** CleanShot capped frame rate at 60 (10, 15, 24, 25, 30, 48, 50, 60) and used H.264 only. Snip's "native ≤ 144" and HEVC/AV1 are extensions. Each is a product decision to keep or drop.
- **REC-07 (audio):** the post-recording merge dialog, with a volume slider per track, is new.
  - gpu-screen-recorder covers both track layouts: repeating `-a` gives separate tracks, and `-a "a|b"` gives one mixed track.
  - The merge with per-track volume would be an FFmpeg job after recording (local gsr man page).
- **REC-13 (GIF):** add the exact GIF settings, the conversion card with Cancel, the "Save as a Video" choice, and GIF trimming.
- **VED-02 / VED-03:** export as GIF and cuts in the middle are not pre-5.0 CleanShot.
  - The owner's "cut, trim, mute or merge audio" maps as follows: trim is the editor's in and out handles; mute is the editor's Audio radio; merging audio happened in the post-recording merge dialog, not in the editor.
  - "Trim Only" versus "Trim & Convert", and the encoding summary, are also new to the spec.
- **REC-05 (controls):** CleanShot let the pill sit inside the recorded area because macOS excluded it from capture. Snip's "always outside the region" is a deliberate difference already recorded in spec §6.
- **REC-14 (safety):** CleanShot 4.x warned about low disk space and did not stop automatically. It also offered "Continue Without Audio" when the mic or computer audio failed.

---

## 9. Summary table

| Feature | CleanShot behaviour (3.x to 4.8.11) | Snip spec | Conf. | Source |
|---|---|---|---|---|
| Ways to start recording | Menu "Record Screen"; global "Record Screen / Stop Recording" (unassigned, toggles); All-In-One R; URL `record-screen` with x, y, width, height, display | REC-01, API-01, OMA-08 | V/D | BIN menu NIB, [API] |
| Area selection | 8 handles, edge snapping, ⌥ uniform, arrows with ⌘/⇧, typed W×H in points | REC-01, SEL-08 | V/D | [IMG], CL 3.1.2/3.8/3.9.4 |
| Window selection | Space (recorder-only, rebindable) snaps the area to a window; hint "Press Space to select a window." | REC-01 (area = window rectangle matches 4.x) | V/D | BIN 0x36ce50, 0x526945; [DS] |
| Full screen | ⤢ "Toggle fullscreen" button; Return with no area records the display | REC-01, SEL-09 (button: NEW) | V/D | BIN, CL 4.2.2 |
| Aspect-ratio lock | Ratio button with dropdown; last ratio remembered | REC-01, SEL-08 | V/D(I list) | BIN, CL 4.1, [9to5] |
| Remember last selection | Setting, default off | REC-01 | V | BIN defaults |
| One display per recording | Display under the cursor; others not dimmed | REC-01 | I/D | [API], CL 3.3.1 |
| Toolbar | Settings, W×H, ⤢ / Mic, Computer audio, Camera, Highlight Clicks, Show Keystrokes / Record GIF ⌥↩, Record Video ↩ | REC-02 | V | [IMG], BIN 0x3c6000 |
| Mic level meter in toolbar | Live meter on the mic button | NEW (REC-02 detail) | V/D | CL 3.9.4, BIN `vuMeter` |
| Recorder keys | Return video, ⌥Return GIF, Space window, Esc cancel | SEL-09 (⌥Return GIF: NEW) | V | BIN shortcut table, [HK], [IMG] |
| Recording settings General tab | Controls on, menu-bar time off, Retina 1x off, Do Not Disturb off, cursor on, clicks off, keys off, remember area off, dim on, countdown off | REC-03/04/05/06/11 | V | BIN NIB and defaults |
| Show cursor toggle | Setting, default on | NEW | V | BIN |
| Max resolution | Original (default), 4k, 1440p, 1080p, 720p, 480p | REC-12 | V | BIN NIB, 0x5ba6c8 |
| Frame rates | 10, 15, 24, 25, 30, 48, 50, 60; default 60 | REC-12 (differs) | V | BIN NIB, getter 0x53b200 |
| Codec | MP4, H.264 Main only; Retina pixel-density tag | REC-12 (HEVC/AV1 are Snip extras) | V | BIN imports |
| Mono | "Record audio in mono", off | REC-07 | V | BIN |
| Track layout | Separate or single track (default single), in Computer Audio Settings… | REC-07 | V/I | BIN NIB |
| Post-recording merge dialog | Mic and computer volume sliders; Merge Audio / Don't Merge; Don't ask again | NEW | V/I(trigger) | BIN 0x4ec4c9 |
| GIF settings | FPS 5–60 (15); quality 0–100 (80); Optimize on; size list (800 x auto default) | REC-13 | V | BIN NIB and getters |
| GIF conversion | Converted after Stop; card with "Cancel Processing..."; Delete / Save as a Video / Cancel | NEW (REC-13 detail) | V | BIN 0x5c2055 |
| GIF trimming | "Trim the GIF...", re-rendered from the kept MP4 with gifsicle | NEW | V/I | BIN, CL 3.3 |
| Click highlight options | Size 25–75 (50); 9 colours with System Accent default; Outline/Filled; Animate on; preview | REC-09 | V | BIN NIB |
| Keystroke options | Size 18–72 (45); 6 positions (Bottom-Center); Dark/Light; Blur on; all keys or command keys only; no password fields | REC-10 | V/I | BIN NIB and getters |
| Camera shapes | Circle (default), Square, Rectangle, Vertical | REC-08 (Vertical: NEW) | V/D | BIN 0x5fd67c, [DS] |
| Camera sizes | Small, Medium, Large, Huge; scale with the area | REC-08 (detail NEW) | V/D | BIN, CL 4.3 |
| Camera mirror | "Flip Camera", default on | REC-08 (default NEW) | V | BIN getter |
| Camera position | Drag anytime; position remembered | REC-08 | D/I | [TSS], BIN keys |
| Camera full screen | Click, or "Toggle Camera Fullscreen" hotkey; fills the area | REC-08 (hotkey NEW) | D/V | CL 4.3, [BT], BIN |
| Controls pill | Stop with timer and level meter; Pause/Resume; Restart; Delete; drag grip | REC-05 | V/D | BIN, [IMG], CL 3.8/3.9.1/4.6 |
| Restart and delete confirmations | Exact texts and buttons (§4.1) | NEW (REC-05 detail) | V | BIN 0x51375c, 0x5140ff |
| Pill placement | Movable, shown in full screen, never captured | REC-05 (Snip places it outside on purpose) | D | CL 4.6 |
| Menu-bar state | Stop/resume icon, click to stop, optional time, GIF progress, one-time "Press to stop recording" | REC-06 (GIF progress: NEW) | V/I | BIN |
| Global hotkeys | Record/Stop, Pause/Resume, Restart, Toggle Camera Fullscreen (all unassigned) | Spec §7 (partly NEW) | V | BIN table |
| Sounds | Start, pause, stop, countdown | SET-09 (countdown: NEW) | V | BIN |
| Countdown | 3-2-1, default off | REC-03 | D/V | CL 4.6, BIN |
| Dim or border | Dim by default; border when off; paused recordings dim only if on | REC-04 | D | CL 4.1, CL 4.7.5 |
| Do Not Disturb, display sleep, desktop clutter | DND optional; sleep prevented; clutter hidden | REC-11, CAP-12, sleep: NEW | D | CL 3.8.1, [F24] |
| After Capture for recordings | Quick Access, copy, save, upload, open editor; plus "Open Video Editor after recording" | SET-03 | V | BIN, inventory §8.1 |
| Video Quick Access card | Duration, size, sound icon, Quick Look, "Trim & encode video (⌘E)", double-click opens editor, context items | QAO-02/03/05 (labels: NEW) | V | BIN `ScreenRecordingPopup` |
| Close-unsaved confirmation | "Are you sure you want to close this recording?" | Not needed (spec §5.0 keeps history) | V | BIN 0x592936 |
| Editor trim | AVKit thumbnail strip, yellow handles, Play, Space; start and end only | VED-01 (VED-03 cuts are post-5.0) | V/D | [IMG], [TSS], CL 4.5 |
| Editor dimensions | "W x H (Original)" and "W x H (~NN%)" presets, Width/Height, Custom | VED-02 | V/I | BIN NIB, 0x5b7b21 |
| Editor quality | Poor, Very Low, Low, Medium, High (default), Very High, Ultra | VED-02 | V | BIN NIB and 0x5b9808, [IMG], [SK] |
| Editor audio | Don't change, Convert to mono, Mute, Change volume 0–200% | VED-02 (mono and volume: NEW) | V | BIN NIB, CL 3.9.1 |
| Editor output | Estimated size; Trim Only or Trim & Convert; Replace Video, Save as New Video, Cancel; summary dialog | VED-02 (Trim Only and summary: NEW) | V | BIN 0x44dcc3, [IMG] |
| GIF export from the editor | Not available before 5.0 | VED-02 lists it (Snip extra) | V | BIN, by absence |
| Crash recovery | "Your screen recording has been successfully recovered." | REC-14 | V/D | BIN, CL 3.9.1 |
| Low disk space | Warning with "Don't warn me again"; no auto-stop | REC-14 (Snip: auto-stop) | V | BIN 0x5c402d |
| Mic and audio failures | Muted (Ignore/Stop); disconnected or audio failed (Continue Without Audio / Stop) | REC-14 ("continue without audio": NEW) | V | BIN 0x5c2c0e, 0x5c32ad, 0x5c3a77 |
| No mic selected; lid closed | "select a microphone first"; built-in mic with lid closed | NEW (low value) | V/I | BIN 0x3c9d2f, CL 3.7.1 |

Everything is in `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research/`:
- `CleanShot-X-4.7.6.dmg`, `CleanShot-X-4.8.11.dmg` (sources)
- `cs476.apfs`, `cs4811.apfs` (decoded images)
- `nibs476/`, `nibs4811/` (parsed NIBs)
- `tree_recprefs476_all.txt` (Recording preferences tree)
- `nibs476/nib_00339a000.txt` (Video Editor)
- `s476.txt` (strings)
- `img/` (official images)
- `changelog.txt`
- the scripts: `udif.py`, `nibparse.py`, `nibtree.py`, `nibtree2.py`, `strrefs.py`, `ctx.py`, `defaults.py`, `boolgetters.py`