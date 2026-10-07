import QtQuick

// A labelled slider for the editor's panels: moved() while dragging (a live
// preview), released() once, so a drag is one undo step.
Item {
    id: root
    property string label: ""
    property real from: 0
    property real to: 100
    property real value: 0
    property string unit: ""
    signal moved(real value)
    signal released(real value)

    implicitWidth: 240
    implicitHeight: 40
    readonly property real frac: Math.max(0, Math.min(1, (value - from) / Math.max(1e-6, to - from)))

    Text { text: root.label; color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 12 }
    Text { anchors.right: parent.right; text: Math.round(root.value) + root.unit; color: Theme.text
           font.family: Theme.fontUi; font.pixelSize: 12; font.features: { "tnum": 1 } }
    Item {
        id: track
        y: 22; width: parent.width; height: 14
        Rectangle { anchors.verticalCenter: parent.verticalCenter; width: parent.width; height: 4; radius: 2
                    color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.15) }
        Rectangle { anchors.verticalCenter: parent.verticalCenter; width: root.frac * parent.width; height: 4; radius: 2; color: Theme.accent }
        Rectangle { x: root.frac * (parent.width - width); anchors.verticalCenter: parent.verticalCenter
                    width: 14; height: 14; radius: 7; color: "white"; border.width: 1; border.color: Theme.hairline }
        MouseArea {
            anchors.fill: parent
            anchors.margins: -6
            function at(m) { return root.from + Math.max(0, Math.min(1, (m.x - 6) / track.width)) * (root.to - root.from) }
            onPressed: (m) => root.moved(at(m))
            onPositionChanged: (m) => { if (pressed) root.moved(at(m)) }
            onReleased: (m) => root.released(at(m))
        }
    }
}
