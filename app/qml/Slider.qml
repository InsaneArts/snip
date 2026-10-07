import QtQuick

// A labelled slider for the editor's panels: moved() while dragging (a live
// preview), released() once, so a drag is one undo step. As in CleanShot
// (BG-05), hovering shows the value; click it to type one, ↑/↓ step it
// (Shift: by 10), Enter or Tab commits, Escape cancels.
Item {
    id: root
    property string label: ""
    property real from: 0
    property real to: 100
    property real value: 0
    property string unit: ""
    property bool editing: false
    readonly property alias valueBox: valueBox
    signal moved(real value)
    signal released(real value)

    implicitWidth: 240
    implicitHeight: 40
    readonly property real frac: Math.max(0, Math.min(1, (value - from) / Math.max(1e-6, to - from)))
    function clamp(v) { return Math.max(from, Math.min(to, v)) }
    function edit() {
        editing = true
        field.text = Math.round(value)
        field.selectAll()
        field.forceActiveFocus()
    }
    function commit() {
        if (!editing) return
        editing = false
        const v = parseFloat(field.text)
        if (!isNaN(v)) released(clamp(v))
    }
    function step(e) {
        const v = clamp((parseFloat(field.text) || 0) + (e.key === Qt.Key_Up ? 1 : -1) * (e.modifiers & Qt.ShiftModifier ? 10 : 1))
        field.text = Math.round(v)
        moved(v)
        e.accepted = true
    }

    HoverHandler { id: hover }
    Text { text: root.label; color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 12 }
    Rectangle {
        id: valueBox
        visible: hover.hovered || root.editing || drag.pressed
        anchors.right: parent.right
        y: -2
        width: Math.max(28, (root.editing ? field.contentWidth : shown.contentWidth) + 12); height: 18; radius: 5
        color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, root.editing ? 0.16 : 0.08)
        border.width: root.editing ? 1 : 0; border.color: Theme.accent
        Text { id: shown; visible: !root.editing; anchors.centerIn: parent; text: Math.round(root.value) + root.unit
               color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 12; font.features: { "tnum": 1 } }
        TextInput {
            id: field
            visible: root.editing
            anchors.centerIn: parent
            color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 12; font.features: { "tnum": 1 }
            validator: IntValidator { bottom: Math.floor(root.from); top: Math.ceil(root.to) }
            onAccepted: root.commit()
            Keys.onTabPressed: root.commit()
            Keys.onUpPressed: (e) => root.step(e)
            Keys.onDownPressed: (e) => root.step(e)
            Keys.onEscapePressed: { root.editing = false; root.moved(root.value) }
            onActiveFocusChanged: if (!activeFocus) root.commit()
        }
        MouseArea { visible: !root.editing; anchors.fill: parent; cursorShape: Qt.IBeamCursor; onClicked: root.edit() }
    }
    Item {
        id: track
        y: 22; width: parent.width; height: 14
        Rectangle { anchors.verticalCenter: parent.verticalCenter; width: parent.width; height: 4; radius: 2
                    color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.15) }
        Rectangle { anchors.verticalCenter: parent.verticalCenter; width: root.frac * parent.width; height: 4; radius: 2; color: Theme.accent }
        Rectangle { x: root.frac * (parent.width - width); anchors.verticalCenter: parent.verticalCenter
                    width: 14; height: 14; radius: 7; color: "white"; border.width: 1; border.color: Theme.hairline }
        MouseArea {
            id: drag
            anchors.fill: parent
            anchors.margins: -6
            preventStealing: true           // a drag that wanders up or down still moves the slider
            function at(m) { return root.from + Math.max(0, Math.min(1, (m.x - 6) / track.width)) * (root.to - root.from) }
            onPressed: (m) => root.moved(at(m))
            onPositionChanged: (m) => { if (pressed) root.moved(at(m)) }
            onReleased: (m) => root.released(at(m))
        }
    }
}
