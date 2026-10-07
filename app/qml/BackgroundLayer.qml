import QtQuick
import QtQuick.Shapes
import QtQuick.Effects
import "editor.js" as Ed

// The Background tool's backdrop (BG-01…06): a gradient, a wallpaper, a
// blurred wallpaper or screenshot, or a colour, around the image; with the
// inset (and Auto-balance's border) and shadow between them. Drawn behind
// the image, in image px.
Item {
    id: layer
    property var bg: null
    property var rect: ({ x: 0, y: 0, w: 0, h: 0 })     // the backdrop
    property var inner: ({ x: 0, y: 0, w: 0, h: 0 })    // the image (or its crop)
    property string imagePath: ""
    property string frameColor: ""                      // Auto-balance's border colour, for the inset frame
    readonly property string kind: bg ? bg.kind : ""

    function resolve(c) {
        if (c === "theme-a") return Theme.accent
        if (c === "theme-b") return Theme.signature.length > 1 ? Theme.signature[1] : Theme.background
        return c
    }
    readonly property var stops: kind === "gradient" ? Ed.gradients[bg.value % Ed.gradients.length].map(resolve) : ["black", "black"]

    Item {
        id: backdrop
        x: layer.rect.x; y: layer.rect.y
        width: layer.rect.w; height: layer.rect.h
        clip: true

        Shape {
            visible: layer.kind === "gradient"
            anchors.fill: parent
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeColor: "transparent"
                fillGradient: LinearGradient {
                    x1: 0; y1: 0; x2: backdrop.width; y2: backdrop.height
                    GradientStop { position: 0; color: layer.stops[0] }
                    GradientStop { position: 0.5; color: layer.stops.length > 2 ? layer.stops[1] : Qt.tint(layer.stops[0], Qt.alpha(layer.stops[1], 0.5)) }
                    GradientStop { position: 1; color: layer.stops[layer.stops.length - 1] }
                }
                PathRectangle { x: 0; y: 0; width: backdrop.width; height: backdrop.height }
            }
        }
        Rectangle {
            visible: layer.kind === "color"
            anchors.fill: parent
            color: layer.kind === "color" ? layer.resolve(layer.bg.value) : "transparent"
        }
        Image {
            id: picture
            visible: layer.kind === "wallpaper"
            anchors.fill: parent
            fillMode: Image.PreserveAspectCrop
            source: layer.kind === "wallpaper" || layer.kind === "blur-wallpaper" ? "file://" + layer.bg.value
                  : layer.kind === "blur-image" ? "file://" + layer.imagePath : ""
            smooth: true
            mipmap: true
        }
        MultiEffect {
            visible: layer.kind === "blur-wallpaper" || layer.kind === "blur-image"
            anchors.fill: parent
            source: picture
            autoPaddingEnabled: false
            blurEnabled: true
            blur: 1.0
            blurMax: 64
            blurMultiplier: 2
        }
    }

    // the shadow, then the inset frame, under the image
    RectangularShadow {
        visible: !!layer.bg && layer.bg.shadow > 0
        x: layer.inner.x - (layer.bg ? layer.bg.inset : 0); y: layer.inner.y - (layer.bg ? layer.bg.inset : 0)
        width: layer.inner.w + 2 * (layer.bg ? layer.bg.inset : 0); height: layer.inner.h + 2 * (layer.bg ? layer.bg.inset : 0)
        radius: layer.bg ? layer.bg.radius + layer.bg.inset : 0
        blur: layer.bg ? Math.max(layer.inner.w, layer.inner.h) * 0.04 * layer.bg.shadow + 8 : 0
        offset.y: layer.bg ? Math.max(layer.inner.w, layer.inner.h) * 0.012 * layer.bg.shadow : 0
        color: Qt.rgba(0, 0, 0, layer.bg ? 0.55 * layer.bg.shadow : 0)
    }
    Rectangle {
        visible: !!layer.bg && (layer.bg.inset > 0 || layer.frameColor !== "")
        x: layer.inner.x - (layer.bg ? layer.bg.inset : 0); y: layer.inner.y - (layer.bg ? layer.bg.inset : 0)
        width: layer.inner.w + 2 * (layer.bg ? layer.bg.inset : 0); height: layer.inner.h + 2 * (layer.bg ? layer.bg.inset : 0)
        radius: layer.bg ? layer.bg.radius + layer.bg.inset : 0
        color: layer.frameColor || (layer.bg && layer.bg.insetColor) || "transparent"
    }
}
