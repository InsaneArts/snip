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
                        SnipCore.copyRegion(win.modelData.name, p.x, p.y, p.width, p.height)
                        sel.r = Qt.rect(0, 0, 0, 0)
                        root.close()
                    }
                }
            }
        }
    }
}
