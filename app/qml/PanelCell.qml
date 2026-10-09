import QtQuick
import Snip.Core

// A cell of the recording panel (D49), CleanShot's: an icon, white with a
// check under it when on, grey when off; its tip after a short rest.
Item {
    id: cell
    property string icon: ""
    property bool on: false
    property bool toggle: true
    property string tip: ""
    property color tint: "transparent"
    property color iconColor: on || !toggle ? Theme.text : Theme.textSecondary
    property int iconSize: 18
    property int inset: 4
    signal tapped()
    width: 44
    height: 44
    Rectangle {
        anchors.fill: parent
        anchors.margins: cell.inset
        radius: 6
        color: hover.hovered ? Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08) : cell.tint
    }
    Icon { anchors.centerIn: parent; anchors.verticalCenterOffset: cell.toggle && cell.on ? -4 : 0; name: cell.icon; size: cell.iconSize; color: cell.iconColor }
    Icon { visible: cell.toggle && cell.on; anchors.horizontalCenter: parent.horizontalCenter; y: parent.height - 14; name: "check"; size: 10; stroke: 2.5; color: Theme.text }
    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
        onHoveredChanged: if (hovered) rest.restart(); else { rest.stop(); hint.shown = false }
    }
    TapHandler { onTapped: { rest.stop(); hint.shown = false; cell.tapped() } }
    Timer { id: rest; interval: 600; onTriggered: hint.shown = true }
    Tip { id: hint; text: cell.tip; above: true }
}
