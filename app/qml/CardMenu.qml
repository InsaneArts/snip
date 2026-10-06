import QtQuick
import QtQuick.Effects
import Snip.Core

// QAO-09 context menu, drawn by us so it follows the theme. Items whose
// feature lands in a later milestone are not shown yet.
Rectangle {
    id: menu
    property var card: null
    signal triggered(string action)

    readonly property var items: [
        { label: "Copy", key: "Ctrl+C", action: "copy", icon: "copy" },
        { label: card && card.saved ? "Show in Files" : "Save", key: card && card.saved ? "" : "Ctrl+S", action: card && card.saved ? "show" : "save", icon: card && card.saved ? "folder" : "save" },
        { label: "Annotate", key: "Ctrl+E", action: "annotate", icon: "pencil" },
        { label: "Extract Text", key: "", action: "text", icon: "scan-text" },
        { label: "Preview", key: "Space", action: "preview", icon: "eye" },
        { sep: true },
        { label: "Move to Trash", key: "", action: "trash", icon: "trash", danger: true },
        { sep: true },
        { label: "Close", key: "Ctrl+W", action: "close", icon: "x" },
        { label: "Close All", key: "", action: "close-all", icon: "layers" },
        { label: "Save All", key: "", action: "save-all", icon: "save" },
        { label: "Hide", key: "", action: "hide", icon: "eye-off" }
    ]

    width: 220
    height: col.implicitHeight + 12
    radius: Math.min(Theme.radius, 12)
    color: Qt.rgba(Theme.surface.r, Theme.surface.g, Theme.surface.b, 0.98)
    border.width: 1
    border.color: Theme.hairline

    RectangularShadow {
        z: -1
        anchors.fill: parent
        radius: parent.radius
        offset.y: 8
        blur: 28
        color: Theme.shadow
    }

    Column {
        id: col
        x: 6
        y: 6
        width: parent.width - 12
        Repeater {
            model: menu.items
            delegate: Item {
                required property var modelData
                width: col.width
                height: modelData.sep ? 9 : 30
                Rectangle {
                    visible: !!parent.modelData.sep
                    anchors.verticalCenter: parent.verticalCenter
                    x: 8
                    width: parent.width - 16
                    height: 1
                    color: Theme.hairline
                }
                Rectangle {
                    visible: !parent.modelData.sep
                    anchors.fill: parent
                    radius: Math.min(Theme.radiusControl, 6)
                    color: ma.pressed ? Theme.controlPressed : ma.containsMouse ? Theme.controlHover : "transparent"
                    Row {
                        x: 10
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 10
                        Icon {
                            name: modelData.icon || "x"
                            size: 15
                            stroke: 1.9
                            color: modelData.danger ? Theme.danger : Theme.textSecondary
                            anchors.verticalCenter: parent.verticalCenter
                        }
                        Text {
                            text: modelData.label || ""
                            color: modelData.danger ? Theme.danger : Theme.text
                            font.family: Theme.fontUi
                            font.pixelSize: 13
                            anchors.verticalCenter: parent.verticalCenter
                        }
                    }
                    Text {
                        anchors.right: parent.right
                        anchors.rightMargin: 10
                        anchors.verticalCenter: parent.verticalCenter
                        text: modelData.key || ""
                        color: Theme.textSecondary
                        font.family: Theme.fontUi
                        font.pixelSize: 11
                    }
                    MouseArea {
                        id: ma
                        anchors.fill: parent
                        hoverEnabled: true
                        enabled: !modelData.sep
                        onClicked: menu.triggered(modelData.action)
                    }
                }
            }
        }
    }
}
