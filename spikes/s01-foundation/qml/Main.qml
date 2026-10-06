import QtQuick
import QtQuick.Window
import org.kde.layershell as LayerShell
import com.snip.spike
import SnipCpp

// One fullscreen overlay layer surface per output, created up front and kept
// unmapped until frames are ready ("warm" path).
QtObject {
    id: root

    property string mode: "interactive"   // interactive | bench | selftest
    property bool overlayVisible: false
    property string cardPath: ""
    property string dragVariant: "uri"
    property string lastDragResult: ""
    property string pinPath: ""
    property bool pinLocked: false
    property string pinLayerPos: ""   // "x y" logical: locked pin rendered as a click-through layer surface
    property int presentedCount: 0
    readonly property bool plain: mode === "selftest"

    function open() {
        presentedCount = 0
        overlayVisible = true
        SnipCore.setOverlayVisible(true)
        SnipCore.log("qml_show", "")
    }

    function close() {
        overlayVisible = false
        SnipCore.setOverlayVisible(false)
        SnipCore.log("qml_hide", "")
    }

    function onWindowPresented() {
        presentedCount += 1
        if (presentedCount === Qt.application.screens.length) {
            SnipCore.log("all_presented", "")
            if (mode !== "interactive")
                settleTimer.restart()
        }
    }

    property Connections coreConn: Connections {
        target: SnipCore
        function onFramesReady(epoch) { root.open() }
        function onCancelRequested() { root.close() }
        function onCardShow(path) { root.cardPath = ""; root.cardPath = path; SnipCore.log("card_shown", "") }
        function onCardVariant(v) { root.dragVariant = v }
        function onPinCommand(cmd, arg) {
            if (cmd === "show") { root.pinPath = arg; root.pinLocked = false }
            else if (cmd === "pinlock") { root.pinLocked = (arg === "on"); root.pinLayerPos = "" }
            else if (cmd === "pinlock-layer") { root.pinLayerPos = arg; root.pinLocked = true }
            else if (cmd === "unpin") root.pinPath = ""
            else if (cmd === "export" && root.editorWin) root.editorWin.exportPng(arg)
            SnipCore.log("pin_cmd", "\"cmd\":\"" + cmd + "\",\"arg\":\"" + arg + "\"")
        }
        function onVerifyDone(report) {
            SnipCore.log("verify_report", "\"report\":" + report)
            root.close()
            if (root.mode === "selftest")
                copyTimer.start()
        }
    }

    property Timer startTimer: Timer {
        interval: 600
        onTriggered: SnipCore.trigger()
    }
    property Timer settleTimer: Timer {
        interval: root.mode === "selftest" ? 250 : 120
        onTriggered: root.mode === "selftest" ? SnipCore.verify() : root.close()
    }
    property Timer copyTimer: Timer {
        interval: 100
        onTriggered: {
            // Copy a 1600x900 physical region, then stay alive so the clipboard can be read.
            SnipCore.copyRegion(Qt.application.screens[0].name, 120, 80, 1600, 900)
            quitTimer.start()
        }
    }
    property Timer quitTimer: Timer {
        interval: 3000
        onTriggered: Qt.quit()
    }

    Component.onCompleted: {
        SnipCore.start(mode)
        if (mode === "selftest")
            startTimer.start()
        if (mode === "editor")
            createEditor()
    }

    property Instantiator windows: Instantiator {
        model: Qt.application.screens

        delegate: Window {
            id: win
            required property var modelData

            screen: modelData
            visible: root.overlayVisible
            color: "black"
            flags: Qt.FramelessWindowHint

            // LayerShellQt >= 6.6: the output must be set here; QWindow::screen alone
            // is not reliably honoured (spike S01: both overlays landed on DP-2).
            LayerShell.Window.screen: ScreenLookup.byName(win.modelData.name)
            LayerShell.Window.scope: "snip-select"
            LayerShell.Window.layer: LayerShell.Window.LayerOverlay
            LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
                                       | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
            LayerShell.Window.exclusionZone: -1
            LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityExclusive

            FrozenFrame {
                id: frame
                anchors.fill: parent
                output: win.modelData.name
                epoch: SnipCore.epoch
                onPresented: root.onWindowPresented()
            }

            Item {
                id: sel
                anchors.fill: parent
                focus: true
                visible: !root.plain

                property point p0
                property rect r: Qt.rect(0, 0, 0, 0)
                readonly property rect phys: frame.toPhysical(r)

                Keys.onEscapePressed: root.close()

                // Dim outside the selection (4 quads; the frozen frame stays untouched inside).
                Rectangle { color: "#73000000"; x: 0; y: 0; width: parent.width; height: sel.r.y }
                Rectangle { color: "#73000000"; x: 0; y: sel.r.y + sel.r.height; width: parent.width; height: parent.height - y }
                Rectangle { color: "#73000000"; x: 0; y: sel.r.y; width: sel.r.x; height: sel.r.height }
                Rectangle { color: "#73000000"; x: sel.r.x + sel.r.width; y: sel.r.y; width: parent.width - x; height: sel.r.height }

                Rectangle {
                    visible: sel.r.width > 0
                    x: sel.r.x - border.width; y: sel.r.y - border.width
                    width: sel.r.width + 2 * border.width; height: sel.r.height + 2 * border.width
                    color: "transparent"
                    border.color: "white"
                    border.width: 1 / frame.windowDpr   // one physical pixel, drawn outside the selection
                }

                Rectangle {
                    visible: sel.r.width > 0
                    color: "#d0101010"
                    radius: 5
                    x: Math.min(sel.r.x + sel.r.width + 8, parent.width - width - 4)
                    y: Math.min(sel.r.y + sel.r.height + 8, parent.height - height - 4)
                    width: label.implicitWidth + 12; height: label.implicitHeight + 6
                    Text {
                        id: label
                        anchors.centerIn: parent
                        color: "white"
                        font.pixelSize: 12
                        font.features: { "tnum": 1 }
                        text: sel.phys.width + " × " + sel.phys.height
                    }
                }

                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.CrossCursor
                    onPressed: (m) => { sel.p0 = Qt.point(m.x, m.y); sel.r = Qt.rect(m.x, m.y, 0, 0) }
                    onPositionChanged: (m) => {
                        sel.r = Qt.rect(Math.min(sel.p0.x, m.x), Math.min(sel.p0.y, m.y),
                                        Math.abs(m.x - sel.p0.x), Math.abs(m.y - sel.p0.y))
                    }
                    onReleased: {
                        let p = sel.phys
                        if (p.width < 4 || p.height < 4)   // click: whole output (spike stand-in for window snap)
                            p = Qt.rect(0, 0, frame.frameSize.width, frame.frameSize.height)
                        SnipCore.captureRegionToCard(win.modelData.name, p.x, p.y, p.width, p.height)
                        sel.r = Qt.rect(0, 0, 0, 0)
                        root.close()
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- QAO card (S3)
    property Window card: Window {
        id: card
        readonly property int pad: 24
        readonly property int cardW: 300
        readonly property real aspect: thumb.status === Image.Ready ? thumb.implicitHeight / Math.max(1, thumb.implicitWidth) : 0.5625
        width: cardW + 2 * pad
        height: Math.round(cardW * aspect) + 2 * pad
        visible: root.cardPath !== ""
        color: "transparent"
        flags: Qt.FramelessWindowHint
        screen: Qt.application.screens[0]

        LayerShell.Window.screen: ScreenLookup.byName(Qt.application.screens[0].name)
        LayerShell.Window.scope: "snip-qao"
        LayerShell.Window.layer: LayerShell.Window.LayerOverlay
        LayerShell.Window.anchors: LayerShell.Window.AnchorBottom | LayerShell.Window.AnchorLeft
        LayerShell.Window.exclusionZone: 0
        LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone

        Rectangle {  // soft shadow
            x: card.pad - 2; y: card.pad + 6
            width: cardBody.width + 4; height: cardBody.height + 4
            radius: 12; color: "#59000000"
        }
        Rectangle {
            id: cardBody
            x: card.pad; y: card.pad
            width: card.cardW; height: card.height - 2 * card.pad
            radius: 10
            color: "#1d1d22"
            border.color: "#33ffffff"; border.width: 1
            clip: true

            Image {
                id: thumb
                anchors.fill: parent
                anchors.margins: 1
                source: root.cardPath ? "file://" + root.cardPath : ""
                fillMode: Image.PreserveAspectCrop
                cache: false
                asynchronous: false
                sourceSize.width: 600
            }

            Rectangle {  // hover scrim
                anchors.fill: parent
                color: "#73000000"
                opacity: hover.containsMouse ? 1 : 0
                Behavior on opacity { NumberAnimation { duration: 120 } }
            }

            // CleanShot-style auto-close: 10 s, paused while hovered or dragging;
            // the capture stays in history (lifecycle §5.0), so closing loses nothing.
            Timer {
                id: autoClose
                interval: 10000
                running: root.cardPath !== "" && !hover.containsMouse && !hover.dragging
                onTriggered: root.cardPath = ""
            }

            MouseArea {
                id: hover
                anchors.fill: parent
                hoverEnabled: true
                property point p0
                property bool dragging: false
                onPressed: (m) => { p0 = Qt.point(m.x, m.y) }
                onPositionChanged: (m) => {
                    if (pressed && !dragging && Math.hypot(m.x - p0.x, m.y - p0.y) > 6) {
                        dragging = true
                        root.lastDragResult = DragHelper.startDrag(cardBody, root.cardPath, root.dragVariant)
                        dragging = false
                    }
                }
            }

            Row {
                anchors.centerIn: parent
                spacing: 8
                opacity: hover.containsMouse ? 1 : 0
                Repeater {
                    model: [ { label: "Copy", act: "copy" }, { label: "Save", act: "save" } ]
                    delegate: Rectangle {
                        required property var modelData
                        width: 72; height: 30; radius: 15
                        color: pill.containsMouse ? "#ffffff" : "#e6ffffff"
                        Text { anchors.centerIn: parent; text: modelData.label; color: "#111"; font.pixelSize: 13; font.weight: Font.DemiBold }
                        MouseArea {
                            id: pill; anchors.fill: parent; hoverEnabled: true
                            onClicked: {
                                if (modelData.act === "copy") SnipCore.copyFile(root.cardPath)
                                SnipCore.log("card_action", "\"action\":\"" + modelData.act + "\"")
                            }
                        }
                    }
                }
            }

            Rectangle {  // close (top-left, CleanShot style)
                x: 8; y: 8; width: 22; height: 22; radius: 11
                color: "#e6ffffff"; opacity: hover.containsMouse ? 1 : 0
                Text { anchors.centerIn: parent; text: "✕"; font.pixelSize: 11; color: "#111" }
                MouseArea { anchors.fill: parent; onClicked: root.cardPath = "" }
            }
        }
    }

    // ---------------------------------------------------------------- S17 theme gallery
    property Window gallery: Window {
        id: gal
        visible: root.mode === "gallery" && themeList.length > 0
        color: "#0b0b0d"
        flags: Qt.FramelessWindowHint
        screen: Qt.application.screens[0]
        LayerShell.Window.screen: ScreenLookup.byName(Qt.application.screens[0].name)
        LayerShell.Window.scope: "snip-gallery"
        LayerShell.Window.layer: LayerShell.Window.LayerOverlay
        LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
                                   | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
        LayerShell.Window.exclusionZone: -1
        LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone

        property var themeList: root.mode === "gallery" ? JSON.parse(SnipCore.themes()) : []
        readonly property int cols: 7
        Component.onCompleted: SnipCore.log("gallery_init", "\"mode\":\"" + root.mode + "\",\"themes\":" + themeList.length + ",\"visible\":" + visible)
        onVisibleChanged: SnipCore.log("gallery_visible", "\"visible\":" + visible)
        Timer { running: gal.visible; interval: 2500; onTriggered: SnipCore.log("gallery_ready", "\"themes\":" + gal.themeList.length) }
        Timer { running: gal.visible; interval: 9000; onTriggered: Qt.quit() }

        Grid {
            anchors.fill: parent; anchors.margins: 10
            columns: gal.cols; spacing: 8
            Repeater {
                model: gal.themeList
                delegate: Item {
                    id: tile
                    required property var modelData
                    readonly property var t: modelData
                    width: (gal.width - 20 - (gal.cols - 1) * 8) / gal.cols
                    height: (gal.height - 20 - 3 * 8) / 4
                    clip: true

                    Rectangle { anchors.fill: parent; color: tile.t.background }
                    Image {
                        anchors.fill: parent
                        source: tile.t.wallpaper ? "file://" + tile.t.wallpaper : ""
                        fillMode: Image.PreserveAspectCrop
                        sourceSize.width: 600; asynchronous: true
                    }
                    // selection with scrim outside
                    readonly property rect sel: Qt.rect(width * 0.36, height * 0.30, width * 0.40, height * 0.34)
                    Rectangle { color: tile.t.scrim; x: 0; y: 0; width: parent.width; height: tile.sel.y }
                    Rectangle { color: tile.t.scrim; x: 0; y: tile.sel.y + tile.sel.height; width: parent.width; height: parent.height - y }
                    Rectangle { color: tile.t.scrim; x: 0; y: tile.sel.y; width: tile.sel.x; height: tile.sel.height }
                    Rectangle { color: tile.t.scrim; x: tile.sel.x + tile.sel.width; y: tile.sel.y; width: parent.width - x; height: tile.sel.height }
                    Rectangle { x: tile.sel.x - 1; y: tile.sel.y - 1; width: tile.sel.width + 2; height: tile.sel.height + 2
                                color: "transparent"; border.color: tile.t.accent; border.width: 1 }
                    Rectangle {  // dimension pill
                        x: tile.sel.x + tile.sel.width - width; y: tile.sel.y + tile.sel.height + 5
                        width: dim.implicitWidth + 12; height: 18; radius: 9; color: tile.t.surface
                        Text { id: dim; anchors.centerIn: parent; text: "1600 × 900"; color: tile.t.text; font.pixelSize: 10; font.features: { "tnum": 1 } }
                    }
                    // toolbar
                    Rectangle {
                        id: bar
                        x: (parent.width - width) / 2; y: 8; height: 26; radius: 8
                        width: tools.implicitWidth + 12; color: tile.t.surface
                        Row {
                            id: tools; anchors.centerIn: parent; spacing: 2
                            Repeater {
                                model: ["↖", "▭", "◯", "╱", "➚", "T", "▦", "①", "✎"]
                                delegate: Rectangle {
                                    required property string modelData
                                    required property int index
                                    width: 20; height: 20; radius: 5
                                    color: index === 4 ? tile.t.accent : "transparent"
                                    Text { anchors.centerIn: parent; text: modelData; font.pixelSize: 11
                                           color: index === 4 ? tile.t.onAccent : tile.t.text }
                                }
                            }
                        }
                    }
                    Rectangle {  // signature gradient
                        anchors.top: bar.bottom; anchors.topMargin: 3; anchors.horizontalCenter: bar.horizontalCenter
                        width: bar.width - 16; height: 2; radius: 1
                        gradient: Gradient {
                            orientation: Gradient.Horizontal
                            GradientStop { position: 0; color: tile.t.signature[0] }
                            GradientStop { position: 1; color: tile.t.signature[tile.t.signature.length - 1] }
                        }
                    }
                    // recording pill
                    Rectangle {
                        anchors.right: parent.right; anchors.top: parent.top; anchors.margins: 8
                        width: 66; height: 22; radius: 11; color: tile.t.surface
                        Row { anchors.centerIn: parent; spacing: 5
                            Rectangle { width: 9; height: 9; radius: 5; color: tile.t.danger; anchors.verticalCenter: parent.verticalCenter }
                            Text { text: "00:12"; color: tile.t.text; font.pixelSize: 10; font.features: { "tnum": 1 } }
                        }
                    }
                    // QAO card
                    Rectangle {
                        x: 8; y: parent.height - height - 30
                        width: parent.width * 0.38; height: width * 0.5625 + 0; radius: 7
                        color: tile.t.surface; border.color: Qt.rgba(1, 1, 1, tile.t.mode === "dark" ? 0.12 : 0.0); border.width: 1
                        clip: true
                        Image { anchors.fill: parent; anchors.margins: 1; source: "file:///home/tgomareli/Development/OpenShotX/spikes/s01-foundation/qao-test/test.png"
                                fillMode: Image.PreserveAspectCrop; sourceSize.width: 300; opacity: 0.85 }
                        Row { anchors.centerIn: parent; spacing: 4
                            Rectangle { width: 40; height: 18; radius: 9; color: tile.t.accent
                                        Text { anchors.centerIn: parent; text: "Copy"; color: tile.t.onAccent; font.pixelSize: 9; font.weight: Font.DemiBold } }
                            Rectangle { width: 40; height: 18; radius: 9; color: tile.t.surface
                                        Text { anchors.centerIn: parent; text: "Save"; color: tile.t.text; font.pixelSize: 9; font.weight: Font.DemiBold } }
                        }
                    }
                    // label
                    Rectangle {
                        anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: parent.bottom
                        height: 22; color: tile.t.surface
                        Text {
                            anchors.verticalCenter: parent.verticalCenter; anchors.left: parent.left; anchors.leftMargin: 6
                            text: tile.t.name + "  ·  " + tile.t.mode
                                  + (tile.t.report.text_adjusted ? "  ·  text adj" : "")
                                  + (tile.t.report.accent_adjusted ? "  ·  accent adj" : "")
                                  + (tile.t.report.danger_fallback ? "  ·  danger fb" : "")
                            color: tile.t.text; font.pixelSize: 10
                        }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter; anchors.right: parent.right; anchors.rightMargin: 6
                            text: "Aa " + tile.t.report.text_contrast.toFixed(1) + ":1"; color: tile.t.textSecondary; font.pixelSize: 10
                        }
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- S15 hide-desktop cover
    // Covers the bar with the matching wallpaper slice, rendered exactly like
    // omarchy-shell's background (same QML Image, same fill, same item size),
    // so the bar keeps its reserved space and windows never re-layout.
    property Window cover: Window {
        id: cov
        readonly property int barHeight: Number(Qt.application.arguments[Qt.application.arguments.indexOf("--cover") + 1]) || 35
        visible: root.mode === "cover" && wp.status === Image.Ready
        height: barHeight
        color: "black"
        flags: Qt.FramelessWindowHint
        LayerShell.Window.screen: ScreenLookup.byName(Qt.application.screens[0].name)
        LayerShell.Window.scope: "snip-cover"
        LayerShell.Window.layer: LayerShell.Window.LayerOverlay
        LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
        LayerShell.Window.exclusionZone: -1
        LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
        onVisibleChanged: if (visible) coverReady.start()
        Timer { id: coverReady; interval: 400; onTriggered: SnipCore.log("cover_ready", "\"bar_h\":" + cov.barHeight + ",\"w\":" + cov.width) }
        Item {
            width: Qt.application.screens[0].width      // full output, logical
            height: Qt.application.screens[0].height
            Image {
                id: wp
                anchors.fill: parent
                source: root.mode === "cover" ? "file://" + SnipCore.wallpaperPath() : ""
                fillMode: Image.PreserveAspectCrop
                asynchronous: false
                cache: true
                mipmap: Qt.application.arguments.indexOf("--cover-mipmap") >= 0
                smooth: Qt.application.arguments.indexOf("--cover-nosmooth") < 0
            }
        }
    }

    // ---------------------------------------------------------------- S6 pin (xdg toplevel)
    property Window pin: Window {
        id: pinWin
        title: "snip-pin"
        visible: root.pinPath !== "" && root.pinLayerPos === ""
        width: 480; height: 270
        color: "transparent"
        flags: Qt.FramelessWindowHint
        // (no input-region call at show: start with Qt's default full input)
        Connections {
            target: root
            function onPinLockedChanged() { PinHelper.setClickThrough(pinWin, root.pinLocked); SnipCore.log("pin_lock", "\"locked\":" + root.pinLocked) }
        }
        Rectangle {
            anchors.fill: parent; radius: 8; color: "#1d1d22"; clip: true
            opacity: root.pinLocked ? 0.75 : 1.0
            Image { anchors.fill: parent; source: root.pinPath ? "file://" + root.pinPath : ""; fillMode: Image.PreserveAspectFit }
        }
        MouseArea {
            anchors.fill: parent
            hoverEnabled: true
            onEntered: SnipCore.log("pin_entered", "")
            onPressed: SnipCore.log("pin_pressed", "")
            onReleased: SnipCore.log("pin_released", "")
            onClicked: (m) => SnipCore.log("pin_clicked", "\"x\":" + Math.round(m.x) + ",\"y\":" + Math.round(m.y))
        }
    }

    // Locked pin as a layer surface: Hyprland honours empty input regions on
    // layer surfaces (not on xdg toplevels, S6), so clicks pass through.
    property Window pinLayer: Window {
        id: pinLay
        visible: root.pinPath !== "" && root.pinLayerPos !== ""
        width: 480; height: 270
        color: "transparent"
        flags: Qt.FramelessWindowHint
        LayerShell.Window.screen: ScreenLookup.byName(Qt.application.screens[0].name)
        LayerShell.Window.scope: "snip-pin-locked"
        LayerShell.Window.layer: LayerShell.Window.LayerOverlay
        LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorLeft
        LayerShell.Window.exclusionZone: -1
        LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
        onVisibleChanged: {
            if (!visible) return
            const p = root.pinLayerPos.split(" ")
            PinHelper.placeLayer(pinLay, Number(p[0]), Number(p[1]))
            applyClickThrough.start()   // the wl_surface exists only after the show completes
            SnipCore.log("pin_layer_shown", "\"at\":\"" + root.pinLayerPos + "\"")
        }
        Timer { id: applyClickThrough; interval: 30; onTriggered: PinHelper.setClickThrough(pinLay, true) }
        Rectangle {
            anchors.fill: parent; radius: 8; color: "#1d1d22"; clip: true; opacity: 0.75
            Image { anchors.fill: parent; source: root.pinPath ? "file://" + root.pinPath : ""; fillMode: Image.PreserveAspectFit }
        }
        MouseArea { anchors.fill: parent; onClicked: SnipCore.log("pin_clicked", "\"layer\":true") }
    }

    // ---------------------------------------------------------------- S8 editor canvas
    // (created from its URL: the cxx-qt qmldir maps Editor.qml to a wrong qrc path)
    property var editorWin: null
    function createEditor() {
        const c = Qt.createComponent("qrc:/qt/qml/com/snip/spike/qml/Editor.qml")
        if (c.status !== Component.Ready) { SnipCore.log("editor_error", "\"msg\":\"" + c.errorString().replace(/"/g, "'").replace(/\n/g, " ") + "\""); return }
        const args = Qt.application.arguments
        const n = args.indexOf("--objects")
        editorWin = c.createObject(null, {
            extraObjects: n >= 0 ? Number(args[n + 1]) : 0,
            animate: args.indexOf("--animate") >= 0,
            imagePath: "/home/tgomareli/Development/OpenShotX/spikes/s01-foundation/qao-test/test.png"
        })
        editorWin.visible = true
        SnipCore.log("editor_loaded", "\"objects\":" + editorWin.extraObjects)
    }
}
