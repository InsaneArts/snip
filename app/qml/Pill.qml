import QtQuick

// Compact pill action (CleanShot's Copy / Save on cards).
Rectangle {
    id: root
    property string label: ""
    property string icon: ""
    property bool primary: false
    signal clicked()

    implicitWidth: row.implicitWidth + 24
    implicitHeight: 30
    radius: height / 2
    opacity: enabled ? 1 : 0.5
    color: primary ? (ma.pressed ? Qt.darker(Theme.accent, 1.15) : ma.containsMouse ? Qt.lighter(Theme.accent, 1.08) : Theme.accent)
                   : (ma.pressed ? Qt.darker(Theme.pillBg, 1.1) : ma.containsMouse ? Qt.lighter(Theme.pillBg, 1.15) : Theme.pillBg)
    border.width: primary ? 0 : 1
    border.color: Theme.hairline
    Behavior on color { ColorAnimation { duration: 90 } }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 6
        Icon {
            visible: root.icon !== ""
            name: root.icon
            size: 14
            stroke: 2
            color: root.primary ? Theme.onAccent : Theme.text
            anchors.verticalCenter: parent.verticalCenter
        }
        Text {
            text: root.label
            color: root.primary ? Theme.onAccent : Theme.text
            font.family: Theme.fontUi
            font.pixelSize: 13
            font.weight: Font.DemiBold
            anchors.verticalCenter: parent.verticalCenter
        }
    }
    MouseArea {
        id: ma
        anchors.fill: parent
        hoverEnabled: true
        enabled: root.enabled
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
