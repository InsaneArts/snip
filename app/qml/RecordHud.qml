import QtQuick
import QtQuick.Window
import QtQuick.Effects
import QtMultimedia
import org.kde.layershell as LayerShell
import Snip.Core

// Screen recording (REC-01…05), CleanShot's (D49): Record Screen shows its
// panel at once, over the whole screen: the size, Full screen and Select
// area, the microphone, the computer's sound, the camera, clicks and keys,
// then Record GIF (Alt+Enter) and Record Video (Enter). Drag anywhere to
// record an area instead (its handles resize it, a drag inside moves it, the
// screen is dimmed outside it, REC-04), Space for a window, or type its size.
// Then a 3-2-1 countdown (REC-03), and while it records, the controls: stop
// with the time, pause, restart, discard (REC-05). The camera (REC-08) is
// a bubble inside the area, there to be recorded: dragged anywhere, before
// and while recording; a click fills the area with it, another brings it back.
// While recording it can also show a ring where the pointer clicks (REC-09)
// and the command keys pressed (REC-10), from Hyprland's own hooks.
//
// gpu-screen-recorder records whatever is on screen in the area, Snip's own
// layers included: once it records, this layer draws nothing inside the area
// but what's meant to be in the video (the camera, clicks, keys); the frame
// and the dimming keep a 2 px margin, and the panel and the countdown are
// gone before the first frame. When the whole monitor is recorded the
// controls would be in the video, so there are none: the Record key, `snip
// record stop` or the bar stops it. While choosing, this full-output layer
// takes the pointer and the keyboard; while recording, only the controls and
// the camera take the pointer.
Window {
    id: hud
    property var controller: null
    property string output: ""
    property rect area: Qt.rect(0, 0, 0, 0)       // logical px on the output
    property bool whole: false
    property string phase: "closed"                // ready | countdown | starting | launching | recording | paused | finishing | closed
    property real scale: 1                          // the output's (Hyprland's: Qt rounds 1.25 up)
    property bool gif: false
    property var mics: []                           // [{id, label}]
    property string mic: ""                         // "" none, or a mic's id
    property bool system: false
    property bool dim: true
    property int seconds: 0                         // the countdown's
    property real elapsedBase: 0                    // ms recorded at `since`
    property real since: 0                          // Date.now() when elapsedBase was right
    property real elapsed: 0                        // ms, shown
    property bool micsOpen: false
    property string camera: ""                      // the camera's name ("" none)
    property string cameraShape: "circle"           // circle | square | rectangle | vertical
    property string cameraSize: "medium"            // small | medium | large | huge
    property bool cameraMirror: true
    property bool fakeCamera: false                 // tests: a test card is a camera
    property bool camerasOpen: false
    property bool cameraFull: false                 // filling the area
    property point cameraAt: Qt.point(-1, -1)       // its corner from the area's (-1: not moved)
    property bool clicks: false                     // REC-09: a ring where the pointer clicks
    property bool keys: false                       // REC-10: the command keys pressed
    property var heldMods: []                       // the modifiers held, by label
    property string combo: ""                       // the last command, e.g. "Ctrl + C"
    property int comboCount: 0                      // pressed again and again
    property real comboAt: 0
    property bool comboShown: false
    property int ringsShown: 0                      // tests: rings drawn so far
    property string asking: ""                      // "restart" | "discard": the controls ask first
    property size screenSize: Qt.size(0, 0)         // the output's, logical px
    property var windows: []                        // [{x, y, w, h, title}] on this output, front-most first
    property bool picking: false                    // Space: a window is chosen
    property bool drawing: false                    // Select area: the next drag draws one
    property int hoveredWindow: -1
    property rect lastArea: Qt.rect(0, 0, 0, 0)     // what Full screen goes back to

    flags: Qt.FramelessWindowHint
    color: "transparent"
    visible: false
    LayerShell.Window.screen: Platform.screen(output !== "" ? output : Qt.application.screens[0].name)
    LayerShell.Window.scope: "snip-record"
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
                               | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
    LayerShell.Window.exclusionZone: -1
    // the keyboard while choosing (Enter, Esc, Space, the size), none while recording
    LayerShell.Window.keyboardInteractivity: phase === "ready" || phase === "countdown" ? LayerShell.Window.KeyboardInteractivityExclusive
                                                                                       : LayerShell.Window.KeyboardInteractivityNone

    function open(s) {
        output = s.output
        screenSize = Qt.size(s.ow || s.w, s.oh || s.h)
        lastArea = Qt.rect(0, 0, 0, 0)
        setArea(Qt.rect(s.x, s.y, s.w, s.h))
        windows = s.windows || []
        picking = false
        drawing = false
        hoveredWindow = -1
        scale = s.scale || 1
        gif = !!s.gif
        mics = s.mics || []
        mic = s.mic || ""
        system = !!s.system
        dim = s.dim !== false
        if (!mediaDevices) mediaDevices = mediaDevicesComponent.createObject(hud)
        camera = s.camera || ""
        cameraShape = s.cameraShape || "circle"
        cameraSize = s.cameraSize || "medium"
        cameraMirror = s.cameraMirror !== false
        fakeCamera = !!s.fakeCamera
        camerasOpen = false
        cameraFull = false
        cameraAt = Qt.point(-1, -1)
        clicks = !!s.clicks
        keys = !!s.keys
        heldMods = []
        combo = ""
        comboShown = false
        asking = ""
        // `snip record area|fullscreen`: recording at once, no panel
        phase = s.now ? "starting" : "ready"
        micsOpen = false
        elapsed = 0
        visible = true
        keyboard.forceActiveFocus()
        Qt.callLater(fitInput)
    }
    function status(s) {
        if (s.state === "countdown") {
            seconds = s.seconds
            phase = "countdown"
            tick.restart()
            return
        }
        phase = s.state
        if (phase !== "recording" && phase !== "paused") asking = ""
        elapsedBase = s.elapsed || 0
        since = Date.now()
        elapsed = elapsedBase
        Qt.callLater(fitInput)
    }
    function close() {
        // a countdown stopped short mustn't go on to "starting"
        tick.stop()
        phase = "closed"
        visible = false
    }
    // REC-06: Snip's bar icon is red while it records
    onPhaseChanged: {
        Platform.setTrayRecording(["launching", "recording", "paused"].includes(phase))
        Qt.callLater(fitInput)
    }
    // Record Video, or GIF: what's chosen is recorded
    function record(asGif) {
        if (phase !== "ready") return
        micsOpen = false
        camerasOpen = false
        picking = false
        gif = asGif
        Engine.recordStart(asGif ? 1 : 0, area.x, area.y, area.width, area.height)
    }

    // ---------------------------------------------------------------- the area (D49)
    function setArea(r) {
        area = Qt.rect(r.x, r.y, r.width, r.height)
        whole = r.x <= 0 && r.y <= 0 && r.width >= screenSize.width && r.height >= screenSize.height
        if (!whole) lastArea = area
    }
    function spanned(x0, y0, x1, y1) {
        const cx = v => Math.max(0, Math.min(screenSize.width, v)), cy = v => Math.max(0, Math.min(screenSize.height, v))
        const a = cx(Math.min(x0, x1)), b = cy(Math.min(y0, y1)), c = cx(Math.max(x0, x1)), d = cy(Math.max(y0, y1))
        return Qt.rect(Math.round(a), Math.round(b), Math.round(c - a), Math.round(d - b))
    }
    function moved(r, dx, dy) {
        return Qt.rect(Math.round(Math.max(0, Math.min(screenSize.width - r.width, r.x + dx))),
                       Math.round(Math.max(0, Math.min(screenSize.height - r.height, r.y + dy))), r.width, r.height)
    }
    // a handle dragged: its edges move, at least 16 px apart
    function resized(r, edge, dx, dy) {
        let x1 = r.x, y1 = r.y, x2 = r.x + r.width, y2 = r.y + r.height
        if (edge.includes("l")) x1 = Math.min(x2 - 16, Math.max(0, x1 + dx))
        if (edge.includes("r")) x2 = Math.max(x1 + 16, Math.min(screenSize.width, x2 + dx))
        if (edge.includes("t")) y1 = Math.min(y2 - 16, Math.max(0, y1 + dy))
        if (edge.includes("b")) y2 = Math.max(y1 + 16, Math.min(screenSize.height, y2 + dy))
        return Qt.rect(Math.round(x1), Math.round(y1), Math.round(x2 - x1), Math.round(y2 - y1))
    }
    function inArea(x, y) { return x >= area.x && y >= area.y && x < area.x + area.width && y < area.y + area.height }
    function windowAt(x, y) {
        for (let i = 0; i < windows.length; i++) {
            const w = windows[i]
            if (x >= w.x && y >= w.y && x < w.x + w.w && y < w.y + w.h) return i
        }
        return -1
    }
    // a typed size, around the area's centre
    function setSize(w, h) {
        w = Math.max(16, Math.min(screenSize.width, w))
        h = Math.max(16, Math.min(screenSize.height, h))
        const cx = area.x + area.width / 2, cy = area.y + area.height / 2
        setArea(Qt.rect(Math.round(Math.max(0, Math.min(screenSize.width - w, cx - w / 2))),
                        Math.round(Math.max(0, Math.min(screenSize.height - h, cy - h / 2))), w, h))
    }
    // Full screen, and back to the area there was
    function toggleFull() {
        if (!whole) return setArea(Qt.rect(0, 0, screenSize.width, screenSize.height))
        const s = screenSize
        setArea(lastArea.width > 0 ? lastArea : Qt.rect(Math.round(s.width * 0.2), Math.round(s.height * 0.2), Math.round(s.width * 0.6), Math.round(s.height * 0.6)))
    }
    function key(e) {
        if (phase === "countdown") {
            if (e.key === Qt.Key_Escape) { Engine.recordCancel(); e.accepted = true }
            return
        }
        if (phase !== "ready") return
        const step = e.modifiers & Qt.ShiftModifier ? 10 : 1
        switch (e.key) {
        case Qt.Key_Return:
        case Qt.Key_Enter: record(e.modifiers & Qt.AltModifier ? true : gif); break
        case Qt.Key_Escape:
            if (micsOpen || camerasOpen) { micsOpen = false; camerasOpen = false }
            else if (picking || drawing) { picking = false; drawing = false }
            else Engine.recordCancel()
            break
        case Qt.Key_Space: picking = !picking; drawing = false; hoveredWindow = -1; break
        case Qt.Key_Left: if (!whole) setArea(moved(area, -step, 0)); break
        case Qt.Key_Right: if (!whole) setArea(moved(area, step, 0)); break
        case Qt.Key_Up: if (!whole) setArea(moved(area, 0, -step)); break
        case Qt.Key_Down: if (!whole) setArea(moved(area, 0, step)); break
        default: return
        }
        e.accepted = true
    }
    Item {
        id: keyboard
        focus: true
        Keys.onPressed: (e) => hud.key(e)
    }
    function setMic(id) {
        mic = id
        micsOpen = false
        Engine.setSetting("recording.mic", JSON.stringify(id))
        Qt.callLater(fitInput)
    }
    function setSystem(on) {
        system = on
        Engine.setSetting("recording.system_audio", JSON.stringify(on))
    }
    // the cameras there are (by name); a test card stands in for one in tests.
    // Asked for when the HUD first opens: from then on QtMultimedia's device
    // lists watch cameras and sound devices, which Snip needn't unless it records
    property var mediaDevices: null
    Component { id: mediaDevicesComponent; MediaDevices {} }
    readonly property var cameras: (fakeCamera ? [{ name: "Test camera", device: null }] : [])
                                   .concat(mediaDevices ? mediaDevices.videoInputs.map(d => ({ name: d.description, device: d })) : [])
    readonly property var cameraEntry: cameras.find(c => c.name === camera) || null
    function setCamera(name) {
        camera = name
        camerasOpen = false
        cameraFull = false
        Engine.setSetting("recording.camera", JSON.stringify(name))
        Qt.callLater(fitInput)
    }
    function setCameraLook(key, value) {
        if (key === "shape") cameraShape = value
        else if (key === "size") cameraSize = value
        else if (key === "mirror") cameraMirror = value
        Engine.setSetting("recording.camera_" + key, JSON.stringify(value))
        Qt.callLater(fitInput)
    }
    // CleanShot's sizes, for the area recorded (Omarchy's three are 18, 25
    // and 33.75 % of its height); shapes: square, 16:9 or 9:16
    readonly property real cameraMargin: Math.min(40, area.width / 10, area.height / 10)
    readonly property size cameraBox: {
        const base = area.height * ({ small: 0.18, medium: 0.25, large: 0.3375, huge: 0.45 }[cameraSize] || 0.25)
        let w = base, h = base
        if (cameraShape === "rectangle") w = base * 16 / 9
        else if (cameraShape === "vertical") { h = base * 1.4; w = h * 9 / 16 }
        // a narrow area: smaller, its margins kept
        const k = Math.min(1, (area.width - 2 * cameraMargin) / w, (area.height - 2 * cameraMargin) / h)
        return Qt.size(Math.max(24, Math.round(w * k)), Math.max(24, Math.round(h * k)))
    }
    // where it is: at the bottom right to start, then where it was dragged (kept inside)
    readonly property real cameraX: Math.round(area.x + Math.max(0, Math.min(area.width - cameraBox.width,
                                     cameraAt.x < 0 ? area.width - cameraMargin - cameraBox.width : cameraAt.x)))
    readonly property real cameraY: Math.round(area.y + Math.max(0, Math.min(area.height - cameraBox.height,
                                     cameraAt.y < 0 ? area.height - cameraMargin - cameraBox.height : cameraAt.y)))

    function setShown(key, on) {
        if (key === "highlight_clicks") clicks = on
        else keys = on
        Engine.setSetting("recording." + key, JSON.stringify(on))
    }
    // a click or a key from Hyprland's hooks (only command keys come)
    function input(e) {
        if (phase !== "recording") return
        if (e.kind === "click") {
            // outside the area it isn't recorded
            if (!clicks || e.x < area.x || e.y < area.y || e.x > area.x + area.width || e.y > area.y + area.height) return
            rings.append({ rx: e.x, ry: e.y, second: e.button !== 272 })
            ringsShown++
            return
        }
        if (!keys || !e.label) return
        if (["Ctrl", "Shift", "Alt", "Super"].includes(e.label)) {
            const rest = heldMods.filter(m => m !== e.label)
            heldMods = e.down ? rest.concat([e.label]) : rest
            return
        }
        if (!e.down) return
        const c = ["Super", "Ctrl", "Alt", "Shift"].filter(m => heldMods.includes(m)).concat([e.label]).join(" + ")
        comboCount = c === combo && comboShown && Date.now() - comboAt < 1500 ? comboCount + 1 : 1
        combo = c
        comboAt = Date.now()
        comboShown = true
        comboFade.restart()
    }
    Timer { id: comboFade; interval: 1500; onTriggered: hud.comboShown = false }
    // what the keystrokes show: the last command a moment, else the modifiers held
    readonly property string keyText: comboShown ? combo + (comboCount > 1 ? "  ×" + comboCount : "")
                                                 : heldMods.length ? heldMods.join(" + ") + " +" : ""

    readonly property string micLabel: {
        if (mic === "") return "No microphone"
        const m = mics.find(x => x.id === mic)
        return (m ? m.label : mic) + (micMuted ? " (muted)" : "")
    }
    // REC-14: a muted microphone records silence; said before it starts
    readonly property bool micMuted: { const m = mics.find(x => x.id === mic); return !!(m && m.muted) }
    function clock(ms) {
        const t = Math.floor(ms / 1000), m = Math.floor(t / 60), s = t % 60
        return m + ":" + String(s).padStart(2, "0")
    }

    // while choosing, all of it takes the pointer; while recording, only the
    // controls and the camera
    function fitInput() {
        if (phase === "ready") return Platform.setInputRects(hud, [Qt.rect(0, 0, width, height)])
        const r = []
        if (micList.visible) r.push(Qt.rect(micList.x, micList.y, micList.width, micList.height))
        if (cameraList.visible) r.push(Qt.rect(cameraList.x, cameraList.y, cameraList.width, cameraList.height))
        if (bubble.visible) r.push(Qt.rect(bubble.x, bubble.y, bubble.width, bubble.height))
        if (controls.visible) r.push(Qt.rect(controls.x, controls.y, controls.width, controls.height))
        Platform.setInputRects(hud, r)
    }
    onMicsOpenChanged: { if (micsOpen) camerasOpen = false; Qt.callLater(fitInput) }
    onCamerasOpenChanged: { if (camerasOpen) micsOpen = false; Qt.callLater(fitInput) }

    // the countdown, one second a step; at zero it's gone before recording starts
    Timer {
        id: tick
        interval: 1000
        repeat: true
        onTriggered: {
            hud.seconds -= 1
            if (hud.seconds <= 0) {
                stop()
                hud.phase = "starting"
                Platform.notifyNextFrame(hud)
            }
        }
    }
    Connections {
        target: Platform
        function onFramePresented(w) { if (w === hud && hud.phase === "starting") Engine.recordGo() }
    }
    // the time recorded so far
    Timer {
        interval: 250
        repeat: true
        running: hud.phase === "recording"
        onTriggered: hud.elapsed = hud.elapsedBase + (Date.now() - hud.since)
    }

    // ---------------------------------------------------------------- outside the area
    readonly property real margin: 2
    readonly property rect hole: Qt.rect(area.x - margin, area.y - margin, area.width + 2 * margin, area.height + 2 * margin)
    readonly property bool showDim: dim && !whole && !picking && phase !== "closed"
    readonly property color shade: Qt.rgba(0, 0, 0, 0.42)
    Rectangle { visible: hud.showDim; x: 0; y: 0; width: hud.width; height: Math.max(0, hud.hole.y); color: hud.shade }
    Rectangle { visible: hud.showDim; x: 0; y: hud.hole.y + hud.hole.height; width: hud.width; height: Math.max(0, hud.height - y); color: hud.shade }
    Rectangle { visible: hud.showDim; x: 0; y: hud.hole.y; width: Math.max(0, hud.hole.x); height: hud.hole.height; color: hud.shade }
    Rectangle { visible: hud.showDim; x: hud.hole.x + hud.hole.width; y: hud.hole.y; width: Math.max(0, hud.width - x); height: hud.hole.height; color: hud.shade }
    // the frame, just outside the area (red while recording)
    Rectangle {
        visible: !hud.whole && !hud.picking && hud.phase !== "closed"
        x: hud.hole.x - 2
        y: hud.hole.y - 2
        width: hud.hole.width + 4
        height: hud.hole.height + 4
        color: "transparent"
        radius: 3
        border.width: 2
        border.color: hud.phase === "recording" ? Theme.danger : hud.phase === "paused" ? Theme.warning : Theme.accent
    }

    // ---------------------------------------------------------------- choosing what to record (D49)
    // a drag outside the area draws a new one, inside it moves it; Space: a window
    MouseArea {
        id: selector
        anchors.fill: parent
        enabled: hud.phase === "ready"
        hoverEnabled: true
        property real sx: 0
        property real sy: 0
        property rect from: Qt.rect(0, 0, 0, 0)
        property string mode: ""
        readonly property bool overArea: !hud.whole && !hud.drawing && hud.inArea(mouseX, mouseY)
        cursorShape: hud.picking ? Qt.PointingHandCursor : overArea ? Qt.SizeAllCursor : Qt.CrossCursor
        onPressed: (m) => {
            hud.micsOpen = false
            hud.camerasOpen = false
            keyboard.forceActiveFocus()
            if (hud.picking) return
            sx = m.x
            sy = m.y
            from = hud.area
            mode = overArea ? "move" : "draw"
        }
        onPositionChanged: (m) => {
            if (hud.picking) { hud.hoveredWindow = hud.windowAt(m.x, m.y); return }
            if (!pressed || mode === "") return
            hud.setArea(mode === "move" ? hud.moved(from, m.x - sx, m.y - sy) : hud.spanned(sx, sy, m.x, m.y))
        }
        onReleased: (m) => {
            if (hud.picking) {
                const i = hud.windowAt(m.x, m.y)
                if (i < 0) return
                const w = hud.windows[i]
                hud.picking = false
                hud.setArea(hud.spanned(w.x, w.y, w.x + w.w, w.y + w.h))
                return
            }
            if (mode === "draw") {
                const r = hud.spanned(sx, sy, m.x, m.y)
                // a click draws nothing
                if (r.width >= 16 && r.height >= 16) { hud.drawing = false; hud.setArea(r) }
                else hud.setArea(from)
            }
            mode = ""
        }
    }
    // the window under the pointer, in window mode
    Rectangle {
        objectName: "record-window"
        readonly property var w: hud.hoveredWindow >= 0 ? hud.windows[hud.hoveredWindow] : null
        visible: hud.phase === "ready" && hud.picking && w !== null
        x: w ? w.x : 0
        y: w ? w.y : 0
        width: w ? w.w : 0
        height: w ? w.h : 0
        color: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.14)
        border.width: 2
        border.color: Theme.accent
    }
    // the handles, on the frame: the area's corners and edges resize it
    Repeater {
        model: ["tl", "t", "tr", "r", "br", "b", "bl", "l"]
        Rectangle {
            required property string modelData
            objectName: "record-handle-" + modelData
            visible: hud.phase === "ready" && !hud.whole && !hud.picking
            width: 10
            height: 10
            radius: 5
            color: "white"
            border.width: 2
            border.color: Theme.accent
            readonly property real out: hud.margin + 1
            x: (modelData.includes("l") ? hud.area.x - out : modelData.includes("r") ? hud.area.x + hud.area.width + out : hud.area.x + hud.area.width / 2) - width / 2
            y: (modelData.includes("t") ? hud.area.y - out : modelData.includes("b") ? hud.area.y + hud.area.height + out : hud.area.y + hud.area.height / 2) - height / 2
            MouseArea {
                anchors.fill: parent
                anchors.margins: -6
                property point start
                property rect from: Qt.rect(0, 0, 0, 0)
                cursorShape: parent.modelData === "t" || parent.modelData === "b" ? Qt.SizeVerCursor
                           : parent.modelData === "l" || parent.modelData === "r" ? Qt.SizeHorCursor
                           : parent.modelData === "tl" || parent.modelData === "br" ? Qt.SizeFDiagCursor : Qt.SizeBDiagCursor
                onPressed: (m) => { from = hud.area; start = mapToItem(hud.contentItem, m.x, m.y) }
                onPositionChanged: (m) => {
                    if (!pressed) return
                    const p = mapToItem(hud.contentItem, m.x, m.y)
                    hud.setArea(hud.resized(from, parent.modelData, p.x - start.x, p.y - start.y))
                }
            }
        }
    }

    // where a bar goes: under the area, else over it, else inside its bottom
    // (only before recording starts) or at the monitor's bottom
    function placeY(h) {
        const below = area.y + area.height + margin + 12
        if (below + h <= height - 8) return below
        const above = area.y - margin - 12 - h
        if (above >= 8) return above
        return height - h - 24
    }
    function placeX(w) { return Math.max(8, Math.min(width - w - 8, area.x + area.width / 2 - w / 2)) }

    // ---------------------------------------------------------------- the camera (REC-08)
    CameraBubble {
        id: bubble
        objectName: "record-camera"
        visible: hud.cameraEntry !== null && hud.phase !== "closed" && hud.phase !== "finishing"
        running: visible
        device: hud.cameraEntry ? hud.cameraEntry.device : null
        pixelScale: hud.scale
        fake: hud.cameraEntry !== null && hud.cameraEntry.device === null
        shape: hud.cameraFull ? "full" : hud.cameraShape
        mirror: hud.cameraMirror
        x: hud.cameraFull ? hud.area.x : hud.cameraX
        y: hud.cameraFull ? hud.area.y : hud.cameraY
        width: hud.cameraFull ? hud.area.width : hud.cameraBox.width
        height: hud.cameraFull ? hud.area.height : hud.cameraBox.height
        onXChanged: hud.fitInput()
        onYChanged: hud.fitInput()
        onWidthChanged: hud.fitInput()
        onHeightChanged: hud.fitInput()
        onVisibleChanged: hud.fitInput()
        // dragged anywhere in the area; a click fills the area, another brings it back
        MouseArea {
            anchors.fill: parent
            property bool dragged: false
            cursorShape: pressed && dragged ? Qt.ClosedHandCursor : Qt.OpenHandCursor
            drag.target: hud.cameraFull ? null : bubble
            drag.threshold: 4
            drag.minimumX: hud.area.x
            drag.maximumX: hud.area.x + hud.area.width - bubble.width
            drag.minimumY: hud.area.y
            drag.maximumY: hud.area.y + hud.area.height - bubble.height
            onPressed: dragged = false
            onPositionChanged: if (drag.active) dragged = true
            onReleased: {
                if (!dragged) { hud.cameraFull = !hud.cameraFull; return }
                hud.cameraAt = Qt.point(bubble.x - hud.area.x, bubble.y - hud.area.y)
                // the drag set x and y: bound again
                bubble.x = Qt.binding(() => hud.cameraFull ? hud.area.x : hud.cameraX)
                bubble.y = Qt.binding(() => hud.cameraFull ? hud.area.y : hud.cameraY)
            }
        }
    }

    // ---------------------------------------------------------------- clicks and keys (REC-09/10)
    // inside the area, to be recorded
    ListModel { id: rings }
    Repeater {
        model: rings
        Rectangle {
            required property real rx
            required property real ry
            required property bool second
            property real size: 16
            x: rx - size / 2
            y: ry - size / 2
            width: size
            height: size
            radius: size / 2
            color: "transparent"
            border.width: 3
            border.color: second ? Theme.warning : Theme.accent
            NumberAnimation on size { from: 16; to: 56; duration: 450; easing.type: Easing.OutCubic }
            // they end in the order they began: the oldest goes
            NumberAnimation on opacity { from: 1; to: 0; duration: 450; easing.type: Easing.InQuad; onFinished: Qt.callLater(() => rings.remove(0)) }
        }
    }
    Rectangle {
        id: keyPill
        objectName: "record-keystrokes"
        visible: hud.keys && hud.keyText !== "" && hud.phase === "recording"
        x: hud.area.x + hud.area.width / 2 - width / 2
        y: hud.area.y + hud.area.height - height - Math.min(48, hud.area.height * 0.08)
        width: keyLabel.implicitWidth + 32
        height: keyLabel.implicitHeight + 16
        radius: 12
        color: Qt.rgba(0, 0, 0, 0.72)
        Text {
            id: keyLabel
            anchors.centerIn: parent
            text: hud.keyText
            color: "white"
            font.family: Theme.fontUi
            font.pixelSize: Math.round(Math.max(14, Math.min(36, hud.area.height * 0.04)))
            font.weight: Font.DemiBold
        }
    }

    // ---------------------------------------------------------------- the countdown (REC-03)
    Rectangle {
        visible: hud.phase === "countdown"
        width: 120
        height: 120
        radius: 60
        x: hud.area.x + hud.area.width / 2 - width / 2
        y: hud.area.y + hud.area.height / 2 - height / 2
        color: Qt.rgba(0, 0, 0, 0.55)
        Text {
            anchors.centerIn: parent
            text: hud.seconds
            color: "white"
            font.family: Theme.fontUi
            font.pixelSize: 64
            font.weight: Font.DemiBold
        }
    }

    // ---------------------------------------------------------------- the panel (REC-02), CleanShot's
    // what to do, in the screen's (or the area's) middle
    Rectangle {
        id: hint
        objectName: "record-hint"
        visible: hud.phase === "ready" && (hud.whole || hud.picking || hud.drawing) && !selector.pressed
        anchors.horizontalCenter: parent.horizontalCenter
        y: 24
        height: 32
        width: hintText.implicitWidth + 28
        radius: 16
        color: Theme.pillBg
        border.width: 1
        border.color: Theme.hairline
        Text {
            id: hintText
            anchors.centerIn: parent
            text: hud.picking ? "Click a window to record it · Space: back"
                : hud.drawing ? "Drag to choose the area to record"
                : "Drag to record an area · Space: a window · Enter: the whole screen"
            color: Theme.text
            font.family: Theme.fontUi
            font.pixelSize: 13
            font.weight: Font.DemiBold
        }
    }
    readonly property color cardColor: Qt.rgba(Theme.surface.r, Theme.surface.g, Theme.surface.b, 0.96)
    Item {
        id: panel
        objectName: "record-panel"
        visible: hud.phase === "ready" && !hud.picking && !selector.pressed
        width: 268
        height: recordCard.y + recordCard.height
        // in the area if it fits, else beside it
        readonly property bool inside: hud.area.width >= width + 40 && hud.area.height >= height + 40
        x: inside ? Math.round(hud.area.x + hud.area.width / 2 - width / 2) : hud.placeX(width)
        y: inside ? Math.round(hud.area.y + hud.area.height / 2 - height / 2) : hud.placeY(height)
        RectangularShadow { anchors.fill: setupCard; radius: setupCard.radius; offset.y: 8; blur: 28; color: Theme.shadow }
        RectangularShadow { anchors.fill: recordCard; radius: recordCard.radius; offset.y: 8; blur: 28; color: Theme.shadow }

        // the size, Full screen, Select area / the sources and what's shown
        Rectangle {
            id: setupCard
            width: parent.width
            height: 89
            radius: 14
            color: hud.cardColor
            border.width: 1
            border.color: Theme.hairline
            // a click on the panel isn't a drag on the screen
            MouseArea { anchors.fill: parent; cursorShape: Qt.ArrowCursor }
            Rectangle { y: 44; width: parent.width; height: 1; color: Theme.hairline }
            Rectangle { x: 44; width: 1; height: 44; color: Theme.hairline }
            Rectangle { x: parent.width - 45; width: 1; height: 44; color: Theme.hairline }
            PanelCell {
                objectName: "record-settings"
                toggle: false
                icon: "sliders"
                tip: "Recording settings"
                onTapped: { Engine.recordCancel(); hud.controller.openSettings("recording") }
            }
            Row {
                x: 45
                width: setupCard.width - 90
                height: 44
                spacing: 6
                leftPadding: (width - implicitWidth) / 2
                Repeater {
                    model: ["w", "h"]
                    Row {
                        id: sizeRow
                        required property string modelData
                        required property int index
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 6
                        Text { visible: sizeRow.index === 1; anchors.verticalCenter: parent.verticalCenter; text: "×"; color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 13 }
                        Rectangle {
                            width: 52
                            height: 26
                            radius: 6
                            color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08)
                            TextInput {
                                objectName: "record-" + sizeRow.modelData
                                anchors.fill: parent
                                horizontalAlignment: TextInput.AlignHCenter
                                verticalAlignment: TextInput.AlignVCenter
                                readonly property int px: Math.round(sizeRow.modelData === "w" ? hud.area.width : hud.area.height)
                                text: px
                                validator: IntValidator { bottom: 16; top: 99999 }
                                color: Theme.text
                                selectionColor: Theme.accent
                                selectedTextColor: Theme.onAccent
                                font.family: Theme.fontUi
                                font.pixelSize: 13
                                font.features: { "tnum": 1 }
                                selectByMouse: true
                                onAccepted: {
                                    const n = Number(text)
                                    if (sizeRow.modelData === "w") hud.setSize(n, Math.round(hud.area.height))
                                    else hud.setSize(Math.round(hud.area.width), n)
                                    keyboard.forceActiveFocus()
                                }
                                onActiveFocusChanged: if (!activeFocus) text = Qt.binding(() => px)
                                Keys.onEscapePressed: keyboard.forceActiveFocus()
                            }
                        }
                    }
                }
                PanelCell {
                    objectName: "record-full"
                    anchors.verticalCenter: parent.verticalCenter
                    width: 26
                    height: 26
                    inset: 0
                    icon: "expand"
                    iconSize: 14
                    on: hud.whole
                    tip: "Full screen"
                    onTapped: hud.toggleFull()
                }
            }
            PanelCell {
                objectName: "record-crop"
                x: setupCard.width - 44
                on: hud.drawing
                icon: "crop"
                tip: "Select an area"
                onTapped: { hud.drawing = !hud.drawing; hud.picking = false }
            }
            Row {
                y: 45
                Repeater {
                    // [name, icon, its icon when off, tip, on]
                    model: [["record-mic", "mic", "mic-off", hud.micMuted ? "The microphone is muted" : hud.micLabel, hud.mic !== "" && !hud.gif],
                            ["record-system", "volume", "volume-x", "Record what the computer plays", hud.system && !hud.gif],
                            ["record-camera-button", "webcam", "webcam-off", hud.cameraEntry ? hud.cameraEntry.name : "No camera", hud.cameraEntry !== null],
                            ["record-clicks", "click", "", "Highlight clicks", hud.clicks],
                            ["record-keys", "command", "", "Show keystrokes (command keys only)", hud.keys]]
                    PanelCell {
                        required property var modelData
                        objectName: modelData[0]
                        width: setupCard.width / 5
                        icon: modelData[1]
                        offIcon: modelData[2]
                        tip: modelData[3]
                        on: modelData[4]
                        // a GIF has no sound
                        opacity: hud.gif && (modelData[0] === "record-mic" || modelData[0] === "record-system") ? 0.4 : 1
                        warn: modelData[0] === "record-mic" && hud.micMuted && !hud.gif ? Qt.rgba(Theme.warning.r, Theme.warning.g, Theme.warning.b, 0.3) : "transparent"
                        onTapped: {
                            switch (modelData[0]) {
                            case "record-mic": if (!hud.gif) hud.micsOpen = !hud.micsOpen; break
                            case "record-system": if (!hud.gif) hud.setSystem(!hud.system); break
                            case "record-camera-button": hud.camerasOpen = !hud.camerasOpen; break
                            case "record-clicks": hud.setShown("highlight_clicks", !hud.clicks); break
                            case "record-keys": hud.setShown("show_keystrokes", !hud.keys); break
                            }
                        }
                    }
                }
            }
        }

        // Record GIF / Record Video
        Rectangle {
            id: recordCard
            y: setupCard.height + 8
            width: parent.width
            height: 89
            radius: 14
            color: hud.cardColor
            border.width: 1
            border.color: Theme.hairline
            MouseArea { anchors.fill: parent; cursorShape: Qt.ArrowCursor }
            Rectangle { y: 44; x: 12; width: parent.width - 24; height: 1; color: Theme.hairline }
            Repeater {
                model: [["record-gif", true, "Record GIF", "Alt ↵"], ["record-video", false, "Record Video", "↵"]]
                Item {
                    id: recRow
                    required property var modelData
                    required property int index
                    objectName: modelData[0]
                    y: index * 45
                    width: recordCard.width
                    height: 44
                    Rectangle { anchors.fill: parent; anchors.margins: 4; radius: 10; color: recHover.hovered ? Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08) : "transparent" }
                    Rectangle {
                        visible: recRow.modelData[1]
                        x: 16
                        anchors.verticalCenter: parent.verticalCenter
                        width: 30
                        height: 18
                        radius: 4
                        color: Theme.text
                        Text { anchors.centerIn: parent; text: "GIF"; color: Theme.surface; font.family: Theme.fontUi; font.pixelSize: 10; font.weight: Font.Bold }
                    }
                    Icon { visible: !recRow.modelData[1]; x: 21; anchors.verticalCenter: parent.verticalCenter; name: "video"; size: 20; color: Theme.text }
                    Text {
                        x: 58
                        anchors.verticalCenter: parent.verticalCenter
                        text: recRow.modelData[2]
                        color: Theme.text
                        font.family: Theme.fontUi
                        font.pixelSize: 15
                        font.weight: Font.DemiBold
                    }
                    Text {
                        anchors.right: parent.right
                        anchors.rightMargin: 16
                        anchors.verticalCenter: parent.verticalCenter
                        text: recRow.modelData[3]
                        color: Theme.textSecondary
                        font.family: Theme.fontUi
                        font.pixelSize: 13
                    }
                    HoverHandler { id: recHover; cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: hud.record(recRow.modelData[1]) }
                }
            }
        }
    }
    // the microphones, under (or over) the panel
    Rectangle {
        id: micList
        objectName: "record-mics"
        visible: hud.micsOpen && hud.phase === "ready"
        width: 280
        height: micCol.implicitHeight + 12
        radius: Math.min(Theme.radius, 12)
        x: Math.max(8, Math.min(hud.width - width - 8, panel.x + panel.width / 2 - width / 2))
        y: panel.y + panel.height + 8 + height <= hud.height - 8 ? panel.y + panel.height + 8 : Math.max(8, panel.y - height - 8)
        color: Qt.rgba(Theme.surface.r, Theme.surface.g, Theme.surface.b, 0.98)
        border.width: 1
        border.color: Theme.hairline
        onVisibleChanged: hud.fitInput()
        onHeightChanged: hud.fitInput()
        MouseArea { anchors.fill: parent; cursorShape: Qt.ArrowCursor }
        Column {
            id: micCol
            x: 6
            y: 6
            width: parent.width - 12
            Repeater {
                model: [{ id: "", label: "No microphone" }].concat(hud.mics)
                Rectangle {
                    required property var modelData
                    required property int index
                    objectName: "record-mic:" + index
                    width: micCol.width
                    height: 30
                    radius: 6
                    color: micItemHover.hovered ? Theme.controlHover : "transparent"
                    Text {
                        x: 10
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - 36
                        elide: Text.ElideRight
                        text: parent.modelData.label
                        color: Theme.text
                        font.family: Theme.fontUi
                        font.pixelSize: 13
                    }
                    Icon { visible: hud.mic === parent.modelData.id; anchors.right: parent.right; anchors.rightMargin: 10; anchors.verticalCenter: parent.verticalCenter; name: "check"; size: 13; color: Theme.accent }
                    HoverHandler { id: micItemHover; cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: hud.setMic(parent.modelData.id) }
                }
            }
        }
    }

    // the cameras, and how the camera looks: CleanShot's sizes and shapes
    Rectangle {
        id: cameraList
        objectName: "record-cameras"
        visible: hud.camerasOpen && hud.phase === "ready"
        width: 330
        height: camCol.implicitHeight + 12
        radius: Math.min(Theme.radius, 12)
        x: Math.max(8, Math.min(hud.width - width - 8, panel.x + panel.width / 2 - width / 2))
        y: panel.y + panel.height + 8 + height <= hud.height - 8 ? panel.y + panel.height + 8 : Math.max(8, panel.y - height - 8)
        color: Qt.rgba(Theme.surface.r, Theme.surface.g, Theme.surface.b, 0.98)
        border.width: 1
        border.color: Theme.hairline
        onVisibleChanged: hud.fitInput()
        onHeightChanged: hud.fitInput()
        MouseArea { anchors.fill: parent; cursorShape: Qt.ArrowCursor }
        Column {
            id: camCol
            x: 6
            y: 6
            width: parent.width - 12
            spacing: 2
            Repeater {
                model: [{ name: "" }].concat(hud.cameras)
                Rectangle {
                    required property var modelData
                    required property int index
                    objectName: "record-camera:" + index
                    width: camCol.width
                    height: 30
                    radius: 6
                    color: camItemHover.hovered ? Theme.controlHover : "transparent"
                    Text {
                        x: 10
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - 36
                        elide: Text.ElideRight
                        text: parent.modelData.name === "" ? (hud.cameras.length ? "No camera" : "No camera (none connected)") : parent.modelData.name
                        color: Theme.text
                        font.family: Theme.fontUi
                        font.pixelSize: 13
                    }
                    Icon {
                        visible: parent.modelData.name === "" ? !hud.cameraEntry : hud.camera === parent.modelData.name
                        anchors.right: parent.right
                        anchors.rightMargin: 10
                        anchors.verticalCenter: parent.verticalCenter
                        name: "check"
                        size: 13
                        color: Theme.accent
                    }
                    HoverHandler { id: camItemHover; cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: hud.setCamera(parent.modelData.name) }
                }
            }
            Rectangle { visible: hud.cameraEntry !== null; width: camCol.width; height: 1; color: Theme.hairline }
            Row {
                visible: hud.cameraEntry !== null
                x: 4
                height: 34
                spacing: 4
                Repeater {
                    model: [["small", "Small"], ["medium", "Medium"], ["large", "Large"], ["huge", "Huge"]]
                    Rectangle {
                        required property var modelData
                        readonly property bool on: hud.cameraSize === modelData[0]
                        objectName: "record-camera-size=" + modelData[0]
                        anchors.verticalCenter: parent.verticalCenter
                        width: sizeText.implicitWidth + 18
                        height: 26
                        radius: 13
                        color: on ? Theme.accent : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, sizeHover.hovered ? 0.12 : 0.06)
                        Text { id: sizeText; anchors.centerIn: parent; text: parent.modelData[1]; color: parent.on ? Theme.onAccent : Theme.text; font.family: Theme.fontUi; font.pixelSize: 12 }
                        HoverHandler { id: sizeHover; cursorShape: Qt.PointingHandCursor }
                        TapHandler { onTapped: hud.setCameraLook("size", parent.modelData[0]) }
                    }
                }
            }
            Row {
                visible: hud.cameraEntry !== null
                x: 4
                height: 34
                spacing: 4
                Repeater {
                    model: [["circle", "Circle"], ["square", "Square"], ["rectangle", "Rectangle"], ["vertical", "Vertical"]]
                    Rectangle {
                        required property var modelData
                        readonly property bool on: hud.cameraShape === modelData[0]
                        objectName: "record-camera-shape=" + modelData[0]
                        anchors.verticalCenter: parent.verticalCenter
                        width: shapeText.implicitWidth + 18
                        height: 26
                        radius: 13
                        color: on ? Theme.accent : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, shapeHover.hovered ? 0.12 : 0.06)
                        Text { id: shapeText; anchors.centerIn: parent; text: parent.modelData[1]; color: parent.on ? Theme.onAccent : Theme.text; font.family: Theme.fontUi; font.pixelSize: 12 }
                        HoverHandler { id: shapeHover; cursorShape: Qt.PointingHandCursor }
                        TapHandler { onTapped: hud.setCameraLook("shape", parent.modelData[0]) }
                    }
                }
            }
            Rectangle {
                visible: hud.cameraEntry !== null
                objectName: "record-camera-mirror"
                width: camCol.width
                height: 30
                radius: 6
                color: mirrorHover.hovered ? Theme.controlHover : "transparent"
                Text { x: 10; anchors.verticalCenter: parent.verticalCenter; text: "Mirror"; color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 13 }
                Icon { visible: hud.cameraMirror; anchors.right: parent.right; anchors.rightMargin: 10; anchors.verticalCenter: parent.verticalCenter; name: "check"; size: 13; color: Theme.accent }
                HoverHandler { id: mirrorHover; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: hud.setCameraLook("mirror", !hud.cameraMirror) }
            }
            Text {
                visible: hud.cameraEntry !== null
                x: 10
                height: 26
                verticalAlignment: Text.AlignVCenter
                text: "Drag the camera anywhere. A click fills the area."
                color: Theme.textSecondary
                font.family: Theme.fontUi
                font.pixelSize: 11
            }
        }
    }

    // ---------------------------------------------------------------- the controls (REC-05)
    Rectangle {
        id: controls
        objectName: "record-controls"
        visible: !hud.whole && (hud.phase === "recording" || hud.phase === "paused")
        width: ctl.implicitWidth + 16
        height: 40
        radius: 20
        x: hud.placeX(width)
        y: hud.placeY(height)
        color: Theme.pillBg
        border.width: 1
        border.color: Theme.hairline
        onXChanged: hud.fitInput()
        onYChanged: hud.fitInput()
        onWidthChanged: hud.fitInput()
        onVisibleChanged: hud.fitInput()
        layer.enabled: true
        layer.effect: MultiEffect { shadowEnabled: true; shadowColor: Theme.shadow; shadowBlur: 0.6; shadowVerticalOffset: 6 }
        Row {
            id: ctl
            anchors.centerIn: parent
            spacing: 4
            // stop, with the time recorded
            Rectangle {
                objectName: "record-stop"
                anchors.verticalCenter: parent.verticalCenter
                width: stopRow.implicitWidth + 20
                height: 28
                radius: 14
                color: stopHover.hovered ? Qt.lighter(Theme.danger, 1.1) : Theme.danger
                Row {
                    id: stopRow
                    anchors.centerIn: parent
                    spacing: 6
                    Rectangle { anchors.verticalCenter: parent.verticalCenter; width: 10; height: 10; radius: 2; color: "white" }
                    Text {
                        objectName: "record-time"
                        anchors.verticalCenter: parent.verticalCenter
                        text: hud.clock(hud.elapsed)
                        color: "white"
                        font.family: Theme.fontUi
                        font.pixelSize: 13
                        font.weight: Font.DemiBold
                        font.features: { "tnum": 1 }
                    }
                }
                HoverHandler { id: stopHover; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: Engine.recordStop() }
            }
            Row {
                visible: hud.asking === ""
                anchors.verticalCenter: parent.verticalCenter
                spacing: 4
                IconButton { objectName: "record-pause"; anchors.verticalCenter: parent.verticalCenter; icon: hud.phase === "paused" ? "play" : "pause"; tip: hud.phase === "paused" ? "Resume" : "Pause"; onClicked: Engine.recordPause(hud.phase !== "paused") }
                IconButton { objectName: "record-restart"; anchors.verticalCenter: parent.verticalCenter; icon: "rotate-ccw"; tip: "Restart"; onClicked: hud.asking = "restart" }
                IconButton { objectName: "record-discard"; anchors.verticalCenter: parent.verticalCenter; icon: "trash"; tip: "Delete"; onClicked: hud.asking = "discard" }
            }
            // Restart and Delete ask first, as CleanShot's do (it goes on recording)
            Row {
                visible: hud.asking !== ""
                anchors.verticalCenter: parent.verticalCenter
                spacing: 6
                leftPadding: 6
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: hud.asking === "restart" ? "Start over? This take is lost." : "Delete this recording? You can't undo this."
                    color: Theme.text
                    font.family: Theme.fontUi
                    font.pixelSize: 13
                }
                TextButton {
                    objectName: "record-confirm"
                    anchors.verticalCenter: parent.verticalCenter
                    primary: true
                    text: hud.asking === "restart" ? "Restart" : "Delete"
                    onClicked: {
                        const what = hud.asking
                        hud.asking = ""
                        if (what === "restart") Engine.recordRestart()
                        else Engine.recordDiscard()
                    }
                }
                TextButton { objectName: "record-confirm-cancel"; anchors.verticalCenter: parent.verticalCenter; text: "Cancel"; onClicked: hud.asking = "" }
            }
        }
    }

    // tests: where a control is (output logical px), and how things stand
    function find(item, name) {
        if (item.objectName === name && item.visible) return item
        for (const c of item.children) {
            if (!c.visible) continue
            const f = find(c, name)
            if (f) return f
        }
        return null
    }
    function debug(a) {
        switch (a[0]) {
        case "where": {
            const it = find(contentItem, a[1])
            // laid out first: just after a change, rows and columns still hold old places
            for (let q = it; q; q = q.parent) if (q.forceLayout) q.forceLayout()
            const p = it ? it.mapToItem(contentItem, it.width / 2, it.height / 2) : null
            Engine.log("record_where", JSON.stringify({ name: a[1], x: p ? p.x : -1, y: p ? p.y : -1 }).slice(1, -1))
            break
        }
        case "keys": Platform.sendKeys(hud, a.slice(1).join(" ")); break
        case "state":
            Engine.log("record_hud", JSON.stringify({ open: visible, phase: phase, gif: gif, mic: mic, system: system, whole: whole,
                                                    time: hud.clock(elapsed), area: [area.x, area.y, area.width, area.height],
                                                    panel: panel.visible ? [panel.x, panel.y, panel.width, panel.height] : null,
                                                    picking: picking, drawing: drawing, windows: windows.length, hint: hint.visible ? hintText.text : "",
                                                    icons: ["record-mic", "record-system", "record-camera-button", "record-clicks", "record-keys"]
                                                           .map(n => { const c = find(contentItem, n); return c ? (c.lit ? "+" : "") + c.shownIcon : "" }),
                                                    controls: controls.visible ? [controls.x, controls.y, controls.width, controls.height] : null,
                                                    camera: bubble.visible ? [bubble.x, bubble.y, bubble.width, bubble.height] : null,
                                                    cameraName: camera, cameraShape: cameraShape, cameraSize: cameraSize, cameraMirror: cameraMirror,
                                                    cameraFull: cameraFull, cameraReady: bubble.ready, cameras: cameras.map(c => c.name),
                                                    camerasOpen: cameraList.visible, micsOpen: micList.visible, clicks: clicks, keys: keys, rings: ringsShown,
                                                    keystrokes: keyPill.visible ? keyText : "", asking: asking,
                                                    cardsAway: !!(controller && controller.quickAccess.capturing) }).slice(1, -1))
            break
        }
    }
}
