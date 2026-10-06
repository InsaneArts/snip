import QtQuick

// CleanShot's hover controls: light frosted capsules (Copy / Save) and
// circles (corner actions) with dark glyphs, on the frosted thumbnail.
Rectangle {
    id: root
    property string label: ""
    property string icon: ""
    property bool round: label === ""
    signal clicked()

    readonly property bool lit: ma.containsMouse
    implicitWidth: round ? 24 : Math.max(72, row.implicitWidth + 28)
    implicitHeight: round ? 24 : 30
    radius: height / 2
    color: ma.pressed ? Qt.rgba(0.82, 0.82, 0.84, 0.96) : lit ? Qt.rgba(1, 1, 1, 0.98) : Qt.rgba(0.93, 0.93, 0.94, 0.92)
    opacity: enabled ? 1 : 0.5
    scale: ma.pressed ? 0.96 : 1
    Behavior on color { ColorAnimation { duration: 90 } }
    Behavior on scale { NumberAnimation { duration: 90; easing.type: Easing.OutCubic } }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 5
        Icon {
            visible: root.icon !== ""
            name: root.icon
            size: root.round ? 13 : 14
            stroke: 2.2
            color: "#1d1d1f"
            anchors.verticalCenter: parent.verticalCenter
        }
        Text {
            visible: root.label !== ""
            text: root.label
            color: "#1d1d1f"
            font.family: Theme.fontUi
            font.pixelSize: 13
            font.weight: Font.DemiBold
            anchors.verticalCenter: parent.verticalCenter
        }
    }
    MouseArea {
        id: ma
        anchors.fill: parent
        anchors.margins: root.round ? -4 : 0      // forgiving hit area on the small circles
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
