import QtQuick
import QtQuick.Window
import QtQuick.Effects
import org.kde.layershell as LayerShell
import Snip.Core

// Design gallery (design doc §6): every M1 surface in its key states,
// rendered with real components for a list of Omarchy themes, one PNG each.
//
//   snipd --gallery [--output NAME] [--themes a,b,c] [--out DIR]
//
// Default themes: the current one, the first light theme and the first
// theme with a gradient border (the three M1 design-review themes).
Window {
    id: g
    required property var controller

    readonly property var args: Qt.application.arguments
    function arg(name, dflt) { const i = args.indexOf(name); return i >= 0 && i + 1 < args.length ? args[i + 1] : dflt }
    readonly property string outputName: arg("--output", Qt.application.screens[0].name)
    readonly property string outDir: arg("--out", "/tmp/snip-gallery")

    property var all: []
    property var queue: []
    property string current: ""

    width: 1600
    height: 1000
    visible: true
    color: "black"
    flags: Qt.FramelessWindowHint
    title: "snip-gallery"

    LayerShell.Window.screen: Platform.screen(outputName)
    LayerShell.Window.scope: "snip-gallery"
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorLeft
    LayerShell.Window.exclusionZone: -1
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone

    property var live: ({})    // the current theme as Engine resolved it (radius etc.)
    property bool started: false
    Component.onCompleted: all = JSON.parse(Engine.themes())

    function begin() {
        live = JSON.parse(Engine.theme || "{}")
        const want = arg("--themes", "")
        let names = want ? want.split(",") : []
        if (!names.length) {
            const cur = live.name
            const light = all.find(t => t.mode === "light")
            const grad = all.find(t => t.signature && t.signature.length > 1 && t.name !== cur)
            names = [cur, light ? light.name : "", grad ? grad.name : ""].filter(n => n)
        }
        queue = names
        Engine.log("gallery_start", "\"themes\":" + JSON.stringify(names) + ",\"out\":" + JSON.stringify(outDir))
        next.start()
    }
    // wait for the Engine's first theme push (it carries Hyprland's rounding)
    Timer { interval: 400; running: !g.started; onTriggered: { g.started = true; g.begin() } }

    Timer {
        id: next
        interval: 100
        onTriggered: {
            if (!g.queue.length) { Engine.log("gallery_done", ""); Qt.quit(); return }
            g.current = g.queue[0]
            g.queue = g.queue.slice(1)
            const t = Object.assign({}, g.all.find(x => x.name === g.current) || g.live)
            t.radius = g.live.radius           // corners follow this machine's Hyprland
            t.roundingPower = g.live.roundingPower
            if (t.name === g.live.name) t.wallpaper = g.live.wallpaper
            Engine.theme = JSON.stringify(t)
            settle.start()
        }
    }
    Timer {
        id: settle
        interval: 700
        onTriggered: scene.grabToImage(r => {
            const f = g.outDir + "/" + g.current + ".png"
            r.saveToFile(f)
            Engine.log("gallery_shot", "\"file\":" + JSON.stringify(f))
            next.start()
        }, Qt.size(scene.width, scene.height))
    }

    // A stand-in for the Quick Access window, for the real Card component.
    QtObject {
        id: host
        property bool dragging: false
        function act(a, c) {}
        function command(c) {}
        function drag(c) {}
        function dismiss(c, d) {}
    }

    Item {
        id: scene
        anchors.fill: parent

        // the "screen": the theme's wallpaper
        Image {
            id: wall
            anchors.fill: parent
            source: Theme.wallpaper ? "file://" + Theme.wallpaper : ""
            fillMode: Image.PreserveAspectCrop
            sourceSize.width: 1600
            asynchronous: false
        }
        Rectangle { anchors.fill: parent; color: Theme.background; visible: wall.status !== Image.Ready }

        // ------------------------------------------------ selection overlay
        Item {
            id: sel
            x: 0; y: 0; width: 1000; height: parent.height
            readonly property rect hole: Qt.rect(330, 250, 520, 300)
            Rectangle { color: Theme.scrim; x: 0; y: 0; width: parent.width; height: sel.hole.y }
            Rectangle { color: Theme.scrim; x: 0; y: sel.hole.y + sel.hole.height; width: parent.width; height: parent.height - y }
            Rectangle { color: Theme.scrim; x: 0; y: sel.hole.y; width: sel.hole.x; height: sel.hole.height }
            Rectangle { color: Theme.scrim; x: sel.hole.x + sel.hole.width; y: sel.hole.y; width: parent.width - x; height: sel.hole.height }
            Rectangle {
                x: sel.hole.x - 1; y: sel.hole.y - 1; width: sel.hole.width + 2; height: sel.hole.height + 2
                color: "transparent"; border.width: 1; border.color: Theme.accent
            }
            Rectangle {   // dimensions pill
                x: sel.hole.x + sel.hole.width - width; y: sel.hole.y + sel.hole.height + 8
                width: dims.implicitWidth + 16; height: 24
                radius: Math.min(Theme.radiusControl, 12)
                color: Theme.pillBg; border.width: 1; border.color: Theme.hairline
                Text { id: dims; anchors.centerIn: parent; text: "650 × 375"; color: Theme.text
                       font.family: Theme.fontUi; font.pixelSize: 12; font.weight: Font.Medium; font.features: { "tnum": 1 } }
            }
            // crosshair + magnifier at a second point
            readonly property point cur: Qt.point(220, 720)
            Rectangle { x: 0; y: sel.cur.y; width: parent.width; height: 1; color: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.85) }
            Rectangle { x: sel.cur.x; y: sel.hole.y + sel.hole.height + 40; width: 1; height: parent.height - y; color: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.85) }
            Item {
                id: mag
                x: sel.cur.x + 24; y: sel.cur.y + 24
                width: 135; height: 135 + 32
                RectangularShadow { anchors.fill: lens; radius: lens.radius; offset.y: 6; blur: 24; color: Theme.shadow }
                Rectangle {
                    id: lens
                    width: 135; height: 135
                    readonly property real r2: Math.min(Theme.radius, 12)
                    radius: r2
                    color: "transparent"
                    Item {
                        anchors.fill: parent
                        layer.enabled: lens.r2 > 0
                        layer.effect: MultiEffect { maskEnabled: true; maskSource: lmask; maskThresholdMin: 0.5; maskSpreadAtMin: 1.0 }
                        Image {   // 15x15 source pixels shown 9x
                            anchors.fill: parent
                            source: wall.source
                            sourceClipRect: Qt.rect(1200, 700, 15, 15)
                            smooth: false
                        }
                        Repeater { model: 14; Rectangle { x: (index + 1) * 9; width: 1; height: 135; color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.15) } }
                        Repeater { model: 14; Rectangle { y: (index + 1) * 9; height: 1; width: 135; color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.15) } }
                        Rectangle { x: 62; y: 62; width: 11; height: 11; color: "transparent"; border.width: 1; border.color: "black"
                                    Rectangle { anchors.fill: parent; anchors.margins: 1; color: "transparent"; border.width: 1; border.color: "white" } }
                    }
                    Item { id: lmask; anchors.fill: parent; visible: false; layer.enabled: true
                           Rectangle { anchors.fill: parent; radius: lens.r2; color: "black" } }
                    Rectangle { anchors.fill: parent; radius: lens.r2; color: "transparent"; border.width: 1; border.color: Theme.hairline }
                }
                Rectangle {
                    anchors.top: lens.bottom; anchors.topMargin: 8; anchors.horizontalCenter: lens.horizontalCenter
                    height: 24; width: hexRow.implicitWidth + 16
                    radius: Math.min(Theme.radiusControl, 12)
                    color: Theme.pillBg; border.width: 1; border.color: Theme.hairline
                    Row { id: hexRow; anchors.centerIn: parent; spacing: 6
                        Rectangle { width: 12; height: 12; radius: 3; color: Theme.accent; anchors.verticalCenter: parent.verticalCenter }
                        Text { text: Platform.hex(Theme.accent); color: Theme.text; font.family: Theme.fontMono; font.pixelSize: 12; anchors.verticalCenter: parent.verticalCenter }
                    }
                }
            }
            // window-mode label and mode hint
            Rectangle {
                anchors.horizontalCenter: parent.horizontalCenter; y: 24
                height: 32; width: hintRow.implicitWidth + 28
                radius: Math.min(Theme.radiusControl, 16)
                color: Theme.pillBg; border.width: 1; border.color: Theme.hairline
                Row { id: hintRow; anchors.centerIn: parent; spacing: 8
                    Icon { name: "window"; size: 15; stroke: 2; color: Theme.accent; anchors.verticalCenter: parent.verticalCenter }
                    Text { text: "Click a window"; color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 13; font.weight: Font.DemiBold; anchors.verticalCenter: parent.verticalCenter }
                    Text { text: "Space for area · Esc to cancel"; color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 12; anchors.verticalCenter: parent.verticalCenter }
                }
            }
        }

        // ------------------------------------------------ Quick Access stack
        Repeater {
            // CleanShot order: the newest on top (accent ring), oldest in the corner
            model: [
                { y: 380, saved: false, hover: true, newest: true, status: "" },
                { y: 593, saved: true, hover: false, newest: false, status: "" },
                { y: 806, saved: false, hover: false, newest: false, status: "copied" }
            ]
            delegate: Card {
                required property var modelData
                x: 1060
                y: modelData.y
                cardWidth: 280
                host: host
                newest: modelData.newest
                forceHover: modelData.hover
                Component.onCompleted: if (modelData.status) flash(modelData.status, 0)
                data_: ({ id: "g" + modelData.y, path: Theme.wallpaper, width: 1600, height: modelData.y === 420 ? 900 : 1000,
                          saved: modelData.saved ? "/tmp/x.png" : null, output: "" })
            }
        }

        CardMenu {
            x: 1356; y: 560
            card: ({ saved: true })
        }

        // toast
        Item {
            anchors.horizontalCenter: sel.horizontalCenter
            y: parent.height - 70
            width: toastPill.width; height: toastPill.height
            RectangularShadow { anchors.fill: toastPill; radius: toastPill.radius; offset.y: 8; blur: 28; color: Theme.shadow }
            Rectangle {
                id: toastPill
                height: 38; width: toastText.implicitWidth + 36
                radius: Math.min(Theme.radius, 19)
                color: Theme.surface; border.width: 1; border.color: Theme.hairline
                Text { id: toastText; anchors.centerIn: parent; text: "Copied 42 characters"; color: Theme.text
                       font.family: Theme.fontUi; font.pixelSize: 13; font.weight: Font.Medium }
            }
        }

        // caption
        Rectangle {
            x: 0; y: 0; width: caption.implicitWidth + 24; height: 28
            color: Qt.rgba(0, 0, 0, 0.55)
            Text { id: caption; anchors.centerIn: parent; color: "white"; font.pixelSize: 12
                   text: Theme.name + " · " + (Theme.light ? "light" : "dark") + " · radius " + Theme.radius
                         + (Theme.signature.length > 1 ? " · gradient border" : "") }
        }
    }
}
