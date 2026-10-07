import QtQuick

// CleanShot's toolbar capsule: a rounded group of controls on the bar.
Rectangle {
    default property alias content: capsuleRow.data
    property real padding: 3
    implicitWidth: capsuleRow.implicitWidth + padding * 2
    implicitHeight: 30
    radius: height / 2
    color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, Theme.light ? 0.07 : 0.09)
    Row { id: capsuleRow; anchors.centerIn: parent; spacing: 1 }
}
