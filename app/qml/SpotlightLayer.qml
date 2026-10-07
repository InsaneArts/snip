import QtQuick
import QtQuick.Shapes
import QtQuick.Effects
import "editor.js" as Ed

// Spotlight (ANN-07): everything outside the spotlights dimmed. One layer
// for all of them, so overlapping spotlights merge; it sits above the image
// and below the other objects, which stay bright.
Item {
    id: layer
    property var items: []
    readonly property var spots: items.filter(o => o.kind === "spotlight")
    readonly property real dim: spots.length ? spots[spots.length - 1].dim : 0
    visible: spots.length > 0

    Rectangle { id: shade; anchors.fill: parent; color: Qt.rgba(0, 0, 0, layer.dim); visible: false }
    Item {
        id: holes
        anchors.fill: parent
        visible: false
        layer.enabled: true
        Repeater {
            model: layer.spots
            delegate: Shape {
                required property var modelData
                preferredRendererType: Shape.CurveRenderer
                ShapePath {
                    fillColor: "white"; strokeColor: "transparent"
                    PathSvg { path: Ed.spotPath(modelData) }
                }
            }
        }
    }
    MultiEffect {
        anchors.fill: parent
        source: shade
        maskEnabled: true
        maskInverted: true
        maskSource: holes
        maskThresholdMin: 0.5
        maskSpreadAtMin: 0.2
    }
}
