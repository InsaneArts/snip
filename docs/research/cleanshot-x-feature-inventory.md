# CleanShot X: feature and UX inventory (latest version 5.0.1, 18 Sep 2026)

## 0. Sources and confidence legend

The tags below are used throughout. If a bullet has no "(unverified)" mark, at least one tagged source supports it.

- **[F]** https://cleanshot.com/features
- **[CL]** https://cleanshot.com/changelog (complete list from 1.0 to 5.0.1)
- **[API]** https://cleanshot.com/docs-api (also at /docs/api)
- **[FAQ]** https://cleanshot.com/faq
- **[SS]** https://cleanshot.com/screenshots
- **[SR]** https://cleanshot.com/screen-recording
- **[S1–S14]** The 14 official settings screenshots from the 5.0 redesign, at https://cleanshot.com/img/features/settings1.png through settings14.png. They show the current settings UI. The toggle values in them are marketing values, which are probably close to the defaults but not guaranteed.
- **[IMG]** Official feature images on cleanshot.com:
  - /img/screenshots/all-in-one.png
  - /img/features/quick_actions.png
  - /img/features/drag_and_drop.png
  - /img/screenshots/history.png
  - /img/recording/quick-video.png
  - /img/recording/studio-mode.png
  - /img/screenshots/header.png (Annotate toolbar and Background tool)
  - /img/features/color-picker.png
  - /_nuxt/show_magnifier_2x.BPGhA_D2.png
  - /img/home/more/pin.png
- **[V-x]** Official CleanShot YouTube videos (narrated captions pulled):
  - First steps: youtu.be/F4DMNp-Vt3A
  - 4.1: ZXKZsQ4nU8Y
  - 4.2: nZsyNyzSAVw
  - 4.3: soqr1EfQA44
  - 4.4: FNHt4WUa13w
  - 4.5: GOjV01brFQg
  - 4.6: WOQsFWzOWlE
  - 4.7: JyqCSh9m9oc
  - 4.8: o5ypDElAMto
  - 5.0: fUB1s3RaD7c
- **[APP]** UI strings, preference keys, NIB labels and asset names taken from the official 5.0.1 DMG at https://updates.getcleanshot.com/v3/CleanShot-X-5.0.1.dmg. Its SHA-256 (85ab95c8…) matches the Homebrew cask. The app was not run. These are vendor-primary for labels and option names. Behavior inferred only from a label is marked "(inferred)".
- **Secondary sources:**
  - **[PF26]** podfeet CleanShot 5 review: https://www.podfeet.com/blog/2026/09/cleanshot-5/
  - **[PF22]** https://www.podfeet.com/blog/2022/04/cleanshot-x/
  - **[ALC]** https://alchemists.io/articles/clean_shot/
  - **[PIE]** https://www.pie-menu.com/shortcuts/cleanshot
  - **[DS]** https://daveswift.com/cleanshot-x/
  - **[LC]** https://leancrew.com/all-this/2025/01/cleanshot-x-and-retrobatch/
  - **[RC]** https://www.raycast.com/Aayush9029/cleanshotx
- **Platform facts:**
  - Requires macOS 13 or later [FAQ]. Runs as a menu-bar-only app (LSUIElement) [APP].
  - The bundle ID is pl.maketheweb.cleanshotx, and it registers the URL scheme `cleanshot` [APP].
  - Document types: it opens PNG/JPG/WebP/HEIC/HEIF and any movie. It owns `.cleanshot` ("CleanShot Document") and `.cleanshotvideo` ("CleanShot Video Project", a package) [APP].

---

## 1. Capture modes

### 1.1 Capture Area
- Opens a selection overlay. The hint reads "Drag to select capture area." [APP].
- In the plain Capture Area mode, releasing the drag captures at once, like macOS ⇧⌘4. There is no adjust step (unverified, but consistent with all walkthroughs). The adjustable selection with a toolbar is All-In-One (§1.9).
- The result goes through the configured **After Capture** actions (§8.1).
- Variant shortcuts each have their own hotkey [S3][APP]:
  - Capture Area & Copy to Clipboard
  - Capture Area & Save
  - Capture Area & Upload to Cloud
  - Capture Area & Annotate
  - Capture Area & Pin to the Screen
  - Capture Area & Send to Raycast AI Chat
- A setting decides whether these variants also run the After Capture actions: "Ignore 'After Capture' actions when using 'Capture Area & …' shortcuts" [APP][CL 4.3.1].
- **Capture Previous Area** repeats the last area rectangle with no UI [F][CL 3.3.1].
- The URL API accepts `x, y, width, height, display` for an instant capture with no UI [API].

### 1.2 Capture Window
- Hovering highlights the window under the cursor and a click captures it [CL 3.4.3]. While in Capture Area, pressing **Space** switches to window mode ("CleanShot has always allowed you to press space to capture a single window" [V-5.0]).
- In All-In-One the window mode is labeled "Capture Window (Space/W)" [APP].
- **Background modes** [S6][S7][F]:
  - **With wallpaper:** adds padding (slider) and puts the window on the desktop wallpaper, a custom image, or a plain colour. The source is set in the Wallpaper tab.
  - **Transparent:** keeps the alpha channel. A "Capture window shadow" toggle sits alongside.
  - Holding **⇧ Shift** while capturing inverts the choice. The help text reads "Hold ⇧ Shift while taking a screenshot to get a transparent background" or "…to capture the window with wallpaper" [APP][S6].
- The shadow is tied to the padding: "Tip: You need to disable shadow as well if you don't want any padding." [APP]. The minimum wallpaper padding is fairly large, about 110 px [LC]. There is a "Reset Defaults" button [S7].
- **Editable window screenshots (4.8):** the background can be changed or removed later in Annotate. It can be blurred, swapped, removed, or added to a transparent capture [CL 4.8][V-4.8]. Some Background-tool options show "These options are unavailable for window screenshots." [APP].
- The window's rounded corners are kept. A sharp-corner bug was fixed in 4.8.8 [CL].
- Compatible with TopNotch [CL 3.9.4].
- Whether occluded windows are captured unobscured, and how multi-window or menu captures work, is unknown (unverified).

### 1.3 Capture Fullscreen
- One-shot capture of a full display [F].
- "Show cursor on screenshots" applies only in Fullscreen and Self-Timer modes [S6].
- "Auto-crop Notch from screenshots of fullscreen apps" [CL 4.6].
- Which display is captured when there are several (cursor display or all) is unknown (unverified).

### 1.4 Scrolling Capture (vertical and horizontal)
1. **Trigger:** shortcut, menu, All-In-One "S", or URL `scrolling-capture` with `start` and `autoscroll` parameters [API].
2. **Hint:** "Drag to capture the scrolling part of the screen." [APP].
3. **First-run onboarding** [APP]. It has three cards:
   - "How to select an area?": only scrollable content; no fixed parts and no scroll bar inside the selection.
   - "Slowly scroll down the content": a live preview updates as you scroll.
   - "Scroll Vertically or Horizontally": you cannot switch direction during a capture.
   - It also lists conditions that break stitching: animations or videos in the area, scrolling that is not straight, scrolling too fast, and starting with an upward scroll.
4. **Control buttons** [APP asset names]:
   - Start Capture
   - Auto-Scroll (vertical) and Auto-Scroll horizontal
   - Help
   - Cancel
   - Done
   - The 4.8 UI asks whether the content is horizontal before auto-scrolling [V-4.8].
5. **Live feedback while capturing:**
   - A real-time stitched preview.
   - A "Please slow down..." warning.
   - A "⚠ Screenshot is very large" warning [APP][CL 4.5.1]. The actual size limit is unknown (unverified).
6. **Auto-Scroll** (added 4.4):
   - CleanShot scrolls by itself until you stop it or the content ends.
   - Moving the cursor out of the area **pauses**; moving it back **resumes**. The hint reads "Move cursor here to start scrolling" [V-4.4][APP].
   - Needs the Accessibility permission ("Auto-Scroll needs … permission") [APP].
7. **Hotkeys:** "Scrolling Capture" and "Start/Stop Capturing" [APP][PIE].
8. **Output:** the result goes to After Capture or the QAO. Tall captures print across several pages, with a "Scale to fit on one page" option in the print dialog [CL 4.8][V-4.8].

### 1.5 Self-Timer
- Opens Capture Area with a self-timer [API]. A countdown overlay follows (SelfTimerOverlay [APP]).
- The interval is set in Screenshots > Self-Timer interval. The screenshot shows 5 seconds [S6]. The range is said to be 2–15 s [PF22] (unverified as primary).
- Captures the cursor if "Show cursor on screenshots" is on [S6].
- Also available in All-In-One as "Timer" (key T) [IMG][APP].

### 1.6 Capture Text (OCR)
- Select an area. The text is recognized **on-device** and copied to the clipboard [F].
- **Feedback** [APP]:
  - Success: "Text has been copied" toast plus an OCR sound.
  - No result: "No text detected".
- **QR codes** (added 4.6):
  - The tool decodes the code and shows the URL.
  - Dialog "Do you want to open this link?" with "Open Link in Browser", "Never open links" or cancel [V-4.6][APP].
- **Settings** (Advanced > Text Recognition) [S12]:
  - **Language:** "Automatically Detect Language" (added 4.8) or a fixed primary language.
  - **Keep line breaks.**
  - **Detect links:** link detection, which can be turned off since 4.5 [CL].
- **Separate hotkeys:** Capture Text, Capture Text With Line Breaks, Capture Text Without Line Breaks [APP][CL 4.0.1].
- **Languages** [APP locale list][CL]:
  - English, French, Italian, German, Spanish, Portuguese (BR)
  - Chinese (Simplified and Traditional), Korean, Japanese
  - Russian, Ukrainian, Thai, Vietnamese
  - Arabic, Arabic (Najdi), Turkish, Indonesian
  - Czech, Danish, Dutch, Norwegian (Bokmål and Nynorsk), Malay, Polish, Romanian, Swedish
  - Some languages need a newer macOS.
  - The marketing claim is "30+ languages" [F].
- **Other entry points:**
  - All-In-One "OCR" (key O)
  - URL `capture-text`, with an optional `filepath` (OCR on an existing file), area, display and `linebreaks` [API]
  - The pinned-screenshot context menu has "OCR" [CL 4.1]
  - The QAO context menu has "Extract text" [APP] (placement inferred)
- The onboarding blurb says it reads "images, videos, PDFs, webpages, photos and even QR codes" [APP].

### 1.7 Freeze screen
- Option: "Freeze screen when taking a screenshot" [S6].
- The screen becomes a static image while you select, so moving objects, hover states and menus can be captured [F].
- Works with All-In-One. A hover-state bug was fixed in 4.8.5 [CL].

### 1.8 Hide desktop icons / clean desktop
- Option: "Hide desktop icons while capturing" [S1]. It also hides widgets (4.7) [CL].
- Implemented as a wallpaper cover. Wallpaper tab options [S5][APP]:
  - Desktop wallpaper
  - Custom wallpaper (image)
  - Plain color
  - "Update wallpaper when switching Spaces"
- The same wallpaper is used as the background behind window screenshots [S5].
- Global **Toggle Desktop Icons** hotkey and a menu-bar item "Hide/Show Desktop Icons" [APP][MainMenu.nib].
- URL commands: `toggle-desktop-icons`, `hide-desktop-icons`, `show-desktop-icons` [API].
- Files can still be dropped on the desktop while icons are hidden [CL 1.3].
- Marketing line: "Hide clutter from your desktop when recording" [F].

### 1.9 All-In-One mode
- One hotkey opens a floating toolbar [IMG all-in-one]:
  - Mode buttons: **Area · Fullscreen · Window · Scrolling · Timer · OCR · Recording**
  - A separate panel with **W × H number fields** and an **aspect-ratio / crop dropdown**
- Mode hotkeys inside All-In-One (4.8) [APP]:

  | Key | Mode |
  |---|---|
  | A | Area |
  | F | Fullscreen |
  | Space or W | Window |
  | S | Scrolling |
  | T | Self-Timer |
  | O | Capture Text |
  | R | Record Screen |

- ⌘C copies at once (added 4.5) [CL].
- Lets you type an exact size and lock an aspect ratio [F][V-4.2].
- **Remembers the last selection**, so the same area can be retaken repeatedly. Setting: Advanced > All-In-One > Remember last selection [S13]. The last chosen ratio is also remembered (oneOverlayLastSelectedRatio [APP]).
- URL `all-in-one` accepts `x, y, width, height, display` [API].
- The onboarding offers to assign the macOS default shortcut to All-In-One ("Do you want to assign system default shortcut to All-In-One mode?") [APP]. This is presumably ⇧⌘5 (inferred).

### 1.10 Cursor in captures
- Screenshots: cursor only in Fullscreen and Self-Timer [S6].
- Recordings: "Show cursor" toggle [S8].

---

## 2. Area selection UI

- **Crosshair mode** (Settings > Screenshots > Crosshair) [S7][APP]:
  - Options: "Disabled", "Always enabled", "When ⌘ Command is pressed".
  - When on, full-screen crosshair lines follow the cursor with an **x/y coordinate readout** next to the cursor [IMG magnifier].
- **Magnifier (loupe):** "Show magnifier". It needs Crosshair Mode ("To enable magnifier, you must enable Crosshair Mode first.") [S7].
  - It is a rounded-square loupe offset below and right of the cursor, with a pixel grid and centre crosshair [IMG].
- **Dimensions label:** a pill with W × H (e.g. "189 x 150") near the selection [S7 illustration].
- **Modifiers while capturing:**

  | Key | Effect | Source |
  |---|---|---|
  | Space | Switch to window selection | [V-5.0][APP] |
  | Control (held) | Copy the result to the clipboard instead of the normal actions | [CL 4.2.2][ALC] |
  | Option (held) | Uniform (square) selection; for recording "select screen recording area uniformly" | [CL 3.9.4][ALC] |
  | ⌘ | Shows crosshair and magnifier when the mode is "When ⌘ pressed" | [ALC][APP] |
  | Shift | Skip an auto-applied Background preset | [CL 4.8.4][S6] |
  | Shift (window capture) | Invert the wallpaper/transparent choice | [S6] |

- Not confirmed as macOS-style during the drag (unverified):
  - Space to move a selection in progress
  - Shift to lock one axis
  - Option to resize from the centre
- **Arrow keys:**
  - The recording area moves and resizes with the arrow keys, using ⌘ and ⇧ modifiers [CL 3.1.2].
  - Arrow nudging for screenshot or All-In-One selections (unverified).
- **Snapping:** the recording area snaps to edges [CL 3.8].
- **Window detection:**
  - Hover highlight exists in window mode [CL 3.4.3].
  - No automatic element or window snapping in plain area mode. The settings page upsells PixelSnap for that: "Automagically select objects to capture with PixelSnap" [S7].
- **Esc:** cancels any capture or selection overlay (universal; also see [CL 3.8] "Fixed ESC key recording stop").
- **Enter:**
  - In the Screen Recorder, Enter with no area starts a **full-screen recording** [CL 4.2.2 and 3.5.1 fixes].
  - Enter for All-In-One or history acts as confirm or restore (unverified for All-In-One).
- **Multi-display:**
  - The URL API takes `display` (1 = main, 2 = secondary) [API].
  - Recording does not dim a second screen [CL 3.3.1].
  - Whether a selection can span displays (unverified).
- **Recording area** extras:
  - Handles to resize after the drag
  - Typed exact size
  - Aspect-ratio lock (16:9 and others)
  - "Remember last selection"
  - Dimming outside the area, or a red border instead [V-4.1][S8][DS]

---

## 3. Quick Access Overlay (QAO)

- **When and where it appears:**
  - A small floating thumbnail card in the bottom corner after each capture or recording [F][PF22].
  - Position on screen: **Left or Right** [S4][APP].
  - **Move to active screen:** "Active screen is the screen where your cursor is pointing." [S4]
  - **Overlay size:** a 5-stop slider [S4].
  - A subtle entrance animation was added in 4.7 [CL].
- **Hover state** [IMG quick_actions][IMG drag_and_drop][V-first]:
  - Centre pill buttons **Copy** and **Save**.
  - Four corner icons: **Close (×)**, **Pin**, **Annotate (pencil)** and **Upload (cloud)**. For videos the edit icon is a trim/video-editor icon (popupTrim [APP]).
  - Close moved to the **left** "to be consistent with macOS" [APP tip][CL 4.1]. The exact current corner of the pin icon is unverified.
  - Video and GIF thumbnails carry type badges and a sound icon (popupGifFile, popupVideoFile, popupWithSound [APP]).
  - With auto-save on, a **Trash** button appears [CL 4.5].
- **Keyboard while hovering** [V-4.1][ALC][V-first][CL 1.1]:

  | Key | Action |
  |---|---|
  | ⌘C | Copy |
  | ⌘S | Save |
  | ⌘W | Close |
  | ⌘U | Upload |
  | ⌘E | Annotate / edit |
  | Space | Quick Look ("Open Quick Look (Space)") |

- **Option-key variants:**
  - ⌥ + Copy copies without closing [CL 4.8].
  - ⌥ + Save inverts the "Save button behavior": Save to "Export location" vs "Ask for destination" [S4].
  - ⌥ while dragging keeps the item ("Close after dragging": "Hold ⌥ (option) to keep the item on Overlay after dragging") [S4].
- **Mouse:**
  - **Double-click** opens Annotate for images and the Video Editor for videos [APP tips].
  - **Drag and drop** the thumbnail into any app. A file is created with the name template ("CleanShot 2021-03…@2x") [IMG drag_and_drop].
  - "Close after dragging" and "Close after uploading" toggles [S4].
- **Swipe gestures** [APP tips][CL 2.4][CL 3.4]:
  - Two-finger swipe sideways **dismisses** that capture.
  - **Swipe down ↓** temporarily hides all overlays. The hotkey "Hide/Show Overlays" brings them back.
- **Right-click context menu** [IMG quick_actions][APP][CL]:
  - Open Annotation Tool…
  - Pin to the Screen
  - Rotate Left
  - Flip Horizontal
  - Scale Retina to 1x
  - Resize…
  - Extract text
  - Upload to Cloud
  - Quick Look
  - Show in Finder
  - Open With ▸
  - Open in Mail…
  - Save As…
  - Print…
  - Move to Trash
  - Send to Raycast AI Chat…
  - Temporarily Hide
  - Close
  - Close All…
  - Save All…
- **Stacking:**
  - Several captures stack as separate cards.
  - A "Newest screenshot indicator" was added in 4.7 [CL].
  - Hotkeys "Save All Overlays" and "Close All Overlays". Close All asks "Do you want to close all overlays? Your media will be lost if you didn't save it." [APP]
  - The stacking direction and maximum count are unverified.
- **Auto-close** [S4][CL]:
  - Toggle.
  - Action: "Save and Close" or "Close".
  - Interval: the screenshot shows 30 seconds. Short intervals were added in 3.1.1, plus 5 and 10 minutes in 3.9.1. The full list is unverified.
- **Restore recently closed:**
  - Hotkey "Restore Last Capture" and URL `restore-recently-closed` [API].
  - Superseded by Capture History, which can restore anything [V-4.4].
- **Edits round-trip:**
  - "Done" in Annotate updates the QAO thumbnail [V-first].
  - Captures reopened from the QAO stay editable [CL 4.0].
- **Nothing is written to disk until you act**, unless "Save" is an After Capture action [V-first].
- Extra notes: "Display additional file information" [F] (the content is unverified). External files can be pushed to a QAO via URL `add-quick-access-overlay?filepath=` for PNG, JPEG or MP4 [API].

---

## 4. Annotate editor

### 4.1 Window and chrome
- **Window behavior:**
  - A resizable native window [CL 3.9]. It can be dragged from the bottom bar [CL 4.5.1] and opens on the correct screen [CL 4.2.2].
  - Settings: **Always on top** and **Show Dock icon** [S10].
  - Dark and light mode [F].
- **Toolbar, left to right** [IMG header]:
  - Group 1: **Crop & Resize**, **Add image (screenshot)**, **Background tool**
  - Tools: **Move**, Rectangle, Filled rectangle, Ellipse, Line, Arrow, Text, Redact (Pixelate), Spotlight, Counter, Draw (pencil), Highlighter
  - Right side: **Save as…**, then the primary action
- **Bottom bar** [PF22][APP]:
  - **"Drag me"** handle to drag the result out. ⌥-drag keeps it pinned [CL 4.5.1].
  - Share sheet, Copy, Upload to Cloud, Pin, Send to Raycast AI Chat.
  - Zoom menu and **Lock canvas** toggle [CL 3.5].
- **Zoom:**
  - Menu entries: Fit Canvas, 50%, 100%, 200%, Zoom In, Zoom Out.
  - ⌘+ / ⌘−; ⌘1 = 100% [V-4.3].
  - Trackpad scroll pans; hold **Space and drag** to pan [CL 3.9.3][V-4.3].
- Transparent areas show a checkerboard [CL 3.1].

### 4.2 Tools and their options

| Tool (hotkey per [PIE]) | Options and behavior |
|---|---|
| Move/Select (V) | Select, move and resize objects with handles. ⇧-drag locks the axis (4.7). ⌥-drag duplicates; ⌥⇧ duplicates on a locked axis [V-4.7]. ⌘C/⌘V copy and paste objects, also between images (4.4). ⌘D duplicates. Delete removes. |
| Crop & Resize (K) | **Crop:** ratio menu with Freeform, Original Ratio, 1:1 (Square), 4:3, 3:2, 16:9, 5:4, 9:16 (4.8.1), Custom Ratio. "Snap to edges"; hold ⌘ to toggle snapping. Cropping **outward expands the canvas** and fills it with the **auto-detected background colour** (4.5). **Rotate** and **Flip** buttons (4.8). **Image size:** W/H in **pixels or percent**, proportion lock, Resize. Resizing offers "Flatten Annotations (Can't be undone)" [APP NIB][V-4.7]. |
| Background (B) | See §4.4. |
| Rectangle (R), Filled rectangle (F), Ellipse (E), Line (L) | Colour, thickness, optional object shadow (global "Draw shadow on objects"). Shift gives perfect shapes or straight lines [CL 2.5]. Corner roundness exists in the model (cornerRoundnessPercent [APP]; UI unverified). |
| Arrow (A) | 4 styles: **Standard, Fancy, Curved, Double** [APP]. Thickness. Curved arrows have a draggable control point. "Inverse arrow direction" setting; **⌥ while drawing** inverts it [S10]. Last style is remembered. |
| Text (T) | **7 styles:** Standard, Box, Rounded, Rounded Box, Outline, Monospaced, Monospaced Box. Box backgrounds use a light tint of the text colour [PF22]. Font-size menu. **Drag the corner square to resize the text** (4.6). Emoji button (smiley) opens Emoji & Symbols. Text wraps at the image edge; long text becomes a left-aligned paragraph (4.7). No arbitrary fonts [PF22]. |
| Redact / Pixelate (P) | Styles: **Pixelate, Blur (smooth), Blur (secure), Black Out**. Intensity is adjustable, also from the keyboard [CL 3.1.1]. Pixelate and secure blur are randomized to resist reversal (3.5 and 4.2). |
| Spotlight (H) | Dims everything outside one or more regions. Shapes: rounded rectangle and ellipse ("Spotlight shape"). Dim opacity adjustable (spotlightOpacity [APP]). |
| Counter (C) | Numbered step markers with styles **1,2,3 / A,B,C / I,II,III**. Starting number can be 0 or higher. Size. **Renumber automatically on delete**; always drawn on top (4.7.5) [PF22][CL]. |
| Draw / Pencil (D) | Free-hand drawing with **auto-smoothing** (setting "Smooth drawing") [S10]. |
| Highlighter (M) | **Smart Highlighter** (4.7): detects text lines, sizes the brush to the text height, snaps to word starts and ends, and uses a rounded-rect cursor. ⌘ disables snapping (4.7.3). Opacity slider. Highlight shape setting. |
| Image objects | Drag and drop another image in, or use "Add screenshot" with Take Screenshot (⇧⌘I), Choose From File… (⌘I) or Paste From Clipboard. **Drop zones** with arrows place it above, below, left or right; **snapping guides** align it; the canvas grows to fit (4.8) [V-4.8][APP]. |

- **Colours** (4.8 Color Picker) [IMG color-picker][V-4.8][APP]:
  - 10 default swatches: black, red, orange, yellow, green, teal, blue, purple, pink, white.
  - Up to **10 saved "My Colors"**. They can be reordered by drag and deleted ("You've reached the limit of saved colors").
  - Expandable panel with a saturation/brightness square, hue slider, **alpha** slider, Hex field, R/G/B/Alpha fields, an **eyedropper that picks from anywhere on screen**, and "Add to My Colors".
  - "Show color names" accessibility setting.
- **Tool size:** "Increase/Decrease Tool Size" hotkeys. Keys 1–6 set object size [CL 2.6] (current mapping unverified; [PIE] lists odd keys).
- **Auto-expand canvas:** "Automatically expand canvas – ensures all annotations fit by dynamically resizing canvas" [S10]. Drawing outside the image grows a transparent canvas [PF22].
- **Undo/Redo:** ⌘Z / ⇧⌘Z [CL 2.1].
- **Rotate and flip** from the crop panel and the QAO menu.
- **No** automatic sensitive-data redaction, blur-faces feature or OCR translation exists. None appears on the site, in the changelog or in the app strings.

### 4.3 Output, saving and the project format

| Action | Shortcut [PIE] / behavior |
|---|---|
| Save | ⌘S, to the export location |
| Save As | ⇧⌘S. Formats include **CleanShot Project (.cleanshot)**, PNG, JPG and WebP. "Select 'CleanShot Project' if you want to edit the screenshot later." The last save folder is remembered. Hold ⌥ to bypass the Save-as dialog [CL 4.1]. |
| Copy screenshot | ⇧⌘C. Closes the editor; ⌥ keeps it open (4.8). |
| Upload | ⌘U |
| Print | ⌘P |
| Raycast AI Chat | ⌘R |
| Close / cancel | ⌘W [CL 2.6] |
| Done | Pushes the edits back to the QAO |

- Closing unsaved work asks "Do you want to save the changes you made to the screenshot?" [APP].
- **The `.cleanshot` format** is non-destructive. It stores the image, the objects and the global settings: document.json, mainImage, objects, imageFormat, retina factor, preview/thumbnail [APP]. Annotations stay editable after reopening [F][CL 4.0].
- There are **Quick Look and thumbnail extensions** for the format [APP PlugIns]. Newer-version files show a "Compatibility warning" [APP].
- HEIC can be **opened** but not saved [V-4.8].

### 4.4 Background tool (4.5 and later)
- **Preset bar:**
  - Dropdown with **None**, saved presets and "Apply Previous Settings".
  - "Add New Preset…" with a name prompt, "Save changes to the preset", trash and + buttons [IMG header][APP][V-4.6].
- **Auto-apply:**
  - "Automatically apply preset to all screenshots", also set in Settings > Screenshots > Background preset (4.7) [S6].
  - **⇧ during capture skips it** (4.8.4).
- **Sources** [IMG header][V-4.5]:
  - **Gradients:** 20 built in, with Show more / Show less.
  - **Wallpapers:** current wallpaper plus custom images added with + or by drag and drop.
  - **Blurred:** blurred wallpaper and blurred screenshot.
  - **Plain color:** swatches, transparent, and a custom colour/hex picker.
  - "Remember if background tool was opened" setting [S10].
- **Layout controls:**
  - **Padding**, **Inset** with an inset colour, **Auto-balance** (equal margins around the detected content), **Shadow**, **Corners** (radius).
  - **Aspect Ratio:** Auto, 1:1, 4:3, 3:2, 16:9, 5:4, 9:16 and portrait variants. The ratio is locked while you tweak.
  - **Alignment:** 9-position grid [V-4.5][APP].
- **Numeric precision:** hovering shows the value; click to type it; ↑/↓ adjust; Tab commits (4.6) [V-4.6].
- Annotations still work on top of the background, and the result updates the QAO [V-4.5].

---

## 5. Pin to screen (floating screenshots)

- **How to pin:**
  - From the QAO pin button
  - After Capture "Pin to the screen"
  - Capture Area & Pin hotkey
  - "Pin Last Screenshot" hotkey
  - "Choose and Pin an Image" (menu "Pin to the Screen…")
  - URL `pin?filepath=`
  - Share extension, Capture History and Annotate [APP][API][CL]
- **Window:** always on top; multiple pins allowed [F][PF22].
- **Appearance settings** (Advanced > Pinned Screenshots): Rounded corners, Shadow, Border [S13].
- **Hover chrome:** **× close** top-left, **lock** top-right [IMG pin], and a **"Drag me"** button to drag the image out. ⌥-drag keeps the pin [CL 4.3][CL 4.5.1].
- **Lock mode** (4.3):
  - The pin stays put and **clicks pass through** to apps underneath ("Lock and hide screenshot on mouse over") [V-4.3][APP].
  - Unlock to restore the normal behavior.
- **Interaction:**
  - Drag to move.
  - **Arrow keys** nudge precisely [F].
  - **Two-finger vertical swipe** adjusts opacity, small or large steps (4.6) [V-4.6].
  - Size adjustable [F]. The mechanism (pinch, scroll or edge drag) is unverified.
  - **Middle-click closes** (4.7).
- **Context menu:** Copy to clipboard, Save, OCR, Open Annotate. Close and Close All are inferred [CL 3.3.6][CL 4.0.1][CL 4.1].
- **Global hotkeys:** **Toggle Pins Visibility** (hide or show all, 4.7) and **Close All Pins** [APP][V-4.7].
- A pin updates after it is cropped in Annotate [CL 4.7.5].

---

## 6. Screen recording

### 6.1 Setup flow
1. Trigger Record Screen (hotkey, menu, All-In-One R, or URL `record-screen` with optional area and display) [API].
2. Hint: "Drag to record a part of the screen. Press Space to select a window." Window recording works since 5.0 and the editor adds a background automatically [APP][V-5.0].
3. The area stays editable:
   - Handles to resize; ⌥ for a uniform selection.
   - Arrow keys with ⌘ or ⇧ to move or resize.
   - A typed exact size (e.g. 1920×1080).
   - **Aspect-ratio lock** menu (4.1).
   - Fullscreen and minimize buttons.
   - Enter starts full-screen recording [V-first][V-4.1][DS][CL].
4. **Recorder toolbar** [APP][V-first][PF26]:
   - Buttons: **Record Video**, **Record GIF**, **Record in Studio Mode** (5.0), an info button ("What is Studio Mode?") and a settings gear.
   - **Microphone** device picker ("Do Not Record Microphone").
   - **System audio** toggle.
   - **Camera** device picker ("Don't Record Camera").
   - **Highlight Clicks** and **Show Keystrokes** toggles.
5. Outside the area: dimmed (default) or full brightness with a red border, via "Dim screen while recording" [V-4.1][S8].
6. Optional **3-second countdown** ("Show countdown") [V-4.6][S8].
7. Start, stop and pause sounds play [APP].

### 6.2 While recording
- **Controls pill** [IMG quick-video][V-4.6][V-first]:
  - **■ stop + elapsed timer**, **pause/resume**, **restart** (confirms "cancel this recording and start a new one?") and **trash** (confirms delete).
  - An **audio-level indicator** under the timer.
  - Draggable anywhere; **excluded from the recording**; also shown during full-screen recordings (4.6).
  - Option "Show controls while recording" [S8].
- **Menu bar:**
  - "Display recording time in menu bar" option [S8].
  - Clicking the menu-bar icon stops ("Press to stop recording") [APP].
- **Hotkeys:**
  - Record Screen / Stop Recording
  - Pause/Resume Recording
  - Restart Recording
  - Select Window
  - Start Video Recording
  - Start GIF Recording
  - Toggle Camera Fullscreen
  - These are listed in [PIE][APP].
- **System behaviors:**
  - "Do Not Disturb while recording" [S8].
  - Display sleep is prevented [CL 3.8.1].
  - **Crash recovery:** "CleanShot recovered your recording" [APP][CL 3.9.1].
  - Low-disk warning, then an automatic stop to prevent data loss [APP].
  - Mic warnings: muted mic ("Microphone is muted… continue?"), disconnected mic ("Continue Without Audio"), lid closed with the built-in mic, camera on with no mic selected [APP][CL 4.6].
- **No maximum duration** is documented (unverified).

### 6.3 Options

**Video** [S9][F]:
- MP4 H.264 output.
- **Frame rate:** shown as 60 fps. "More FPS options" exist (4.6); the exact list is unverified.
- **Max resolution:** Original, 1440p and lower presets (list partly unverified).
- **Scale Retina videos to 1x.**

**Audio** [S9][V-4.6]:
- Microphone.
- **Record system audio:** a built-in engine since 4.6, with no driver needed.
- "Record audio in mono."
- **Audio tracks:** Single or Separate (mic and system on separate tracks for external editors).
- A post-recording merge dialog offers per-track volumes [APP].

**Cursor** [S8][F][APP]:
- Show cursor.
- **Highlight clicks** with Options…:
  - Colour (default "System Accent color")
  - Size
  - Style **Outline or Filled**
  - Animate clicks

**Keystrokes** [S8][F][APP][CL]:
- Show keystrokes, with Options…:
  - **Size**
  - **Appearance Dark or Light**
  - Blur background
  - **Position:** Top-Left, Top-Center, Top-Right, Bottom-Left, Bottom-Center, Bottom-Right
  - **Show all keys** or **only command keys**
- Modifiers stay visible while held (4.7).
- Repeated presses are animated (4.2.2).
- Needs Input Monitoring/Accessibility permission.

**Camera** [F][V-4.3][CL]:
- Bubble shape: **Circle**, Square, Rectangle (and Vertical in Studio).
- Size scales with the recording area.
- **Drag to any corner or edge.**
- **Flip.**
- **Click the bubble to toggle the camera full screen**, for example to open full-screen and then cut to the screen.
- Virtual cameras supported; Presenter Overlay supported (4.6.2).

### 6.4 GIF
- **Settings** [S9]:
  - Frame rate (shown as 15 fps)
  - Resolution ("800 x auto (default)")
  - **Optimize GIFs** (gifsicle-based lossy optimization [APP])
  - Quality slider Low–High
- A "Creating GIF…" progress appears. Conversion can be stopped and the recording saved as a video instead [APP].
- GIF trimming is supported [CL 3.3].

### 6.5 Video Editor (5.0) and Studio Mode
- **Standard recording:** the result is available at once. The new editor opens with **limited** options: cut and trim, manual zooms, background and aspect ratio, motion blur, audio toggles.
- **What Studio adds:** cursor, keystroke and camera customization, Follow-Cursor zooms, and auto zooms. These show "…only available for videos recorded in Studio Mode" in standard recordings [V-5.0][PF26][APP].
- **Studio Mode** "records your screen as an editable project". The project is a `.cleanshotvideo` package holding the screen video, a separate camera video, mouse and keyboard event logs and metadata [APP]. Cursor, clicks, keystrokes, camera and audio can be changed afterwards.
- **Layout** [IMG studio-mode][PF26]:
  - Preview at top right.
  - Left sidebar with 6 sections: Cursor, Keystrokes, Audio, Camera, Background & Aspect Ratio, Animations/Motion Blur. A Cut tool (scissors) sits at the bottom.
  - Transport row: timecode, previous edge, play, next edge, and a timeline-zoom slider (also trackpad pinch).
  - Timeline with a **zoom track** above a **thumbnail clip track**.
  - Full-screen preview toggle.
  - Editable project title.
- **Zooms:**
  - Auto-placed by a "smart algorithm" in Studio Mode.
  - Click a zoom for its level (e.g. 150%, 300%), mode (**Follow Cursor** or **Manual** position) and animation (**Smooth** or **Dynamic**).
  - "Click or drag to add zoom".
  - Drag the ends to resize and the middle to move.
  - **Abut two zooms to pan from one position to another.**
  - Backspace deletes.
  - Right-click: Apply Zoom Level to All, Remove All Zooms, Split Zoom at Playhead [V-5.0][APP].
- **Cutting:**
  - Trim handles at both ends.
  - Cut tool (**B**) or **⌘B** to split at the playhead.
  - Select a segment and delete it.
  - Cut marks show the removed duration (e.g. "✂ 4s").
  - Remove Cut [V-5.0][IMG][PF26][APP].
- **Cursor:**
  - Show or hide the cursor.
  - Style: several macOS cursor sets (legacy, Tahoe, "Golden Gate", modern, touch [APP]).
  - Size.
  - Motion: **Natural** (light path cleanup) or **Smooth**.
  - Clicks: **Press effect** (the cursor shrinks) and **Ripple animation** (colour, filled or hollow).
  - **Hide cursor when not moving** and **Hide cursor when typing** [V-5.0][PF26].
- **Keystrokes:** size, dark or light, 9 positions, command keys only or all keys. Applies to the whole project [PF26].
- **Audio:** include the mic plus volume, and include system audio plus volume. Either can be turned on after the fact [V-5.0][PF26].
- **Camera:** toggle, size 0.5×–2×, shape (circle, square, rectangle, vertical), 9 positions, flip, "Shrink camera on zoom" [PF26][APP].
- **Background:**
  - Gradients, colours, wallpapers or custom images.
  - Aspect ratio: Auto, 16:9, 9:16, 1:1, 4:3, 3:4 and others.
  - Padding %, shadow and corner %.
  - **While a slider is dragged, the preview drops out of any zoom** so the frame is visible [PF26].
- **Motion blur:** Low to High, default Medium; it affects zooms and fast cursor movement [V-5.0].
- **Export** (top right) [V-5.0][APP]:
  - Choices: resolution, quality, include audio.
  - Shows the **estimated file size**; uses hardware encoding.
  - Variants: Export, Export & Upload to Cloud, Export Current Frame as Image.
  - The result appears as a QAO.
  - Projects can be saved and reopened.
- **Editor shortcuts** (all bindable) [APP]:
  - Play/Pause (Space)
  - Selection Tool, Cut Tool, Deselect All
  - Split Clip and Split Zoom at Playhead
  - Move Playhead to Start, End, Next or Previous Clip Edge, ±1 frame, ±10 frames
  - Zoom Timeline In, Out, and Fit
  - Toggle Full Screen
  - Export, Upload
- **The pre-5.0 Video Editor/Trimmer** is still referenced in [V-first] and [APP]:
  - Trim
  - Quality ("Very low" … ) with an estimated size
  - Dimensions / resolution
  - Audio: keep, mono, mute, volume boost or cut
  - Choice of "Replace Video" or "Save as New Video"
  - Opened by double-clicking a video QAO or via After Capture "Open Video Editor"

---

## 7. Capture History (4.4 and later)

- **Opening it:** menu "Capture History…", a hotkey, or URL `open-history` [API].
- **Layout:** a full-width band across the top of the screen with large thumbnails and a "⏎ Restore" pill under the focused item [IMG history][V-4.4].
- **Filters:** All / Screenshots / Videos / GIFs [IMG], plus "Video Projects" in 5.0 [APP].
- **Navigation:** arrow keys or scroll. Space opens Quick Look. ⌘C copies a PNG. Drag out to apps.
- **Restore:** **Enter** or click restores to the QAO with **annotations still editable**. Several items can be restored at once; the panel stays until you click outside or press Esc [V-4.5].
- **Other actions:**
  - Double-click opens Annotate (4.6).
  - Pin or Annotate from history (4.5.1).
  - **Backspace deletes**, with a confirmation that can be turned off ("Don't ask for confirmation").
  - "Clear History…" / "Delete All" [V-4.5][APP].
- **Retention:** Advanced > Keep history: **Never, 1 day, 3 days, 1 week, 1 month** [APP]. The screenshot shows 3 days [S12]; the 4.4 default was 1 day [V-4.4]. When set to Never: "Capture History is disabled."
- Includes external files opened with CleanShot (4.7). "Restore Last Capture" hotkey [S3].

---

## 8. Settings (5.0 redesign)

The sidebar order is General, Shortcuts, Quick Access, Wallpaper, Screenshots, Screen Recording, Annotate, Cloud, Advanced, About [S1]. The URL tab IDs are `general, wallpaper, shortcuts, quickaccess, recording, screenshots, annotate, cloud, advanced, about` [API].

### 8.1 General [S1][S2]
- **App:**
  - Launch at login.
  - **Show menu bar icon.** Hiding it warns: "In order to show the Settings window again, launch CleanShot twice." [APP]
- **Capture:** Hide desktop icons while capturing, with a link to the wallpaper settings.
- **Sounds:**
  - Play sounds.
  - **Shutter sound:** System Default plus bundled sounds (files grab-classic, -bigsur, -pop, -subtle, -8bit [APP]; the display names are inferred).
- **Export:** Export location (default Desktop) with Choose…. It is used by QAO Save, After Capture and other Save actions.
- **After Capture matrix:**
  - Columns **Screenshot | Recording**. Rows:

  | Action | Screenshot | Recording |
  |---|---|---|
  | Show Quick Access Overlay | yes | yes |
  | Copy file to clipboard | yes | yes |
  | Save | yes | yes |
  | Upload to Cloud & copy link | yes | yes |
  | Open Annotate tool | yes | n/a |
  | Pin to the screen | yes | n/a |
  | Open Video Editor | n/a | yes |

  - At least one action must stay on ("At least one action needs to be enabled").
  - Two clipboard actions together trigger a "Choose what should be copied" prompt [APP].

### 8.2 Shortcuts [S3][APP]
- A searchable list ("Search shortcuts", "No shortcuts match…").
- Each row has a recorder ("Record shortcut" / "Type shortcut").
- Conflict detection: "Shortcut is used for another action … is assigned to …", "Shortcut is reserved for macOS", "Use old shortcut", "Reset to default".
- "Restore Defaults" and "Use System Default Shortcuts".
- The "Ignore After Capture for Capture Area & …" option lives here.
- Supports F-keys and other keyboard layouts.
- Sections: General, Screenshots, Screen Recording, Scrolling Capture, OCR, Quick Access Overlay, Pin, Annotate, Video Editor. The full action list is in §10.

### 8.3 Quick Access [S4]
- Position on screen (Left/Right)
- Move to active screen
- Overlay size
- Auto-close, with Action (Save and Close / Close) and Interval
- Close after dragging (⌥ inverts)
- Close after uploading
- Save button behavior: Save to "Export location" or Ask for destination (⌥ inverts)

### 8.4 Wallpaper [S5]
- Wallpaper: Desktop wallpaper, Custom wallpaper or Plain color. Used both to hide icons and behind window screenshots.
- Update wallpaper when switching Spaces.

### 8.5 Screenshots [S6][S7]
- **Output:**
  - File format: PNG, JPG, WebP. HEIC is listed in the app (unverified as a save option).
  - Convert to sRGB profile (4.8).
  - Scale Retina screenshots to 1x.
  - Add 1px border to all screenshots (warns that quality decreases).
  - Background preset: None or a preset (⇧ skips it).
- **Capture:** Self-Timer interval, Show cursor on screenshots, Freeze screen.
- **Window Screenshots:** Background (With wallpaper / Transparent), Padding slider, Capture window shadow, Reset Defaults.
- **Crosshair:** Crosshair Mode (Disabled / Always enabled / When ⌘ pressed), Show magnifier.
- A PixelSnap promo.

### 8.6 Screen Recording [S8][S9]
- **General:**
  - Show controls while recording
  - Remember last selection
  - Display recording time in menu bar
  - Dim screen while recording
  - Show countdown
  - "Do Not Disturb" while recording
- **Cursor:** Show cursor; Highlight clicks plus Options….
- **Keystrokes:** Show keystrokes plus Options….
- **Video:** Frame rate, Max resolution ("Set maximum resolution to reduce file size and upload time."), Scale Retina videos to 1x.
- **Audio:** Record audio in mono, Record system audio, Audio tracks (Single/Separate).
- **GIF:** Frame rate, Resolution, Optimize GIFs, Quality.

### 8.7 Annotate [S10]
- **Tools:**
  - Inverse arrow direction (⌥ inverts)
  - Smooth drawing
  - Remember if background tool was opened
  - Draw shadow on objects
  - Automatically expand canvas
- **Window:** Always on top, Show Dock icon.
- **Accessibility:** Show color names.

### 8.8 Cloud
Out of scope [S11].

### 8.9 Advanced [S12][S13][S14]
- **File Name:**
  - Ask for name after every capture. The dialog has a Discard button (4.7).
  - **File name format** via Customize…, which opens a token editor [S14][APP NIB]:
    - "Type text and drag elements to create a custom format."
    - Live **Preview**.
    - Default `CleanShot %y-%m-%d at %H.%M.%S` gives "CleanShot 2026-08-24 at 13.50.01".
    - Tokens:

    | Token | Meaning |
    |---|---|
    | %y | Year |
    | %m | Month (numeric) |
    | %n | Month (name) |
    | %d | Day |
    | %w | Day of week |
    | %t | Window title |
    | %a | App name |
    | %H | Hour |
    | %M | Minutes |
    | %S | Seconds |
    | %p | AM/PM |
    | %r | Random characters |
    | %i | Auto-increment |

    - "Auto-increment next number" field.
    - "Remove illegal characters" and "Use UTC time zone" checkboxes.
    - Restore Defaults / Cancel / OK.
  - Add "@2x" suffix to Retina screenshots.
- **Clipboard:** Copy to clipboard = File & Image (default) / Image only / File only. The help text says it is for clipboard-manager problems.
- **Capture History:** Keep history.
- **Text Recognition:** Language, Keep line breaks, Detect links.
- **All-In-One:** Remember last selection.
- **Pinned Screenshots:** Rounded corners, Shadow, Border.
- **API:** Allow URL scheme API.
- Also: "Reset All Warning Dialogs" [APP][CL 3.8].

### 8.10 About [APP]
- License Manager, Unlink Device, Manage Account
- Automatically check for updates, Check for Updates
- Share usage statistics
- Links: Website, Changelog, Contact, X, YouTube
- Acknowledgments

### 8.11 Menu-bar menu [APP MainMenu.nib]
1. All-In-One
2. Capture Area
3. Capture Previous Area
4. Capture Fullscreen
5. Capture Window
6. Self-Timer
7. Scrolling Capture
8. Capture Text (OCR)
9. Record Screen
10. Open…
11. Open from Clipboard
12. Pin to the Screen…
13. Capture History…
14. Hide/Show Desktop Icons
15. Preferences…
16. Check for Updates…
17. About CleanShot…
18. Quit

Additional states:
- "Activate…" appears when unlicensed.
- An update indicator.
- Cloud "recent uploads" (out of scope).
- During recording the icon becomes a stop or resume glyph with the timer.

### 8.12 URL-scheme API (complete as documented) [API]

Format: `cleanshot://<command>?k=v`. Coordinates use a lower-left origin. `display` is 1 = main, 2 = secondary.

| Command | Parameters |
|---|---|
| `all-in-one` | `x,y,width,height,display` |
| `capture-area` | `x,y,width,height,display,action` |
| `capture-previous-area` | `action` |
| `capture-fullscreen` | `action` |
| `capture-window` | `action` |
| `self-timer` | `action` |
| `scrolling-capture` | `x,y,width,height,display,start=true/false,autoscroll=true/false` |
| `pin` | `filepath` (optional; otherwise a file picker) |
| `record-screen` | `x,y,width,height,display` |
| `capture-text` | `filepath` or `x,y,width,height,display`, `linebreaks=true/false` |
| `open-annotate` | `filepath` (optional) |
| `open-from-clipboard` | none |
| `toggle-desktop-icons`, `hide-desktop-icons`, `show-desktop-icons` | none |
| `add-quick-access-overlay` | `filepath` (required; PNG/JPEG/MP4) |
| `open-history` | none |
| `restore-recently-closed` | none |
| `open-settings` | `tab` |

- `action` takes `copy`, `save`, `annotate`, `upload` or `pin`.
- **Undocumented internal commands** [APP]: `capture-area-raycast-aichat`, `raycast-private-upload-file`, `share-extension-annotate`, `share-extension-pin`.
- **Security:**
  - Settings toggle "Allow URL scheme API".
  - On first use, a consent dialog: "An external application wants to control CleanShot… allow third-party apps to execute commands and access your screen?"
  - Afterwards, a notice "Another app used CleanShot to capture your screen" [APP].
- The URL scheme is the only automation surface. There is no CLI or AppleScript dictionary (per a third-party skill, ljagiello/agent-skills; consistent with the app strings).

### 8.13 Integrations
- **Raycast:**
  - Community extension with about 23 commands, including "Record Screen at Preset" and recording presets [RC].
  - First-party "Send to Raycast AI Chat" from Annotate (⌘R) and the "Capture Area & Send to Raycast AI Chat" hotkey [CL 4.7.2–4.7.3].
- **macOS Share-menu extensions:** "Annotate" and "Pin" [CL 3.5.1][APP].
- **Finder:** "Open With CleanShot" for images and videos.
- **PixelSnap 2** integration, and **TopNotch** compatibility.
- **Alfred:** no official integration found; community workflows use the URL scheme (unverified).

---

## 9. Other features and recent changes

- **Open from clipboard:** image, or MP4 since 4.6, into Annotate or the QAO [API][CL].
- **Open file in Annotate:** "Open…" menu item or hotkey.
- **"Annotate Last Screenshot"** hotkey (4.6).
- **Print** from Annotate and the QAO, multi-page for tall images.
- **Rotate, flip, Scale Retina to 1x and Resize** from the QAO menu.
- **No** OCR translation, **no** measure or ruler tool (that is PixelSnap), **no** standalone screen colour picker outside Annotate's eyedropper, and **no** automatic redaction. None were found on the site, in the changelog or in the app strings.

**Timeline 2024–2026** [CL]:

- **4.7 (May 2024):**
  - Resize tool and Smart Highlighter.
  - Auto-apply background preset.
  - Paragraph text formatting.
  - +10 backgrounds (20 total).
  - Hide widgets with the desktop icons.
  - 1-month history; history includes external files.
  - QAO animation and a newest-capture indicator.
  - ⇧ axis-lock when moving objects.
  - Toggle Pins Visibility and Close All Pins.
  - Middle-click closes pins.
  - Held modifiers stay visible in keystroke display.
  - New URL endpoints and parameters.
  - Discard button in the name dialog.
- **4.7.2 / 4.7.3:** Raycast AI Chat; ⌘R.
- **4.7.5 (Feb 2025):** counters always on top; better QAO multi-close.
- **4.8 (May 2025):**
  - Colour Picker.
  - Editable window backgrounds.
  - Horizontal scrolling capture.
  - Rotate and flip.
  - OCR language auto-detect and Arabic.
  - WebP save; HEIC open.
  - Drop-zone stitching.
  - Multi-page print.
  - sRGB conversion.
  - ⌥ copy-without-closing.
  - All-In-One mode hotkeys.
- **4.8.1:** 5:4 and 9:16 ratios.
- **4.8.4 (Oct 2025):** 10 new OCR languages on Tahoe; ⇧ skips the preset.
- **4.8.5 (Dec 2025):** Tahoe UI.
- **4.8.8–4.8.11 (2026):** audio-interface mic fix, rounded window corners, macOS "Golden Gate" compatibility, a security update.
- **5.0 (1 Sep 2026):**
  - Studio Mode and the new Video Editor: smart zooms, cuts, smooth cursor, motion blur, recording backgrounds, post-hoc cursor, keystroke and camera changes, hardware-encoded export, `.cleanshotvideo` projects.
  - Space to record a window.
  - Redesigned settings.
- **5.0.1 (18 Sep 2026):** fixes.

---

## 10. Default keyboard shortcuts and every bindable action

### Out-of-box global hotkeys
- Almost none are assigned ("a lot of these don't have keyboard shortcuts by default" [V-first]).
- Onboarding offers to take over the macOS screenshot keys after you disable them in System Settings: **⇧⌘3 = Capture Fullscreen** and **⇧⌘4 = Capture Area** [S3][PF22]. It also offers the system shortcut for **All-In-One**, presumably ⇧⌘5 (inferred from the onboarding strings [APP]).

### Global bindable actions [S3][APP][PIE]
- **General:** All-In-One, Toggle Desktop Icons, Open Capture History, Restore Last Capture.
- **Screenshots:**
  - Capture Area, Capture Previous Area, Capture Fullscreen, Capture Window, Self-Timer
  - Capture Area & Copy, & Save, & Upload, & Annotate, & Pin, & Send to Raycast AI Chat
- **Recording:** Record Screen / Stop Recording, Select Window, Start Video Recording, Start GIF Recording, Pause/Resume, Restart, Toggle Camera Fullscreen.
- **Scrolling:** Scrolling Capture, Start/Stop Capturing.
- **OCR:** Capture Text, With Line Breaks, Without Line Breaks.
- **QAO:** Hide/Show Overlays, Save All Overlays, Close All Overlays.
- **Pin:** Choose and Pin an Image, Pin Last Screenshot, Toggle Pins Visibility, Close All Pins.
- **Annotate:** Open (file), Open From Clipboard, Annotate Last Screenshot.

### Fixed or contextual keys

| Context | Keys |
|---|---|
| Capture | Esc cancel; Space window mode; Ctrl copy to clipboard; ⌥ uniform selection; ⌘ crosshair (if set); ⇧ skip preset / invert window background; arrows (+⌘/⇧) move/resize recording area; Enter full-screen recording in the recorder |
| All-In-One | A, F, Space/W, S, T, O, R; ⌘C |
| QAO (hovered) | ⌘C, ⌘S, ⌘W, ⌘U, ⌘E, Space; ⌥ modifies Copy, Save and Drag |
| Annotate tools [PIE] | V move, K crop, B background, R rectangle, F filled rectangle, E ellipse, L line, A arrow, T text, P redact, H spotlight, C counter, D draw, M highlighter |
| Annotate commands | ⌘Z/⇧⌘Z; ⌘C/⌘V objects; ⌘D duplicate; ⌘S; ⇧⌘S; ⇧⌘C copy image; ⌘U; ⌘P; ⌘I add from file; ⇧⌘I add new screenshot; ⌘R Raycast; ⌘W close; ⌘+/⌘−/⌘1 zoom; Space-drag pan; ⇧ constrain; ⌥-drag duplicate; ⌥ while drawing an arrow inverts; ⌘ toggles crop/highlighter snapping; Delete removes. Tool size keys are unverified (1–6 per [CL 2.6]). |
| History | ←/→ or scroll; Space; ⌘C; Enter restore; Backspace delete; Esc close |
| Video editor | Space play/pause; B cut tool; ⌘B split; Backspace delete zoom/segment; all others bindable (defaults unverified) |
| Pins | Arrow keys move; middle-click close; two-finger scroll for opacity |

---

## 11. UX principles that make CleanShot feel great (synthesis)

1. **One gesture to a result.** One hotkey and a drag release give a capture. There are no dialogs, and in the default flow nothing touches disk until you choose.
2. **Non-modal, deferred commitment.** The QAO waits in a corner without stealing focus. It stacks, can be swiped away or hidden, and auto-closes on your terms. The user decides later whether to copy, save, edit, pin or drag.
3. **Every object can be dragged everywhere.** The QAO thumbnail, Annotate's "Drag me", pins and history items all drag as real files into any app.
4. **Three paths for every action.** Each has a click, a keyboard path (single letters, ⌘-keys while hovering) and a drag path, and tooltips teach the shortcuts.
5. **Modifiers invert defaults instead of adding modes.** ⌥ flips save destination, close-after-drag, copy-without-close and arrow direction. ⇧ flips the window background and skips the preset. Ctrl means copy. Space means window.
6. **Smart defaults with deep but organized settings.** The After Capture matrix is set separately for screenshots and recordings. The many "Remember last …" states keep continuity: area, ratio, tool, colour, arrow style, text size and save folder.
7. **Forgiveness everywhere.**
   - Undo and redo.
   - Restore last capture and a one-month history.
   - Re-editable `.cleanshot` and `.cleanshotvideo` projects.
   - Crash-recovered recordings.
   - Restart and trash during recording.
   - Confirmations before data loss, "Don't ask again", and "Reset all warnings".
8. **Precision on demand, never forced.** Crosshair and magnifier come only when wanted. Typed sizes, ratio locks, arrow nudging, edge snapping, snapping guides, and numeric fields with arrow and Tab support are all available.
9. **Beautiful by default.**
   - Wallpaper and shadow on window shots.
   - Auto-balance, gradients and presets.
   - Smooth drawing, the smart highlighter and curved arrows.
   - Studio auto-zooms, smooth cursor and motion blur.
10. **Stay out of the shot.** Controls are excluded from recordings, desktop icons and DND are handled automatically, there is a dim-or-border choice, and locked pins are click-through.
11. **Teach in context.** First-use tips (Space to preview, drag to apps, swipe down to hide), scrolling-capture and OCR guides, and contextual "only available in Studio Mode" hints.
12. **Privacy and security as features.** On-device OCR, randomized pixelate and secure blur, and consent before external apps control capture.
13. **Fast and native.** Instant start of recording, hardware export, low memory use, and an editor that opens quickly.

---

## 12. Mac-only features that need a Linux (Omarchy/Hyprland) reinterpretation

- **Menu-bar icon, recording timer and click-to-stop:** a Waybar custom module or StatusNotifierItem tray, plus Omarchy menu and Walker entries.
- **Global hotkeys and "take over system screenshot keys":**
  - Wayland apps cannot grab keys. Ship a CLI/IPC and Hyprland `bind`s; use the GlobalShortcuts portal where available.
  - Onboarding should offer to replace Omarchy's Print-key bindings (`omarchy-cmd-screenshot`) in `~/.config/hypr/bindings.conf`.
- **Capture backend:**
  - ScreenCaptureKit maps to wlr-screencopy (grim-like) for outputs and regions.
  - Window capture uses hyprland-toplevel-export, ext-image-copy-capture or the portal. Occluded-window capture depends on this.
  - Window geometry comes from `hyprctl clients -j`.
- **Window shadow, rounded corners and transparent window shots:** Hyprland decorations (rounding, border, shadow) are compositor-side. Recreate the mask and shadow synthetically, and decide whether to include Hyprland borders.
- **Hide desktop icons:** there are no icons on Hyprland. Reinterpret as "clean capture": temporarily hide Waybar and notifications, and use the current Omarchy theme wallpaper (hyprpaper/swaybg) as the window-shot background. "Update when switching Spaces" maps to per-workspace or per-monitor wallpaper.
- **Do Not Disturb:** mako DND mode (`makoctl mode -a do-not-disturb`, as Omarchy uses), restored afterwards.
- **Retina (@2x, Scale to 1x, @2x suffix):** Hyprland integer and fractional monitor scales. Capture physical pixels, offer "scale to logical size", and add a suffix when scale ≥ 2.
- **Freeze screen, crosshair, magnifier and selection overlay:** a fullscreen layer-shell surface on every output, showing a frozen frame. Handle mixed-scale multi-monitor.
- **QAO and pins as floating, always-on-top, non-focus-stealing windows:**
  - Use layer-shell surfaces, or Hyprland windowrules (float, pin, nofocus, noanim).
  - Pin **lock mode** needs an empty input region (click-through) and alpha for opacity.
  - Keyboard shortcuts while hovering need on-demand keyboard interactivity.
- **Two-finger swipes:** libinput scroll and axis events on the surface (swipe-dismiss, swipe-down hide, pin opacity).
- **Drag and drop to apps:** Wayland DnD offering `text/uri-list` and `image/png`. **Clipboard "File & Image"** maps to wl-clipboard-style multi-MIME offers (image/png, text/uri-list, x-special/gnome-copied-files).
- **Quick Look (Space):** a built-in preview window, since there is no system Quick Look.
- **Finder actions:**
  - Show in Finder uses `org.freedesktop.FileManager1.ShowItems` (Nautilus).
  - Open With uses mimeapps and GIO.
  - Open in Mail is weak on Linux (`xdg-email --attach`); consider dropping it.
  - The macOS share sheet has no equivalent; replace it with Open With.
- **Share extensions, Quick Look and thumbnail plugins:**
  - `.desktop` MimeType handlers ("Annotate with…", "Pin")
  - Nautilus scripts
  - A freedesktop thumbnailer for the project files
  - An `x-scheme-handler` for the URL API
- **System audio and mic:** PipeWire monitor and input nodes; separate tracks or mixdown. **Camera:** PipeWire camera or V4L2. Presenter Overlay is not applicable.
- **Keystroke display and click highlights:** these need global input access (evdev or libinput with the `input` group, or a privileged helper). This is a security and UX trade-off to state clearly. Cursor position comes from Hyprland IPC.
- **Hardware encoding:** VA-API or NVENC through GStreamer/FFmpeg (or gpu-screen-recorder or wf-recorder as backends). Studio Mode needs separate cursor and event logs, so record without the cursor where possible.
- **macOS cursor styles in Studio:** use the user's Xcursor theme or bundled sets.
- **OCR (Apple Vision):** Tesseract with tesseract-data-* packs or an ONNX OCR (PaddleOCR/RapidOCR); QR codes via zbar. Auto language detection and quality will differ.
- **Shutter and system sounds:** the freedesktop sound theme (canberra "camera-shutter") plus bundled sounds.
- **Launch at login:** Hyprland `exec-once`, a systemd user unit or XDG autostart. **Updates and licensing:** pacman/AUR.
- **Dark/light and native look:** follow the current Omarchy theme colours and fonts. Annotate's "Show Dock icon" becomes an app_id or class for windowrules.
- **Print:** CUPS via the toolkit print dialog. **sRGB conversion and colour profiles:** limited on Wayland today; low priority.
- **Not applicable on Linux:**
  - Notch auto-crop
  - TopNotch and PixelSnap integrations
  - Mission Control and Spaces wording
  - Sparkle and macOS permission prompts; wlr protocols mostly need none, while portals add consent

---

## Notable gaps (unverified items)

The following could not be pinned down from primary sources:
- The exact QAO auto-close interval list, the video FPS list and the max-resolution list.
- Where the pin icon now sits on the QAO.
- QAO stacking order and the maximum number of cards.
- How a pin is resized.
- The scrolling-capture length limit.
- Fullscreen behavior with several displays.
- Area modifiers during a drag (Space to move, Shift to lock an axis, Option to resize from the centre).
- Arrow nudging in screenshot or All-In-One selections.
- Current default keys for tool size and the Video Editor.
