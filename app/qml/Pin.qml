import QtQuick
import QtQuick.Window
import QtQuick.Effects
import Snip.Core

// A pinned screenshot (PIN-01…08), CleanShot's: the image floats above every
// window, on every workspace of its monitor (Hyprland's pin rule, matched by
// the title). Drag (or arrow keys) to move, scroll to fade, Ctrl+scroll to
// zoom, middle-click to close; hovering shows close, the zoom and a grip to
// drag the image out; right-click opens the menu.
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
    readonly property real radius: 10
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

    function close_() { visible = false; finished() }
    function act(action) {
        menuPopup.visible = false
        switch (action) {
        case "copy": Engine.copy(capture.id); controller.toast.show("Copied", capture.output); break
        case "save": Engine.save(capture.id); break
        case "annotate": Engine.annotate(capture.id); break
        case "close": close_(); break
        case "close-all": controller.closePins(); break
        }
    }
    function zoomBy(f) { zoom = Math.max(0.1, Math.min(4, zoom * f)) }
    // PIN-07: hidden pins come back where they were (Hyprland would centre them)
    property var hiddenAt: null
    function hide() {
        hiddenAt = JSON.parse(Engine.windowPosition(title))
        visible = false
    }
    function show() {
        frame.visible = false           // until it is back in place
        visible = true
        restoreTimer.start()
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
        visible = true
    }

    Item {
        id: frame
        x: pin.margin; y: pin.margin
        width: pin.width - 2 * pin.margin; height: pin.height - 2 * pin.margin
        opacity: pin.fade

        RectangularShadow {
            anchors.fill: parent
            radius: pin.radius
            blur: 14; offset.y: 4
            color: Qt.rgba(0, 0, 0, 0.35)
        }
        Image {
            id: picture
            anchors.fill: parent
            source: "file://" + pin.capture.path + "?" + (pin.capture.rev || 0)
            smooth: true; mipmap: true
            visible: false
        }
        Item {
            id: roundMask
            anchors.fill: parent
            visible: false
            layer.enabled: true
            Rectangle { anchors.fill: parent; radius: pin.radius; color: "white" }
        }
        MultiEffect {
            anchors.fill: parent
            source: picture
            maskEnabled: true
            maskSource: roundMask
        }
        Rectangle {
            anchors.fill: parent
            radius: pin.radius
            color: "transparent"
            border.width: 1; border.color: Qt.rgba(1, 1, 1, 0.18)
        }
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
        visible: area.containsMouse || closeHover.hovered || gripHover.hovered
        FrostButton {
            x: 8; y: 8; icon: "x"
            HoverHandler { id: closeHover }
            onClicked: pin.close_()
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
                                                    baseW: baseW, baseH: baseH, margin: margin, menu: menuPopup.visible }).slice(1, -1))
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
                { sep: true },
                { label: "Close", key: "Ctrl+W", action: "close", icon: "x" },
                { label: "Close All Pins", key: "", action: "close-all", icon: "layers" }
            ]
            onTriggered: (action) => pin.act(action)
        }
    }
}
