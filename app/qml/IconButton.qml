import QtQuick

// Round icon button for card corners (close, pin, annotate, more).
Rectangle {
    id: root
    property string icon: "x"
    property string tip: ""
    signal clicked()

    width: 26
    height: 26
    radius: 13
    opacity: enabled ? 1 : 0.45
    color: ma.pressed ? Qt.darker(Theme.pillBg, 1.1) : ma.containsMouse ? Qt.lighter(Theme.pillBg, 1.2) : Theme.pillBg
    border.width: 1
    border.color: Theme.hairline
    Behavior on color { ColorAnimation { duration: 90 } }

    Icon {
        anchors.centerIn: parent
        name: root.icon
        size: 14
        stroke: 2
        color: Theme.text
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
