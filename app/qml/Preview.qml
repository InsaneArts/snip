import QtQuick
import QtQuick.Effects
import QtQuick.Window
import org.kde.layershell as LayerShell
import Snip.Core

// Space on a card (QAO-03): a large preview of the capture. Any click,
// Space or Esc closes it.
Window {
    id: pv
    property var card: null
    property string output: ""

    function open(c) { card = c; output = c.output || output || Qt.application.screens[0].name }
    function close() { card = null }

    visible: card !== null && output !== ""
    color: "transparent"
    flags: Qt.FramelessWindowHint
    title: "snip-preview"

    LayerShell.Window.screen: Platform.screen(output)
    LayerShell.Window.scope: "snip-preview"
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
                               | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
    LayerShell.Window.exclusionZone: -1
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityExclusive

    Rectangle { anchors.fill: parent; color: Theme.scrim }

    Item {
        id: frameBox
        readonly property real maxW: pv.width * 0.86
        readonly property real maxH: pv.height * 0.86
        readonly property real iw: pv.card ? pv.card.width / Screen.devicePixelRatio : 1
        readonly property real ih: pv.card ? pv.card.height / Screen.devicePixelRatio : 1
        readonly property real k: Math.min(1, maxW / iw, maxH / ih)
        width: Math.round(iw * k)
        height: Math.round(ih * k)
        anchors.centerIn: parent

        RectangularShadow {
            anchors.fill: parent
            radius: Math.min(Theme.radius, 12)
            offset.y: Theme.shadowOffset
            blur: Theme.shadowBlur
            color: Theme.shadow
        }
        Image {
            anchors.fill: parent
            source: pv.card ? "file://" + pv.card.path : ""
            fillMode: Image.PreserveAspectFit
            asynchronous: true
            mipmap: true
            sourceSize.width: Math.round(parent.width * Screen.devicePixelRatio)
        }
    }

    MouseArea { anchors.fill: parent; onClicked: pv.close() }
    Item {
        focus: true
        Keys.onPressed: (e) => {
            if (e.key === Qt.Key_Escape || e.key === Qt.Key_Space) { pv.close(); e.accepted = true }
        }
    }
    onVisibleChanged: if (visible) requestActivate()
}
