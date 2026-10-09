import QtQuick
import QtQuick.Effects
import QtMultimedia

// The camera in a recording (REC-08), CleanShot's: a circle, square,
// rectangle or vertical bubble inside the recorded area, so it's recorded as
// seen. Whatever it shows is in the video, so it shows nothing when the
// camera fails, is taken by another app, or stops sending frames (unplugged).
//
// Qt opens the camera through V4L2 and decodes MJPEG on the CPU (up to a
// quarter of a core at 1080p), so the bubble asks for the smallest
// uncompressed format that covers it. The camera is opened only while the
// bubble is on screen: an idle Camera with a device keeps it open.
Item {
    id: bubble
    property var device: null              // a cameraDevice
    property bool fake: false              // tests: a card stands in for a camera
    property string shape: "circle"
    property bool mirror: true
    property real pixelScale: 1             // physical px per logical px
    property bool running: false           // on screen: the camera runs
    property int frames: 0
    readonly property bool ready: fake || frames > 0
    property var cam: null

    // the smallest format at 29 fps or more that covers `needPx`, uncompressed
    // before MJPEG (29); none covers it: the largest there is
    function pickFormat(d, needPx) {
        const JPEG = 29
        const edge = f => Math.min(f.resolution.width, f.resolution.height)
        let biggest = null, best = null
        for (const f of d.videoFormats) {
            if (f.maxFrameRate < 29) continue
            if (!biggest || edge(f) > edge(biggest)) biggest = f
            if (edge(f) < needPx) continue
            if (!best || (f.pixelFormat !== JPEG && best.pixelFormat === JPEG)
                    || ((f.pixelFormat === JPEG) === (best.pixelFormat === JPEG) && edge(f) < edge(best)))
                best = f
        }
        return best || biggest
    }
    function start() {
        if (fake || !device || cam) return
        frames = 0
        cam = cameraComponent.createObject(bubble, { cameraDevice: device })
        // after the device: setting one resets the format
        const f = pickFormat(device, Math.ceil(Math.min(width, height) * pixelScale))
        if (f) cam.cameraFormat = f
        session.camera = cam
        cam.active = true
        watchdog.interval = 4000                 // a camera takes a moment to start
        watchdog.restart()
    }
    function stop() {
        watchdog.stop()
        if (!cam) return
        session.camera = null
        cam.active = false
        cam.destroy()
        cam = null
        frames = 0
        output.clearOutput()
    }
    onRunningChanged: running ? start() : stop()
    onDeviceChanged: { stop(); if (running) Qt.callLater(start) }
    Component.onDestruction: stop()

    Component {
        id: cameraComponent
        // "No camera detected", "Camera is in use", "Cannot open camera"
        Camera { onErrorOccurred: (error, errorString) => { console.warn("camera:", errorString); bubble.stop() } }
    }
    CaptureSession { id: session; videoOutput: output }
    // an unplugged camera stops sending frames but stays active, and never
    // streams again (QTBUG-136979): a new Camera, a moment later
    Connections {
        target: output.videoSink
        function onVideoFrameChanged() { bubble.frames++; watchdog.interval = 1500; watchdog.restart() }
    }
    Timer {
        id: watchdog
        interval: 1500
        onTriggered: if (bubble.cam) { bubble.stop(); again.restart() }
    }
    Timer { id: again; interval: 250; onTriggered: if (bubble.running) bubble.start() }

    Item {
        id: feed
        anchors.fill: parent
        visible: bubble.ready
        // drawn through its shape (as a source elsewhere, a hidden layer drew nothing)
        layer.enabled: true
        layer.effect: MultiEffect {
            maskEnabled: true
            maskSource: shapeMask
        }
        VideoOutput {
            id: output
            anchors.fill: parent
            visible: !bubble.fake
            fillMode: VideoOutput.PreserveAspectCrop
            mirrored: bubble.mirror
            endOfStreamPolicy: VideoOutput.KeepLastFrame
        }
        // the tests' card: two colours and a letter, mirrored as a camera is
        Rectangle {
            visible: bubble.fake
            anchors.fill: parent
            gradient: Gradient {
                GradientStop { position: 0; color: "#2f7df6" }
                GradientStop { position: 1; color: "#18c29c" }
            }
            Text {
                id: letter
                anchors.centerIn: parent
                text: "R"
                color: "white"
                font.pixelSize: Math.max(8, parent.height * 0.5)
                font.weight: Font.Bold
                transform: Scale { origin.x: letter.width / 2; xScale: bubble.mirror ? -1 : 1 }
            }
        }
    }
    Item {
        id: shapeMask
        anchors.fill: parent
        visible: false
        layer.enabled: true
        Rectangle {
            anchors.fill: parent
            // "full": filling the recorded area
            radius: bubble.shape === "circle" ? Math.min(width, height) / 2 : bubble.shape === "full" ? 0 : Math.min(width, height) * 0.12
            color: "white"
        }
    }
}
