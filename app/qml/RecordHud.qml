import QtQuick
import QtQuick.Window
import QtQuick.Effects
import org.kde.layershell as LayerShell
import Snip.Core

// Screen recording (REC-01…05), CleanShot's: once the area is chosen, a frame
// around it, the screen dimmed outside it (REC-04), and the toolbar beside
// it: Video or GIF, the microphone, system audio, Record (Enter) and Cancel
// (Esc). Then a 3-2-1 countdown (REC-03), and while it records, the controls:
// stop with the time, pause, restart, discard (REC-05).
//
// gpu-screen-recorder records whatever is on screen in the area, Snip's own
// layers included, so this layer never draws inside it: the frame and the
// dimming keep a 2 px margin, and the countdown is gone before the first
// frame. When the whole monitor is recorded the controls would be in the
// video, so there are none: the Record key, `snip record stop` or the bar
// stops it. A full-output layer that takes input only on the toolbar or the
// controls.
Window {
    id: hud
    property var controller: null
    property string output: ""
    property rect area: Qt.rect(0, 0, 0, 0)       // logical px on the output
    property bool whole: false
    property string phase: "closed"                // ready | countdown | starting | recording | paused | finishing | closed
    property bool gif: false
    property var mics: []                           // [{id, label}]
    property string mic: ""                         // "" none, or a mic's id
    property bool system: false
    property bool dim: true
    property int seconds: 0                         // the countdown's
    property real elapsedBase: 0                    // ms recorded at `since`
    property real since: 0                          // Date.now() when elapsedBase was right
    property real elapsed: 0                        // ms, shown
    property bool micsOpen: false

    flags: Qt.FramelessWindowHint
    color: "transparent"
    visible: false
    LayerShell.Window.screen: Platform.screen(output !== "" ? output : Qt.application.screens[0].name)
    LayerShell.Window.scope: "snip-record"
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
                               | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
    LayerShell.Window.exclusionZone: -1
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone

    function open(s) {
        output = s.output
        area = Qt.rect(s.x, s.y, s.w, s.h)
        whole = !!s.whole
        gif = !!s.gif
        mics = s.mics || []
        mic = s.mic || ""
        system = !!s.system
        dim = s.dim !== false
        // `snip record area|fullscreen`: recording at once, no toolbar
        phase = s.now ? "starting" : "ready"
        micsOpen = false
        elapsed = 0
        visible = true
        Qt.callLater(fitInput)
    }
    function status(s) {
        if (s.state === "countdown") {
            seconds = s.seconds
            phase = "countdown"
            tick.restart()
            return
        }
        phase = s.state
        elapsedBase = s.elapsed || 0
        since = Date.now()
        elapsed = elapsedBase
        Qt.callLater(fitInput)
    }
    function close() {
        phase = "closed"
        visible = false
    }
    function record() {
        if (phase !== "ready") return
        Engine.setSetting("recording.kind", JSON.stringify(gif ? "gif" : "video"))
        Engine.recordStart(gif ? 1 : 0)
    }
    function setMic(id) {
        mic = id
        micsOpen = false
        Engine.setSetting("recording.mic", JSON.stringify(id))
        Qt.callLater(fitInput)
    }
    function setSystem(on) {
        system = on
        Engine.setSetting("recording.system_audio", JSON.stringify(on))
    }
    readonly property string micLabel: {
        if (mic === "") return "No microphone"
        const m = mics.find(x => x.id === mic)
        return m ? m.label : mic
    }
    function clock(ms) {
        const t = Math.floor(ms / 1000), m = Math.floor(t / 60), s = t % 60
        return m + ":" + String(s).padStart(2, "0")
    }

    // only the toolbar (or the controls) take the pointer
    function fitInput() {
        const r = []
        if (toolbar.visible) r.push(Qt.rect(toolbar.x, toolbar.y, toolbar.width, toolbar.height))
        if (micList.visible) r.push(Qt.rect(micList.x, micList.y, micList.width, micList.height))
        if (controls.visible) r.push(Qt.rect(controls.x, controls.y, controls.width, controls.height))
        Platform.setInputRects(hud, r)
    }
    onMicsOpenChanged: Qt.callLater(fitInput)

    // the countdown, one second a step; at zero it's gone before recording starts
    Timer {
        id: tick
        interval: 1000
        repeat: true
        onTriggered: {
            hud.seconds -= 1
            if (hud.seconds <= 0) {
                stop()
                hud.phase = "starting"
                Platform.notifyNextFrame(hud)
            }
        }
    }
    Connections {
        target: Platform
        function onFramePresented(w) { if (w === hud && hud.phase === "starting") Engine.recordGo() }
    }
    // the time recorded so far
    Timer {
        interval: 250
        repeat: true
        running: hud.phase === "recording"
        onTriggered: hud.elapsed = hud.elapsedBase + (Date.now() - hud.since)
    }

    // ---------------------------------------------------------------- outside the area
    readonly property real margin: 2
    readonly property rect hole: Qt.rect(area.x - margin, area.y - margin, area.width + 2 * margin, area.height + 2 * margin)
    readonly property bool showDim: dim && !whole && phase !== "closed"
    readonly property color shade: Qt.rgba(0, 0, 0, 0.42)
    Rectangle { visible: hud.showDim; x: 0; y: 0; width: hud.width; height: Math.max(0, hud.hole.y); color: hud.shade }
    Rectangle { visible: hud.showDim; x: 0; y: hud.hole.y + hud.hole.height; width: hud.width; height: Math.max(0, hud.height - y); color: hud.shade }
    Rectangle { visible: hud.showDim; x: 0; y: hud.hole.y; width: Math.max(0, hud.hole.x); height: hud.hole.height; color: hud.shade }
    Rectangle { visible: hud.showDim; x: hud.hole.x + hud.hole.width; y: hud.hole.y; width: Math.max(0, hud.width - x); height: hud.hole.height; color: hud.shade }
    // the frame, just outside the area (red while recording)
    Rectangle {
        visible: !hud.whole && hud.phase !== "closed"
        x: hud.hole.x - 2
        y: hud.hole.y - 2
        width: hud.hole.width + 4
        height: hud.hole.height + 4
        color: "transparent"
        radius: 3
        border.width: 2
        border.color: hud.phase === "recording" ? Theme.danger : hud.phase === "paused" ? Theme.warning : Theme.accent
    }

    // where a bar goes: under the area, else over it, else inside its bottom
    // (only before recording starts) or at the monitor's bottom
    function placeY(h) {
        const below = area.y + area.height + margin + 12
        if (below + h <= height - 8) return below
        const above = area.y - margin - 12 - h
        if (above >= 8) return above
        return height - h - 24
    }
    function placeX(w) { return Math.max(8, Math.min(width - w - 8, area.x + area.width / 2 - w / 2)) }

    // ---------------------------------------------------------------- the countdown (REC-03)
    Rectangle {
        visible: hud.phase === "countdown"
        width: 120
        height: 120
        radius: 60
        x: hud.area.x + hud.area.width / 2 - width / 2
        y: hud.area.y + hud.area.height / 2 - height / 2
        color: Qt.rgba(0, 0, 0, 0.55)
        Text {
            anchors.centerIn: parent
            text: hud.seconds
            color: "white"
            font.family: Theme.fontUi
            font.pixelSize: 64
            font.weight: Font.DemiBold
        }
    }

    // ---------------------------------------------------------------- the toolbar (REC-02)
    Rectangle {
        id: toolbar
        objectName: "record-toolbar"
        visible: hud.phase === "ready"
        width: tools.implicitWidth + 16
        height: 44
        radius: 22
        x: hud.placeX(width)
        y: hud.placeY(height)
        color: Theme.pillBg
        border.width: 1
        border.color: Theme.hairline
        onXChanged: hud.fitInput()
        onYChanged: hud.fitInput()
        onWidthChanged: hud.fitInput()
        onVisibleChanged: hud.fitInput()
        layer.enabled: true
        layer.effect: MultiEffect { shadowEnabled: true; shadowColor: Theme.shadow; shadowBlur: 0.6; shadowVerticalOffset: 6 }

        Row {
            id: tools
            anchors.centerIn: parent
            spacing: 6
            // Video | GIF
            Row {
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2
                Repeater {
                    model: [[false, "Video", "video"], [true, "GIF", "image"]]
                    Rectangle {
                        required property var modelData
                        readonly property bool on: hud.gif === modelData[0]
                        objectName: modelData[0] ? "record-kind-gif" : "record-kind-video"
                        width: kindRow.implicitWidth + 20
                        height: 30
                        radius: 15
                        color: on ? Theme.accent : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, kindHover.hovered ? 0.12 : 0.06)
                        Row {
                            id: kindRow
                            anchors.centerIn: parent
                            spacing: 6
                            Icon { anchors.verticalCenter: parent.verticalCenter; name: parent.parent.modelData[2]; size: 14; color: parent.parent.on ? Theme.onAccent : Theme.text }
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: parent.parent.modelData[1]
                                color: parent.parent.on ? Theme.onAccent : Theme.text
                                font.family: Theme.fontUi
                                font.pixelSize: 12
                                font.weight: Font.DemiBold
                            }
                        }
                        HoverHandler { id: kindHover; cursorShape: Qt.PointingHandCursor }
                        TapHandler { onTapped: hud.gif = parent.modelData[0] }
                    }
                }
            }
            Rectangle { width: 1; height: 22; anchors.verticalCenter: parent.verticalCenter; color: Theme.hairline }
            // the microphone (a GIF has no sound)
            Rectangle {
                objectName: "record-mic"
                anchors.verticalCenter: parent.verticalCenter
                width: micRow.implicitWidth + 20
                height: 30
                radius: 15
                opacity: hud.gif ? 0.4 : 1
                color: hud.mic !== "" && !hud.gif ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.22)
                                                  : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, micHover.hovered ? 0.12 : 0.06)
                Row {
                    id: micRow
                    anchors.centerIn: parent
                    spacing: 6
                    Icon { anchors.verticalCenter: parent.verticalCenter; name: "mic"; size: 14; color: Theme.text }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        width: Math.min(implicitWidth, 160)
                        elide: Text.ElideRight
                        text: hud.micLabel
                        color: Theme.text
                        font.family: Theme.fontUi
                        font.pixelSize: 12
                    }
                }
                HoverHandler { id: micHover; cursorShape: Qt.PointingHandCursor }
                TapHandler { enabled: !hud.gif; onTapped: hud.micsOpen = !hud.micsOpen }
            }
            // system audio
            Rectangle {
                objectName: "record-system"
                anchors.verticalCenter: parent.verticalCenter
                width: 30
                height: 30
                radius: 15
                opacity: hud.gif ? 0.4 : 1
                color: hud.system && !hud.gif ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.22)
                                              : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, sysHover.hovered ? 0.12 : 0.06)
                Icon { anchors.centerIn: parent; name: "volume"; size: 15; color: hud.system && !hud.gif ? Theme.accent : Theme.textSecondary }
                HoverHandler { id: sysHover; cursorShape: Qt.PointingHandCursor }
                TapHandler { enabled: !hud.gif; onTapped: hud.setSystem(!hud.system) }
                Tip { text: hud.system ? "Recording what the computer plays" : "Record what the computer plays" }
            }
            Rectangle { width: 1; height: 22; anchors.verticalCenter: parent.verticalCenter; color: Theme.hairline }
            TextButton { objectName: "record-cancel"; anchors.verticalCenter: parent.verticalCenter; text: "Cancel"; tip: "Esc"; onClicked: Engine.recordCancel() }
            TextButton {
                objectName: "record-go"
                anchors.verticalCenter: parent.verticalCenter
                primary: true
                text: hud.gif ? "Record GIF" : "Record"
                tip: "Enter"
                onClicked: hud.record()
            }
        }
    }
    // the microphones, over (or under) the toolbar
    Rectangle {
        id: micList
        objectName: "record-mics"
        visible: hud.micsOpen && hud.phase === "ready"
        width: 280
        height: micCol.implicitHeight + 12
        radius: Math.min(Theme.radius, 12)
        x: Math.max(8, Math.min(hud.width - width - 8, toolbar.x))
        y: toolbar.y - height - 8 >= 8 ? toolbar.y - height - 8 : toolbar.y + toolbar.height + 8
        color: Qt.rgba(Theme.surface.r, Theme.surface.g, Theme.surface.b, 0.98)
        border.width: 1
        border.color: Theme.hairline
        onVisibleChanged: hud.fitInput()
        onHeightChanged: hud.fitInput()
        Column {
            id: micCol
            x: 6
            y: 6
            width: parent.width - 12
            Repeater {
                model: [{ id: "", label: "No microphone" }].concat(hud.mics)
                Rectangle {
                    required property var modelData
                    required property int index
                    objectName: "record-mic:" + index
                    width: micCol.width
                    height: 30
                    radius: 6
                    color: micItemHover.hovered ? Theme.controlHover : "transparent"
                    Text {
                        x: 10
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - 36
                        elide: Text.ElideRight
                        text: parent.modelData.label
                        color: Theme.text
                        font.family: Theme.fontUi
                        font.pixelSize: 13
                    }
                    Icon { visible: hud.mic === parent.modelData.id; anchors.right: parent.right; anchors.rightMargin: 10; anchors.verticalCenter: parent.verticalCenter; name: "check"; size: 13; color: Theme.accent }
                    HoverHandler { id: micItemHover; cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: hud.setMic(parent.modelData.id) }
                }
            }
        }
    }

    // ---------------------------------------------------------------- the controls (REC-05)
    Rectangle {
        id: controls
        objectName: "record-controls"
        visible: !hud.whole && (hud.phase === "recording" || hud.phase === "paused")
        width: ctl.implicitWidth + 16
        height: 40
        radius: 20
        x: hud.placeX(width)
        y: hud.placeY(height)
        color: Theme.pillBg
        border.width: 1
        border.color: Theme.hairline
        onXChanged: hud.fitInput()
        onYChanged: hud.fitInput()
        onWidthChanged: hud.fitInput()
        onVisibleChanged: hud.fitInput()
        layer.enabled: true
        layer.effect: MultiEffect { shadowEnabled: true; shadowColor: Theme.shadow; shadowBlur: 0.6; shadowVerticalOffset: 6 }
        Row {
            id: ctl
            anchors.centerIn: parent
            spacing: 4
            // stop, with the time recorded
            Rectangle {
                objectName: "record-stop"
                anchors.verticalCenter: parent.verticalCenter
                width: stopRow.implicitWidth + 20
                height: 28
                radius: 14
                color: stopHover.hovered ? Qt.lighter(Theme.danger, 1.1) : Theme.danger
                Row {
                    id: stopRow
                    anchors.centerIn: parent
                    spacing: 6
                    Rectangle { anchors.verticalCenter: parent.verticalCenter; width: 10; height: 10; radius: 2; color: "white" }
                    Text {
                        objectName: "record-time"
                        anchors.verticalCenter: parent.verticalCenter
                        text: hud.clock(hud.elapsed)
                        color: "white"
                        font.family: Theme.fontUi
                        font.pixelSize: 13
                        font.weight: Font.DemiBold
                        font.features: { "tnum": 1 }
                    }
                }
                HoverHandler { id: stopHover; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: Engine.recordStop() }
            }
            IconButton { objectName: "record-pause"; anchors.verticalCenter: parent.verticalCenter; icon: hud.phase === "paused" ? "play" : "pause"; tip: hud.phase === "paused" ? "Resume" : "Pause"; onClicked: Engine.recordPause(hud.phase !== "paused") }
            IconButton { objectName: "record-restart"; anchors.verticalCenter: parent.verticalCenter; icon: "rotate-ccw"; tip: "Restart"; onClicked: Engine.recordRestart() }
            IconButton { objectName: "record-discard"; anchors.verticalCenter: parent.verticalCenter; icon: "trash"; tip: "Discard"; onClicked: Engine.recordDiscard() }
        }
    }

    // tests: where a control is (output logical px), and how things stand
    function find(item, name) {
        if (item.objectName === name && item.visible) return item
        for (const c of item.children) {
            if (!c.visible) continue
            const f = find(c, name)
            if (f) return f
        }
        return null
    }
    function debug(a) {
        switch (a[0]) {
        case "where": {
            const it = find(contentItem, a[1])
            // laid out first: just after a change, rows and columns still hold old places
            for (let q = it; q; q = q.parent) if (q.forceLayout) q.forceLayout()
            const p = it ? it.mapToItem(contentItem, it.width / 2, it.height / 2) : null
            Engine.log("record_where", JSON.stringify({ name: a[1], x: p ? p.x : -1, y: p ? p.y : -1 }).slice(1, -1))
            break
        }
        case "state":
            Engine.log("record_hud", JSON.stringify({ open: visible, phase: phase, gif: gif, mic: mic, system: system, whole: whole,
                                                    time: hud.clock(elapsed), area: [area.x, area.y, area.width, area.height],
                                                    toolbar: toolbar.visible ? [toolbar.x, toolbar.y, toolbar.width, toolbar.height] : null,
                                                    controls: controls.visible ? [controls.x, controls.y, controls.width, controls.height] : null }).slice(1, -1))
            break
        }
    }
}
