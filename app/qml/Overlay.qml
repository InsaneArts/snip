import QtQuick
import QtQuick.Window
import org.kde.layershell as LayerShell
import Snip.Core

// One selection overlay per output (SEL-01..11): the frozen frame, dimmed
// outside the selection, with a 1-physical-pixel accent border. Created up
// front and kept unmapped until frames arrive, so it appears instantly.
Window {
    id: win
    required property var modelData
    required property var controller
    readonly property var c: controller
    readonly property string outputName: modelData.name
    readonly property var out: c.session && c.session.outputs ? c.session.outputs[outputName] : undefined
    readonly property bool active: c.overlayOpen && out !== undefined
    readonly property var wins: out ? out.windows : []

    visible: active
    color: frame.hasFrame && c.overlayShown ? "black" : "transparent"
    flags: Qt.FramelessWindowHint
    screen: modelData
    // the compositor configures the real size; start from it so the first
    // frame is already laid out for this output
    width: modelData.width
    height: modelData.height
    title: "snip-select"

    LayerShell.Window.screen: Platform.screen(outputName)
    LayerShell.Window.scope: "snip-select"
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
                               | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
    LayerShell.Window.exclusionZone: -1
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityExclusive

    // ---------------------------------------------------------------- state
    property point mouse: Qt.point(-1, -1)        // logical, window coordinates
    property bool hovered: false
    property bool pressed: false
    property bool dragging: false
    property bool moving: false                  // Space held while dragging (SEL-07)
    property point p0
    property point lastMouse
    property rect raw: Qt.rect(0, 0, 0, 0)       // logical selection as dragged
    property int pick: -1                        // Tab-picked window (SEL-06)

    // Physical px per logical px, and the frame's upright size. Mapping is
    // done here (not in C++) so every binding follows size changes.
    readonly property real s: frame.hasFrame ? frame.frameSize.width / Math.max(1, width) : Screen.devicePixelRatio
    readonly property int fw: frame.hasFrame ? frame.frameSize.width : Math.round(width * s)
    readonly property int fh: frame.hasFrame ? frame.frameSize.height : Math.round(height * s)
    readonly property rect phys: toPhys(raw)
    readonly property rect shown: toShown(phys)
    readonly property bool crosshair: hovered && !c.windowMode && !dragging
                                      && (c.crosshairMode === "always" || (c.crosshairMode === "ctrl" && c.ctrlHeld))
    readonly property point cursorPhys: Qt.point(Math.floor(mouse.x * s), Math.floor(mouse.y * s))
    readonly property color magnifierColor: mag.centre

    // Logical -> physical with edges rounded independently (Hyprland's
    // CBox::round), clamped to the frame: adjacent selections tile exactly.
    function toPhys(l) {
        const x0 = Math.max(0, Math.round(l.x * s)), y0 = Math.max(0, Math.round(l.y * s))
        const x1 = Math.min(fw, Math.round((l.x + l.width) * s)), y1 = Math.min(fh, Math.round((l.y + l.height) * s))
        return Qt.rect(x0, y0, Math.max(0, x1 - x0), Math.max(0, y1 - y0))
    }
    function toShown(p) { return Qt.rect(p.x / s, p.y / s, p.width / s, p.height / s) }
    function winPhys(w) { return toPhys(Qt.rect(w.x, w.y, w.w, w.h)) }
    function fullPhys() { return Qt.rect(0, 0, fw, fh) }

    // The window a click would capture (window mode only): the smallest one
    // under the cursor. Tab picks one explicitly in any mode.
    readonly property int hoverIndex: {
        if (pick >= 0 && pick < wins.length) return pick
        if (!hovered || dragging || !c.windowMode) return -1
        let best = -1, area = Infinity
        for (let i = 0; i < wins.length; i++) {
            const w = wins[i]
            if (mouse.x >= w.x && mouse.x < w.x + w.w && mouse.y >= w.y && mouse.y < w.y + w.h && w.w * w.h < area) {
                area = w.w * w.h
                best = i
            }
        }
        return best
    }
    readonly property var hoverWin: hoverIndex >= 0 ? wins[hoverIndex] : null
    readonly property rect targetPhys: hoverWin ? winPhys(hoverWin) : Qt.rect(0, 0, 0, 0)
    readonly property bool selecting: dragging && raw.width > 0
    readonly property rect hole: selecting ? shown : toShown(targetPhys)
    readonly property bool hasHole: hole.width > 0 && hole.height > 0
    // Area mode leaves the display undimmed until a drag starts.
    readonly property bool dimmed: selecting || c.windowMode || hoverWin !== null

    // the first frame of a session: then take the pointer (Engine.refocusPointer)
    property bool awaitRefocus: false
    Connections {
        target: Platform
        function onFramePresented(w) {
            if (w === win && win.awaitRefocus) { win.awaitRefocus = false; win.c.refocusPointer() }
        }
    }

    function resetSession() {
        awaitRefocus = true
        Platform.notifyNextFrame(win)
        pressed = false; dragging = false; moving = false; pick = -1
        raw = Qt.rect(0, 0, 0, 0)
        hovered = !!(c.session && c.session.cursor && c.session.cursor.output === outputName)
        if (hovered) {
            mouse = Qt.point(c.session.cursor.x, c.session.cursor.y)
            c.activeOverlay = win
        }
        keys.forceActiveFocus()
    }
    onActiveChanged: if (active) resetSession()
    Connections {
        target: win.c
        // reopened before the previous close finished unmapping
        function onOverlayShownChanged() { if (win.c.overlayShown && win.active) win.resetSession() }
    }
    // Closing clears content and input at once; the unmap follows (Main).
    Component.onCompleted: {
        contentItem.opacity = Qt.binding(() => win.c.overlayShown ? 1 : 0)
        contentItem.enabled = Qt.binding(() => win.c.overlayShown)
    }

    function recompute() {
        if (moving) {
            const dx = mouse.x - lastMouse.x, dy = mouse.y - lastMouse.y
            lastMouse = mouse
            const nx = Math.max(0, Math.min(width - raw.width, raw.x + dx))
            const ny = Math.max(0, Math.min(height - raw.height, raw.y + dy))
            p0 = Qt.point(p0.x + (nx - raw.x), p0.y + (ny - raw.y))
            raw = Qt.rect(nx, ny, raw.width, raw.height)
            return
        }
        let dx = mouse.x - p0.x, dy = mouse.y - p0.y
        if (c.shiftHeld) {   // square
            const m = Math.max(Math.abs(dx), Math.abs(dy))
            dx = (dx < 0 ? -1 : 1) * m
            dy = (dy < 0 ? -1 : 1) * m
        }
        let r = c.altHeld   // from the centre
            ? Qt.rect(p0.x - Math.abs(dx), p0.y - Math.abs(dy), 2 * Math.abs(dx), 2 * Math.abs(dy))
            : Qt.rect(Math.min(p0.x, p0.x + dx), Math.min(p0.y, p0.y + dy), Math.abs(dx), Math.abs(dy))
        const x0 = Math.max(0, r.x), y0 = Math.max(0, r.y)
        const x1 = Math.min(width, r.x + r.width), y1 = Math.min(height, r.y + r.height)
        raw = Qt.rect(x0, y0, Math.max(0, x1 - x0), Math.max(0, y1 - y0))
    }

    function startMove() {
        if (!dragging) return false
        moving = true
        lastMouse = mouse
        return true
    }

    function cycle(step) {
        if (wins.length === 0) return
        const cur = hoverIndex >= 0 ? hoverIndex : (step > 0 ? -1 : 0)
        pick = (cur + step + wins.length) % wins.length
    }

    // A click without a drag: only meaningful on a window in window mode.
    function clicked() {
        if (c.windowMode && hoverWin) confirm()
    }

    // Enter: the picked window (clean toplevel buffer in window mode), else
    // the whole monitor.
    function confirm() {
        if (hoverWin) {
            if (c.windowMode && c.session.mode !== "text" && c.session.toplevel)
                c.finishWindow(win, hoverWin, winPhys(hoverWin))
            else
                c.finishArea(win, winPhys(hoverWin), "window")
        } else if (!c.windowMode) {
            c.finishArea(win, fullPhys(), "fullscreen")
        }
    }

    // ---------------------------------------------------------------- frame
    FrozenFrame {
        id: frame
        anchors.fill: parent
        output: win.outputName
        epoch: win.out && win.out.frozen ? Engine.epoch : 0
        visible: hasFrame
    }

    // ---------------------------------------------------------------- scrim (four quads around the hole)
    Item {
        anchors.fill: parent
        visible: win.dimmed
        readonly property rect h: win.hasHole ? win.hole : Qt.rect(0, 0, 0, 0)
        Rectangle { color: Theme.scrim; x: 0; y: 0; width: parent.width; height: parent.h.y }
        Rectangle { color: Theme.scrim; x: 0; y: parent.h.y + parent.h.height; width: parent.width; height: parent.height - y }
        Rectangle { color: Theme.scrim; x: 0; y: parent.h.y; width: parent.h.x; height: parent.h.height }
        Rectangle { color: Theme.scrim; x: parent.h.x + parent.h.width; y: parent.h.y; width: parent.width - x; height: parent.h.height }
    }

    // ---------------------------------------------------------------- border: 1 physical px, outside the hole
    Rectangle {
        readonly property real b: 1 / win.s
        visible: win.hasHole && (win.selecting || win.hoverWin !== null)
        x: win.hole.x - b
        y: win.hole.y - b
        width: win.hole.width + 2 * b
        height: win.hole.height + 2 * b
        color: "transparent"
        border.width: b
        border.color: Theme.accent
        opacity: 1
    }

    // ---------------------------------------------------------------- window label (window mode)
    Rectangle {
        visible: win.c.windowMode && win.hoverWin !== null && !win.selecting
        x: Math.max(8, Math.min(win.width - width - 8, win.hole.x + (win.hole.width - width) / 2))
        y: Math.max(8, Math.min(win.height - height - 8, win.hole.y + (win.hole.height - height) / 2))
        height: 30
        width: Math.min(420, wl.implicitWidth + 24)
        radius: Math.min(Theme.radiusControl, 15)
        color: Theme.pillBg
        border.width: 1
        border.color: Theme.hairline
        Row {
            id: wl
            anchors.centerIn: parent
            spacing: 8
            Icon { name: "window"; size: 14; stroke: 2; color: Theme.accent; anchors.verticalCenter: parent.verticalCenter }
            Text {
                text: win.hoverWin ? (win.hoverWin.class || "Window") : ""
                color: Theme.text
                font.family: Theme.fontUi
                font.pixelSize: 13
                font.weight: Font.DemiBold
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                text: win.hoverWin ? win.targetPhys.width + " × " + win.targetPhys.height : ""
                color: Theme.textSecondary
                font.family: Theme.fontUi
                font.pixelSize: 12
                font.features: { "tnum": 1 }
                anchors.verticalCenter: parent.verticalCenter
            }
        }
    }

    // ---------------------------------------------------------------- dimensions pill (SEL-03, physical px)
    Rectangle {
        visible: win.selecting
        readonly property real below: win.hole.y + win.hole.height + 8
        x: Math.max(4, Math.min(win.width - width - 4, win.hole.x + win.hole.width - width))
        y: below + height + 4 < win.height ? below : Math.max(4, win.hole.y - height - 8)
        width: dim.implicitWidth + 16
        height: 24
        radius: Math.min(Theme.radiusControl, 12)
        color: Theme.pillBg
        border.width: 1
        border.color: Theme.hairline
        Text {
            id: dim
            anchors.centerIn: parent
            text: win.phys.width + " × " + win.phys.height
            color: Theme.text
            font.family: Theme.fontUi
            font.pixelSize: 12
            font.weight: Font.Medium
            font.features: { "tnum": 1 }
        }
    }

    // ---------------------------------------------------------------- crosshair (SEL-04)
    Item {
        anchors.fill: parent
        visible: win.crosshair
        readonly property real cx: win.cursorPhys.x / win.s
        readonly property real cy: win.cursorPhys.y / win.s
        readonly property color line: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.85)
        Rectangle { x: 0; y: parent.cy; width: parent.width; height: 1 / win.s; color: parent.line }
        Rectangle { x: parent.cx; y: 0; width: 1 / win.s; height: parent.height; color: parent.line }
        Rectangle {
            visible: !(win.c.session && win.c.session.magnifier && frame.hasFrame)
            x: Math.min(parent.cx + 14, parent.width - width - 4)
            y: Math.min(parent.cy + 14, parent.height - height - 4)
            width: coord.implicitWidth + 12
            height: 22
            radius: Math.min(Theme.radiusControl, 11)
            color: Theme.pillBg
            Text {
                id: coord
                anchors.centerIn: parent
                text: win.cursorPhys.x + ", " + win.cursorPhys.y
                color: Theme.text
                font.family: Theme.fontUi
                font.pixelSize: 11
                font.features: { "tnum": 1 }
            }
        }
    }

    // ---------------------------------------------------------------- magnifier (SEL-05)
    Magnifier {
        id: mag
        visible: win.crosshair && win.c.session && win.c.session.magnifier && frame.hasFrame
        output: win.outputName
        epoch: frame.epoch
        px: win.cursorPhys.x
        py: win.cursorPhys.y
        readonly property real gap: 24
        x: win.mouse.x + gap + width < win.width ? win.mouse.x + gap : win.mouse.x - gap - width
        y: win.mouse.y + gap + height < win.height ? win.mouse.y + gap : win.mouse.y - gap - height
    }

    // ---------------------------------------------------------------- mode hint (window / text modes)
    Rectangle {
        visible: win.hovered && (win.c.windowMode || (win.c.session && win.c.session.mode === "text"))
        anchors.horizontalCenter: parent.horizontalCenter
        y: 24
        height: 32
        width: hint.implicitWidth + 28
        radius: Math.min(Theme.radiusControl, 16)
        color: Theme.pillBg
        border.width: 1
        border.color: Theme.hairline
        Row {
            id: hint
            anchors.centerIn: parent
            spacing: 8
            Icon {
                name: win.c.windowMode ? "window" : "scan-text"
                size: 15; stroke: 2; color: Theme.accent
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                text: win.c.windowMode ? "Click a window" : "Select text to copy"
                color: Theme.text
                font.family: Theme.fontUi
                font.pixelSize: 13
                font.weight: Font.DemiBold
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                text: win.c.spaceHeld ? "Release Space for area" : "Esc to cancel"
                color: Theme.textSecondary
                font.family: Theme.fontUi
                font.pixelSize: 12
                anchors.verticalCenter: parent.verticalCenter
            }
        }
    }

    // ---------------------------------------------------------------- input
    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: win.c.windowMode ? Qt.PointingHandCursor : Qt.CrossCursor

        onEntered: { win.hovered = true; win.c.activeOverlay = win }
        onExited: if (!win.pressed) win.hovered = false
        onPositionChanged: (m) => {
            win.mouse = Qt.point(m.x, m.y)
            win.hovered = true
            win.c.activeOverlay = win
            if (win.pick >= 0 && !win.pressed) win.pick = -1
            if (!win.pressed) return
            if (!win.dragging && Math.hypot(m.x - win.p0.x, m.y - win.p0.y) > 3 && !win.c.windowMode) {
                win.dragging = true
                win.pick = -1
            }
            if (win.dragging) win.recompute()
        }
        onPressed: (m) => {
            if (m.button === Qt.RightButton) { win.c.cancel(); return }
            win.pressed = true
            win.p0 = Qt.point(m.x, m.y)
            win.mouse = win.p0
            win.raw = Qt.rect(m.x, m.y, 0, 0)
        }
        onReleased: (m) => {
            if (m.button !== Qt.LeftButton) return
            const wasDragging = win.dragging
            win.pressed = false
            win.dragging = false
            win.moving = false
            if (wasDragging && win.phys.width >= 2 && win.phys.height >= 2)
                win.c.finishArea(win, win.phys, "area")
            else if (!wasDragging)
                win.clicked()
            win.raw = Qt.rect(0, 0, 0, 0)
        }
    }

    Item {
        id: keys
        focus: true
        Keys.onPressed: (e) => win.c.keyPressed(e, win)
        Keys.onReleased: (e) => win.c.keyReleased(e, win)
    }
}
