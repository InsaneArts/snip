import QtQuick
import QtQuick.Window
import QtQuick.Effects
import org.kde.layershell as LayerShell
import Snip.Core
import "editor.js" as Ed

// BG-03 "Automatically apply preset to all screenshots": a new screenshot
// drawn with the preset's background exactly as the editor exports it, then
// committed as an edit (doc {bg}), so the background stays editable. A grab
// needs a mapped window: a 1×1 layer behind everything that takes no input;
// the scene sits outside it, so nothing shows.
Window {
    id: r
    required property var capture              // a card's JSON, its output's scale, bg in points
    required property int token
    signal done()

    flags: Qt.FramelessWindowHint
    color: "transparent"
    width: 1; height: 1
    LayerShell.Window.screen: Platform.screen(capture.output || "")
    LayerShell.Window.scope: "snip-render"
    LayerShell.Window.layer: LayerShell.Window.LayerBackground
    LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorLeft
    LayerShell.Window.exclusionZone: -1
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone

    // the preset in this capture's pixels (as Editor.bgPixels does)
    readonly property real unit: capture.scale || 1
    readonly property var bg: {
        const o = Object.assign(Ed.defaultBackground(unit), capture.bg)
        for (const k of ["padding", "inset", "radius"]) o[k] = Math.round(o[k] * unit)
        return o
    }
    readonly property var full: ({ x: 0, y: 0, w: capture.width, h: capture.height })
    readonly property var balance: bg.balance ? JSON.parse(Engine.contentMargins(capture.path, 0, 0, capture.width, capture.height)) : null
    readonly property var inner: {
        const b = balance
        if (!b) return full
        const m = Math.max(b.l, b.t, b.r, b.b)
        return { x: -(m - b.l), y: -(m - b.t), w: full.w + 2 * m - b.l - b.r, h: full.h + 2 * m - b.t - b.b }
    }
    readonly property var rect: Ed.bgRect(bg, inner)

    Item {
        id: scene
        x: 2                                   // outside the 1×1 window
        Item {
            x: -r.rect.x; y: -r.rect.y
            BackgroundLayer {
                bg: r.bg
                rect: r.rect
                inner: r.inner
                frameColor: r.balance ? r.balance.color : ""
                imagePath: r.capture.path
            }
            Item {
                id: frame
                width: r.capture.width; height: r.capture.height
                layer.enabled: true
                layer.textureSize: Qt.size(r.capture.width, r.capture.height)
                layer.effect: MultiEffect { maskEnabled: true; maskSource: mask; maskThresholdMin: 0.5; maskSpreadAtMin: 1.0 }
                Image {
                    id: picture
                    anchors.fill: parent
                    source: "file://" + r.capture.path
                    cache: false
                    smooth: true; mipmap: true
                    fillMode: Image.Stretch
                    onStatusChanged: if (status === Image.Ready) go.start()
                }
            }
            Item {
                id: mask
                visible: false
                width: r.capture.width; height: r.capture.height
                layer.enabled: true
                layer.textureSize: Qt.size(r.capture.width, r.capture.height)
                Rectangle { anchors.fill: parent; radius: r.bg.radius; color: "white" }
            }
        }
    }

    Component.onCompleted: {
        visible = true
        Platform.setInputRects(r, [])
    }
    Timer { id: go; interval: 60; onTriggered: Platform.grabExact(scene, Math.round(r.rect.w), Math.round(r.rect.h), r.token) }
    Connections {
        target: Platform
        function onGrabbed(token, ok) {
            if (token !== r.token) return
            Engine.editorCommit(r.capture.id, ok ? r.token : -1,
                                JSON.stringify({ v: 1, items: [], crop: null, counterStart: 1, bg: r.bg }), "auto-bg")
            r.visible = false
            r.done()
        }
    }
}
