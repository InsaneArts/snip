import QtQuick
import QtQuick.Window
import Snip.Core

// snipd's UI root: owns the overlay session state shared by every output's
// overlay, and the long-lived surfaces (Quick Access, toast, preview).
QtObject {
    id: root

    property string mode: "interactive"   // interactive | test | gallery

    readonly property var settings: {
        try { return JSON.parse(Engine.settings || "{}") } catch (e) { return {} }
    }
    readonly property string crosshairMode: settings.capture ? settings.capture.crosshair : "ctrl"

    // ---------------------------------------------------------------- overlay session
    property bool overlayOpen: false     // overlay surfaces mapped
    property bool overlayShown: false    // overlay content drawn (cleared first on close, see closeOverlay)
    property var session: ({})
    // Window picking happens only while Space is held (or for `snip window`);
    // otherwise the whole display is free space for dragging an area.
    property bool stickyWindow: false
    property bool spaceHeld: false
    readonly property bool windowMode: stickyWindow || spaceHeld
    property bool ctrlHeld: false
    property bool shiftHeld: false
    property bool altHeld: false
    property var activeOverlay: null
    property string lastOutput: ""

    function openSession() {
        try { session = JSON.parse(Engine.session || "{}") } catch (e) { session = {} }
        stickyWindow = session.mode === "window"
        spaceHeld = false
        ctrlHeld = shiftHeld = altHeld = false
        if (session.cursor) lastOutput = session.cursor.output
        unmapOverlay.stop()
        overlayShown = true
        overlayOpen = true
    }

    // Clear the content now, unmap a moment later: Hyprland fades unmapped
    // layers out, which would leave a dimmed frozen frame on screen.
    function closeOverlay() {
        overlayShown = false
        stickyWindow = false
        spaceHeld = false
        quickAccess.capturing = false
        unmapOverlay.restart()
    }
    property Timer unmapOverlay: Timer { interval: 60; onTriggered: root.overlayOpen = false }

    function cancel() {
        closeOverlay()
        Engine.cancel()
    }

    // Read everything before closing: QML rects are live references, and
    // closing resets the overlay's selection.
    function finishArea(win, phys, kind) {
        const out = win.outputName, x = phys.x, y = phys.y, w = phys.width, h = phys.height
        lastOutput = out
        closeOverlay()
        Engine.finishArea(out, x, y, w, h, kind)
    }

    function finishWindow(win, w, phys) {
        const out = win.outputName, addr = w.address, invert = shiftHeld
        const x = phys.x, y = phys.y, pw = phys.width, ph = phys.height
        lastOutput = out
        closeOverlay()
        Engine.finishWindow(addr, out, x, y, pw, ph, invert)
    }

    function setMods(e) {
        ctrlHeld = (e.modifiers & Qt.ControlModifier) !== 0
        shiftHeld = (e.modifiers & Qt.ShiftModifier) !== 0
        altHeld = (e.modifiers & Qt.AltModifier) !== 0
    }

    // Keys reach whichever overlay has focus; act on the one under the pointer.
    function keyPressed(e, from) {
        const w = activeOverlay || from
        setMods(e)
        switch (e.key) {
        case Qt.Key_Control: ctrlHeld = true; break
        case Qt.Key_Shift: shiftHeld = true; if (w.dragging) w.recompute(); break
        case Qt.Key_Alt: altHeld = true; if (w.dragging) w.recompute(); break
        case Qt.Key_Escape: cancel(); break
        case Qt.Key_Space:
            // held while dragging: move the selection; otherwise: pick a window
            if (e.isAutoRepeat) break
            if (!w.startMove()) spaceHeld = true
            break
        case Qt.Key_Tab: case Qt.Key_Right: case Qt.Key_Down: w.cycle(1); break
        case Qt.Key_Backtab: case Qt.Key_Left: case Qt.Key_Up: w.cycle(-1); break
        case Qt.Key_Return: case Qt.Key_Enter: w.confirm(); break
        case Qt.Key_C:
            if (w.crosshair && session.magnifier) {
                const hex = Platform.hex(w.magnifierColor)
                Engine.copyText(hex)
                toast.show("Copied " + hex, w.outputName)
            }
            break
        default: return
        }
        e.accepted = true
    }

    function keyReleased(e, from) {
        const w = activeOverlay || from
        if (e.key === Qt.Key_Control) ctrlHeld = false
        else if (e.key === Qt.Key_Shift) { shiftHeld = false; if (w.dragging) w.recompute() }
        else if (e.key === Qt.Key_Alt) { altHeld = false; if (w.dragging) w.recompute() }
        else if (e.key === Qt.Key_Space && !e.isAutoRepeat) { w.moving = false; spaceHeld = false }
    }

    // ---------------------------------------------------------------- engine events
    property Connections engineConn: Connections {
        target: Engine
        function onFramesReady(epoch) { root.openSession() }
        function onCancelRequested() { root.closeOverlay() }
        function onPrepareCapture(req) { root.quickAccess.prepareCapture(req) }
        function onCaptureDone() { if (!root.overlayOpen) root.quickAccess.capturing = false }
        function onCardAdd(card) { root.quickAccess.add(JSON.parse(card)) }
        function onCardUpdate(id, card) { root.quickAccess.update(id, JSON.parse(card)) }
        function onCardsCommand(cmd) { root.quickAccess.command(cmd) }
        function onToast(text) { root.toast.show(text, root.lastOutput || root.quickAccess.output || Qt.application.screens[0].name) }
        function onQuitRequested() { Qt.quit() }
        function onDebugCommand(cmd) { root.debug(cmd) }
    }

    // Test hooks (snip debug ui ...): keys the virtual pointer can't send.
    function debug(cmd) {
        const a = cmd.split(" ")
        const w = activeOverlay
        if (a[0] === "qa") { quickAccess.command(a[1]); return }
        const map = { "Escape": Qt.Key_Escape, "space": Qt.Key_Space, "Return": Qt.Key_Return, "Tab": Qt.Key_Tab,
                      "c": Qt.Key_C, "Shift": Qt.Key_Shift, "Alt": Qt.Key_Alt, "Control": Qt.Key_Control }
        const mods = { "Shift": Qt.ShiftModifier, "Alt": Qt.AltModifier, "Control": Qt.ControlModifier }
        if (!w || !(a[0] in { key: 1, keydown: 1, keyup: 1 })) return
        // modifiers as they'd be reported with the event
        let m = (shiftHeld ? Qt.ShiftModifier : 0) | (altHeld ? Qt.AltModifier : 0) | (ctrlHeld ? Qt.ControlModifier : 0)
        if (a[1] in mods) m = a[0] === "keyup" ? (m & ~mods[a[1]]) : (m | mods[a[1]])
        const ev = { key: map[a[1]], modifiers: m, isAutoRepeat: false, accepted: false }
        if (a[0] !== "keyup") keyPressed(ev, w)
        if (a[0] !== "keydown") keyReleased(ev, w)
    }

    Component.onCompleted: {
        Engine.start(mode === "interactive" ? "interactive" : "test")
        if (mode === "gallery") gallery = galleryComponent.createObject(null)
    }

    // ---------------------------------------------------------------- surfaces
    property Instantiator overlays: Instantiator {
        model: root.mode === "gallery" ? [] : Qt.application.screens
        delegate: Overlay { controller: root }
    }

    property QuickAccess quickAccess: QuickAccess { controller: root }
    property Toast toast: Toast {}
    property Preview preview: Preview {}

    // (a Loader-created Window never maps: spike S8; create it directly)
    property Component galleryComponent: Component { Gallery { controller: root } }
    property var gallery: null
}
