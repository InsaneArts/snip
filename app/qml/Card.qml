import QtQuick
import QtQuick.Effects
import Snip.Core

// One Quick Access card (QAO-01..11): the capture is the hero; actions fade
// in on hover. Copy/Save in the middle, round buttons in the corners.
Item {
    id: card
    required property var data_       // the card JSON
    required property var host        // the QuickAccess window
    property bool newest: false
    property real cardWidth: 280
    readonly property string cardId: data_.id
    readonly property real aspect: data_.width > 0 ? data_.height / data_.width : 0.5625
    readonly property real clamped: Math.max(0.42, Math.min(1.25, aspect))
    property bool forceHover: false   // design gallery
    readonly property bool hovered: forceHover || bodyHover.hovered || hover.pressed
    property bool copied: false
    readonly property bool saved: !!data_.saved
    property real swipe: 0                // horizontal gesture offset (QAO-08)

    signal closeRequested(bool animated)
    signal menuRequested(real x, real y)

    width: cardWidth
    height: Math.round(cardWidth * clamped)

    RectangularShadow {
        anchors.fill: body
        radius: body.radius
        offset.y: Theme.shadowOffset
        blur: Theme.shadowBlur
        spread: -4
        color: Theme.shadow
        opacity: body.opacity
    }

    Item {
        id: body
        anchors.fill: parent
        readonly property real radius: Theme.radius
        x: card.swipe
        opacity: 1 - Math.min(0.7, Math.abs(card.swipe) / card.width)

        Item {
            id: content
            anchors.fill: parent
            layer.enabled: body.radius > 0
            layer.effect: MultiEffect {
                maskEnabled: true
                maskSource: mask
                maskThresholdMin: 0.5
                maskSpreadAtMin: 1.0
            }
            Rectangle { anchors.fill: parent; color: Theme.surface }
            Image {
                id: thumb
                anchors.fill: parent
                source: "file://" + (card.data_.thumb || card.data_.path)
                fillMode: Math.abs(card.aspect - card.clamped) < 0.001 ? Image.PreserveAspectCrop : Image.PreserveAspectFit
                asynchronous: false
                cache: false
                smooth: true
                mipmap: true
            }
            Rectangle {   // hover scrim (design §4: gentle, 120 ms)
                anchors.fill: parent
                color: Theme.hoverScrim
                opacity: card.hovered ? 1 : 0
                Behavior on opacity { NumberAnimation { duration: 120 } }
            }
        }
        Item {
            id: mask
            anchors.fill: parent
            visible: false
            layer.enabled: true
            Rectangle { anchors.fill: parent; radius: body.radius; color: "black" }
        }
        Rectangle {   // hairline, and the newest-capture indicator
            anchors.fill: parent
            radius: body.radius
            color: "transparent"
            border.width: card.newest ? 2 : 1
            border.color: card.newest ? Theme.accent : Theme.hairline
        }

        HoverHandler { id: bodyHover }

        // the card body: drag out, double-click, right-click, gestures
        MouseArea {
            id: hover
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            property point p0
            property bool dragged: false
            onPressed: (m) => {
                p0 = Qt.point(m.x, m.y)
                dragged = false
                if (m.button === Qt.RightButton) card.menuRequested(m.x, m.y)
            }
            onPositionChanged: (m) => {
                if (!pressed || dragged || !(m.buttons & Qt.LeftButton)) return
                if (Math.hypot(m.x - p0.x, m.y - p0.y) > 6) {
                    dragged = true
                    card.host.dragging = true
                    const path = Engine.dragFile(card.cardId)
                    Platform.startDrag(card, path, card.data_.thumb || "")
                    card.host.dragging = false
                }
            }
            onDoubleClicked: (m) => { if (m.button === Qt.LeftButton) card.host.act("annotate", card) }
        }

        WheelHandler {
            id: wheel
            acceptedDevices: PointerDevice.TouchPad
            orientation: Qt.Horizontal | Qt.Vertical
            onWheel: (e) => {
                const dx = e.pixelDelta.x, dy = e.pixelDelta.y
                if (Math.abs(dx) >= Math.abs(dy)) {
                    card.swipe += (e.inverted ? -dx : dx) * -1
                    gestureEnd.restart()
                } else if (dy < -24 && !e.inverted || dy > 24 && e.inverted) {
                    card.host.command("hide")
                }
            }
        }
        Timer {
            id: gestureEnd
            interval: 120
            onTriggered: {
                if (Math.abs(card.swipe) > card.width * 0.33) card.closeRequested(true)
                else swipeBack.start()
            }
        }
        NumberAnimation { id: swipeBack; target: card; property: "swipe"; to: 0; duration: 220; easing.type: Easing.OutBack }

        // --- saved badge (always visible once saved)
        Rectangle {
            visible: card.saved
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: parent.top
            anchors.topMargin: 8
            height: 22
            width: savedRow.implicitWidth + 14
            radius: 11
            color: Theme.pillBg
            opacity: card.hovered ? 0 : 1
            Behavior on opacity { NumberAnimation { duration: 120 } }
            Row {
                id: savedRow
                anchors.centerIn: parent
                spacing: 4
                Icon { name: "check"; size: 12; stroke: 2.5; color: Theme.success; anchors.verticalCenter: parent.verticalCenter }
                Text { text: "Saved"; color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 11; font.weight: Font.DemiBold; anchors.verticalCenter: parent.verticalCenter }
            }
        }

        // --- hover actions
        Item {
            anchors.fill: parent
            opacity: card.hovered ? 1 : 0
            visible: opacity > 0
            Behavior on opacity { NumberAnimation { duration: 120 } }

            Column {
                anchors.centerIn: parent
                spacing: 8
                Pill {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: Math.max(implicitWidth, 112)
                    label: card.copied ? "Copied" : "Copy"
                    icon: card.copied ? "check" : ""
                    onClicked: card.host.act("copy", card)
                }
                Pill {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: Math.max(implicitWidth, 112)
                    label: card.saved ? "Show in Files" : "Save"
                    icon: card.saved ? "folder" : ""
                    onClicked: card.host.act(card.saved ? "show" : "save", card)
                }
            }
            IconButton {
                x: 8; y: 8
                icon: "x"
                onClicked: card.closeRequested(true)
            }
            IconButton {
                x: 8; y: parent.height - height - 8
                icon: "pencil"
                onClicked: card.host.act("annotate", card)
            }
            IconButton {
                x: parent.width - width - 8; y: parent.height - height - 8
                icon: "more"
                onClicked: card.menuRequested(x + width / 2, y)
            }
        }
    }

    Timer {
        id: copiedReset
        interval: 1400
        onTriggered: card.copied = false
    }
    function flashCopied() { copied = true; copiedReset.restart() }
}
