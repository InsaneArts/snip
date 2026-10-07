import QtQuick

// A hover hint under (or above) a control, after a short rest.
Rectangle {
    id: tip
    property string text: ""
    property bool shown: false
    property bool above: false
    z: 100
    visible: opacity > 0 && text !== ""
    opacity: shown ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: 110; easing.type: Easing.OutCubic } }
    // centred on the control, but kept inside the window
    x: {
        const gx = shown ? parent.mapToItem(null, 0, 0).x : 0
        return Math.max(parent.width / 2 - width / 2, 6 - gx)
    }
    y: above ? -height - 8 : parent.height + 8
    width: label.implicitWidth + 16
    height: label.implicitHeight + 8
    radius: height / 2
    color: Theme.light ? "#1d1d1f" : Qt.lighter(Theme.surface, 1.6)
    Text {
        id: label
        anchors.centerIn: parent
        text: tip.text
        color: Theme.light ? "white" : Theme.text
        font.family: Theme.fontUi
        font.pixelSize: 12
    }
}
