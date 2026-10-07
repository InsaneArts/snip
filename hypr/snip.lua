-- Snip for Hyprland (OMA-02). Load it from ~/.config/hypr/hyprland.lua,
-- after Omarchy's defaults and your own bindings:
--
--   dofile("/usr/share/snip/hypr/snip.lua")
--
-- Deleting that line gives Omarchy its own bindings back. Options (set before
-- the dofile):
--
--   snip_print_keys = true     -- also take PRINT (area) and SUPER+CTRL+PRINT (text)
--   snip_keys = false          -- rules only; bind snip:<action> yourself
--
-- Every binding dispatches a global shortcut to the running snipd
-- (hl.dsp.global): no process is spawned, so the overlay appears instantly.
-- Actions: area, fullscreen, window, previous, text, all-in-one, restore, close-all,
-- timer-area, timer-fullscreen, edit-clipboard (open the clipboard's image in the editor).

local function bind(keys, action, description)
  pcall(hl.unbind, keys) -- the key may not be bound yet
  hl.bind(keys, hl.dsp.global("snip:" .. action), { description = "Snip: " .. description })
end

if snip_keys ~= false then
  -- macOS muscle memory (product spec §7). These replace Omarchy's "move
  -- window to workspace 3/4/5"; SUPER+SHIFT+ALT+3/4/5 (move silently) stay.
  bind("SUPER + SHIFT + code:12", "fullscreen", "capture fullscreen")
  bind("SUPER + SHIFT + code:13", "area", "capture area")
  bind("SUPER + SHIFT + code:14", "all-in-one", "all-in-one")

  if snip_print_keys then
    bind("PRINT", "area", "capture area")
    bind("SUPER + CTRL + PRINT", "text", "capture text")
  end
end

-- Snip's surfaces appear instantly: no layer fade (which would also show up
-- in the first frames of a recording). snipd re-applies this at runtime too.
hl.layer_rule({ match = { namespace = "^snip-" }, no_anim = true, animation = "none" })

-- The editor is a floating window, centred, sized to the image.
-- (Hyprland 0.56: a rule matching class and title together, or a title
-- pattern with non-ASCII characters, never matches.)
hl.window_rule({ match = { title = "^Snip Editor" }, float = true, center = true, opacity = "1 1" })
