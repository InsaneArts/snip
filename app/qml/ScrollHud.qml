import QtQuick
import QtQuick.Window
import QtQuick.Effects
import org.kde.layershell as LayerShell
import Snip.Core
import "probe.js" as Probe

// Scrolling capture (SCR-01, 02, 04, 05), CleanShot's: once the area is
// chosen, a frame around it and a panel beside it. Ready: Start Capture
// (Enter), Auto-Scroll, or Cancel (Esc), with tips the first time. Capturing:
// the stitched preview growing as the page scrolls, its height, how the last
// frame went (too fast; very large); Done or Cancel. A full-output layer that
// takes input only on the panel, so the page under the frame scrolls as
// usual, and draws nothing inside the area.
Window {
    id: hud
    property var controller: null
    property string output: ""
    property rect area: Qt.rect(0, 0, 0, 0)     // logical px on the output
    property string phase: "ready"               // ready | capturing
    property int stitched: 0                     // the image's height so far (px)
    property string step: ""                     // the last frame: moved | still | unsure | lost
    property string preview: ""                  // the stitched image, small (file URL)
    property bool auto: false                    // Snip scrolls (SCR-02)
    property bool paused: false                  // auto-scroll waits for the pointer to come back
    // SCR-05: tips until "Got it"
    property bool tips: JSON.parse(Engine.editorPrefs() || "{}").scrollTips !== false

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
        auto = false
        paused = false
        tips = JSON.parse(Engine.editorPrefs() || "{}").scrollTips !== false
        visible = true
        Qt.callLater(fitInput)
    }
    function status(s) {
        phase = s.phase
        stitched = s.height
        step = s.step
        auto = !!s.auto
        paused = !!s.paused
        if (s.preview) preview = s.preview
    }
    function gotIt() {
        tips = false
        Engine.setEditorPrefs(JSON.stringify(Object.assign(JSON.parse(Engine.editorPrefs() || "{}"), { scrollTips: false })))
    }
    readonly property bool large: stitched > 30000
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
        width: 330
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
                    : hud.paused ? "Paused: move the pointer back into the frame"
                    : hud.step === "lost" ? (hud.auto ? "Lost the page: scroll back a little" : "Too fast: scroll back a little")
                    : hud.auto ? "Scrolling…"
                    : hud.step === "unsure" ? "Keep scrolling…"
                    : "Scroll slowly"
                color: hud.step === "lost" ? Theme.danger : hud.paused ? Theme.warning : Theme.text
                font.family: Theme.fontUi
                font.pixelSize: 14
                font.weight: Font.DemiBold
            }
            Text {
                width: parent.width
                visible: hud.phase === "ready" && !hud.tips
                wrapMode: Text.Wrap
                text: "Start, then scroll the page inside the frame. Or let Snip scroll it."
                color: Theme.textSecondary
                font.family: Theme.fontUi
                font.pixelSize: 12
            }
            // SCR-05: what to select, and what breaks stitching
            Rectangle {
                visible: hud.phase === "ready" && hud.tips
                width: parent.width
                height: tipsCol.implicitHeight + 20
                radius: 8
                color: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.12)
                Column {
                    id: tipsCol
                    x: 10; y: 10
                    width: parent.width - 20
                    spacing: 6
                    Repeater {
                        model: ["Select only the part that scrolls: leave out sidebars and toolbars.",
                                "Scroll down at a steady pace. Too fast, and Snip asks you to scroll back.",
                                "Videos, animations and pages that change while you scroll can't be stitched."]
                        Text {
                            required property string modelData
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: "•  " + modelData
                            color: Theme.text
                            font.family: Theme.fontUi
                            font.pixelSize: 12
                        }
                    }
                    TextButton { objectName: "scroll-tips-ok"; text: "Got it"; implicitHeight: 26; anchors.right: parent.right; onClicked: hud.gotIt() }
                }
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
                width: parent.width
                wrapMode: Text.Wrap
                // SCR-04
                text: hud.large ? hud.stitched + " px: very large. Some apps can't open images this tall." : hud.stitched + " px"
                color: hud.large ? Theme.warning : Theme.textSecondary
                font.family: Theme.fontUi
                font.pixelSize: 12
                font.features: { "tnum": 1 }
            }

            Row {
                spacing: 8
                anchors.right: parent.right
                TextButton { objectName: "scroll-cancel"; text: "Cancel"; onClicked: Engine.scrollCancel() }
                TextButton { objectName: "scroll-auto"; visible: hud.phase === "ready"; text: "Auto-Scroll"; onClicked: Engine.scrollAuto() }
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
        case "where": Engine.log("scroll_where", JSON.stringify(Probe.where(hud, a[1])).slice(1, -1)); break
        case "state":
            Engine.log("scroll_hud", JSON.stringify({ open: visible, phase: phase, height: stitched, step: step, auto: auto, paused: paused,
                                                     tips: tips, panel: [panel.x, panel.y, panel.width, panel.height] }).slice(1, -1))
            break
        }
    }
}
