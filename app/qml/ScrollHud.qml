import QtQuick
import QtQuick.Window
import QtQuick.Effects
import org.kde.layershell as LayerShell
import Snip.Core

// Scrolling capture (SCR-01), CleanShot's: once the area is chosen, a frame
// around it and a panel beside it. Ready: Start Capture or Cancel (Enter,
// Esc). Capturing: the stitched preview growing as you scroll, its height
// and how the last frame went; Done or Cancel. A full-output layer that takes
// input only on the panel, so the page under the frame scrolls as usual, and
// draws nothing inside the area.
Window {
    id: hud
    property var controller: null
    property string output: ""
    property rect area: Qt.rect(0, 0, 0, 0)     // logical px on the output
    property string phase: "ready"               // ready | capturing
    property int stitched: 0                     // the image's height so far (px)
    property string step: ""                     // the last frame: moved | still | unsure | lost
    property string preview: ""                  // the stitched image, small (file URL)

    flags: Qt.FramelessWindowHint
    color: "transparent"
    visible: false
    LayerShell.Window.screen: Platform.screen(output !== "" ? output : Qt.application.screens[0].name)
    LayerShell.Window.scope: "snip-scroll"
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
                               | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
    LayerShell.Window.exclusionZone: -1
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone

    function open(s) {
        output = s.output
        area = Qt.rect(s.x, s.y, s.w, s.h)
        phase = "ready"
        stitched = 0
        step = ""
        preview = ""
        visible = true
        Qt.callLater(fitInput)
    }
    function status(s) {
        phase = s.phase
        stitched = s.height
        step = s.step
        if (s.preview) preview = s.preview
    }
    function close() { visible = false }

    // only the panel takes the pointer
    function fitInput() { Platform.setInputRects(hud, [Qt.rect(panel.x, panel.y, panel.width, panel.height)]) }

    // the frame: just outside the area, so it's never in the capture
    Rectangle {
        x: hud.area.x - 3
        y: hud.area.y - 3
        width: hud.area.width + 6
        height: hud.area.height + 6
        color: "transparent"
        radius: 3
        border.width: 2
        border.color: hud.step === "lost" ? Theme.danger : hud.step === "unsure" ? Theme.warning : Theme.accent
    }

    Rectangle {
        id: panel
        readonly property bool besideRight: hud.area.x + hud.area.width + 16 + width <= hud.width - 8
        width: 236
        height: col.implicitHeight + 24
        x: besideRight ? hud.area.x + hud.area.width + 16 : Math.max(8, hud.area.x - 16 - width)
        y: Math.max(8, Math.min(hud.area.y, hud.height - height - 8))
        radius: Math.max(10, Theme.radius)
        color: Theme.pillBg
        border.width: 1
        border.color: Theme.hairline
        onXChanged: hud.fitInput()
        onYChanged: hud.fitInput()
        onHeightChanged: hud.fitInput()
        layer.enabled: true
        layer.effect: MultiEffect { shadowEnabled: true; shadowColor: Theme.shadow; shadowBlur: 0.6; shadowVerticalOffset: 6 }

        Column {
            id: col
            x: 12
            y: 12
            width: parent.width - 24
            spacing: 10

            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: hud.phase === "ready" ? "Scrolling Capture"
                    : hud.step === "lost" ? "Too fast: scroll back a little"
                    : hud.step === "unsure" ? "Keep scrolling…"
                    : "Scroll slowly"
                color: hud.step === "lost" ? Theme.danger : Theme.text
                font.family: Theme.fontUi
                font.pixelSize: 14
                font.weight: Font.DemiBold
            }
            Text {
                width: parent.width
                visible: hud.phase === "ready"
                wrapMode: Text.Wrap
                text: "Start, then scroll the page inside the frame."
                color: Theme.textSecondary
                font.family: Theme.fontUi
                font.pixelSize: 12
            }

            // the stitched image so far
            Rectangle {
                visible: hud.phase === "capturing" && hud.preview !== ""
                width: parent.width
                height: Math.min(320, Math.max(60, shot.paintedHeight + 8))
                color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.05)
                radius: 6
                clip: true
                Image {
                    id: shot
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 4
                    width: parent.width - 8
                    fillMode: Image.PreserveAspectFit
                    source: hud.preview
                    cache: false
                    asynchronous: true
                }
            }
            Text {
                visible: hud.phase === "capturing"
                text: hud.stitched + " px"
                color: Theme.textSecondary
                font.family: Theme.fontUi
                font.pixelSize: 12
                font.features: { "tnum": 1 }
            }

            Row {
                spacing: 8
                anchors.right: parent.right
                TextButton { objectName: "scroll-cancel"; text: "Cancel"; onClicked: Engine.scrollCancel() }
                TextButton {
                    objectName: "scroll-go"
                    primary: true
                    text: hud.phase === "ready" ? "Start Capture" : "Done"
                    onClicked: hud.phase === "ready" ? Engine.scrollStart() : Engine.scrollDone()
                }
            }
            Text {
                anchors.right: parent.right
                text: "Enter · Esc"
                color: Theme.textSecondary
                font.family: Theme.fontUi
                font.pixelSize: 11
            }
        }
    }

    // tests: where a control is, in output logical px
    function debug(a) {
        switch (a[0]) {
        case "state":
            Engine.log("scroll_hud", JSON.stringify({ open: visible, phase: phase, height: stitched, step: step,
                                                     panel: [panel.x, panel.y, panel.width, panel.height] }).slice(1, -1))
            break
        }
    }
}
