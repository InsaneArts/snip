import QtQuick
import QtQuick.Effects
import QtMultimedia
import Snip.Core

// Studio Mode (D55): one frame of a recording as its export draws it, the
// size of its canvas (the background's rect, or the recording itself). The
// background stays still; the recording's card (the video, its corners,
// inset and shadow) is drawn where the zoom evaluator puts it at `ms`
// (snip_core::studio; D56: the whole card grows). The video editor shows it
// fitted in its stage.
Item {
    id: frame
    property var bg: null                                  // the background, or null
    property var outer: ({ x: 0, y: 0, w: 1, h: 1 })     // the canvas, in the recording's px
    property var inner: ({ x: 0, y: 0, w: 1, h: 1 })     // the recording, unzoomed
    property var studio: ({})                              // the edit's zooms and their zoom_style
    property real ms: 0                                    // the time of the frame shown
    property real pixelScale: 1                            // device px per px here, for the corners' mask
    readonly property alias video: video

    // where the card is now, in the recording's px
    readonly property rect card: {
        const r = JSON.parse(Engine.studioCard(JSON.stringify(studio), ms, outer.w, outer.h, inner.x - outer.x, inner.y - outer.y, inner.w, inner.h))
        return Qt.rect(outer.x + r[0], outer.y + r[1], r[2], r[3])
    }
    readonly property real zoom: card.width / Math.max(1, inner.w)
    // its corners, inset and shadow grow with it
    readonly property var cardBg: bg ? Object.assign({}, bg, { radius: bg.radius * zoom, inset: bg.inset * zoom }) : null

    width: outer.w
    height: outer.h
    clip: true
    // the recording's px, its corner at 0, 0
    Item {
        x: -frame.outer.x
        y: -frame.outer.y
        BackgroundLayer { visible: frame.bg !== null; bg: frame.cardBg; rect: frame.outer; inner: ({ x: frame.card.x, y: frame.card.y, w: frame.card.width, h: frame.card.height }) }
        VideoOutput {
            id: video
            objectName: "video-preview"
            x: frame.card.x
            y: frame.card.y
            width: frame.card.width
            height: frame.card.height
            // its corners rounded, at the size it shows
            readonly property size shown: Qt.size(Math.ceil(width * frame.pixelScale), Math.ceil(height * frame.pixelScale))
            layer.enabled: !!frame.cardBg && frame.cardBg.radius > 0
            layer.textureSize: shown
            layer.effect: MultiEffect { maskEnabled: true; maskSource: videoMask; maskThresholdMin: 0.5; maskSpreadAtMin: 1.0 }
        }
        Item {
            id: videoMask
            visible: false
            width: video.width
            height: video.height
            layer.enabled: true
            layer.textureSize: video.shown
            Rectangle { anchors.fill: parent; radius: frame.cardBg ? frame.cardBg.radius : 0; color: "white" }
        }
    }
}
