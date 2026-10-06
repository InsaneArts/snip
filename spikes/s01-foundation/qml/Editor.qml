import QtQuick
import QtQuick.Shapes
import QtQuick.Window
import com.snip.spike

// S8: annotation canvas. Shown at 100 % (1 image px = 1 device px), exported
// with grabToImage at native size, and compared with an on-screen capture.
Window {
    id: ed
    title: "snip-editor"
    flags: Qt.FramelessWindowHint
    color: "#101010"

    property string imagePath: ""
    property int extraObjects: 0
    property bool animate: false
    readonly property real dpr: Screen.devicePixelRatio > 0 ? Screen.devicePixelRatio : 1
    // logical size such that the 1600x900 test image maps 1:1 to device pixels at 1.25
    width: 1280
    height: 720
    property real effectiveDpr: 1.25

    Item {
        id: canvas
        anchors.fill: parent

        Image { id: img; anchors.fill: parent; source: ed.imagePath ? "file://" + ed.imagePath : ""; smooth: false }

        // pixelate (redact): low-res texture scaled up with nearest sampling
        ShaderEffectSource {
            x: 620 / ed.effectiveDpr; y: 520 / ed.effectiveDpr; width: 320 / ed.effectiveDpr; height: 120 / ed.effectiveDpr
            sourceItem: img
            sourceRect: Qt.rect(x, y, width, height)
            textureSize: Qt.size(20, 8)
            smooth: false
        }

        // highlighter
        Rectangle { x: 120 / ed.effectiveDpr; y: 300 / ed.effectiveDpr; width: 420 / ed.effectiveDpr; height: 46 / ed.effectiveDpr; color: "#66ffd60a"; radius: 4 }

        // rectangles
        Rectangle { x: 80 / ed.effectiveDpr; y: 80 / ed.effectiveDpr; width: 520 / ed.effectiveDpr; height: 260 / ed.effectiveDpr
                    color: "transparent"; border.color: "#ff3b30"; border.width: 6 / ed.effectiveDpr; radius: 8 / ed.effectiveDpr }
        Rectangle { x: 1000 / ed.effectiveDpr; y: 600 / ed.effectiveDpr; width: 300 / ed.effectiveDpr; height: 160 / ed.effectiveDpr
                    color: "#ff3b30"; radius: 6 / ed.effectiveDpr }

        // ellipse
        Shape {
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeColor: "#0a84ff"; strokeWidth: 6 / ed.effectiveDpr; fillColor: "transparent"
                PathAngleArc { centerX: 1250 / ed.effectiveDpr; centerY: 250 / ed.effectiveDpr; radiusX: 180 / ed.effectiveDpr; radiusY: 110 / ed.effectiveDpr; sweepAngle: 360 }
            }
        }

        // arrows (straight + curved), CleanShot-like heads
        component Arrow: Shape {
            id: arrow
            property point a; property point b; property point c: Qt.point((a.x + b.x) / 2, (a.y + b.y) / 2)
            property color color: "#ff3b30"
            property real w: 7 / ed.effectiveDpr
            readonly property real ang: Math.atan2(b.y - c.y, b.x - c.x)
            readonly property real head: w * 4.2
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeColor: arrow.color; strokeWidth: arrow.w; fillColor: "transparent"; capStyle: ShapePath.RoundCap
                startX: arrow.a.x; startY: arrow.a.y
                PathQuad { x: arrow.b.x - Math.cos(arrow.ang) * arrow.head * 0.6; y: arrow.b.y - Math.sin(arrow.ang) * arrow.head * 0.6
                           controlX: arrow.c.x; controlY: arrow.c.y }
            }
            ShapePath {
                strokeColor: "transparent"; fillColor: arrow.color
                startX: arrow.b.x; startY: arrow.b.y
                PathLine { x: arrow.b.x - Math.cos(arrow.ang - 0.42) * arrow.head; y: arrow.b.y - Math.sin(arrow.ang - 0.42) * arrow.head }
                PathLine { x: arrow.b.x - Math.cos(arrow.ang + 0.42) * arrow.head; y: arrow.b.y - Math.sin(arrow.ang + 0.42) * arrow.head }
                PathLine { x: arrow.b.x; y: arrow.b.y }
            }
        }
        Arrow { a: Qt.point(700 / ed.effectiveDpr, 120 / ed.effectiveDpr); b: Qt.point(560 / ed.effectiveDpr, 300 / ed.effectiveDpr) }
        Arrow { id: liveArrow; a: Qt.point(300 / ed.effectiveDpr, 820 / ed.effectiveDpr); b: Qt.point(780 / ed.effectiveDpr, 640 / ed.effectiveDpr)
                c: Qt.point(420 / ed.effectiveDpr, 560 / ed.effectiveDpr); color: "#30d158" }

        // text: outline style and box style
        Text { x: 150 / ed.effectiveDpr; y: 380 / ed.effectiveDpr; text: "Click here first"; color: "white"
               style: Text.Outline; styleColor: "#000000"; font.pixelSize: 40 / ed.effectiveDpr; font.weight: Font.Bold }
        Rectangle {
            x: 900 / ed.effectiveDpr; y: 420 / ed.effectiveDpr; radius: 8 / ed.effectiveDpr; color: "#0a84ff"
            width: boxText.implicitWidth + 28 / ed.effectiveDpr; height: boxText.implicitHeight + 14 / ed.effectiveDpr
            Text { id: boxText; anchors.centerIn: parent; text: "Snip ✓ ქართული"; color: "white"; font.pixelSize: 30 / ed.effectiveDpr; font.weight: Font.DemiBold }
        }

        // counters
        Repeater {
            model: 3
            delegate: Rectangle {
                required property int index
                x: (180 + index * 90) / ed.effectiveDpr; y: 700 / ed.effectiveDpr
                width: 56 / ed.effectiveDpr; height: width; radius: width / 2; color: "#ff3b30"
                border.color: "white"; border.width: 3 / ed.effectiveDpr
                Text { anchors.centerIn: parent; text: index + 1; color: "white"; font.pixelSize: 28 / ed.effectiveDpr; font.weight: Font.Bold }
            }
        }

        // perf: extra objects
        Repeater {
            model: ed.extraObjects
            delegate: Arrow {
                required property int index
                a: Qt.point((37 * index) % 1500 / ed.effectiveDpr, (53 * index) % 860 / ed.effectiveDpr)
                b: Qt.point(((37 * index) % 1500 + 90) / ed.effectiveDpr, ((53 * index) % 860 + 40) / ed.effectiveDpr)
                color: index % 2 ? "#ff9f0a" : "#bf5af2"
            }
        }
    }

    // perf: drag the live arrow's head every frame, measure frame times
    property int frames: 0
    property real t: 0
    FrameAnimation {
        running: ed.animate
        onTriggered: {
            ed.t += frameTime
            liveArrow.b = Qt.point((780 + 260 * Math.sin(ed.t * 2.3)) / ed.effectiveDpr, (640 - 180 * Math.cos(ed.t * 1.7)) / ed.effectiveDpr)
        }
    }
    onFrameSwapped: {
        if (!ed.animate) return
        ed.frames += 1
        SnipCore.log("editor_frame", "")
    }

    function exportPng(path) {
        canvas.grabToImage(function (r) {
            const ok = r.saveToFile(path)
            SnipCore.log("export_done", "\"ok\":" + ok + ",\"w\":" + r.image.width + ",\"h\":" + r.image.height)
        }, Qt.size(canvas.width, canvas.height))   // logical size: grabToImage applies the window DPR -> 1600x900
    }
}
