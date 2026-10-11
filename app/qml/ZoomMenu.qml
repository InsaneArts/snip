import QtQuick
import QtQuick.Effects
import Snip.Core

// A zoom's right-click menu (D55; CleanShot's): its level to every zoom,
// split it at the playhead (the halves touch, so the view pans from one to
// the other once one is aimed elsewhere), or no zooms at all.
Rectangle {
    id: menu
    property var ed: null                   // the video editor
    property int index: -1                  // the zoom it's for; -1: closed
    readonly property var zoom: ed && index >= 0 ? ed.studio.zooms[index] || null : null
    readonly property bool canSplit: !!zoom && ed.headMs >= zoom.from + ed.zoomLeast && ed.headMs <= zoom.to - ed.zoomLeast
    function openAt(i, px, py, room) {
        index = i
        x = Math.max(8, Math.min(room.width - width - 8, px))
        y = Math.max(8, py - height - 4)
    }
    function close() { index = -1 }

    visible: zoom !== null
    width: 230
    height: rows.implicitHeight + 12
    radius: Math.min(Theme.radius, 12)
    color: Qt.rgba(Theme.surface.r, Theme.surface.g, Theme.surface.b, 0.98)
    border.width: 1
    border.color: Theme.hairline
    RectangularShadow { z: -1; anchors.fill: parent; radius: parent.radius; offset.y: 8; blur: 28; color: Theme.shadow }

    Column {
        id: rows
        x: 6
        y: 6
        width: parent.width - 12
        Repeater {
            model: [
                { label: "Apply Level to All", action: "level", icon: "layers", on: !!menu.ed && menu.ed.studio.zooms.length > 1 },
                { label: "Split Zoom at Playhead", action: "split", icon: "scissors", on: menu.canSplit },
                { label: "Remove All Zooms", action: "clear", icon: "trash", on: true, danger: true }
            ]
            delegate: Rectangle {
                id: row
                required property var modelData
                objectName: "studio-zoom-menu:" + modelData.action
                width: rows.width
                height: 30
                radius: Math.min(Theme.radiusControl, 6)
                opacity: modelData.on ? 1 : 0.4
                color: rowArea.pressed ? Theme.controlPressed : rowArea.containsMouse && modelData.on ? Theme.controlHover : "transparent"
                Row {
                    x: 10
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 10
                    Icon {
                        anchors.verticalCenter: parent.verticalCenter
                        name: row.modelData.icon
                        size: 15
                        stroke: 1.9
                        color: row.modelData.danger ? Theme.danger : Theme.textSecondary
                    }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: row.modelData.label
                        color: row.modelData.danger ? Theme.danger : Theme.text
                        font.family: Theme.fontUi
                        font.pixelSize: 13
                    }
                }
                MouseArea {
                    id: rowArea
                    anchors.fill: parent
                    hoverEnabled: true
                    enabled: row.modelData.on
                    onClicked: {
                        // closed, the menu builds its rows again: this one goes
                        const i = menu.index, action = row.modelData.action, ed = menu.ed
                        menu.close()
                        if (action === "level") ed.levelToAll(i)
                        else if (action === "split") ed.splitZoom(i, ed.headMs)
                        else ed.removeAllZooms()
                    }
                }
            }
        }
    }
}
