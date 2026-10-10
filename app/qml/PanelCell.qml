import QtQuick
import Snip.Core

// A cell of the recording panel (D49). On, its icon sits on a soft accent
// square, as Full screen's does; off, the icon is grey, or crossed out for a
// source that's off (no microphone, no camera, no computer audio). A live
// level fills the square from the bottom (D53: the microphone's). The icon
// never moves. Its tip shows after a short rest.
Item {
    id: cell
    property string icon: ""
    property string offIcon: ""               // off: this one (crossed out), else the icon in grey
    property bool on: false
    property bool toggle: true
    property string tip: ""
    property color warn: "transparent"        // a warning instead (a muted microphone)
    property int iconSize: 18
    property int inset: 4
    property real level: -1                   // 0–1, live: the square fills to it (none below 0)
    readonly property bool lit: toggle && on
    readonly property bool warned: warn.a > 0
    readonly property string shownIcon: toggle && !on && offIcon !== "" ? offIcon : icon
    signal tapped()
    width: 44
    height: 44
    // a square, however wide the cell
    Rectangle {
        id: square
        anchors.centerIn: parent
        width: Math.min(parent.width, parent.height) - 2 * cell.inset
        height: width
        radius: 8
        color: cell.warned ? cell.warn
             : cell.lit ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, hover.hovered ? 0.3 : 0.2)
             : hover.hovered ? Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08) : "transparent"
        // the level: the square's own shape, cut flat at its height
        Item {
            objectName: cell.objectName + ":level"
            visible: cell.level > 0
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
            height: square.height * Math.min(1, cell.level)
            clip: true
            Rectangle {
                anchors.bottom: parent.bottom
                width: square.width
                height: square.height
                radius: square.radius
                color: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.35)
            }
        }
    }
    Icon {
        anchors.centerIn: parent
        name: cell.shownIcon
        size: cell.iconSize
        color: cell.warned ? Theme.warning : cell.lit ? Theme.accent : cell.toggle ? Theme.textSecondary : Theme.text
    }
    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
        onHoveredChanged: if (hovered) rest.restart(); else { rest.stop(); hint.shown = false }
    }
    TapHandler { onTapped: { rest.stop(); hint.shown = false; cell.tapped() } }
    Timer { id: rest; interval: 600; onTriggered: hint.shown = true }
    Tip { id: hint; text: cell.tip; above: true }
}
