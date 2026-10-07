import QtQuick
import QtQuick.Window
import QtQuick.Effects
import org.kde.layershell as LayerShell
import Snip.Core

// A pinned screenshot (PIN-01…08), CleanShot's: the image floats above every
// window, on every workspace of its monitor (Hyprland's pin rule, matched by
// the title). Drag (or arrow keys) to move, scroll to fade, Ctrl+scroll to
// zoom, middle-click to close; hovering shows close, the zoom, the lock and a
// grip to drag the image out; right-click opens the menu. Locked, it lets
// clicks through and fades under the pointer (unlock: snip pins unlock).
Window {
    id: pin
    required property var capture           // a card's JSON plus its output's scale
    property var controller: null
    signal finished()

    property int number: 0                  // unique, so Hyprland can tell pins apart
    title: "Snip Pin " + number
    flags: Qt.FramelessWindowHint
    color: "transparent"

    readonly property real margin: 16       // room for the shadow
    // PIN-08: [pins] in the config
    readonly property var look: (controller && controller.settings && controller.settings.pins) || ({})
    readonly property real radius: look.corner_radius !== undefined ? look.corner_radius : 10
    readonly property real baseW: capture.width / (capture.scale || 1)
    readonly property real baseH: capture.height / (capture.scale || 1)
    property real zoom: 1
    property real fade: 1                   // the image's opacity
    width: Math.round(baseW * zoom + 2 * margin)
    height: Math.round(baseH * zoom + 2 * margin)
    // Hyprland keeps a floating window's size: ask it for the new one
    onWidthChanged: if (visible) resizeTimer.restart()
    onHeightChanged: if (visible) resizeTimer.restart()
    Timer { id: resizeTimer; interval: 16; onTriggered: Engine.resizeWindow(pin.title, pin.width, pin.height) }

    function close_() { if (lockLayer) lockLayer.destroy(); visible = false; finished() }
    function act(action) {
        menuPopup.visible = false
        switch (action) {
        case "copy": Engine.copy(capture.id); controller.toast.show("Copied", capture.output); break
        case "save": Engine.save(capture.id); break
        case "annotate": Engine.annotate(capture.id); break
        case "close": close_(); break
        case "close-all": controller.closePins(); break
        case "lock": lock(); break
        }
    }
    function zoomBy(f) { zoom = Math.max(0.1, Math.min(4, zoom * f)) }
    // PIN-07: hidden pins come back where they were (Hyprland would centre them)
    property var hiddenAt: null
    function hide() {
        if (locked) { lockLayer.visible = false; return }
        hiddenAt = JSON.parse(Engine.windowPosition(title))
        visible = false
    }
    function show() {
        if (locked) { lockLayer.visible = true; Platform.setInputRects(lockLayer, []); return }
        frame.visible = false           // until it is back in place
        visible = true
        restoreTimer.start()
    }
    readonly property bool shown: visible || (locked && lockLayer.visible)

    // PIN-05: locked, the pin is a layer surface in the same place with an
    // empty input region (Hyprland ignores those on windows: spike S04), so
    // clicks reach what is under it; it fades while the pointer is over it.
    property bool locked: false
    property var lockLayer: null
    property var lockAt: null               // where the window was, in layout px
    property bool lockHover: false
    function lock() {
        const place = JSON.parse(Engine.windowPlace(title))
        lockAt = JSON.parse(Engine.windowPosition(title))
        if (!place || !lockAt) return
        menuPopup.visible = false
        lockLayer = lockComponent.createObject(null, { output: place.output })
        Platform.placeLayer(lockLayer, place.x, place.y)
        lockLayer.visible = true
        Platform.setInputRects(lockLayer, [])
        locked = true
        visible = false
    }
    function unlock() {
        if (!locked) return
        lockLayer.destroy()
        lockLayer = null
        locked = false
        lockHover = false
        hiddenAt = lockAt
        show()
    }
    property Component lockComponent: Component {
        Window {
            property string output: ""
            flags: Qt.FramelessWindowHint
            color: "transparent"
            width: pin.width; height: pin.height
            LayerShell.Window.screen: Platform.screen(output)
            LayerShell.Window.scope: "snip-pin"
            LayerShell.Window.layer: LayerShell.Window.LayerOverlay
            LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorLeft
            LayerShell.Window.exclusionZone: -1
            LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
            Face {
                x: pin.margin; y: pin.margin
                width: pin.width - 2 * pin.margin; height: pin.height - 2 * pin.margin
                opacity: pin.fade * (pin.lockHover ? 0.15 : 1)
                Behavior on opacity { NumberAnimation { duration: 120 } }
                source: pin.imageUrl
                radius: pin.radius
                shadow: pin.look.shadow !== false
                border: pin.look.border !== false
            }
        }
    }
    // the pointer can't be seen through an empty input region: ask Hyprland
    Timer {
        running: pin.locked && !!pin.lockLayer && pin.lockLayer.visible
        interval: 120; repeat: true
        onTriggered: {
            const c = JSON.parse(Engine.cursorPosition()), a = pin.lockAt
            pin.lockHover = !!c && c[0] >= a[0] + pin.margin && c[0] < a[0] + pin.width - pin.margin
                                && c[1] >= a[1] + pin.margin && c[1] < a[1] + pin.height - pin.margin
        }
    }
    Timer {
        id: restoreTimer
        interval: 60
        onTriggered: {
            if (pin.hiddenAt) Engine.moveWindow(pin.title, pin.hiddenAt[0], pin.hiddenAt[1], false)
            pin.hiddenAt = null
            showTimer.start()
        }
    }
    Timer { id: showTimer; interval: 60; onTriggered: frame.visible = true }

    Component.onCompleted: {
        // never larger than most of the screen
        const scr = Platform.screen(capture.output || "")
        const aw = scr ? scr.availableGeometry.width : 1600, ah = scr ? scr.availableGeometry.height : 1000
        zoom = Math.min(1, aw * 0.8 / baseW, ah * 0.8 / baseH)
        // Capture Area & Pin: exactly where the area was, at its own size
        if (capture.at) { zoom = 1; hiddenAt = [capture.at[0] - margin, capture.at[1] - margin]; show() }
        else visible = true
    }

    // the pin's look: the image with rounded corners, a soft shadow, a hairline
    component Face: Item {
        id: face
        property url source
        property real radius: 10
        property bool shadow: true
        property bool border: true
        RectangularShadow {
            visible: face.shadow
            anchors.fill: parent
            radius: face.radius
            blur: 14; offset.y: 4
            color: Qt.rgba(0, 0, 0, 0.35)
        }
        Image {
            id: picture
            anchors.fill: parent
            source: face.source
            smooth: true; mipmap: true
            visible: false
        }
        Item {
            id: roundMask
            anchors.fill: parent
            visible: false
            layer.enabled: true
            Rectangle { anchors.fill: parent; radius: face.radius; color: "white" }
        }
        MultiEffect {
            anchors.fill: parent
            source: picture
            maskEnabled: true
            maskSource: roundMask
        }
        Rectangle {
            visible: face.border
            anchors.fill: parent
            radius: face.radius
            color: "transparent"
            border.width: 1; border.color: Qt.rgba(1, 1, 1, 0.18)
        }
    }
    readonly property url imageUrl: "file://" + capture.path + "?" + (capture.rev || 0)

    Face {
        id: frame
        x: pin.margin; y: pin.margin
        width: pin.width - 2 * pin.margin; height: pin.height - 2 * pin.margin
        opacity: pin.fade
        source: pin.imageUrl
        radius: pin.radius
        shadow: pin.look.shadow !== false
        border: pin.look.border !== false
    }

    MouseArea {
        id: area
        anchors.fill: frame
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
        cursorShape: Qt.OpenHandCursor
        property point pressAt
        // a drag moves the pin (Hyprland does it); a click only focuses it
        onPositionChanged: (m) => {
            if ((m.buttons & Qt.LeftButton) && Math.hypot(m.x - pressAt.x, m.y - pressAt.y) > 3) {
                pin.startSystemMove()
                // Hyprland has the pointer now: this press never sees its release
                enabled = false
                Qt.callLater(() => enabled = true)
            }
        }
        onPressed: (m) => {
            if (m.button === Qt.LeftButton) pressAt = Qt.point(m.x, m.y)
            else if (m.button === Qt.MiddleButton) pin.close_()
            else {
                const g = area.mapToGlobal(m.x, m.y)
                menuPopup.x = g.x; menuPopup.y = g.y
                menuPopup.visible = true
            }
        }
        onWheel: (w) => {
            const steps = (w.angleDelta.y || w.angleDelta.x) / 120
            if (w.modifiers & Qt.ControlModifier) pin.zoomBy(Math.pow(1.1, steps))
            else pin.fade = Math.max(0.2, Math.min(1, pin.fade + steps * 0.05))
        }
    }

    // hover chrome, as CleanShot's: close at the top left, the zoom at the top
    Item {
        anchors.fill: frame
        visible: area.containsMouse || closeHover.hovered || gripHover.hovered || lockHover_.hovered
        FrostButton {
            x: 8; y: 8; icon: "x"
            HoverHandler { id: closeHover }
            onClicked: pin.close_()
        }
        // PIN-05: lock (click-through)
        FrostButton {
            x: parent.width - width - 8; y: 8; icon: "lock"
            HoverHandler { id: lockHover_ }
            onClicked: pin.lock()
        }
        // PIN-03: drag the image out into another app
        FrostButton {
            id: dragOut
            x: parent.width - width - 8; y: parent.height - height - 8
            icon: "grip"
            HoverHandler { id: gripHover }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.OpenHandCursor
                property point at
                onPressed: (m) => at = Qt.point(m.x, m.y)
                onPositionChanged: (m) => {
                    if (!pressed || Math.hypot(m.x - at.x, m.y - at.y) < 6) return
                    const path = Engine.dragFile(pin.capture.id)
                    if (path) Platform.startDrag(dragOut, path, path)
                }
            }
        }
        Rectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            y: 10
            width: zoomText.implicitWidth + 16; height: 20; radius: 10
            color: Qt.rgba(0.93, 0.93, 0.94, 0.92)
            Text { id: zoomText; anchors.centerIn: parent; text: Math.round(pin.zoom * 100) + "%"
                   color: "#1d1d1f"; font.family: Theme.fontUi; font.pixelSize: 11; font.weight: Font.DemiBold
                   font.features: { "tnum": 1 } }
        }
    }

    Item {
        focus: true
        Keys.onPressed: (e) => {
            const ctrl = e.modifiers & Qt.ControlModifier
            if (ctrl && e.key === Qt.Key_C) pin.act("copy")
            else if (ctrl && e.key === Qt.Key_S) pin.act("save")
            else if (ctrl && e.key === Qt.Key_E) pin.act("annotate")
            else if (ctrl && (e.key === Qt.Key_W)) pin.close_()
            else if (ctrl && (e.key === Qt.Key_Plus || e.key === Qt.Key_Equal)) pin.zoomBy(1.1)
            else if (ctrl && e.key === Qt.Key_Minus) pin.zoomBy(1 / 1.1)
            else if (ctrl && e.key === Qt.Key_0) pin.zoom = 1
            // PIN-04: arrows nudge it (Shift: 10 px)
            else if (e.key === Qt.Key_Left || e.key === Qt.Key_Right || e.key === Qt.Key_Up || e.key === Qt.Key_Down) {
                const d = (e.modifiers & Qt.ShiftModifier) ? 10 : 1
                Engine.moveWindow(pin.title, e.key === Qt.Key_Left ? -d : e.key === Qt.Key_Right ? d : 0,
                                  e.key === Qt.Key_Up ? -d : e.key === Qt.Key_Down ? d : 0, true)
            }
            else return
            e.accepted = true
        }
    }

    // test hooks (snip debug ui pin ...)
    function debug(a) {
        switch (a[0]) {
        case "zoom": zoom = parseFloat(a[1]); break
        case "fade": fade = parseFloat(a[1]); break
        case "act": act(a[1]); break
        case "keys": Platform.sendKeys(pin, a.slice(1).join(" ")); break
        case "state":
            Engine.log("pin_state", JSON.stringify({ id: capture.id, zoom: zoom, fade: fade, w: width, h: height,
                                                    baseW: baseW, baseH: baseH, margin: margin, menu: menuPopup.visible,
                                                    locked: locked, lockHover: lockHover, shown: shown, radius: radius,
                                                    shadow: look.shadow !== false }).slice(1, -1))
            break
        }
    }

    // PIN-06: the menu, a popup of its own so it can reach past a small pin
    Window {
        id: menuPopup
        transientParent: pin
        flags: Qt.Popup | Qt.FramelessWindowHint
        color: "transparent"
        width: pinMenu.width + 40; height: pinMenu.height + 40
        CardMenu {
            id: pinMenu
            x: 4; y: 4
            items: [
                { label: "Copy", key: "Ctrl+C", action: "copy", icon: "copy" },
                { label: "Save", key: "Ctrl+S", action: "save", icon: "save" },
                { label: "Annotate", key: "Ctrl+E", action: "annotate", icon: "pencil" },
                { label: "Lock (Click Through)", key: "", action: "lock", icon: "lock" },
                { sep: true },
                { label: "Close", key: "Ctrl+W", action: "close", icon: "x" },
                { label: "Close All Pins", key: "", action: "close-all", icon: "layers" }
            ]
            onTriggered: (action) => pin.act(action)
        }
    }
}
