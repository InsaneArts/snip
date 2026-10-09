import QtQuick
import Snip.Core

// A cell of the recording panel (D49). On, its icon sits on a soft accent
// square, as Full screen's does; off, the icon is grey, or crossed out for a
// source that's off (no microphone, no camera, no computer audio). The icon
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
    readonly property bool lit: toggle && on
    readonly property bool warned: warn.a > 0
    readonly property string shownIcon: toggle && !on && offIcon !== "" ? offIcon : icon
    signal tapped()
    width: 44
    height: 44
    // a square, however wide the cell
    Rectangle {
        anchors.centerIn: parent
        width: Math.min(parent.width, parent.height) - 2 * cell.inset
        height: width
        radius: 8
        color: cell.warned ? cell.warn
             : cell.lit ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, hover.hovered ? 0.3 : 0.2)
             : hover.hovered ? Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08) : "transparent"
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
