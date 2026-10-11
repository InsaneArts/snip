import QtQuick
import QtQuick.Effects
import QtQuick.Window
import org.kde.layershell as LayerShell
import Snip.Core

// Short confirmations ("Copied 42 characters"), bottom centre of the
// monitor in use. Click-through, never takes focus.
Window {
    id: toast
    property string output: ""
    property string message: ""
    property bool shown: false

    function show(text, out) {
        Engine.log("toast", JSON.stringify({ text: text }).slice(1, -1))
        message = text
        if (out && out !== output) { shown = false; output = out }
        shown = true
        hideTimer.restart()
    }

    visible: shown && output !== ""
    color: "transparent"
    flags: Qt.FramelessWindowHint
    width: pill.width + 80
    height: pill.height + 80
    title: "snip-toast"

    LayerShell.Window.screen: Platform.screen(output)
    LayerShell.Window.scope: "snip-toast"
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorBottom
    LayerShell.Window.exclusionZone: 0
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
    onVisibleChanged: if (visible) { Platform.setMargins(toast, 0, 0, 0, 40); Platform.setInputRects(toast, []) }

    Timer { id: hideTimer; interval: 1900; onTriggered: toast.shown = false }

    RectangularShadow {
        anchors.fill: pill
        radius: pill.radius
        offset.y: 8
        blur: 28
        color: Theme.shadow
    }
    Rectangle {
        id: pill
        anchors.centerIn: parent
        height: 38
        width: Math.min(640, label.implicitWidth + 36)
        radius: Math.min(Theme.radius, 19)
        color: Theme.surface
        border.width: 1
        border.color: Theme.hairline
        Text {
            id: label
            anchors.centerIn: parent
            width: Math.min(implicitWidth, 604)
            elide: Text.ElideRight
            text: toast.message
            color: Theme.text
            font.family: Theme.fontUi
            font.pixelSize: 13
            font.weight: Font.Medium
            font.features: { "tnum": 1 }
        }
    }
}
