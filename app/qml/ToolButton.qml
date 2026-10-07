import QtQuick

// An editor toolbar button: a glyph on a rounded accent rectangle when its
// tool is selected (CleanShot's toolbar).
Item {
    id: root
    property string icon: ""
    property string label: ""          // text instead of a glyph ("20 pt")
    property bool selected: false
    property bool chevron: false       // opens a dropdown
    property string tip: ""
    property bool tipAbove: false
    property real leftPad: 0           // room for content drawn before the glyph (the colour dot)
    signal clicked()

    implicitWidth: label !== "" || chevron ? row.implicitWidth + 16 + leftPad : 34
    implicitHeight: 24

    Rectangle {
        anchors.fill: parent
        radius: 7
        color: root.selected ? Theme.accent : ma.pressed ? Theme.controlPressed : ma.containsMouse ? Theme.controlHover : "transparent"
        Behavior on color { ColorAnimation { duration: 90 } }
    }
    Row {
        id: row
        x: (parent.width - implicitWidth + root.leftPad) / 2
        anchors.verticalCenter: parent.verticalCenter
        spacing: 4
        Icon {
            visible: root.icon !== ""
            name: root.icon
            size: 16
            stroke: 2
            color: root.selected ? Theme.onAccent : Theme.text
            anchors.verticalCenter: parent.verticalCenter
        }
        Text {
            visible: root.label !== ""
            text: root.label
            color: root.selected ? Theme.onAccent : Theme.text
            font.family: Theme.fontUi
            font.pixelSize: 13
            font.weight: Font.DemiBold
            anchors.verticalCenter: parent.verticalCenter
        }
        Icon {
            visible: root.chevron
            name: "chevron-down"
            size: 11
            stroke: 2.2
            color: Theme.textSecondary
            anchors.verticalCenter: parent.verticalCenter
        }
    }
    MouseArea {
        id: ma
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: { tipRest.stop(); hint.shown = false; root.clicked() }
        onContainsMouseChanged: if (containsMouse) tipRest.restart(); else { tipRest.stop(); hint.shown = false }
    }
    Timer { id: tipRest; interval: 600; onTriggered: hint.shown = true }
    Tip { id: hint; text: root.tip; above: root.tipAbove }
}
