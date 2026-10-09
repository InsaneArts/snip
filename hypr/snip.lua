-- Snip for Hyprland (OMA-02). Load it from ~/.config/hypr/hyprland.lua,
-- after Omarchy's defaults and your own bindings:
--
--   dofile("/usr/share/snip/hypr/snip.lua")
--
-- Deleting that line gives Omarchy its own bindings back. Options (set before
-- the dofile):
--
--   snip_print_keys = true     -- also take PRINT (area) and SUPER+CTRL+PRINT (text)
--   snip_keys = false          -- no keys; bind snip:<action> yourself
--
-- Every binding dispatches a global shortcut to the running snipd
-- (hl.dsp.global): no process is spawned, so the overlay appears instantly.
-- Settings › Shortcuts lists every action, with its keys. While a scrolling
-- capture runs, Snip binds Enter and Esc itself.
--
-- Settings › Shortcuts records other keys for any action; snipd binds those
-- over these defaults (config.toml [shortcuts]), and Restore Defaults calls
-- snip_bind_defaults() again. A bind's description names its action (the
-- action's name in snipd), which is how Settings finds the keys.

local function bind(keys, action, name)
  pcall(hl.unbind, keys) -- the key may not be bound yet
  hl.bind(keys, hl.dsp.global("snip:" .. action), { description = "Snip: " .. name })
end

function snip_bind_defaults()
  if snip_keys == false then
    return
  end
  -- macOS muscle memory (product spec §7). These replace Omarchy's "move
  -- window to workspace 3/4/5/6"; SUPER+SHIFT+ALT+3..6 (move silently) stay.
  bind("SUPER + SHIFT + code:12", "fullscreen", "Capture fullscreen")
  bind("SUPER + SHIFT + code:13", "area", "Capture area")
  bind("SUPER + SHIFT + code:14", "all-in-one", "All-In-One")
  bind("SUPER + SHIFT + code:15", "edit-clipboard", "Edit the image on the clipboard")

  if snip_print_keys then
    bind("PRINT", "area", "Capture area")
    bind("SUPER + CTRL + PRINT", "text", "Capture text (OCR)")
  end
end

snip_bind_defaults()

-- Snip's window and layer rules (floating, centred editors and Settings,
-- pins on every workspace, no fade for its overlays) come from snipd itself,
-- at start and after every config reload, so they live in one place.
