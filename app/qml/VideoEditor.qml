import QtQuick
import QtQuick.Window
import QtQuick.Effects
import QtMultimedia
import Snip.Core
import "editor.js" as Ed
import "probe.js" as Probe

// The video editor (VED-01…03), laid out as CleanShot 5's (D50): the file's
// name and Export along the top; a sidebar of sections (Audio, Background)
// with the cut tool; the recording in the middle, with play controls under
// it; its frames along a timeline across the bottom, which zooms. Drag the
// ends to trim; B (or the scissors) splits at the playhead; Delete cuts the
// part that's selected (again: brings it back). Each audio track gets a
// volume (0 mutes it; several are mixed into one), and mono. A background
// goes around the recording as around a screenshot, into the export too.
// Export as MP4 or GIF, at a quality and size, with an estimate of the
// file's size: Replace the recording, or Save as New. Playback skips what's
// cut, so you see the result.
Window {
    id: ve
    property var info: ({})                 // {id, path, duration, fps, width, height, tracks, bytes, title, output}
    property var thumbs: []                 // the timeline's frames: they come a moment after it opens
    property var controller: null
    signal finished()

    title: "Snip Video Editor — " + (info.title || "Recording")
    color: Theme.background
    // 1180 x 780, or 90 % of its screen's free space when that's less (on 1280 x 720
    // logical px the timeline and Export were off screen)
    readonly property rect avail: { const s = Platform.screen(info.output || ""); return s ? s.availableGeometry : Qt.rect(0, 0, 1600, 1000) }
    width: Math.min(1180, Math.round(avail.width * 0.9))
    height: Math.min(780, Math.round(avail.height * 0.9))
    minimumWidth: Math.min(900, width)
    minimumHeight: Math.min(600, height)
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
    // Studio Mode (D55): the zooms on this recording and how they move ({zooms, zoom_style})
    property var studio: ({ zooms: [], zoom_style: "smooth" })
    property string section: "audio"          // the sidebar's (D50)
    property bool exportOpen: false           // the Export popover
    property real zoom: 1                     // the timeline's: 1 (all of it) to 8

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
    // the ruler's: whole seconds, or tenths when they're that close
    function stamp(ms, tenths) {
        const t = Math.round(ms / 100) / 10, m = Math.floor(t / 60), s = t - m * 60
        return m + ":" + (s < 10 ? "0" : "") + (tenths ? s.toFixed(1) : Math.round(s))
    }
    // the background (D50), as a screenshot's (BG-01…06), in the recording's
    // pixels: around it in the preview, and drawn into the export
    property var bg: null
    property var bgPreview: null                  // while a slider is dragged
    readonly property var bgLive: bg ? Object.assign({}, bg, bgPreview || {}) : null
    readonly property real unit: info.scale || 1   // pixels per point
    function setBg(patch) {
        bgPreview = null
        bg = patch === null ? null : Object.assign({}, bg || Ed.defaultBackground(unit), patch)
    }
    function previewBg(patch) {
        if (!bg) setBg({})
        bgPreview = Object.assign({}, bgPreview || {}, patch)
    }
    function focusKeys() { keyboard.forceActiveFocus() }
    // the recording, and the result around it
    readonly property var inner: ({ x: 0, y: 0, w: info.width || 16, h: info.height || 9 })
    readonly property var outer: bgLive ? Ed.bgRect(bgLive, inner) : inner
    function setVolume(i, v) {
        const a = volumes.slice()
        a[i] = Math.max(0, Math.min(2, v))
        volumes = a
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
        // Studio Mode (D55): with zooms, snipd draws every frame with this scene
        if (studio.zooms.length > 0)
            return send(replace, null, { bg: bgLive, outer: outer, inner: inner, studio: studio, theme: Engine.theme })
        if (!bgLive) return send(replace, null)
        // the background first, as the export draws it: a grab, then ffmpeg
        coverReplace = replace
        cover.active = true
    }
    function send(replace, background, scene) {
        const cfg = controller && controller.settings.config ? controller.settings.config.recording : null
        Engine.videoExport(info.id, JSON.stringify({
            keep: kept, tracks: volumes, mono: mono, height: maxHeight > 0 ? maxHeight : null, quality: quality,
            gif: gif ? { fps: cfg ? cfg.gif_fps : 15, width: cfg ? cfg.gif_width : 800 } : null, background: background,
            scene: scene || null
        }), replace)
    }
    property bool coverReplace: false
    function coverDrawn(token, ok) {
        cover.active = false
        if (ok) return send(coverReplace, { token: token, video: [-outer.x, -outer.y, inner.w, inner.h] })
        progress = -1
        error = "The background couldn't be drawn"
        exportOpen = true
    }
    function exportStatus(s) {
        if (s.done) {
            progress = -1
            if (s.ok) close()
            else { error = s.error || "The export failed"; exportOpen = true }
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
        videoOutput: studioFrame.video
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
    // ⏮ ⏭: to the edge of a part before or after the playhead (a split, or the trim)
    function jump(dir) {
        player.pause()
        const edges = [inMs].concat(cuts.filter(c => c > inMs && c < outMs), [outMs]), eps = frameMs / 2
        const t = dir < 0 ? edges.filter(x => x < headMs - eps).pop() : edges.find(x => x > headMs + eps)
        seek(t === undefined ? (dir < 0 ? inMs : outMs) : t)
    }
    // the timeline zooms around the playhead, which stays where it was on screen
    function setZoom(z) {
        z = Math.max(1, Math.min(8, z))
        const at = timeline.x + headMs * timeline.k - tlView.contentX
        zoom = z
        scrollTo(timeline.x + headMs * timeline.k - at)
    }
    function scrollTo(x) { tlView.contentX = Math.max(0, Math.min(tlView.contentWidth - tlView.width, x)) }
    // zoomed in, the timeline keeps the playhead in view
    onHeadMsChanged: {
        const x = timeline.x + headMs * timeline.k
        if (x < tlView.contentX || x > tlView.contentX + tlView.width) scrollTo(x - tlView.width * 0.1)
    }
    // the time of the frame shown: zooms follow the picture, every frame
    FrameTime { id: frameTime; sink: studioFrame.video.videoSink }
    Connections {
        target: studioFrame.video.videoSink
        function onVideoFrameChanged() { if (ve.seeking) { seekGuard.stop(); ve.seekDone() } }
    }

    Item {
        id: keyboard
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
            else if (e.key === Qt.Key_Escape && ve.exportOpen) ve.exportOpen = false
            else if (e.key === Qt.Key_Escape || (ctrl && e.key === Qt.Key_W)) ve.close()
            else return
            e.accepted = true
        }
    }

    // ---------------------------------------------------------------- the title bar: the name, Export
    Rectangle {
        id: header
        anchors { left: parent.left; right: parent.right; top: parent.top }
        height: 48
        color: Theme.surface
        Rectangle { anchors { left: parent.left; right: parent.right; bottom: parent.bottom } height: 1; color: Theme.hairline }
        Text {
            anchors.centerIn: parent
            width: Math.min(implicitWidth, parent.width - 360)
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideMiddle
            text: ve.info.title || "Recording"
            color: Theme.text
            font.family: Theme.fontUi
            font.pixelSize: 13
            font.weight: Font.DemiBold
        }
        TextButton {
            objectName: "video-export"
            anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
            primary: true
            text: ve.progress >= 0 ? "Exporting… " + Math.round(ve.progress * 100) + "%" : "Export"
            onClicked: ve.exportOpen = !ve.exportOpen
        }
    }

    // ---------------------------------------------------------------- the sidebar: sections, and the cut tool
    Rectangle {
        id: sidebar
        anchors { left: parent.left; top: header.bottom; bottom: timelineArea.top }
        width: 52
        color: Theme.surface
        Rectangle { anchors { right: parent.right; top: parent.top; bottom: parent.bottom } width: 1; color: Theme.hairline }
        Column {
            anchors.horizontalCenter: parent.horizontalCenter
            y: 10
            spacing: 6
            Repeater {
                // [section, icon, name]
                model: [["audio", "volume", "Audio"], ["background", "image", "Background"]]
                SideButton {
                    required property var modelData
                    objectName: "video-section=" + modelData[0]
                    icon: modelData[1]
                    tip: modelData[2]
                    on: ve.section === modelData[0]
                    onClicked: ve.section = modelData[0]
                }
            }
        }
        // level with the play controls
        SideButton {
            objectName: "video-split"
            anchors { horizontalCenter: parent.horizontalCenter; verticalCenter: parent.bottom; verticalCenterOffset: -transport.height / 2 }
            icon: "scissors"
            tip: "Split at the playhead (B)"
            onClicked: ve.split(ve.headMs)
        }
    }

    // ---------------------------------------------------------------- the section's settings
    Rectangle {
        id: inspector
        anchors { left: sidebar.right; top: header.bottom; bottom: timelineArea.top }
        width: 264                                // seven colours to a row, as the Background tool's
        color: Theme.surface
        Rectangle { anchors { right: parent.right; top: parent.top; bottom: parent.bottom } width: 1; color: Theme.hairline }
        BackgroundPanel {
            visible: ve.section === "background"
            anchors { fill: parent; rightMargin: 1 }
            ed: ve
            video: true
        }
        Flickable {
            visible: ve.section === "audio"
            anchors.fill: parent
            anchors.margins: 16
            contentHeight: audioPanel.implicitHeight
            boundsBehavior: Flickable.StopAtBounds
            clip: true
            // Audio (VED-03): each track's volume (0 mutes it; several are mixed into one), and mono
            Column {
                id: audioPanel
                width: parent.width
                spacing: 14
                Text { text: "Audio"; color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 13; font.weight: Font.DemiBold }
                Text {
                    visible: (ve.info.tracks || 0) === 0
                    width: parent.width
                    wrapMode: Text.WordWrap
                    text: "This recording has no sound."
                    color: Theme.textSecondary
                    font.family: Theme.fontUi
                    font.pixelSize: 12
                }
                Repeater {
                    model: ve.info.tracks || 0
                    Item {
                        id: trackRow
                        required property int index
                        readonly property real volume: ve.volumes[index] || 0
                        width: audioPanel.width
                        height: 40
                        Rectangle {
                            objectName: "video-mute:" + trackRow.index
                            y: 16
                            width: 24
                            height: 24
                            radius: 6
                            color: muteHover.hovered ? Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08) : "transparent"
                            Icon { anchors.centerIn: parent; name: trackRow.volume > 0 ? "volume" : "volume-x"; size: 15; color: trackRow.volume > 0 ? Theme.text : Theme.textSecondary }
                            HoverHandler { id: muteHover; cursorShape: Qt.PointingHandCursor }
                            TapHandler { onTapped: ve.setVolume(trackRow.index, trackRow.volume > 0 ? 0 : 1) }
                        }
                        Slider {
                            objectName: "video-volume:" + trackRow.index
                            x: 32
                            width: parent.width - 32
                            label: (ve.info.tracks || 0) > 1 ? "Track " + (trackRow.index + 1) : "Sound"
                            from: 0
                            to: 200
                            unit: "%"
                            value: trackRow.volume * 100
                            onMoved: (v) => ve.setVolume(trackRow.index, v / 100)
                            onReleased: (v) => { ve.setVolume(trackRow.index, v / 100); keyboard.forceActiveFocus() }
                            onCancelled: keyboard.forceActiveFocus()
                        }
                    }
                }
                Check { objectName: "video-mono"; visible: (ve.info.tracks || 0) > 0; text: "Mono"; on: ve.mono; onToggled: ve.mono = !ve.mono }
                Text {
                    visible: (ve.info.tracks || 0) > 1
                    width: parent.width
                    wrapMode: Text.WordWrap
                    text: "The tracks are mixed into one when it's exported."
                    color: Theme.textSecondary
                    font.family: Theme.fontUi
                    font.pixelSize: 12
                }
            }
        }
    }

    // ---------------------------------------------------------------- the recording
    Rectangle {
        id: stage
        anchors { left: inspector.right; right: parent.right; top: header.bottom; bottom: transport.top }
        color: Qt.rgba(0, 0, 0, Theme.light ? 0.06 : 0.35)
        // the result fitted in: the recording in its background
        Item {
            id: result
            objectName: "video-result"
            readonly property real k: Math.min((stage.width - 48) / ve.outer.w, (stage.height - 48) / ve.outer.h)
            anchors.centerIn: parent
            width: ve.outer.w * k
            height: ve.outer.h * k
            // the recording in its background, as the export draws it (Studio Mode, D55)
            StudioFrame {
                id: studioFrame
                transform: Scale { xScale: result.k; yScale: result.k }
                bg: ve.bgLive
                outer: ve.outer
                inner: ve.inner
                studio: ve.studio
                ms: frameTime.ms
                pixelScale: result.k * Screen.devicePixelRatio
            }
        }
        MouseArea { anchors.fill: parent; onPressed: ve.focusKeys(); onClicked: ve.playPause() }
    }
    // the background for the export: drawn off screen at the result's size, a
    // hole where the recording shows (ffmpeg puts it on top), then grabbed
    Loader {
        id: cover
        active: false
        x: -100000
        sourceComponent: Item {
            id: coverScene
            readonly property var r: ve.outer
            Item {
                width: coverScene.r.w
                height: coverScene.r.h
                layer.enabled: true
                layer.textureSize: Qt.size(width, height)
                layer.effect: MultiEffect { maskEnabled: true; maskInverted: true; maskSource: hole; maskThresholdMin: 0.5; maskSpreadAtMin: 1.0 }
                BackgroundLayer { x: -coverScene.r.x; y: -coverScene.r.y; bg: ve.bgLive; rect: coverScene.r; inner: ve.inner }
            }
            Item {
                id: hole
                visible: false
                width: coverScene.r.w
                height: coverScene.r.h
                layer.enabled: true
                layer.textureSize: Qt.size(width, height)
                Rectangle { x: -coverScene.r.x; y: -coverScene.r.y; width: ve.inner.w; height: ve.inner.h; radius: ve.bgLive ? ve.bgLive.radius : 0; color: "white" }
            }
            // a frame for the wallpaper to load, as the screenshot's does
            Timer { interval: 60; running: true; onTriggered: Platform.grabExact(coverScene, coverScene.r.w, coverScene.r.h, (token, ok) => ve.coverDrawn(token, ok)) }
        }
    }

    // the time; ⏮ ▶ ⏭; the timeline's zoom
    Rectangle {
        id: transport
        anchors { left: inspector.right; right: parent.right; bottom: timelineArea.top }
        height: 48
        color: Theme.surface
        Rectangle { anchors { left: parent.left; right: parent.right; top: parent.top } height: 1; color: Theme.hairline }
        Row {
            anchors { left: parent.left; leftMargin: 16; verticalCenter: parent.verticalCenter }
            spacing: 14
            Text {
                objectName: "video-time"
                anchors.verticalCenter: parent.verticalCenter
                text: ve.clock(ve.headMs) + "  /  " + ve.clock(ve.duration)
                color: Theme.text
                font.family: Theme.fontUi
                font.pixelSize: 13
                font.features: { "tnum": 1 }
            }
            TextButton {
                objectName: "video-cut"
                visible: ve.selected >= 0 && ve.parts.length > 1
                anchors.verticalCenter: parent.verticalCenter
                text: ve.parts[ve.selected] && ve.parts[ve.selected].cut ? "Restore Part" : "Cut Part"
                tip: "The part selected (Delete)"
                onClicked: ve.toggleCut(ve.selected)
            }
        }
        Row {
            anchors.centerIn: parent
            spacing: 6
            Repeater {
                // [name, icon, tip, size]
                model: [["video-prev", "skip-back", "To the part before (Home: the start)", 16],
                        ["video-play", player.playbackState === MediaPlayer.PlayingState ? "pause" : "play", "Play or pause (Space)", 20],
                        ["video-next", "skip-forward", "To the next part (End: the end)", 16]]
                Rectangle {
                    id: playButton
                    required property var modelData
                    objectName: modelData[0]
                    anchors.verticalCenter: parent.verticalCenter
                    width: 36
                    height: 36
                    radius: 18
                    color: playHover.hovered ? Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.1) : "transparent"
                    Icon { anchors.centerIn: parent; name: playButton.modelData[1]; size: playButton.modelData[3]; filled: true; stroke: 1.5; color: Theme.text }
                    HoverHandler { id: playHover; cursorShape: Qt.PointingHandCursor }
                    TapHandler {
                        onTapped: {
                            if (playButton.modelData[0] === "video-play") ve.playPause()
                            else ve.jump(playButton.modelData[0] === "video-prev" ? -1 : 1)
                        }
                    }
                }
            }
        }
        Row {
            anchors { right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
            spacing: 8
            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: "minus"; size: 14; color: Theme.textSecondary
                TapHandler { onTapped: ve.setZoom(ve.zoom / 1.5) }
            }
            Item {
                id: zoomBar
                objectName: "video-zoom"
                anchors.verticalCenter: parent.verticalCenter
                width: 110
                height: 20
                readonly property real frac: Math.log(ve.zoom) / Math.log(8)
                Rectangle { anchors.verticalCenter: parent.verticalCenter; width: parent.width; height: 4; radius: 2; color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.15) }
                Rectangle { x: zoomBar.frac * (parent.width - width); anchors.verticalCenter: parent.verticalCenter; width: 14; height: 14; radius: 7; color: "white"; border.width: 1; border.color: Theme.hairline }
                MouseArea {
                    anchors.fill: parent
                    anchors.margins: -4
                    preventStealing: true
                    function at(m) { return Math.pow(8, Math.max(0, Math.min(1, (m.x - 4) / zoomBar.width))) }
                    onPressed: (m) => ve.setZoom(at(m))
                    onPositionChanged: (m) => { if (pressed) ve.setZoom(at(m)) }
                }
            }
            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: "plus"; size: 14; color: Theme.textSecondary
                TapHandler { onTapped: ve.setZoom(ve.zoom * 1.5) }
            }
        }
    }

    // ---------------------------------------------------------------- the timeline, across the bottom
    Rectangle {
        id: timelineArea
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: 148
        color: Theme.background
        Rectangle { anchors { left: parent.left; right: parent.right; top: parent.top } height: 1; color: Theme.hairline }
        Flickable {
            id: tlView
            anchors { fill: parent; leftMargin: 10; rightMargin: 10; topMargin: 8; bottomMargin: 14 }
            contentWidth: timeline.width + 2 * timeline.x
            contentHeight: height
            interactive: false                    // a drag edits; the wheel scrolls it (below)
            clip: true
            Item {
                id: timeline
                objectName: "video-timeline"
                x: 6                              // room for the playhead's knob at either end
                width: (tlView.width - 2 * x) * ve.zoom
                height: tlView.height
                readonly property real k: width / ve.duration          // px per ms
                function msAt(x) { return Math.max(0, Math.min(ve.duration, x / k)) }

                // the ruler: a time every 180 px or more, a tick between; a press moves the playhead
                Item {
                    id: ruler
                    width: parent.width
                    height: 26
                    readonly property real step: {
                        for (const s of [100, 500, 1000, 2000, 5000, 10000, 15000, 30000, 60000, 120000, 300000])
                            if (s * timeline.k >= 180) return s
                        return 600000
                    }
                    Repeater {
                        model: Math.floor(ve.duration / ruler.step * 2) + 1
                        Item {
                            required property int index
                            x: index * ruler.step / 2 * timeline.k
                            Rectangle { y: parent.index % 2 ? 21 : 18; width: 1; height: parent.index % 2 ? 3 : 6; color: Theme.textSecondary; opacity: 0.6 }
                            Text {
                                visible: parent.index % 2 === 0
                                x: 7
                                y: 2
                                text: ve.stamp(parent.index / 2 * ruler.step, ruler.step < 1000)
                                color: Theme.textSecondary
                                font.family: Theme.fontUi
                                font.pixelSize: 11
                                font.features: { "tnum": 1 }
                            }
                        }
                    }
                    MouseArea {
                        anchors.fill: parent
                        onPressed: (m) => { ve.focusKeys(); player.pause(); ve.seek(timeline.msAt(m.x)) }
                        onPositionChanged: (m) => { if (pressed) ve.seek(timeline.msAt(m.x)) }
                    }
                }

                // the frames, as many as fit, each the one nearest its time
                Item {
                    id: track
                    y: ruler.height + 6
                    width: parent.width
                    height: 64
                    readonly property real cell: height * Math.max(0.5, (ve.info.width || 16) / Math.max(1, ve.info.height || 9))
                    Rectangle { anchors.fill: parent; color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.06) }
                    Item {
                        anchors.fill: parent
                        clip: true
                        Repeater {
                            model: ve.thumbs.length ? Math.ceil(track.width / track.cell) : 0
                            Image {
                                required property int index
                                x: index * track.cell
                                width: track.cell
                                height: track.height
                                source: "file://" + ve.thumbs[Math.min(ve.thumbs.length - 1, Math.floor((index + 0.5) * track.cell / track.width * ve.thumbs.length))]
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
                            height: track.height
                            color: modelData.cut ? Qt.rgba(0, 0, 0, 0.62) : "transparent"
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
                        Rectangle { required property real modelData; x: modelData * timeline.k - 1; width: 2; height: track.height; color: "white"; opacity: 0.85 }
                    }
                    // trimmed off: dimmed outside the handles
                    Rectangle { x: 0; width: ve.inMs * timeline.k; height: track.height; color: Qt.rgba(0, 0, 0, 0.62) }
                    Rectangle { x: ve.outMs * timeline.k; width: track.width - x; height: track.height; color: Qt.rgba(0, 0, 0, 0.62) }
                    // the strip's rounded ends
                    Rectangle { anchors.fill: parent; anchors.margins: -4; radius: 10; color: "transparent"; border.width: 4; border.color: timelineArea.color }
                    // the part selected
                    Rectangle {
                        readonly property var part: ve.parts[ve.selected] || null
                        visible: part !== null && ve.parts.length > 1
                        x: part ? part.a * timeline.k : 0
                        width: part ? (part.b - part.a) * timeline.k : 0
                        height: track.height
                        radius: 4
                        color: "transparent"
                        border.width: 2
                        border.color: Theme.accent
                    }
                    // a press selects the part under it and moves the playhead there
                    MouseArea {
                        anchors.fill: parent
                        onPressed: (m) => { ve.focusKeys(); const ms = timeline.msAt(m.x); ve.selected = ve.partAt(ms); player.pause(); ve.seek(ms) }
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
                            height: track.height + 8
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
                }

                // the playhead, from the ruler through the frames
                Item {
                    x: ve.headMs * timeline.k
                    Rectangle { x: -1; y: 8; width: 2; height: track.y + track.height + 4 - 8; color: Theme.danger }
                    Rectangle { x: -6; y: 2; width: 12; height: 12; radius: 6; color: Theme.danger }
                }
            }
        }
        // the wheel scrolls along it, Ctrl+wheel zooms
        WheelHandler {
            onWheel: (e) => {
                if (e.modifiers & Qt.ControlModifier) { ve.setZoom(ve.zoom * (e.angleDelta.y > 0 ? 1.25 : 0.8)); return }
                ve.scrollTo(tlView.contentX - (e.angleDelta.x !== 0 ? e.angleDelta.x : e.angleDelta.y))
            }
        }
        // zoomed in: which part of it shows
        Rectangle {
            visible: ve.zoom > 1
            x: tlView.x + tlView.contentX / tlView.contentWidth * tlView.width
            y: parent.height - 7
            width: tlView.width * tlView.width / tlView.contentWidth
            height: 3
            radius: 1.5
            color: Theme.textSecondary
            opacity: 0.5
        }
    }

    // ---------------------------------------------------------------- Export (VED-02): a popover under its button
    MouseArea {
        anchors.fill: parent
        visible: ve.exportOpen
        onPressed: ve.exportOpen = false              // a click outside closes it
    }
    Rectangle {
        objectName: "video-export-panel"
        visible: ve.exportOpen
        anchors { right: parent.right; rightMargin: 12; top: header.bottom; topMargin: 6 }
        width: 380
        height: exportCol.implicitHeight + 32
        radius: 12
        color: Theme.surface
        border.width: 1
        border.color: Theme.hairline
        RectangularShadow { z: -1; anchors.fill: parent; radius: parent.radius; offset.y: 8; blur: 28; color: Theme.shadow }
        MouseArea { anchors.fill: parent }           // a click in it stays in it
        Column {
            id: exportCol
            x: 16
            y: 16
            width: parent.width - 32
            spacing: 12
            Row {
                spacing: 12
                Text { width: 56; anchors.verticalCenter: parent.verticalCenter; text: "Format"; color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 12 }
                Segmented { objectName: "video-format"; anchors.verticalCenter: parent.verticalCenter; options: [[false, "MP4"], [true, "GIF"]]; current: ve.gif; onPicked: (v) => ve.gif = v }
            }
            Row {
                visible: !ve.gif
                spacing: 12
                Text { width: 56; anchors.verticalCenter: parent.verticalCenter; text: "Quality"; color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 12 }
                Segmented {
                    objectName: "video-quality"
                    anchors.verticalCenter: parent.verticalCenter
                    options: [[32, "Low"], [27, "Medium"], [23, "High"], [19, "Very high"]]
                    current: ve.quality
                    onPicked: (v) => ve.quality = v
                }
            }
            Row {
                visible: !ve.gif
                spacing: 12
                Text { width: 56; anchors.verticalCenter: parent.verticalCenter; text: "Size"; color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 12 }
                Segmented {
                    objectName: "video-size"
                    anchors.verticalCenter: parent.verticalCenter
                    options: [[0, "Original"], [1080, "1080p"], [720, "720p"], [480, "480p"]]
                    current: ve.maxHeight
                    onPicked: (v) => ve.maxHeight = v
                }
            }
            Rectangle { width: parent.width; height: 1; color: Theme.hairline }
            // an export error, whole and wrapped: ffmpeg's are long, and the estimate's
            // line cut them to a few words
            Text {
                objectName: "video-error"
                visible: ve.error !== ""
                width: parent.width
                wrapMode: Text.Wrap
                text: ve.error
                color: Theme.danger
                font.family: Theme.fontUi
                font.pixelSize: 12
            }
            Item {
                width: parent.width
                height: 30
                Text {
                    objectName: "video-estimate"
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - exportButtons.width - 8
                    elide: Text.ElideRight
                    text: ve.progress >= 0 ? "Exporting… " + Math.round(ve.progress * 100) + "%"
                        : ve.clock(ve.keptMs) + "  ·  ≈ " + ve.size(ve.estimate)
                    color: Theme.textSecondary
                    font.family: Theme.fontUi
                    font.pixelSize: 12
                    font.features: { "tnum": 1 }
                }
                Row {
                    id: exportButtons
                    anchors.right: parent.right
                    spacing: 8
                    TextButton { objectName: "video-save-new"; text: "Save as New"; tip: "Ctrl+Shift+S"; enabled: ve.progress < 0 && ve.kept.length > 0; onClicked: ve.exportIt(false) }
                    TextButton { objectName: "video-replace"; text: "Replace"; primary: true; tip: "Ctrl+S"; enabled: ve.progress < 0 && ve.kept.length > 0; onClicked: ve.exportIt(true) }
                }
            }
        }
    }

    // a section's button in the sidebar
    component SideButton: Rectangle {
        id: side
        property string icon: ""
        property string tip: ""
        property bool on: false
        signal clicked()
        width: 34
        height: 34
        radius: 8
        color: on ? Theme.accent : sideHover.hovered ? Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08) : "transparent"
        Icon { anchors.centerIn: parent; name: side.icon; size: 17; color: side.on ? Theme.onAccent : Theme.textSecondary }
        HoverHandler {
            id: sideHover
            cursorShape: Qt.PointingHandCursor
            onHoveredChanged: if (hovered) sideRest.restart(); else { sideRest.stop(); sideTip.shown = false }
        }
        TapHandler { onTapped: { sideRest.stop(); sideTip.shown = false; side.clicked() } }
        Timer { id: sideRest; interval: 600; onTriggered: sideTip.shown = true }
        Tip { id: sideTip; text: side.tip }
    }
    // a choice of a few
    component Segmented: Row {
        id: seg
        property var options: []
        property var current
        signal picked(var value)
        spacing: 2
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
    function debug(a) {
        switch (a[0]) {
        case "split": split(Number(a[1])); break
        case "cut": toggleCut(Number(a[1])); break
        case "seek": player.pause(); seek(Number(a[1])); break
        case "trim": inMs = Number(a[1]); outMs = Number(a[2]); break
        case "volume": { const v = volumes.slice(); v[Number(a[1])] = Number(a[2]); volumes = v; break }
        case "bg": setBg(JSON.parse(a.slice(1).join(" "))); break
        case "keys": Platform.sendKeys(ve, a.slice(1).join(" ")); break
        case "export": exportIt(a[1] === "replace"); break
        case "error": error = a.slice(1).join(" "); break
        case "studio": studio = JSON.parse(a.slice(1).join(" ")); break
        case "where": Engine.log("video_where", JSON.stringify(Probe.where(ve, a[1])).slice(1, -1)); break
        case "state":
            Engine.log("video_state", JSON.stringify({ id: info.id, duration: duration, cuts: cuts, cut: parts.filter(p => p.cut).map(p => [p.a, p.b]),
                                                     in: inMs, out: outMs, kept: kept, keptMs: keptMs, selected: selected, volumes: volumes,
                                                     playing: player.playbackState === MediaPlayer.PlayingState, position: headMs,
                                                     progress: progress, error: error, estimate: Math.round(estimate), thumbs: thumbs.length,
                                                     tracks: info.tracks || 0, gif: gif, exportOpen: exportOpen, zoom: zoom,
                                                     section: section, bg: bgLive, outer: outer,
                                                     result: (p => [p.x, p.y, result.width, result.height])(result.mapToItem(contentItem, 0, 0))
                                                   }).slice(1, -1))
            break
        }
    }
}
