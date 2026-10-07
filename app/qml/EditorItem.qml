import QtQuick
import QtQuick.Shapes
import QtQuick.Effects

// One editor object (editor.js), drawn in image pixels. The same item draws
// the selection halo (`halo`), a pale outline `pad` px around the shape,
// outside the exported scene.
Item {
    id: root
    property var o: null
    property int number: 1            // counters: their place in the document
    property bool halo: false
    property real pad: 4
    property string base: ""          // the image file, for Pixelate / Blur tiles
    readonly property color haloColor: Qt.rgba(0.04, 0.47, 1, 0.28)
    readonly property string kind: o ? o.kind : ""
    // the rendered text box, for hit-testing (image px)
    readonly property size box: textLoader.item ? Qt.size(textLoader.item.implicitWidth, textLoader.item.implicitHeight) : Qt.size(0, 0)

    Loader {
        active: root.kind === "arrow"
        sourceComponent: Shape {
            id: arrow
            readonly property var a: root.o
            readonly property real w: a.size + (root.halo ? root.pad * 2 : 0)
            // the head follows the curve's direction at its end
            readonly property real ang: {
                const dx = a.x2 - a.cx, dy = a.y2 - a.cy
                return Math.hypot(dx, dy) > 0.5 ? Math.atan2(dy, dx) : Math.atan2(a.y2 - a.y1, a.x2 - a.x1)
            }
            readonly property real len: a.size * 3.2 + 4
            readonly property real half: len * 0.62
            readonly property real bx: a.x2 - Math.cos(ang) * len * 0.8
            readonly property real by: a.y2 - Math.sin(ang) * len * 0.8
            readonly property color ink: root.halo ? root.haloColor : a.color
            width: 1; height: 1
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeColor: arrow.ink; strokeWidth: arrow.w; fillColor: "transparent"
                capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin
                startX: arrow.a.x1; startY: arrow.a.y1
                PathQuad { x: arrow.bx; y: arrow.by; controlX: arrow.a.cx; controlY: arrow.a.cy }
            }
            ShapePath {
                // the head, its corners softened by a round-joined stroke
                strokeColor: arrow.ink; strokeWidth: root.halo ? root.pad * 2 + arrow.a.size * 0.5 : arrow.a.size * 0.5
                fillColor: arrow.ink; joinStyle: ShapePath.RoundJoin
                startX: arrow.a.x2; startY: arrow.a.y2
                PathLine { x: arrow.a.x2 - Math.cos(arrow.ang) * arrow.len + Math.sin(arrow.ang) * arrow.half
                           y: arrow.a.y2 - Math.sin(arrow.ang) * arrow.len - Math.cos(arrow.ang) * arrow.half }
                PathLine { x: arrow.a.x2 - Math.cos(arrow.ang) * arrow.len - Math.sin(arrow.ang) * arrow.half
                           y: arrow.a.y2 - Math.sin(arrow.ang) * arrow.len + Math.cos(arrow.ang) * arrow.half }
                PathLine { x: arrow.a.x2; y: arrow.a.y2 }
            }
        }
    }

    Loader {
        active: root.kind === "rect"
        sourceComponent: Rectangle {
            readonly property var r: root.o
            readonly property real grow: root.halo ? root.pad : 0
            x: r.x - grow; y: r.y - grow
            width: r.w + grow * 2; height: r.h + grow * 2
            radius: (r.filled ? 6 : r.size * 1.2) + grow
            color: r.filled ? (root.halo ? "transparent" : r.color) : "transparent"
            border.width: r.filled ? (root.halo ? grow * 2 : 0) : r.size + grow * 2
            border.color: root.halo ? root.haloColor : r.color
        }
    }

    Loader {
        active: root.kind === "ellipse"
        sourceComponent: Shape {
            id: ellipse
            readonly property var e: root.o
            width: 1; height: 1
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeColor: root.halo ? root.haloColor : ellipse.e.color
                strokeWidth: ellipse.e.size + (root.halo ? root.pad * 2 : 0)
                fillColor: "transparent"
                PathAngleArc {
                    // the stroke sits inside the box, like the rectangle's border
                    centerX: ellipse.e.x + ellipse.e.w / 2; centerY: ellipse.e.y + ellipse.e.h / 2
                    radiusX: Math.max(0.5, ellipse.e.w / 2 - ellipse.e.size / 2)
                    radiusY: Math.max(0.5, ellipse.e.h / 2 - ellipse.e.size / 2)
                    startAngle: 0; sweepAngle: 360
                }
            }
        }
    }

    Loader {
        active: root.kind === "line"
        sourceComponent: Shape {
            id: line
            readonly property var l: root.o
            width: 1; height: 1
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeColor: root.halo ? root.haloColor : line.l.color
                strokeWidth: line.l.size + (root.halo ? root.pad * 2 : 0)
                fillColor: "transparent"
                capStyle: ShapePath.RoundCap
                startX: line.l.x1; startY: line.l.y1
                PathLine { x: line.l.x2; y: line.l.y2 }
            }
        }
    }

    Loader {
        id: textLoader
        active: root.kind === "text"
        sourceComponent: Item {
            readonly property var t: root.o
            implicitWidth: label.implicitWidth
            implicitHeight: label.implicitHeight
            x: t.x; y: t.y
            Rectangle {
                visible: root.halo
                x: -root.pad * 2; y: -root.pad
                width: label.implicitWidth + root.pad * 4; height: label.implicitHeight + root.pad * 2
                radius: root.pad * 1.5
                color: "transparent"
                border.width: Math.max(1, root.pad / 2)
                border.color: Qt.rgba(0.04, 0.47, 1, 0.8)
            }
            Text {
                id: label
                visible: !root.halo
                text: parent.t.text
                color: parent.t.color
                font.family: Theme.fontUi
                font.pixelSize: parent.t.size
                font.weight: Font.Bold
                textFormat: Text.PlainText
            }
        }
    }

    Loader {
        active: root.kind === "counter"
        sourceComponent: Item {
            readonly property var c: root.o
            readonly property real d: c.size + (root.halo ? root.pad * 2 : 0)
            x: c.x - d / 2; y: c.y - d / 2
            width: d; height: d
            RectangularShadow {
                visible: !root.halo
                anchors.fill: disc
                radius: disc.radius
                blur: parent.c.size * 0.22
                offset.y: parent.c.size * 0.06
                color: Qt.rgba(0, 0, 0, 0.28)
            }
            Rectangle {
                id: disc
                anchors.fill: parent
                radius: width / 2
                color: root.halo ? root.haloColor : parent.c.color
            }
            Text {
                visible: !root.halo
                anchors.centerIn: parent
                text: root.number
                color: Qt.colorEqual(parent.c.color, "#ffffff") || Qt.colorEqual(parent.c.color, "#ffcc00") ? "#1d1d1f" : "white"
                font.family: Theme.fontUi
                font.pixelSize: parent.c.size * 0.56
                font.weight: Font.Bold
                font.features: { "tnum": 1 }
            }
        }
    }

    Loader {
        active: root.kind === "redact"
        sourceComponent: Item {
            readonly property var r: root.o
            readonly property real grow: root.halo ? root.pad : 0
            x: r.x - grow; y: r.y - grow
            width: r.w + grow * 2; height: r.h + grow * 2
            Rectangle {
                visible: root.halo
                anchors.fill: parent
                radius: root.pad
                color: "transparent"
                border.width: root.pad * 2
                border.color: root.haloColor
            }
            Rectangle {
                visible: !root.halo && parent.r.style === "black"
                anchors.fill: parent
                color: "black"
            }
            // secure tiles: built from the pixels around the region only
            Image {
                visible: !root.halo && parent.r.style !== "black"
                anchors.fill: parent
                source: visible && root.base !== ""
                        ? "image://snip-redact/" + (parent.r.style === "blur" ? 1 : 0) + "/" + parent.r.seed + "/"
                          + [parent.r.x, parent.r.y, parent.r.w, parent.r.h].map(Math.round).join(",") + "/" + encodeURIComponent(root.base)
                        : ""
                smooth: parent.r.style === "blur"
                cache: false
                fillMode: Image.Stretch
            }
        }
    }
}
