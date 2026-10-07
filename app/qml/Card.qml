import QtQuick
import QtQuick.Effects
import QtQuick.Shapes
import Snip.Core

// One Quick Access card, modelled on CleanShot X's (measured from its demo
// videos, docs/design/qao-motion.md): a fixed-size thumbnail that fills the
// card; on hover it frosts (blur + tint) under light capsule buttons and
// corner circles; the newest card carries an accent ring outside its edge.
Item {
    id: card
    required property var data_       // the card JSON
    required property var host        // the QuickAccess window
    property bool newest: false
    property real cardWidth: 280
    property bool forceHover: false   // design gallery
    // "" | copied | saved | text (reading) | text-copied | qr-copied | no-text | error
    property string status: ""
    property string detail: ""        // second line under the status, e.g. "42 characters"
    readonly property bool success: status === "copied" || status === "saved" || status === "text-copied" || status === "qr-copied"
    property bool dragging: false
    property real swipe: 0             // horizontal gesture offset (QAO-08)

    readonly property string cardId: data_.id
    readonly property bool saved: !!data_.saved
    readonly property bool hovered: forceHover || bodyHover.hovered || hover.pressed
    // 0..1: thumbnail frosting and hover controls (CleanShot: ~150 ms)
    readonly property bool frosted: (hovered && !dragging) || status !== ""
    property real frost: frosted ? 1 : 0
    Behavior on frost { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }

    signal closeRequested(bool animated)
    signal menuRequested(real x, real y)

    width: cardWidth
    height: Math.round(cardWidth * 0.705)    // CleanShot's card aspect

    // ---------------------------------------------------------------- shadow
    RectangularShadow {
        anchors.fill: body
        radius: body.radius
        offset.y: Theme.shadowOffset * 0.6
        blur: Theme.shadowBlur * 0.8
        spread: -2
        color: Theme.shadow
        opacity: body.opacity
    }

    // ---------------------------------------------------------------- newest ring (outside the card)
    Rectangle {
        readonly property real w: 2.5
        x: body.x - w; y: body.y - w
        width: body.width + 2 * w; height: body.height + 2 * w
        radius: body.radius > 0 ? body.radius + w : 0
        color: "transparent"
        border.width: w
        border.color: Theme.accent
        opacity: card.newest && !card.dragging ? body.opacity : 0
        Behavior on opacity { NumberAnimation { duration: 200 } }
    }

    Item {
        id: body
        width: card.width
        height: card.height
        readonly property real radius: Theme.radius
        x: card.swipe
        opacity: (card.dragging ? 0.5 : 1) * (1 - Math.min(0.8, Math.abs(card.swipe) / card.width))
        Behavior on opacity { NumberAnimation { duration: 140 } }

        Item {
            id: content
            anchors.fill: parent
            layer.enabled: true
            layer.effect: MultiEffect {
                maskEnabled: body.radius > 0
                maskSource: mask
                maskThresholdMin: 0.5
                maskSpreadAtMin: 1.0
            }
            Rectangle { anchors.fill: parent; color: Theme.surface }
            Image {
                id: thumb
                anchors.fill: parent
                source: "file://" + (card.data_.thumb || card.data_.path) + "?" + (card.data_.rev || 0)
                fillMode: Image.PreserveAspectCrop
                asynchronous: false
                cache: false
                smooth: true
                mipmap: true
                sourceSize.width: Math.round(card.width * 2)
            }
            // frosted copy of the thumbnail, scaled up so the blur's soft
            // edges stay outside the card
            MultiEffect {
                anchors.fill: thumb
                source: thumb
                visible: card.frost > 0.01
                opacity: card.frost
                scale: 1.12
                blurEnabled: true
                blur: 1.0
                blurMax: 48
                blurMultiplier: 0.6
                saturation: -0.15
                brightness: Theme.light ? -0.04 : -0.12
            }
            Rectangle {   // tint: the frosted glass colour
                anchors.fill: parent
                // a mid-tone in light themes too, so the light controls read (CleanShot does the same)
                color: Theme.light ? Qt.rgba(0.10, 0.08, 0.08, 0.26) : Qt.rgba(0.04, 0.05, 0.08, 0.38)
                opacity: card.frost
            }
            Rectangle {   // a status (white text) needs a darker glass on light screenshots
                anchors.fill: parent
                color: Qt.rgba(0, 0, 0, 0.30)
                opacity: card.status !== "" ? card.frost : 0
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
        Rectangle {   // hairline
            anchors.fill: parent
            radius: body.radius
            color: "transparent"
            border.width: 1
            border.color: Theme.light ? Qt.rgba(0, 0, 0, 0.10) : Qt.rgba(1, 1, 1, 0.10)
        }

        HoverHandler { id: bodyHover }

        // the card body: drag out, double-click, right-click
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
                    card.host.drag(card)
                }
            }
            onDoubleClicked: (m) => { if (m.button === Qt.LeftButton) card.host.act("annotate", card) }
        }

        // two-finger swipes (QAO-08): sideways dismisses, down hides all
        WheelHandler {
            acceptedDevices: PointerDevice.TouchPad
            orientation: Qt.Horizontal | Qt.Vertical
            onWheel: (e) => {
                const dx = e.pixelDelta.x, dy = e.pixelDelta.y
                if (Math.abs(dx) >= Math.abs(dy)) {
                    card.swipe += e.inverted ? dx : -dx
                    gestureEnd.restart()
                } else if ((e.inverted ? dy : -dy) > 24) {
                    card.host.command("hide")
                }
            }
        }
        Timer {
            id: gestureEnd
            interval: 120
            onTriggered: {
                if (Math.abs(card.swipe) > card.width * 0.3) card.host.dismiss(card, card.swipe)
                else swipeBack.start()
            }
        }
        NumberAnimation { id: swipeBack; target: card; property: "swipe"; to: 0; duration: 260; easing.type: Easing.OutBack }

        // ---------------------------------------------------------------- Saved chip (at rest)
        Rectangle {
            visible: opacity > 0
            opacity: card.saved && card.frost < 0.5 ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: 140 } }
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: parent.top
            anchors.topMargin: 8
            height: 20
            width: savedRow.implicitWidth + 14
            radius: 10
            color: Qt.rgba(0.93, 0.93, 0.94, 0.9)
            Row {
                id: savedRow
                anchors.centerIn: parent
                spacing: 4
                Icon { name: "check"; size: 11; stroke: 2.6; color: "#1d1d1f"; anchors.verticalCenter: parent.verticalCenter }
                Text { text: "Saved"; color: "#1d1d1f"; font.family: Theme.fontUi; font.pixelSize: 11; font.weight: Font.DemiBold; anchors.verticalCenter: parent.verticalCenter }
            }
        }

        // ---------------------------------------------------------------- hover controls
        Item {
            anchors.fill: parent
            opacity: card.status === "" ? card.frost : 0
            visible: opacity > 0.01
            enabled: card.hovered && card.status === ""

            Column {
                anchors.centerIn: parent
                spacing: 10
                FrostButton {
                    anchors.horizontalCenter: parent.horizontalCenter
                    label: "Copy"
                    onClicked: card.host.act("copy", card)
                }
                FrostButton {
                    anchors.horizontalCenter: parent.horizontalCenter
                    label: card.saved ? "Show in Files" : "Save"
                    onClicked: card.host.act(card.saved ? "show" : "save", card)
                }
            }
            FrostButton { x: 8; y: 8; icon: "x"; onClicked: card.host.dismiss(card, -1) }
            // CleanShot's corners: close, pin; annotate, (cloud →) text. Right-click: the menu.
            FrostButton { x: parent.width - width - 8; y: 8; icon: "pin"; onClicked: card.host.act("pin", card) }
            FrostButton { x: 8; y: parent.height - height - 8; icon: "pencil"; onClicked: card.host.act("annotate", card) }
            FrostButton { x: parent.width - width - 8; y: parent.height - height - 8; icon: "scan-text"; onClicked: card.host.act("text", card) }
        }

        // ---------------------------------------------------------------- status: work in progress and results
        // (CleanShot draws "Uploading…" / "Link copied" the same way, on the frosted card)
        Column {
            anchors.centerIn: parent
            width: parent.width - 24
            spacing: 6
            opacity: card.status !== "" ? card.frost : 0
            visible: opacity > 0.01
            scale: 0.9 + 0.1 * opacity

            Rectangle {
                anchors.horizontalCenter: parent.horizontalCenter
                width: 34; height: 34; radius: 17
                color: Qt.rgba(0.95, 0.95, 0.96, 0.95)
                Icon {
                    visible: card.status !== "text"
                    anchors.centerIn: parent
                    name: card.success ? "check" : card.status === "no-text" ? "scan-text" : "x"
                    size: 17; stroke: 2.6; color: "#1d1d1f"
                }
                Shape {   // spinner while reading
                    visible: card.status === "text"
                    anchors.fill: parent
                    preferredRendererType: Shape.CurveRenderer
                    ShapePath {
                        strokeColor: "#1d1d1f"; strokeWidth: 2.4; fillColor: "transparent"; capStyle: ShapePath.RoundCap
                        PathAngleArc { centerX: 17; centerY: 17; radiusX: 8; radiusY: 8; startAngle: -90; sweepAngle: 260 }
                    }
                    RotationAnimator on rotation { from: 0; to: 360; duration: 800; loops: Animation.Infinite; running: card.status === "text" }
                }
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: ({ "copied": "Copied", "saved": "Saved", "text": "Reading text…", "text-copied": "Text copied",
                         "qr-copied": "QR code copied", "no-text": "No text found", "error": "Couldn't read text" })[card.status] || ""
                color: "white"
                font.family: Theme.fontUi
                font.pixelSize: 13
                font.weight: Font.DemiBold
            }
            Text {
                visible: card.detail !== ""
                anchors.horizontalCenter: parent.horizontalCenter
                width: Math.min(implicitWidth, parent.width)
                horizontalAlignment: Text.AlignHCenter
                elide: Text.ElideMiddle
                text: card.detail
                color: Qt.rgba(1, 1, 1, 0.78)
                font.family: Theme.fontUi
                font.pixelSize: 11
                font.features: { "tnum": 1 }
            }
        }
    }

    Timer { id: statusReset; onTriggered: { card.status = ""; card.detail = "" } }
    // show a status for `ms` (0 = until replaced; work in progress gives up after 20 s)
    function flash(s, ms, d) {
        status = s
        detail = d || ""
        statusReset.interval = ms > 0 ? ms : 20000
        statusReset.restart()
    }
    // a result from the engine (Extract Text)
    function showResult(s, d) {
        flash(s, s === "qr-copied" ? 2200 : s === "text-copied" ? 1700 : s === "error" ? 2400 : 1500, d)
    }
}
