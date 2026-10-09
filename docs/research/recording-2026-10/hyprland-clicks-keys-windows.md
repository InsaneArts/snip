# Snip recording research: click and keystroke capture, and window recording, on Hyprland 0.56.2

This report covers questions A1–A3 and B1. It builds on `docs/research/platform-feasibility-hyprland.md` §9–§10 and `docs/02-architecture.md` §4.3/§6.3. Facts that are already there are not repeated. Where an earlier statement was wrong or incomplete, the correction is marked.

**Evidence tags**
- **[local]**: run on this machine with read-only probes: `hyprctl repl/eval` type checks, `hyprctl devices/binds/clients/monitors/getprop/cursorpos`, `gpu-screen-recorder --list-*`, `ss`. Also scratch-only programs.
- **[src]**: read in the shipped source.
  - Hyprland v0.56.2 at commit efb5099. This is the installed build.
  - XDPH v1.4.1 at commit cc8e5ef. This is the installed package.
  - gpu-screen-recorder (gsr) 6.1.0.
- **[doc]**: vendor docs, the Hyprland wiki, or articles.
- **[inf]**: inference.

No hooks, binds, recordings or windows were created in your session.

---

## 0. Answers in brief

- **A1. Keystrokes**
  - `hl.on("input.keyboard.key", fn)` calls `fn(keycode, time_ms, state)`.
    - `keycode` is the XKB keycode (evdev code + 8).
    - `time_ms` is the 32-bit millisecond time of the input event. It is not a Unix time, despite what the wiki says.
    - `state` is 0 (released) or 1 (pressed). In practice 2 never occurs.
  - There is no keysym, no modifier state and no device in the payload.
  - It fires for every key on every enabled keyboard, before binds, the IME and shortcut inhibitors. That includes virtual keyboards and **the lock screen's password**.
  - The compositor cannot tell when a password field has focus. It sees password keystrokes.
  - On this machine fcitx5 owns a virtual keyboard that is currently the "main" keyboard, so keys typed into IME-aware apps reach the hook **twice**. Dedupe them.
  - `hl.on` returns an `HL.EventSubscription` with `:remove()` and `:is_active()`. Calling `:remove()` twice is **safe**. The crash on a double remove applies only to `HL.Keybind`.
- **A2. Clicks**
  - In 0.56.2 Lua has no mouse event.
  - Use a non-consuming bind, one per button:
    `hl.bind("MOD3 + MOD5 + mouse:272", fn, { non_consuming = true, ignore_mods = true, transparent = true, submap_universal = true, dont_inhibit = true })`
  - Inside `fn`, call `hl.get_cursor_pos()`. It returns global logical floats, the same space as `hyprctl cursorpos` [local].
  - Clicks and drags still reach apps. A SUPER+drag bind still fires.
  - Hazard: `HL.Keybind:remove()` removes **every** bind with the same modmask and key. On an expired handle it segfaults. A modmask that nobody else uses avoids the first problem. A `tostring()` guard avoids the second.
- **A3. Transport**
  - Recommendation: socket2 `custom>>` events through `hl.dispatch(hl.dsp.event(...))`.
    - Hyprland writes them without blocking, inside the input event.
    - The hop from Hyprland to the reader measured 3.1 µs median and 46 µs p99 [local, synthetic].
    - Snip already reads socket2.
  - A 0600 file in the runtime dir watched with inotify has the same latency (2.9 µs median). It is private, but it is more code.
  - `os.execute`, `io.popen` and `hl.exec_cmd` are not acceptable. Each event would fork a 250 MB process on the compositor thread, and `os.execute` also blocks until the child exits.
  - This is open decision Q-S5 (§8).
- **B1. Window recording**
  - **v1: (i) record the window's rectangle with gsr `-w region` (KMS).**
    - This is what CleanShot X 4.x did: Space snaps the recording area to the window.
    - CleanShot's binaries for 4.7.6, 4.8.11 and 5.0.1 use only display capture plus a crop rectangle. None of them reference ScreenCaptureKit's single-window capture.
    - Snip's on-screen overlays (keystrokes, clicks, webcam) are recorded for free.
  - (ii) The portal route is feasible but costly: a second picker, or taking over `xdph.conf`; restore tokens restore by class; overlays and translucency are lost; Omarchy keeps it opt-in because of DMA-BUF failures.
  - (iii) Snip's own toplevel capture is the right P2 path for CleanShot-5-style "window plus background", but it is weeks of work.

---

## 1. A1: `hl.on("input.keyboard.key", fn)`

### Payload [src]

The payload is built in `LuaEventHandler.cpp` L179–185:

```cpp
dispatch("input.keyboard.key", 3, [&](lua_State* L) {
    lua_pushinteger(L, keyEvent.keycode + 8); // Because to xkbcommon it's +8 from libinput
    lua_pushinteger(L, keyEvent.timeMs);
    lua_pushinteger(L, keyEvent.state);
});
```

| Arg | Meaning | Notes |
|---|---|---|
| 1 | XKB keycode (evdev + 8) | For example 38 = A, 37 = Control_L, 133 = Super_L. Snip's `keys.rs` table already uses these codes. |
| 2 | `SKeyEvent.timeMs` (uint32 ms) | The input event's own time. For libinput keyboards this is libinput's millisecond event time, which is CLOCK_MONOTONIC in practice [inf]. For virtual keyboards it is whatever the client sent. The wiki calls it a "Unix timestamp", which is wrong. At runtime, check it against the low 32 bits of Snip's own CLOCK_MONOTONIC in milliseconds. |
| 3 | `wl_keyboard_key_state`: 0 released, 1 pressed | The wiki lists 2 = repeated. libinput keyboards never produce it, and key repeat is client-side on Wayland, so a held key gives one press and one release. Ignore anything other than 0 and 1 [src+inf]. |

- There is no keysym, modifier mask or device in the payload. Modifier keys arrive as their own key events.
- `hl.is_key_down("Control_L" | keycode)` exists. It checks `KeybindManager::m_pressedKeys`, which holds physical keys only [src LuaBindingsToplevel.cpp L418–458].
- The return value of the callback is ignored. A Lua hook **cannot swallow keys** (`nresults = 0`; the `SCallbackInfo` is unused).

### When it fires [src InputManager.cpp L1619–1632]

`Event::bus()->m_events.input.keyboard.key.emit(event, info)` is the first thing `onKeyboardKey` does, after the `m_enabled`/`m_allowed` device check. It therefore fires:
- for every key whatever has focus: an app, a layer, nothing, or a fullscreen game;
- for keys that Hyprland binds then consume;
- while a client inhibits shortcuts, and during input-capture;
- for **virtual keyboards** (wtype, IME keyboards). Virtual keyboards go through the same `setupKeyboard` and `onKeyboardKey` path (L1156–1159);
- **while the session is locked**. There is no lock check before the emit.

It does not fire for a keyboard disabled in config (`enabled = false`) or denied by the permission system.

### Duplicates on this machine [local + src]

`hyprctl devices -j` lists `hl-virtual-keyboard-fcitx5` with `"main": true`. "Main" means it is the seat's active keyboard, which `SeatManager::setKeyboard` sets when it delivers a key [src HyprCtl.cpp L797, SeatManager.cpp L161–172].

When fcitx5 holds the keyboard grab, Hyprland handles each keystroke like this:
1. The physical key (from `keyd-virtual-keyboard`) fires the hook, then goes to the IME.
2. fcitx5 forwards the key back through its virtual keyboard.
3. That virtual keyboard fires the hook again (`DISALLOWACTION` path).

Dedupe with a state machine: drop a press for a key already down and a release for a key already up. The module in §5 does this. The forwarded copies probably carry the original time [inf], so Snip can also drop exact `(code, state, ms)` repeats.

### Privacy

- **The hook sees everything**: browser and terminal password fields, pinentry, and the session-lock prompt.
- macOS is different. Password fields turn on Secure Event Input, which "protects keyboard entry so that keyboard events cannot be intercepted by a keyboard intercept process" [doc Apple TN2150]. On macOS CleanShot never sees those keys. Hyprland has no equivalent.
- Mitigations, all in §5:
  - Filter at the source. "Command keys only" is the default: modifiers, special keys, and keys pressed while Ctrl, Alt or Super is held.
  - Stay silent for private window classes and private layer namespaces.
    - Omarchy's polkit prompt is a **layer**, `omarchy-polkit`, with exclusive keyboard focus [local PolkitAgent.qml L221–228]. A window-class check alone misses it.
  - Mute from Snip while the session is locked. Lua cannot see the lock state in 0.56.2 (§5.3).

### Lifetime and removal [src]

| Object | Remove | Removing twice | After `hyprctl reload` |
|---|---|---|---|
| `HL.EventSubscription` (from `hl.on`) | `sub:remove()`; also `sub:is_active()` | **Safe**: guarded by an `active` flag (`LuaEventSubscription.cpp` L34–43) | Gone. The reload calls `clearEvents()` and then `reinitLuaState()`, a new `lua_State` (`ConfigManager.cpp` L700–725, L506–520) |
| `HL.Timer` (from `hl.timer`) | No remove. `t:set_enabled(false)` | `set_enabled` on an expired timer is a no-op (`LuaTimer.cpp` L37–51) | Cancelled (`cleanTimers`) |
| `HL.Keybind` (from `hl.bind`) | `b:remove()`, the same as `b:unbind()` | **Crash**: see §2 | Gone (`clearKeybinds`) |

Other constraints:
- Callbacks run synchronously on the compositor thread under a **50 ms** instruction-count watchdog. Keybind callbacks get 100 ms, timers 50 ms and eval 250 ms (`ConfigManager.hpp` L107–114).
- A callback error pops an **on-screen notification** ("Runtime error in lua", 5 s) (`ConfigManager.cpp` L857–870). It would land in the recording. Wrap callback bodies in `pcall`.
- Hyprland runs **Lua 5.5** (`_VERSION` [local]), with `io`, `os` and `package` available and `debug.sethook` removed [local + src L520–529].
- In Lua 5.5 a for-loop variable is constant. Assigning to it is an error [local, lua5.5].

---

## 2. A2: clicks

**No Lua mouse event exists in 0.56.2.** The event bus has `input.mouse.button` (`InputManager.cpp` L750), but `knownEvents()` exposes only `input.keyboard.key` (`LuaEventHandler.cpp` L266–303). A plugin could expose it, but plugins break on every Hyprland ABI change. That leaves a bind.

### Exact syntax [src LuaBindingsToplevel.cpp L58–256; wiki binds/flags.md]

```lua
local h = hl.bind("MOD3 + MOD5 + mouse:272", function() pcall(on_click) end, {
  non_consuming = true,    -- the press and release still reach the app
  ignore_mods = true,      -- also fires on Ctrl/Shift+click and SUPER+drag; modmask is then ignored for matching
  transparent = true,      -- can't be shadowed by other binds
  submap_universal = true, -- also fires inside submaps (you have a "service" submap)
  dont_inhibit = true,     -- also fires when the focused app inhibits shortcuts (VMs, remote desktops)
  description = "Snip: recording clicks",
})
```

- Modifiers come first, then a special symbol `mouse:<button>`: 272 is left, 273 right, 274 middle.
- The callback gets **no arguments**: no button and no time. Use one bind per button with the button captured in a closure. Snip timestamps clicks when they arrive, which is sub-millisecond.
- Do not use the `click`/`drag` options. They imply `release` and drag-threshold logic.
- For a release event, add a second bind with `release = true, non_consuming = true` on a different unused modmask, for example `"MOD2 + MOD3 + MOD5 + mouse:272"`.

### Behaviour, from `KeybindManager.cpp` [src]

- **Passes through.** On press, a non-consuming bind leaves `found = false`, so `passEvent = true` and `sent = true` (L499–505, L835). The release is suppressed only if the press wasn't sent (L512–515). Clicks, drags, DnD and text selection are unaffected.
  - Roadmap spike S9 asks for this ("doesn't break drags"). The source says yes; an e2e test should confirm it (§7).
- **All matching binds fire.** Matches are collected into `bindsHit` and every one runs (L631–763). On SUPER+drag, Omarchy's consuming "Move window" bind still drags, and Snip's bind also reports the click.
- **Not while locked.** Without `locked = true` a bind is skipped while the session is locked (L649). That is the right default.
- Inhibitors (L643), submaps and modmask (L652), and input-capture (L637) are handled by the options above.

### `HL.Keybind` hazards [src LuaKeybind.cpp L11–14, L46–87, L104–171; KeybindManager.cpp L197–202]

1. **Collateral removal.** `remove()` calls `removeKeybind(modmask, key)`. That erases **every** bind with the same modmask and key string, in every submap. The key string is case-sensitive.
   - Your binds include SUPER+mouse:272 "Move window", modmask 64.
   - Modmask 160 (MOD3|MOD5) is used by none of your binds [local `hyprctl binds`], so the module uses it with `ignore_mods`.
2. **Crash on an expired handle.** `getKeybindFromUserdata` returns `std::optional<SP>` built from `WP::lock()`. The optional is always engaged, so `if (!keybind)` never catches a dead bind, and `(*keybind)->...` dereferences null.
   - The crash happens on `remove()`, `set_enabled()`, `is_enabled()` **and on reading any field** of a handle whose bind is gone. The bind may be gone because you removed it, because a same-key bind was removed (point 1), or because of `hl.unbind`.
   - `tostring(h)` is safe: it returns `"HL.Keybind(expired)"` (L34–44). So is `==`. The module guards with `tostring`.

### Cursor position [src LuaBindingsQuery.cpp L282–296; local]

`hl.get_cursor_pos()` returns `{x, y}` from `getMouseCoordsInternal()`. These are global logical floats in the same space as `hyprctl cursorpos`, which truncates them. Probe: `hl.get_cursor_pos()` gave `1339.391, 572.619` where `hyprctl cursorpos` gave `1339, 572`.

Call it inside the bind callback, which runs synchronously at the press. Asking `hyprctl cursorpos` afterwards is an extra round trip and races with pointer motion.

To get physical pixels: `(p − monitor.pos) × scale`. On DP-2 at 1.25 that gives (1674.24, 715.77).

---

## 3. A3: transport from Lua to Snip

| Option | Latency | Compositor cost and risk | Privacy | Notes |
|---|---|---|---|---|
| **socket2 `custom>>`** via `hl.dispatch(hl.dsp.event(s))` | Write is immediate. Measured hop: **3.1 µs median, 46 µs p99** [local synthetic, `hop.c`, 3000 samples] | Non-blocking `write()` to each socket2 client inside the event. The payload is cut at 1024 characters and newlines become spaces. A client with 64 queued events is disconnected (`EventManager.cpp` L126–192) [src] | Broadcast to every socket2 client. Here that is snipd, quickshell (omarchy-shell) and Omarchy's `socat` loop in `omarchy-hyprland-monitor-watch` [local `ss -xp`] | Official API (`event(string)`: "Send an event to socket2", wiki dispatchers.md L59). Snip's `snip_hypr::subscribe` already reads socket2 |
| 0600 file in `$XDG_RUNTIME_DIR` (tmpfs) via `io.open(path,"a")` plus inotify in Snip | **2.9 µs median, 44 µs p99** [local synthetic] | A tmpfs `write()` never blocks. Lua must handle `io.open` failing | Private: only a process that opens the file can read it | More code: create, watch, offsets, delete. `io` is an incidental API, not part of `hl.*` |
| FIFO or `io.popen(cmd,"w")` relay | n/a | **Blocks the compositor** when the reader stalls (64 KiB pipe). Hyprland ignores SIGPIPE (`Compositor.cpp` L783), but a full pipe still freezes it | | Reject |
| `os.execute`, `io.popen(...,"r")`, `hl.exec_cmd` per event | ≥ 1 ms | `fork()` of a 250 MB-RSS Hyprland per event [local ps]. `os.execute` blocks until the child exits, and the Lua watchdog cannot interrupt C calls | | Reject |
| `hl.dsp.global` per key | Wayland event | One registered shortcut per keycode, no payload | Private | Reject (a hack) |

**Recommendation: socket2 `custom>>`.**
- The latency is the same as the file, and it is negligible next to frame pacing: 7–14 ms at 144 Hz and 17–33 ms at 60 Hz before the overlay can show anything.
- It is the official mechanism, and Snip already reads socket2.
- The privacy cost is bounded:
  - Filtering at the source means plain typing never leaves the compositor in the default mode.
  - Same-user processes can already install their own `hl.on` hook with `hyprctl eval`, so broadcasting creates no new capability. The real risk is accidental: a debug script that logs raw socket2 traffic.
- Switch to the file if "all keys" becomes the default or if Q-S5 is decided for privacy. It is a small change inside the bridge module: `emit()` writes to `S.f` instead of dispatching.

---

## 4. B1: recording a window

### What CleanShot did

- **4.x had window recording by Space.** The 4.7.6 binary carries `"Drag to record a part of the screen."` followed by the Space-key asset `selectWindowVideo` and `" to select a window."`. So does 4.8.11 [doc: vendor binaries, extracted in scratch].
  - A March 2024 review says: "the ability to snap to a specific window by pressing the spacebar" [doc daveswift.com].
  - The 1.0 changelog lists "Single window recording" [doc cleanshot.com/changelog].
- **It records a display rectangle, not a window buffer.**
  - 4.7.6, 4.8.11 and 5.0.1 all reference ScreenCaptureKit's `initWithDisplay:excludingApplications:exceptingWindows:` and `setSourceRect:`.
  - None references `initWithDesktopIndependentWindow:` or `initWithDisplay:includingWindows:` [local grep of the vendor images; the 5.0.1 DMG checks out as sha256 85ab95c8…].
  - Conclusion: Space snaps the recorded rectangle to the window. Following the window is not documented; `updateConfiguration:` exists, but I found no evidence that it is used for tracking [inf].
- **5.0 adds a background.** "Press space to capture a single window… now that same trick works for screen recordings too… the video editor automatically adds a background" [doc Digital Trends, Sep 2026]. The background is a 5.0 editor feature, outside the pre-Studio scope.

### Options compared

| | (i) gsr `-w region` (KMS) | (ii) gsr `-w portal` (XDPH window share) | (iii) Snip's own toplevel capture plus encoder |
|---|---|---|---|
| Follows the window | No: fixed rectangle | Yes (hyprland-toplevel-export of the window) [src Screencopy.cpp L375–381] | Yes (ext-image-copy-capture toplevel source, as for screenshots) |
| Covered by other windows | Records what is on screen | Never covered | Never covered |
| Window resized | Fixed rectangle | PipeWire renegotiates. gsr keeps the **initial** video size and scales the content into it on black [src gsr 6.1.0 portal.c L288–301, L340–373] | Snip must handle it |
| Snip's keystroke, click and webcam overlays | **Recorded** (on-screen layers) | Not recorded. Needs burn-in (gsr `-p` plugin) or post-processing | Not recorded. Can be composited in-process |
| Translucent client areas | As seen | Alpha cleared to **black** [src portal.c L360] | Alpha available, so a CleanShot-5 background is possible |
| `no_screen_share` windows | Recorded (KMS ignores it) | Black | "Denied" texture |
| Outputs | DRM outputs only. Here gsr lists **only DP-2**, so HEADLESS-1 (wayvnc) cannot be recorded [local `--list-monitors`] | Any | Any |
| User interface | Snip's overlay only | The XDPH picker **every time** (here `hyprland-preview-share-picker`), unless Snip becomes the picker (below) | Snip's overlay only |
| Robustness here | Omarchy's default path | Omarchy keeps the portal opt-in: it "can fail EGL DMA-BUF modifier import" [local omarchy-capture-screenrecording L9–15]. Windows are capped at 60 fps (XDPH default) [src] | New code on NVIDIA: dmabuf plus EGL→CUDA or Vulkan Video |
| Audio | gsr | gsr | Snip: PipeWire capture plus muxing |
| e2e-testable in nested Hyprland | Yes | No: the portal is tied to your real session [inf] | Yes |
| Cost | About days: reuse the REC area flow | About a week, plus taking over the user's config | Weeks |

**Making Snip the picker for (ii)** [src XDPH ScreencopyShared.cpp L28–128, PortalManager.cpp L70–82, share-picker main.cpp L168–174; wiki xdph]:
- XDPH runs `screencopy:custom_picker_binary` synchronously, passing `--allow-token` when `allow_token_by_default` is set.
- It sets `XDPH_WINDOW_SHARING_LIST`: entries of the form `handleLo[HC>]class[HT>]title[HE>]<Hyprland address, decimal>[HA>]`.
- It expects `[SELECTION]<flags>/window:<handleLo>` on stdout.
- A shim could match Snip's target window by its address and otherwise `exec` the real picker. That requires:
  - editing `~/.config/hypr/xdph.conf` (needs your consent);
  - restarting XDPH, because it reads its config only at start;
  - a short-lived handoff file. Another app's share request could grab it.
- Restore tokens do not help. XDPH restores by window handle, or else **the first window of the same class** (Screencopy.cpp L197–226), which can be the wrong window.

**Cost of (iii)**, beyond the existing screenshot code (`crates/snip-wayland/src/state.rs`, one-shot shm sessions):
- **Shm path.** A 1600×1000 logical window at 1.25 is 2000×1250 px, 10 MB per frame. Shm costs a synchronous `glReadPixels` on Hyprland's main thread for each frame, roughly 2–6 ms (scaled from the platform doc's 4–12 ms per 20–33 MB frame) [inf]. At 60 fps that stutters the whole desktop.
- **Dmabuf path.** Needs GBM buffers with the advertised modifiers and zero-copy into NVENC (EGL→CUDA, as gsr does) or into Vulkan encoders. ffmpeg 9 here has `h264_vulkan`, `hevc_vulkan` and `av1_vulkan`; gsr calls Vulkan codecs "experimental".
- **Everything else:** PipeWire audio, A/V sync, pause and resume, and a crash-safe container.
- Qt 6.11's `QVideoFrameInput` plus `QMediaRecorder` are installed (headers present). They could handle encoding for a prototype. Hardware encode and dmabuf zero-copy through Qt are unverified [inf].

### Recommendation

**v1: (i).** It matches CleanShot 4.x and needs no new pipeline. It already fits REC-01 ("window's rectangle in P0") and the §4.3 snapping rule. Details:
- The rectangle is the client area, `at`/`size` without the border, as `omarchy-capture-region` uses [local L48–49].
- Clamp it to the window's monitor. Snap it **inward** to gsr's constraints.
- Read the window's geometry again by its address just before gsr starts (after the countdown).
- Keep the keystroke overlay, webcam and click rings **inside** the rectangle, and the controls outside it (REC-05).
- Hide window and area recording on outputs that gsr KMS doesn't list (HEADLESS-*).

**P2 (CleanShot-5 style, with a background): (iii).** (ii) is at most an opt-in toggle, as Omarchy does with `OMARCHY_SCREENRECORD_USE_PORTAL`.

---

## 5. Recommended design

### 5.1 The Lua module (exact; checked against a mock of the 0.56.2 semantics: 33 checks pass under lua5.5 and lua5.4)

Snip sends this whole module, followed by a `return SNIP_REC.on(...)` line, at record start and after every `configreloaded`. It is idempotent: `on()` begins with `off()`, the timer is created once per Lua state and reused, and handles are cleared before removal.

```lua
-- Snip recording hooks for Hyprland 0.56 (Lua config). Sent whole with
-- an eval or repl request at record start and after every configreloaded event;
-- idempotent. SNIP_REC.off() removes everything.
SNIP_REC = SNIP_REC or {}
local S = SNIP_REC

-- Remove everything this module installed. Safe to call any number of times.
function S.off()
  local key, btns = S.key, S.btns
  S.live, S.key, S.btns = false, nil, nil -- clear first: no handle is removed twice
  if key then key:remove() end -- HL.EventSubscription:remove() is idempotent
  for _, b in ipairs(btns or {}) do
    -- HL.Keybind:remove() on an expired handle segfaults 0.56.2; tostring() is safe
    if tostring(b) ~= "HL.Keybind(expired)" then b:remove() end
  end
  if S.wd then S.wd:set_enabled(false) end
  return "off"
end

-- cfg.sid: session id. cfg.keys: nil (no keystrokes) or { all = bool,
-- mods, cmd, special = sets of xkb keycodes }. cfg.private: set of window
-- classes, cfg.private_layers: set of layer namespaces (a password prompt
-- such as "omarchy-polkit") to stay silent in. cfg.buttons: { 272, 273 } or {}.
function S.on(cfg)
  S.off()
  S.sid, S.muted, S.miss, S.down = cfg.sid, false, 0, {}
  S.private, S.private_layers = cfg.private or {}, cfg.private_layers or {}
  local event, dispatch, fmt = hl.dsp.event, hl.dispatch, string.format
  local function emit(s) dispatch(event("snip:" .. S.sid .. ":" .. s)) end

  local function private()
    local w = hl.get_active_window()
    if w ~= nil and S.private[w.class] then return true end
    for ns in pairs(S.private_layers) do
      for _, l in ipairs(hl.get_layers({ namespace = ns })) do
        if l.mapped then return true end
      end
    end
    return false
  end

  local k = cfg.keys
  if k then
    local function cmd_held()
      for c in pairs(k.cmd) do
        if S.down[c] then return true end
      end
      return false
    end
    local function on_key(code, ms, st)
      if st ~= 0 and st ~= 1 then return end
      local down = st == 1
      if (S.down[code] == true) == down then return end -- an IME's virtual keyboard echo
      S.down[code] = down or nil
      if not (k.all or k.mods[code] or k.special[code] or cmd_held()) then return end
      if S.muted or private() then return end
      emit(fmt("k:%d:%d:%d", code, st, ms))
    end
    S.key = hl.on("input.keyboard.key", function(code, ms, st) pcall(on_key, code, ms, st) end)
  end

  S.btns = {}
  for _, btn in ipairs(cfg.buttons or {}) do
    local function on_click()
      if S.muted then return end
      local p = hl.get_cursor_pos()
      if p then emit(fmt("c:%d:%.2f:%.2f", btn, p.x, p.y)) end
    end
    -- MOD3+MOD5 only gives the bind a modmask nobody else uses: remove() takes out
    -- every bind with the same modmask and key. ignore_mods makes it match anyway.
    S.btns[#S.btns + 1] = hl.bind("MOD3 + MOD5 + mouse:" .. btn, function() pcall(on_click) end, {
      non_consuming = true,
      ignore_mods = true,
      transparent = true,
      submap_universal = true,
      dont_inhibit = true,
      description = "Snip: recording clicks",
    })
  end

  -- Watchdog: one timer per Lua state, reused; off() once Snip stops beating.
  S.wd = S.wd or hl.timer(function()
    if not S.live then return end
    S.miss = S.miss + 1
    if S.miss > 3 then S.off() end
  end, { timeout = 1000, type = "repeat" })
  S.live = true
  S.wd:set_enabled(true)
  return "on"
end

function S.beat(sid)
  if S.live and sid == S.sid then
    S.miss = 0
    return "on"
  end
  return "off"
end

function S.mute(on)
  S.muted = on == true
  return S.muted and "muted" or "unmuted"
end
```

### 5.2 Requests Snip sends over `.socket.sock`

Always send a **leading `/`** and **`shutdown(Write)` after writing**. §6 explains both.

| When | Request | Reply |
|---|---|---|
| Install (start, and after `configreloaded>>`) | `/repl <module>\nreturn SNIP_REC.on({ sid = "7f3a9c21", keys = { all = false, mods = {[37]=true,[105]=true,[50]=true,[62]=true,[64]=true,[108]=true,[133]=true,[134]=true}, cmd = {[37]=true,[105]=true,[64]=true,[108]=true,[133]=true,[134]=true}, special = {[9]=true,[22]=true,[23]=true,[36]=true,[119]=true,[111]=true,[113]=true,[114]=true,[116]=true} }, private = {["1password"]=true,["1Password"]=true,["Bitwarden"]=true,["chrome-nngceckbapebfimnlniiiahkandclblb-Default"]=true}, private_layers = {["omarchy-polkit"]=true}, buttons = {272, 273} })` | `on` |
| Heartbeat (every 1 s while recording) | `/repl SNIP_REC and SNIP_REC.beat("7f3a9c21") or "off"` | `on`, or `off`, which means reinstall |
| Mute, unmute (lock, pause) | `/eval SNIP_REC and SNIP_REC.mute(true)` | `ok` |
| Stop, discard, restart | `/eval SNIP_REC and SNIP_REC.off()` | `ok` |

- Snip should compute `mods`, `cmd` and `special` from its own xkb keymap, so remaps such as Caps→Ctrl are respected. The codes above are the US defaults.
- Without a heartbeat the watchdog removes everything within 4–5 s, which covers `kill -9` and a hung Snip. A config reload removes it immediately.
- An alternative to the heartbeat is to check `/proc/<snip pid>/stat` from the timer. It needs no IPC, but it misses a hung Snip.

### 5.3 Snip side

- **Events.** These lines arrive on the existing socket2 reader:
  - `custom>>snip:<sid>:k:<code>:<state>:<ms>`
  - `custom>>snip:<sid>:c:<button>:<x>:<y>`

  Ignore any other sid. Feed key events to `xkb_state_update_key` (the layout comes from `activelayout>>`) to get labels, held modifiers and repeat counts. Map clicks to the right monitor and physical pixels. Drop clicks that land on Snip's own controls pill.
- **Lock.** Lua cannot see the lock state in 0.56.2. There is no event, no API and no socket2 event; even Omarchy notes "Hyprland reports no lock state directly" [local omarchy-hyprland-session-locked].
  1. At lock, Hyprland calls `rawSurfaceFocus(nullptr)`, which posts `activewindowv2>>` with **empty** data [src SessionLockManager.cpp L96, FocusState.cpp L241–247]. On that event, mute at once.
  2. Then check `j/monitors`. If any `solitaryBlockedBy` contains `"LOCK"` [src Monitor.cpp L1834–1837, HyprCtl.cpp L150], stay muted. Otherwise unmute.
  3. Repeat the check on every heartbeat. This is the backstop for locking while focus was already empty, when no event fires.
- **Private windows beyond the static set (optional).** On `activewindowv2>>ADDR`, run `getprop address:0xADDR no_screen_share` (it works here [local]) and mute while it is true. This follows your own rules as well as Omarchy's 1Password and Bitwarden rules.
- **Pause:** mute while paused.

### 5.4 Window recording v1 flow

1. In the REC overlay, Space switches to window mode. Hover uses the socket2-cached client list. Click picks the window.
2. The rectangle is the client area from `at`/`size`, clamped to the window's monitor and snapped inward (§4.3).
3. Countdown.
4. Read the window again by its address.
5. Start `gsr -w <WxH+X+Y>`. The rectangle stays fixed, as in CleanShot 4.x.
6. Disable window recording for outputs missing from `gpu-screen-recorder --list-monitors`.

---

## 6. Side findings that affect existing code

1. **`repl` returns values.** CLAUDE.md says `hyprctl eval` never answers with a value; that is true for `eval`. `hyprctl repl '<expr>'` returns the value [src HyprCtl.cpp L1110–1124; wiki using-hyprctl.md L34–41; local `hyprctl repl 'hl.version()'` → `0.56.2`]. An empty-string result comes back as `unknown request` [local].
2. **Send raw socket requests with a leading `/`.** Without it, if the request contains a `/` anywhere, `getReply` parses the first word as flags (L2069–2096).
   - The `r` in `repl` sets `reloadAll`. That schedules a monitor-rule reload, resets keyboard layout and pointer configs, and re-applies window rules (L2126–2150).
   - `eval` only sets the harmless `a` flag.
   - Snip's `request()` sends `eval …` today. It should send `/eval …` and `/repl …`, as hyprctl does.
3. **A request whose length is an exact multiple of 1023 bytes can freeze Hyprland.**
   - The server reads 1023 bytes at a time on a **blocking** socket (`accept4(..., SOCK_CLOEXEC)`) and stops only on a short read (L2242, L2271–2281). With an exact multiple it blocks on the main thread until the client gives up. That is 2 s with Snip's read timeout, 5 s with hyprctl's.
   - Large `eval` chunks make this reachable. Fix: `UnixStream::shutdown(Shutdown::Write)` after `write_all` in `crates/snip-hypr/src/lib.rs::request`.
   - Not reproduced live, on purpose.
4. **`keys.rs::session_lua` (scroll-capture Enter and Esc).**
   - It binds bare `"Return"` and `"Escape"` (modmask 0). Removing them also removes any user bind written exactly `Return` or `Escape` with no modifiers, in any submap, until the next reload. Your service submap uses `"escape"`, which is spared only because of its spelling.
   - Each session adds a new 15-minute one-shot timer. An older timer can unbind a later session's keys mid-capture.
   - The module above avoids both problems: a unique modmask with `ignore_mods`, and one reused timer.
5. The `hyprctl binds` parser should skip binds described as `Snip: recording clicks` (modmask 160).
6. The wiki's main branch is ahead of 0.56.2. It lists `config.unload` and `window.bell`, which 0.56.2 lacks, and it calls the key timestamp a "Unix timestamp". Use `/usr/share/hypr/stubs/hl.meta.lua` as the authority for 0.56.

---

## 7. Verification plan (S9, in the nested e2e Hyprland)

The `input.keyboard.key` hook fires for wtype keys, because the emit runs before any bind logic. Harness clicks go through `onMouseButton`, so they trigger binds. Both features can therefore be tested in the nested session:

- **keys**: install, send Ctrl+C with wtype, and check that the `k:` lines arrive in order within 5 ms.
- **click passthrough**: a test client receives press and release, plus a full drag, while the `c:272` lines arrive.
- **watchdog**: stop the heartbeats. Within 5 s `hyprctl binds` has no "Snip: recording clicks" and `beat` answers `off`.
- **reload**: run `hyprctl reload`. The hooks are gone and Snip reinstalls them.
- **idempotent**: install twice and `off()` twice. `hyprctl version` still answers.

---

## 8. Decisions for the product owner (next D-numbers)

1. **Q-S5, transport:** socket2 `custom>>` (recommended) or the private runtime file.
2. **Lock during a recording:** muting keystrokes is required. Should the video also pause? I recommend pausing.
3. **Keystroke default:** "command keys only" (recommended for privacy) or "all keys". Also: should Backspace, Enter and Tab count as special keys?
4. **Window recording v1:** a fixed rectangle as in CleanShot 4.x. True window capture with a background stays P2.
5. **Windows on outputs gsr KMS cannot record** (HEADLESS-1): hide the option, or use the portal as a fallback?

---

## 9. Sources

- **Hyprland v0.56.2** (`https://github.com/hyprwm/Hyprland/blob/v0.56.2/…`; clone at `…/scratchpad/research/hyprland-src`):
  - `src/config/lua/LuaEventHandler.cpp` L24–82, L179–185, L266–303
  - `src/config/lua/bindings/LuaBindingsToplevel.cpp` L58–256, L377–402, L418–528
  - `src/config/lua/bindings/LuaBindingsQuery.cpp` L282–296, L343–374
  - `src/config/lua/objects/LuaKeybind.cpp`, `LuaEventSubscription.cpp`, `LuaTimer.cpp`
  - `src/config/lua/ConfigManager.cpp` L458–520, L700–760, L857–945; `ConfigManager.hpp` L107–114
  - `src/managers/input/InputManager.cpp` L748–789, L861–873, L1156–1159, L1619–1680
  - `src/managers/KeybindManager.cpp` L197–219, L347–431, L477–527, L602–852
  - `src/managers/EventManager.cpp` L126–192
  - `src/config/shared/actions/ConfigActions.cpp` L1268–1271
  - `src/debug/HyprCtl.cpp` L150, L1110–1124, L2062–2150, L2242–2281
  - `src/managers/SessionLockManager.cpp` L51–100
  - `src/desktop/state/FocusState.cpp` L223–248
  - `src/output/Monitor.cpp` L1800–1840
  - `hyprctl/src/main.cpp` L201–282
- **XDPH v1.4.1** (`https://github.com/hyprwm/xdg-desktop-portal-hyprland/blob/v1.4.1/…`):
  - `src/shared/ScreencopyShared.cpp` L28–128
  - `src/portals/Screencopy.cpp` L17–38, L197–246, L585–598
  - `src/core/PortalManager.cpp` L70–82
  - `hyprland-share-picker/main.cpp` L168–174
- **gpu-screen-recorder 6.1.0**: `src/capture/portal.c` L288–301, L340–373; the man page at `…/scratchpad/gsr-man.txt`; `/usr/include/gsr/plugin.h`.
- **Hyprland wiki** (main @ ede823b, `https://github.com/hyprwm/hyprland-wiki/blob/main/content/…`):
  - `configuring/core/advanced-configuration/events.md` L38
  - `…/using-hyprctl.md` L17–45
  - `…/lua-utilities.md` L43–53, L113–125
  - `configuring/core/binds/flags.md` L25–37
  - `configuring/core/binds/_index.md` L99–118
  - `configuring/core/dispatchers.md` L59
  - `hypr-ecosystem/user/xdg-desktop-portal-hyprland.md` L163–164
- **Local files**:
  - `/usr/share/hypr/stubs/hl.meta.lua` (L5–36, L437–453, L624–672, L820–868)
  - `/usr/share/omarchy/bin/omarchy-capture-screenrecording` (L9–15, L126–170), `omarchy-hyprland-session-locked`, `omarchy-capture-region` (L48–49)
  - `/usr/share/omarchy/shell/plugins/polkit/PolkitAgent.qml` (L221–228)
  - `/usr/share/omarchy/default/hypr/apps/{1password,bitwarden}.lua`, `…/bindings/utilities.lua` (L44–82)
  - `~/.config/hypr/xdph.conf`, read only
  - `crates/snip-hypr/src/lib.rs` (L22–31), `crates/snip-hypr/src/keys.rs` (L39–53)
- **CleanShot**:
  - https://cleanshot.com/changelog
  - https://daveswift.com/cleanshot-x/ (2024-03-18)
  - https://www.digitaltrends.com/computing/the-best-screenshot-app-for-mac-just-became-a-screen-recording-powerhouse/
  - Vendor binaries 4.7.6 and 4.8.11 (extracted in scratch by another agent) and 5.0.1 (https://updates.getcleanshot.com/v3/CleanShot-X-5.0.1.dmg, inspected, then deleted)
- **Apple TN2150**: https://developer.apple.com/library/archive/technotes/tn2150/_index.html

## Files

Scratch only, under `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research/`:
- `snip-rec.lua`: the module
- `mock-test.lua`: the mock of the 0.56.2 semantics, 33 checks; `lua5.5 mock-test.lua`
- `hop.c`: the transport latency benchmark
- `hyprland-src/` (v0.56.2), `xdph-src/` (v1.4.1), `hyprland-wiki/`: source clones
- `cleanshot-changelog.txt`