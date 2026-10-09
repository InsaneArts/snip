import QtQuick
import QtQuick.Window
import QtMultimedia
import Snip.Core

// The video editor (VED-01…03), CleanShot's trimmer before Studio Mode: the
// recording plays above a timeline of its frames. Drag the ends to trim; B
// splits at the playhead; Delete cuts the part that's selected (again:
// brings it back). Each audio track gets a volume (0 mutes it; several are
// mixed into one), and mono. Export as MP4 or GIF, at a quality and size,
// with an estimate of the file's size: Replace the recording, or Save as New.
// Playback skips what's cut, so you see the result.
Window {
    id: ve
    property var info: ({})                 // {id, path, duration, fps, width, height, tracks, bytes, title, output}
    property var thumbs: []                 // the timeline's frames: they come a moment after it opens
    property var controller: null
    signal finished()

    title: "Snip Video Editor — " + (info.title || "Recording")
    color: Theme.background
    width: 1100
    height: 760
    minimumWidth: 780
    minimumHeight: 540
    visible: true

    // ---------------------------------------------------------------- the edit
    readonly property real duration: Math.max(1, info.duration || 1)
    readonly property real frameMs: 1000 / Math.max(1, info.fps || 30)
    property var cuts: [0, duration]          // where the parts meet, sorted (ms)
    property var removed: ({})                // a part's start (ms, as text) → cut
    property real inMs: 0
    property real outMs: duration
    property int selected: -1                 // the part selected (an index into parts)
    property var volumes: []                  // per audio track, 0…2
    property bool mono: false
    property bool gif: false
    property int quality: 23                  // H.264 constant quality: lower is better
    property int maxHeight: 0                 // 0: as recorded
    property real progress: -1                // an export's, 0…1; -1: none
    property string error: ""

    Component.onCompleted: {
        const v = []
        for (let i = 0; i < (info.tracks || 0); i++) v.push(1)
        volumes = v
    }

    readonly property var parts: {
        const out = []
        for (let i = 0; i + 1 < cuts.length; i++)
            out.push({ a: cuts[i], b: cuts[i + 1], cut: !!removed[String(cuts[i])] })
        return out
    }
    // what's kept, in seconds: inside the trim, not cut
    readonly property var kept: {
        const out = []
        for (const p of parts) {
            if (p.cut) continue
            const a = Math.max(p.a, inMs), b = Math.min(p.b, outMs)
            if (b - a > 1) out.push([a / 1000, b / 1000])
        }
        return out
    }
    readonly property real keptMs: kept.reduce((s, k) => s + (k[1] - k[0]) * 1000, 0)

    function split(ms) {
        ms = Math.round(ms)
        if (ms <= inMs + frameMs || ms >= outMs - frameMs || cuts.includes(ms)) return
        cuts = cuts.concat([ms]).sort((x, y) => x - y)
        selected = cuts.indexOf(ms)
    }
    function toggleCut(i) {
        if (i < 0 || i >= parts.length) return
        const r = Object.assign({}, removed), k = String(parts[i].a)
        if (r[k]) delete r[k]; else r[k] = true
        removed = r
    }
    function partAt(ms) {
        for (let i = 0; i < parts.length; i++) if (ms >= parts[i].a && ms < parts[i].b) return i
        return parts.length - 1
    }
    function clock(ms) {
        const t = Math.max(0, ms) / 1000, m = Math.floor(t / 60), s = t - m * 60
        return m + ":" + (s < 10 ? "0" : "") + s.toFixed(1)
    }

    // the result's size, roughly: the recording's bitrate for the time kept,
    // scaled by the area and the quality (H.264 doubles every 6 steps); a
    // GIF by its pixels
    readonly property real outHeight: maxHeight > 0 ? Math.min(maxHeight, info.height || maxHeight) : (info.height || 1)
    readonly property real estimate: {
        const frac = keptMs / duration, area = Math.pow(outHeight / Math.max(1, info.height || 1), 2)
        if (gif) {
            const cfg = controller && controller.settings.config ? controller.settings.config.recording : null
            const fps = cfg ? cfg.gif_fps : 15, w = Math.min(cfg ? cfg.gif_width : 800, info.width || 800)
            const h = w * (info.height || 1) / Math.max(1, info.width || 1)
            return keptMs / 1000 * fps * w * h * 0.12
        }
        return (info.bytes || 0) * frac * area * Math.pow(2, (19 - quality) / 6)
    }
    function size(b) {
        return b >= 1e9 ? (b / 1e9).toFixed(1) + " GB" : b >= 1e6 ? (b / 1e6).toFixed(1) + " MB" : Math.max(1, Math.round(b / 1e3)) + " KB"
    }

    function exportIt(replace) {
        if (progress >= 0 || kept.length === 0) return
        error = ""
        progress = 0
        player.pause()
        const cfg = controller && controller.settings.config ? controller.settings.config.recording : null
        Engine.videoExport(info.id, JSON.stringify({
            keep: kept, tracks: volumes, mono: mono, height: maxHeight > 0 ? maxHeight : null, quality: quality,
            gif: gif ? { fps: cfg ? cfg.gif_fps : 15, width: cfg ? cfg.gif_width : 800 } : null
        }), replace)
    }
    function exportStatus(s) {
        if (s.done) {
            progress = -1
            if (s.ok) close()
            else error = s.error || "The export failed"
            Engine.log("video_exported_ui", JSON.stringify({ ok: !!s.ok, id: s.id || "", error: s.error || "" }).slice(1, -1))
        } else {
            progress = s.progress
        }
    }
    onClosing: { player.stop(); finished() }

    // ---------------------------------------------------------------- playback
    MediaPlayer {
        id: player
        source: ve.info.path ? "file://" + ve.info.path : ""
        videoOutput: video
        // paused, the first frame shows (stopped, the picture stays black)
        onMediaStatusChanged: if (mediaStatus === MediaPlayer.LoadedMedia) pause()
        audioOutput: AudioOutput {
            muted: ve.volumes.length === 0 || ve.volumes.every(v => v <= 0)
            volume: Math.min(1, Math.max(...ve.volumes, 0))
        }
        // what's cut or trimmed off is skipped, as in the result
        onPositionChanged: {
            if (playbackState !== MediaPlayer.PlayingState) return
            ve.headMs = position
            if (position >= ve.outMs - 1) { pause(); return }
            const p = ve.parts[ve.partAt(position)]
            if (position < ve.inMs) position = ve.inMs
            else if (p && p.cut) {
                let next = -1
                for (const q of ve.parts) if (q.a >= p.b && !q.cut) { next = q.a; break }
                if (next < 0 || next >= ve.outMs) pause(); else position = next
            }
        }
    }
    function playPause() {
        if (player.playbackState === MediaPlayer.PlayingState) { player.pause(); return }
        // from the playhead (a seek may still be on its way there)
        seekNext = -1
        player.position = headMs >= outMs - frameMs || headMs < inMs ? inMs : headMs
        player.play()
    }
    // From the frame shown (the first one that ends at or after the
    // playhead: on a frame's first instant Qt shows the one before), to the
    // middle of another
    function step(frames) {
        player.pause()
        const shown = Math.max(0, Math.ceil(headMs / frameMs - 1e-6) - 1)
        const n = Math.max(0, Math.min(Math.floor((duration - 1) / frameMs), shown + frames))
        seek(Math.round((n + 0.5) * frameMs))
    }
    // Where the playhead is: the frame asked for, or where playback is.
    // One seek at a time, the newest waiting: each new seek drops the one
    // under way, so seeking on every move of a drag showed nothing until
    // the drag stopped.
    property real headMs: 0
    property real seekNext: -1
    property bool seeking: false
    function seek(ms) {
        headMs = ms
        if (seeking) { seekNext = ms; return }
        seeking = true
        seekGuard.restart()
        player.position = ms
    }
    function seekDone() {
        seeking = false
        if (seekNext >= 0) { const ms = seekNext; seekNext = -1; seek(ms) }
    }
    Timer { id: seekGuard; interval: 600; onTriggered: ve.seekDone() }
    Connections {
        target: video.videoSink
        function onVideoFrameChanged() { if (ve.seeking) { seekGuard.stop(); ve.seekDone() } }
    }

    Item {
        focus: true
        Keys.onPressed: (e) => {
            const ctrl = e.modifiers & Qt.ControlModifier, shift = e.modifiers & Qt.ShiftModifier
            if (e.key === Qt.Key_Space) ve.playPause()
            else if (e.key === Qt.Key_B) ve.split(ve.headMs)
            else if (e.key === Qt.Key_Delete || e.key === Qt.Key_Backspace) ve.toggleCut(ve.selected)
            else if (e.key === Qt.Key_Left) ve.step(shift ? -10 : -1)
            else if (e.key === Qt.Key_Right) ve.step(shift ? 10 : 1)
            else if (e.key === Qt.Key_Home) { player.pause(); ve.seek(ve.inMs) }
            else if (e.key === Qt.Key_End) { player.pause(); ve.seek(ve.outMs) }
            else if (ctrl && e.key === Qt.Key_S) ve.exportIt(!shift)
            else if (e.key === Qt.Key_Escape || (ctrl && e.key === Qt.Key_W)) ve.close()
            else return
            e.accepted = true
        }
    }

    // ---------------------------------------------------------------- layout
    Rectangle {
        id: stage
        anchors { left: parent.left; right: parent.right; top: parent.top; bottom: transport.top; margins: 16 }
        radius: Math.min(Theme.radius, 10)
        color: Qt.rgba(0, 0, 0, Theme.light ? 0.06 : 0.35)
        VideoOutput {
            id: video
            anchors.fill: parent
            anchors.margins: 8
            fillMode: VideoOutput.PreserveAspectFit
        }
        MouseArea { anchors.fill: parent; onClicked: ve.playPause() }
    }

    // play, the time, split, cut
    Row {
        id: transport
        anchors { left: parent.left; leftMargin: 16; bottom: timeline.top; bottomMargin: 10 }
        height: 34
        spacing: 10
        IconButton { objectName: "video-play"; anchors.verticalCenter: parent.verticalCenter; icon: player.playbackState === MediaPlayer.PlayingState ? "pause" : "play"; tip: "Play or pause (Space)"; onClicked: ve.playPause() }
        Text {
            objectName: "video-time"
            anchors.verticalCenter: parent.verticalCenter
            text: ve.clock(ve.headMs) + " / " + ve.clock(ve.keptMs)
            color: Theme.text
            font.family: Theme.fontUi
            font.pixelSize: 13
            font.features: { "tnum": 1 }
        }
        Item { width: 12; height: 1 }
        TextButton { objectName: "video-split"; anchors.verticalCenter: parent.verticalCenter; text: "Split"; tip: "At the playhead (B)"; onClicked: ve.split(ve.headMs) }
        TextButton {
            objectName: "video-cut"
            anchors.verticalCenter: parent.verticalCenter
            enabled: ve.selected >= 0
            opacity: enabled ? 1 : 0.4
            text: ve.selected >= 0 && ve.parts[ve.selected] && ve.parts[ve.selected].cut ? "Restore" : "Cut"
            tip: "The part selected (Delete)"
            onClicked: ve.toggleCut(ve.selected)
        }
    }
    // the sound: each track's volume, mono
    Row {
        anchors { right: parent.right; rightMargin: 16; verticalCenter: transport.verticalCenter }
        spacing: 12
        visible: (ve.info.tracks || 0) > 0
        Repeater {
            model: ve.info.tracks || 0
            Row {
                required property int index
                spacing: 6
                anchors.verticalCenter: parent.verticalCenter
                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: "volume"; size: 15
                    color: (ve.volumes[index] || 0) > 0 ? Theme.text : Theme.textSecondary
                    TapHandler { onTapped: { const v = ve.volumes.slice(); v[index] = v[index] > 0 ? 0 : 1; ve.volumes = v } }
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: (ve.info.tracks > 1 ? "Track " + (index + 1) + " " : "") + Math.round((ve.volumes[index] || 0) * 100) + "%"
                    color: Theme.textSecondary
                    font.family: Theme.fontUi
                    font.pixelSize: 12
                    font.features: { "tnum": 1 }
                }
                Slider {
                    objectName: "video-volume:" + index
                    anchors.verticalCenter: parent.verticalCenter
                    width: 110
                    from: 0; to: 2
                    value: ve.volumes[index] || 0
                    onMoved: (v) => { const a = ve.volumes.slice(); a[index] = v; ve.volumes = a }
                }
            }
        }
        Check { objectName: "video-mono"; anchors.verticalCenter: parent.verticalCenter; text: "Mono"; on: ve.mono; onToggled: ve.mono = !ve.mono }
    }

    // ---------------------------------------------------------------- the timeline
    Item {
        id: timeline
        objectName: "video-timeline"
        anchors { left: parent.left; right: parent.right; bottom: exportBar.top; margins: 16; bottomMargin: 14 }
        height: 76
        readonly property real k: width / ve.duration          // px per ms
        function msAt(x) { return Math.max(0, Math.min(ve.duration, x / k)) }

        Row {
            anchors.fill: parent
            clip: true
            Repeater {
                model: ve.thumbs
                Image {
                    required property string modelData
                    width: timeline.width / Math.max(1, ve.thumbs.length)
                    height: timeline.height
                    source: "file://" + modelData
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                }
            }
        }
        // the parts: cut ones dimmed, with how long they were
        Repeater {
            model: ve.parts
            Rectangle {
                required property var modelData
                required property int index
                objectName: "video-part:" + index
                x: modelData.a * timeline.k
                width: (modelData.b - modelData.a) * timeline.k
                height: timeline.height
                color: modelData.cut ? Qt.rgba(0, 0, 0, 0.62) : "transparent"
                border.width: ve.selected === index ? 2 : 0
                border.color: Theme.accent
                Text {
                    visible: parent.modelData.cut && parent.width > 40
                    anchors.centerIn: parent
                    text: "✂ " + ((parent.modelData.b - parent.modelData.a) / 1000).toFixed(1) + " s"
                    color: "white"
                    font.family: Theme.fontUi
                    font.pixelSize: 12
                    font.weight: Font.DemiBold
                }
            }
        }
        // where the parts meet
        Repeater {
            model: ve.cuts.slice(1, -1)
            Rectangle { required property real modelData; x: modelData * timeline.k - 1; width: 2; height: timeline.height; color: "white"; opacity: 0.85 }
        }
        // trimmed off: dimmed outside the handles
        Rectangle { x: 0; width: ve.inMs * timeline.k; height: timeline.height; color: Qt.rgba(0, 0, 0, 0.62) }
        Rectangle { x: ve.outMs * timeline.k; width: timeline.width - x; height: timeline.height; color: Qt.rgba(0, 0, 0, 0.62) }
        // click: the part under it is selected and the playhead goes there
        MouseArea {
            anchors.fill: parent
            onPressed: (m) => { const ms = timeline.msAt(m.x); ve.selected = ve.partAt(ms); player.pause(); ve.seek(ms) }
            onPositionChanged: (m) => { if (pressed) ve.seek(timeline.msAt(m.x)) }
        }
        // the trim handles
        Repeater {
            model: ["in", "out"]
            Rectangle {
                required property string modelData
                readonly property bool isIn: modelData === "in"
                objectName: "video-" + modelData
                width: 12
                height: timeline.height + 8
                y: -4
                radius: 4
                x: (isIn ? ve.inMs * timeline.k : ve.outMs * timeline.k - width)
                color: Theme.warning
                Rectangle { anchors.centerIn: parent; width: 2; height: 18; radius: 1; color: Qt.rgba(0, 0, 0, 0.5) }
                MouseArea {
                    anchors.fill: parent
                    anchors.margins: -6
                    cursorShape: Qt.SizeHorCursor
                    property real grab: 0
                    onPressed: (m) => { grab = m.x; player.pause() }
                    onPositionChanged: (m) => {
                        if (!pressed) return
                        const x = parent.x + m.x - grab + (parent.isIn ? 0 : parent.width)
                        const ms = timeline.msAt(x)
                        if (parent.isIn) ve.inMs = Math.min(ms, ve.outMs - 10 * ve.frameMs)
                        else ve.outMs = Math.max(ms, ve.inMs + 10 * ve.frameMs)
                        ve.seek(parent.isIn ? ve.inMs : ve.outMs)
                    }
                }
            }
        }
        // the playhead
        Rectangle {
            x: ve.headMs * timeline.k - 1
            y: -6
            width: 2
            height: timeline.height + 12
            color: Theme.accent
        }
    }

    // ---------------------------------------------------------------- export (VED-02)
    Rectangle {
        id: exportBar
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: 60
        color: Theme.surface
        Rectangle { anchors { left: parent.left; right: parent.right; top: parent.top } height: 1; color: Theme.hairline }
        Row {
            anchors { left: parent.left; leftMargin: 16; verticalCenter: parent.verticalCenter }
            spacing: 14
            Segmented { objectName: "video-format"; options: [[false, "MP4"], [true, "GIF"]]; current: ve.gif; onPicked: (v) => ve.gif = v }
            Segmented {
                visible: !ve.gif
                objectName: "video-quality"
                options: [[32, "Low"], [27, "Medium"], [23, "High"], [19, "Very high"]]
                current: ve.quality
                onPicked: (v) => ve.quality = v
            }
            Segmented {
                visible: !ve.gif
                objectName: "video-size"
                options: [[0, "Original"], [1080, "1080p"], [720, "720p"], [480, "480p"]]
                current: ve.maxHeight
                onPicked: (v) => ve.maxHeight = v
            }
            Text {
                objectName: "video-estimate"
                anchors.verticalCenter: parent.verticalCenter
                text: ve.progress >= 0 ? "Exporting… " + Math.round(ve.progress * 100) + "%" : ve.error !== "" ? ve.error : "≈ " + ve.size(ve.estimate)
                color: ve.error !== "" ? Theme.danger : Theme.textSecondary
                font.family: Theme.fontUi
                font.pixelSize: 12
                font.features: { "tnum": 1 }
                width: Math.min(implicitWidth, 260)
                elide: Text.ElideRight
            }
        }
        Row {
            anchors { right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
            spacing: 8
            TextButton { objectName: "video-save-new"; text: "Save as New"; tip: "Ctrl+Shift+S"; enabled: ve.progress < 0 && ve.kept.length > 0; onClicked: ve.exportIt(false) }
            TextButton { objectName: "video-replace"; text: "Replace"; primary: true; tip: "Ctrl+S"; enabled: ve.progress < 0 && ve.kept.length > 0; onClicked: ve.exportIt(true) }
        }
    }

    // a choice of a few
    component Segmented: Row {
        id: seg
        property var options: []
        property var current
        signal picked(var value)
        spacing: 2
        anchors.verticalCenter: parent ? parent.verticalCenter : undefined
        Repeater {
            model: seg.options
            Rectangle {
                required property var modelData
                readonly property bool on: seg.current === modelData[0]
                objectName: seg.objectName + "=" + modelData[0]
                width: segLabel.implicitWidth + 20
                height: 28
                radius: Theme.radiusControl
                color: on ? Theme.accent : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, segHover.hovered ? 0.12 : 0.06)
                Text {
                    id: segLabel
                    anchors.centerIn: parent
                    text: parent.modelData[1]
                    color: parent.on ? Theme.onAccent : Theme.text
                    font.family: Theme.fontUi
                    font.pixelSize: 12
                    font.weight: parent.on ? Font.DemiBold : Font.Normal
                }
                HoverHandler { id: segHover; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: seg.picked(parent.modelData[0]) }
            }
        }
    }
    component Check: Row {
        property string text: ""
        property bool on: false
        signal toggled()
        spacing: 6
        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 16; height: 16
            radius: Math.min(4, Theme.radiusControl)
            color: parent.on ? Theme.accent : "transparent"
            border.width: parent.on ? 0 : 1
            border.color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.3)
            Icon { anchors.centerIn: parent; visible: parent.parent.on; name: "check"; size: 12; stroke: 3; color: Theme.onAccent }
        }
        Text { anchors.verticalCenter: parent.verticalCenter; text: parent.text; color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 12 }
        TapHandler { onTapped: parent.toggled() }
    }

    // ---------------------------------------------------------------- tests
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
        case "split": split(Number(a[1])); break
        case "cut": toggleCut(Number(a[1])); break
        case "seek": player.pause(); seek(Number(a[1])); break
        case "trim": inMs = Number(a[1]); outMs = Number(a[2]); break
        case "volume": { const v = volumes.slice(); v[Number(a[1])] = Number(a[2]); volumes = v; break }
        case "keys": Platform.sendKeys(ve, a.slice(1).join(" ")); break
        case "export": exportIt(a[1] === "replace"); break
        case "where": {
            const it = find(contentItem, a[1])
            for (let q = it; q; q = q.parent) if (q.forceLayout) q.forceLayout()
            const p = it ? it.mapToItem(contentItem, it.width / 2, it.height / 2) : null
            Engine.log("video_where", JSON.stringify({ name: a[1], x: p ? p.x : -1, y: p ? p.y : -1 }).slice(1, -1))
            break
        }
        case "state":
            Engine.log("video_state", JSON.stringify({ id: info.id, duration: duration, cuts: cuts, cut: parts.filter(p => p.cut).map(p => [p.a, p.b]),
                                                     in: inMs, out: outMs, kept: kept, keptMs: keptMs, selected: selected, volumes: volumes,
                                                     playing: player.playbackState === MediaPlayer.PlayingState, position: headMs,
                                                     progress: progress, error: error, estimate: Math.round(estimate), thumbs: thumbs.length,
                                                     tracks: info.tracks || 0, gif: gif }).slice(1, -1))
            break
        }
    }
}
