import QtQuick
import QtQuick.Window
import QtMultimedia
import Snip.Core
import "probe.js" as Probe

// Quick Look (D51), CleanShot's: a recording (or a GIF) plays in a small
// window, from its card's eye button or Space. It floats, centred, at the
// recording's size: at most 60 % of the screen, at least 480 px wide. Space
// or a click plays and pauses, ←/→ go 5 s back or on (a GIF: a frame), Esc
// closes it. The controls show while the pointer is over it, or it's paused.
Window {
    id: ql
    property var card: null                   // the card's JSON
    property bool muted: false
    readonly property bool gif: !!card && card.media === "gif"

    title: "Snip Quick Look"
    color: "black"
    minimumWidth: 320
    minimumHeight: 180
    visible: false

    // the recording's size in points, fitted to its screen
    function fit(c) {
        const scr = Platform.screen(c.output || "")
        const sw = scr ? scr.geometry.width : 1920, sh = scr ? scr.geometry.height : 1080
        const iw = Math.max(1, c.width / (c.scale || 1)), ih = Math.max(1, c.height / (c.scale || 1))
        const k = Math.min(sw * 0.6 / iw, sh * 0.6 / ih, Math.max(1, 480 / iw))
        return Qt.size(Math.round(iw * k), Math.round(ih * k))
    }
    // the eye button or Space: show this one, or close it if it's showing
    function toggle(c) {
        if (visible && card && card.id === c.id) return dismiss()
        const s = fit(c)
        // a floating window can't resize itself: Hyprland does it
        if (visible) Engine.resizeWindow(title, s.width, s.height)
        else { width = s.width; height = s.height }
        card = c
        visible = true
        raise()
        requestActivate()
        if (gif) anim.playing = true
        else { player.position = 0; player.play() }
    }
    function dismiss() {
        player.stop()
        visible = false
        card = null
    }
    onClosing: { player.stop(); card = null }

    // ---------------------------------------------------------------- playing
    MediaPlayer {
        id: player
        source: ql.card && !ql.gif ? "file://" + ql.card.path : ""
        videoOutput: video
        audioOutput: AudioOutput { muted: ql.muted }
    }
    readonly property bool playing: gif ? anim.playing : player.playbackState === MediaPlayer.PlayingState
    // how far along, 0…1
    readonly property real frac: gif ? (anim.frameCount > 1 ? anim.currentFrame / (anim.frameCount - 1) : 0)
                                     : (player.duration > 0 ? player.position / player.duration : 0)
    function playPause() {
        if (gif) { anim.playing = !anim.playing; return }
        if (playing) return player.pause()
        // at the end: from the start again
        if (player.duration > 0 && player.position >= player.duration - 100) player.position = 0
        player.play()
    }
    function seekTo(f) {
        f = Math.max(0, Math.min(1, f))
        if (gif) anim.currentFrame = Math.round(f * Math.max(0, anim.frameCount - 1))
        else player.position = Math.round(f * player.duration)
    }
    function step(dir) {
        if (!gif) { player.position = Math.max(0, Math.min(player.duration, player.position + dir * 5000)); return }
        anim.playing = false
        anim.currentFrame = (anim.currentFrame + dir + anim.frameCount) % Math.max(1, anim.frameCount)
    }
    function clock(ms) {
        const t = Math.floor(Math.max(0, ms) / 1000)
        return Math.floor(t / 60) + ":" + String(t % 60).padStart(2, "0")
    }

    VideoOutput {
        id: video
        objectName: "quicklook-video"
        visible: !ql.gif
        anchors.fill: parent
        fillMode: VideoOutput.PreserveAspectFit
    }
    AnimatedImage {
        id: anim
        visible: ql.gif
        anchors.fill: parent
        source: ql.card && ql.gif ? "file://" + ql.card.path : ""
        fillMode: Image.PreserveAspectFit
        // by default every frame stays decoded
        cache: false
    }
    MouseArea { anchors.fill: parent; onClicked: ql.playPause() }
    HoverHandler { id: hover }
    Item {
        id: keyboard
        focus: true
        Keys.onPressed: (e) => {
            if (e.key === Qt.Key_Space) ql.playPause()
            else if (e.key === Qt.Key_Escape) ql.dismiss()
            else if (e.key === Qt.Key_Left) ql.step(-1)
            else if (e.key === Qt.Key_Right) ql.step(1)
            else return
            e.accepted = true
        }
    }

    // with no title bar, a close button: top left, shown with the controls
    Rectangle {
        objectName: "quicklook-close"
        x: 10
        y: 10
        width: 28
        height: 28
        radius: 14
        color: closeArea.containsMouse ? Qt.rgba(0.2, 0.2, 0.22, 0.9) : Qt.rgba(0.08, 0.08, 0.09, 0.78)
        opacity: hover.hovered || !ql.playing ? 1 : 0
        visible: opacity > 0.01
        Behavior on opacity { NumberAnimation { duration: 160 } }
        Icon { anchors.centerIn: parent; name: "x"; size: 14; stroke: 2; color: "white" }
        MouseArea { id: closeArea; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: ql.dismiss() }
    }

    // ---------------------------------------------------------------- the controls
    Rectangle {
        objectName: "quicklook-controls"
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom; margins: 10 }
        height: 36
        radius: 10
        color: Qt.rgba(0.08, 0.08, 0.09, 0.78)
        opacity: hover.hovered || !ql.playing ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 160 } }
        MouseArea { anchors.fill: parent }        // a click here isn't one on the video
        Rectangle {
            id: playButton
            objectName: "quicklook-play"
            x: 4
            anchors.verticalCenter: parent.verticalCenter
            width: 28
            height: 28
            radius: 8
            color: playHover.hovered ? Qt.rgba(1, 1, 1, 0.14) : "transparent"
            Icon { anchors.centerIn: parent; name: ql.playing ? "pause" : "play"; size: 14; stroke: 1.5; filled: true; color: "white" }
            HoverHandler { id: playHover; cursorShape: Qt.PointingHandCursor }
            TapHandler { onTapped: ql.playPause() }
        }
        Text {
            id: time
            visible: !ql.gif
            anchors { left: playButton.right; leftMargin: 6; verticalCenter: parent.verticalCenter }
            text: ql.clock(player.position) + " / " + ql.clock(player.duration)
            color: "white"
            font.family: Theme.fontUi
            font.pixelSize: 12
            font.features: { "tnum": 1 }
        }
        // where it is, and a press or drag goes there
        Item {
            objectName: "quicklook-scrubber"
            anchors { left: ql.gif ? playButton.right : time.right; leftMargin: 12; right: muteButton.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
            height: 20
            Rectangle { anchors.verticalCenter: parent.verticalCenter; width: parent.width; height: 4; radius: 2; color: Qt.rgba(1, 1, 1, 0.25) }
            Rectangle { anchors.verticalCenter: parent.verticalCenter; width: ql.frac * parent.width; height: 4; radius: 2; color: "white" }
            Rectangle { x: ql.frac * parent.width - width / 2; anchors.verticalCenter: parent.verticalCenter; width: 12; height: 12; radius: 6; color: "white" }
            MouseArea {
                anchors.fill: parent
                preventStealing: true
                onPressed: (m) => ql.seekTo(m.x / width)
                onPositionChanged: (m) => { if (pressed) ql.seekTo(m.x / width) }
            }
        }
        Rectangle {
            id: muteButton
            objectName: "quicklook-mute"
            visible: !ql.gif
            anchors { right: parent.right; rightMargin: 4; verticalCenter: parent.verticalCenter }
            width: visible ? 28 : 0
            height: 28
            radius: 8
            color: muteHover.hovered ? Qt.rgba(1, 1, 1, 0.14) : "transparent"
            Icon { anchors.centerIn: parent; name: ql.muted ? "volume-x" : "volume"; size: 15; color: "white" }
            HoverHandler { id: muteHover; cursorShape: Qt.PointingHandCursor }
            TapHandler { onTapped: ql.muted = !ql.muted }
        }
    }

    // tests
    function debug(a) {
        switch (a[0]) {
        case "keys": Platform.sendKeys(ql, a.slice(1).join(" ")); break
        case "where": Engine.log("quicklook_where", JSON.stringify(Probe.where(ql, a[1])).slice(1, -1)); break
        case "state":
            Engine.log("quicklook_state", JSON.stringify({ open: visible, id: card ? card.id : "", gif: gif, playing: playing, frac: frac,
                                                         position: gif ? anim.currentFrame : player.position, muted: muted }).slice(1, -1))
            break
        }
    }
}
