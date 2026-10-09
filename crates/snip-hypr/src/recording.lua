-- Snip's recording hooks (REC-09/10): the command keys and the clicks, sent to
-- snipd as socket2 events ("custom>>snip:<sid>:k:<code>:<0|1>" and
-- "custom>>snip:<sid>:c:<button>:<x>:<y>", global logical px) while it records.
-- Sent whole, so it's idempotent; SNIP_REC.off() removes everything, and a
-- timer does it once snipd's heartbeat stops (a crash leaves nothing behind).
SNIP_REC = SNIP_REC or {}
local S = SNIP_REC

function S.off()
  local key, btns = S.key, S.btns
  S.live, S.key, S.btns = false, nil, nil -- cleared first: no handle is removed twice
  if key then key:remove() end
  for _, b in ipairs(btns or {}) do
    -- removing an expired bind handle crashes Hyprland 0.56; tostring() is safe
    if tostring(b) ~= "HL.Keybind(expired)" then b:remove() end
  end
  if S.wd then S.wd:set_enabled(false) end
  return "off"
end

-- cfg.sid: the recording's. cfg.keys: nil, or { mods, cmd, special } (sets of
-- xkb keycodes): only command keys leave the compositor, never typing.
-- cfg.private / cfg.private_layers: window classes and layer namespaces (a
-- password prompt) to stay silent in. cfg.buttons: { 272, 273 } or {}.
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
      if (S.down[code] == true) == down then return end -- an input method's echo
      S.down[code] = down or nil
      if not (k.mods[code] or k.special[code] or cmd_held()) then return end
      if S.muted or private() then return end
      emit(fmt("k:%d:%d", code, st))
    end
    S.key = hl.on("input.keyboard.key", function(code, ms, st) pcall(on_key, code, ms, st) end)
  end

  S.btns = {}
  for _, btn in ipairs(cfg.buttons or {}) do
    local function on_click()
      if S.muted then return end
      local p = hl.get_cursor_pos()
      if p then emit(fmt("c:%d:%.1f:%.1f", btn, p.x, p.y)) end
    end
    -- MOD3+MOD5 gives the bind a modmask nobody else uses: remove() takes out
    -- every bind with the same modmask and key. ignore_mods matches anyway.
    S.btns[#S.btns + 1] = hl.bind("MOD3 + MOD5 + mouse:" .. btn, function() pcall(on_click) end, {
      non_consuming = true,
      ignore_mods = true,
      transparent = true,
      submap_universal = true,
      dont_inhibit = true,
      description = "Snip: recording clicks",
    })
  end

  -- the watchdog: one timer per Lua state, reused
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
  if S.live and sid == S.sid then S.miss = 0 end
end

function S.mute(on)
  S.muted = on == true
end
