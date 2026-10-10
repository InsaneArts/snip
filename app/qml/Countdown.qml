import QtQuick
import QtQuick.Window
import QtQuick.Effects
import QtQuick.Shapes
import org.kde.layershell as LayerShell
import Snip.Core

// Self-timer countdown (CAP-06): a frosted circle with the seconds left and a
// draining ring, centred on the area about to be captured (outlined) or on
// the monitor. Click-through and without keyboard, so menus can be opened
// underneath it. It is never in the capture: at zero it turns transparent,
// and the capture runs once that frame is on screen.
Window {
    id: cd
    property string output: ""
    property rect area: Qt.rect(-1, -1, 0, 0)    // logical; x < 0: whole output
    property int total: 5
    property int left: 0
    property bool running: false
    property bool finishing: false
    // Stay mapped (transparent) until well after the capture: unmapping makes
    // Hyprland fade out a snapshot of the layer, which on NVIDIA can still be
    // the last opaque frame and would land in the capture.
    property bool holding: false
    property real progress: 1                      // 1 → 0 across the countdown

    readonly property bool hasArea: area.x >= 0 && area.width > 0
    readonly property point centre: hasArea ? Qt.point(area.x + area.width / 2, area.y + area.height / 2)
                                            : Qt.point(width / 2, height / 2)

    function start(out, x, y, w, h, seconds) {
        output = out
        area = Qt.rect(x, y, w, h)
        total = Math.max(1, seconds)
        left = total
        finishing = false
        progress = 1
        running = true
        drain.duration = total * 1000
        drain.restart()
        tick.restart()
    }

    function stop() {
        running = false
        finishing = false
        holding = false
        tick.stop()
        drain.stop()
    }

    visible: (running || finishing || holding) && output !== ""
    color: "transparent"
    flags: Qt.FramelessWindowHint
    title: "snip-countdown"

    LayerShell.Window.screen: Platform.screen(output)
    LayerShell.Window.scope: "snip-countdown"
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
                               | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
    LayerShell.Window.exclusionZone: -1
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
    onVisibleChanged: if (visible) Platform.setInputRects(cd, [])

    Timer {
        id: tick
        interval: 1000
        repeat: true
        onTriggered: {
            cd.left -= 1
            if (cd.left > 0) return
            // Off screen first, then capture (Snip's UI is never in a capture).
            // Keep the window mapped throughout: even a momentary unmap makes
            // Hyprland fade out a snapshot of the opaque dial, into the capture.
            tick.stop()
            drain.stop()
            cd.finishing = true
            cd.running = false
            Platform.afterFrames([cd], () => { if (cd.finishing) settle.start() })
        }
    }
    NumberAnimation { id: drain; target: cd; property: "progress"; from: 1; to: 0 }
    // "Swapped" is not "shown": the NVIDIA driver hands frames to the
    // compositor from its own thread, behind any still queued while the ring
    // animated. A short settle costs nothing at the end of a countdown.
    Timer {
        id: settle
        interval: 50
        onTriggered: { cd.finishing = false; cd.holding = true; release.start(); Engine.countdownDone() }
    }
    Timer { id: release; interval: 600; onTriggered: cd.holding = false }

    Item {
        anchors.fill: parent
        opacity: cd.running ? 1 : 0

        // what will be captured
        Rectangle {
            visible: cd.hasArea
            x: cd.area.x - 1
            y: cd.area.y - 1
            width: cd.area.width + 2
            height: cd.area.height + 2
            color: "transparent"
            border.width: 1.5
            border.color: Theme.accent
            radius: 2
        }

        Item {
            id: dial
            width: 96
            height: 96
            x: cd.centre.x - width / 2
            y: cd.centre.y - height / 2

            RectangularShadow { anchors.fill: disc; radius: disc.radius; blur: 30; offset.y: 8; color: Theme.shadow }
            Rectangle {
                id: disc
                anchors.fill: parent
                radius: width / 2
                color: Qt.rgba(Theme.surface.r, Theme.surface.g, Theme.surface.b, 0.94)
                border.width: 1
                border.color: Theme.hairline
            }
            Shape {
                anchors.fill: parent
                preferredRendererType: Shape.CurveRenderer
                ShapePath {
                    strokeColor: Theme.accent
                    strokeWidth: 3.5
                    fillColor: "transparent"
                    capStyle: ShapePath.RoundCap
                    PathAngleArc { centerX: 48; centerY: 48; radiusX: 41; radiusY: 41; startAngle: -90; sweepAngle: 360 * cd.progress }
                }
            }
            Text {
                id: number
                anchors.centerIn: parent
                text: Math.max(cd.left, 1)
                color: Theme.text
                font.family: Theme.fontUi
                font.pixelSize: 38
                font.weight: Font.DemiBold
                font.features: { "tnum": 1 }
                onTextChanged: pop.restart()
                NumberAnimation on scale { id: pop; from: 1.18; to: 1; duration: 220; easing.type: Easing.OutBack; running: false }
            }
        }
    }
}
