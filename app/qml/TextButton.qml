import QtQuick

// A labelled capsule button (CleanShot's "Save as…" and "Done").
Rectangle {
    id: root
    property string text: ""
    property string tip: ""
    property bool primary: false
    signal clicked()

    implicitWidth: label.implicitWidth + 30
    implicitHeight: 30
    radius: height / 2
    readonly property color base: primary ? Theme.accent : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, Theme.light ? 0.07 : 0.11)
    color: ma.pressed ? Qt.darker(base, 1.15) : ma.containsMouse ? Qt.lighter(base, primary ? 1.08 : 1.35) : base
    Behavior on color { ColorAnimation { duration: 90 } }
    opacity: enabled ? 1 : 0.45             // it does nothing now: it doesn't look as if it would

    Text {
        id: label
        anchors.centerIn: parent
        text: root.text
        color: root.primary ? Theme.onAccent : Theme.text
        font.family: Theme.fontUi
        font.pixelSize: 13
        font.weight: Font.DemiBold
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
    Tip { id: hint; text: root.tip }
}
