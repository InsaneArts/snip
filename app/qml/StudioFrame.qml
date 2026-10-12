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
    property string log: ""                                // the take's input log: what follow zooms follow
    property real ms: 0                                    // the time of the frame shown
    property real pixelScale: 1                            // device px per px here, for the corners' mask
    // a studio take's pointer, redrawn (D55): {log, theme, size, scale}, or null;
    // and how the edit draws it (its "pointer": D60's defaults where unset)
    property var pointer: null
    property var pointerLook: ({})
    // a studio take's clicks and keys, drawn after the fact: how ({show, color,
    // filled}; {show, size, look, position}), or null; and the recording's px per point
    property var rippleLook: null
    property var keysLook: null
    property real unit: 1
    // the parts of the recording kept (ms, joined where they meet): clicks and
    // keys from before the part shown, and cut out, end where it begins
    property var keep: []
    readonly property real partFrom: {
        let at = -1e12
        for (const p of keep) if (p[0] <= ms) at = p[0]
        return at
    }
    readonly property alias video: video
    readonly property var cursorImage: pointer ? JSON.parse(Engine.studioCursor(pointer.theme)) : null
    readonly property var pointerNow: pointer && cursorImage ? JSON.parse(Engine.studioPointer(pointer.log, JSON.stringify(pointerLook), ms, inner.w, inner.h)) : null
    readonly property var inputNow: log && ((rippleLook && rippleLook.show) || (keysLook && keysLook.show)) ? JSON.parse(Engine.studioInput(log, ms, partFrom, inner.w, inner.h)) : null

    // where the card is now, in the recording's px
    readonly property rect card: {
        const r = JSON.parse(Engine.studioCard(JSON.stringify(studio), log, ms, outer.w, outer.h, inner.x - outer.x, inner.y - outer.y, inner.w, inner.h))
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
        // the click ripples, in the card under the pointer (they grow with a zoom)
        Item {
            x: frame.card.x
            y: frame.card.y
            width: frame.card.width
            height: frame.card.height
            clip: true
            visible: !!frame.rippleLook && frame.rippleLook.show && frame.inputNow !== null
            Repeater {
                model: frame.inputNow && frame.rippleLook && frame.rippleLook.show ? frame.inputNow.rings : []
                Rectangle {
                    required property var modelData
                    readonly property real across: modelData.size * frame.zoom
                    readonly property color tint: frame.rippleLook.color === "accent" ? (modelData.right ? Theme.warning : Theme.accent) : frame.rippleLook.color
                    x: modelData.x * frame.zoom - across / 2
                    y: modelData.y * frame.zoom - across / 2
                    width: across
                    height: across
                    radius: across / 2
                    opacity: modelData.alpha
                    color: frame.rippleLook.filled ? Qt.rgba(tint.r, tint.g, tint.b, 0.3) : "transparent"
                    border.width: frame.rippleLook.filled ? 0 : 3 * frame.unit * frame.zoom
                    border.color: tint
                }
            }
        }
        // the pointer, in the card (it grows with a zoom; never drawn outside it)
        Item {
            x: frame.card.x
            y: frame.card.y
            width: frame.card.width
            height: frame.card.height
            clip: true
            visible: frame.pointerNow !== null && frame.pointerNow.opacity > 0 && frame.pointerLook.show !== false
            Image {
                readonly property var img: frame.cursorImage || ({ w: 0, h: 0, hot: [0, 0], nominal: 1 })
                readonly property var now: frame.pointerNow || ({ x: 0, y: 0, opacity: 0, press: 0 })
                // the theme's size, in the recording's px, times the edit's size and the zoom; smaller while pressed
                readonly property real k: (frame.pointer ? frame.pointer.size * frame.pointer.scale : 30) * (frame.pointerLook.size || 2) / img.nominal
                                          * frame.zoom * (1 - 0.2 * now.press)
                source: frame.cursorImage ? "file://" + frame.cursorImage.path : ""
                width: img.w * k
                height: img.h * k
                x: now.x * frame.zoom - img.hot[0] * k
                y: now.y * frame.zoom - img.hot[1] * k
                opacity: now.opacity
                smooth: true
                mipmap: true
            }
        }
    }
    // the keystrokes, on the canvas: they don't zoom (the live HUD's pill, sized
    // as it is for the recording, times the edit's size)
    Rectangle {
        id: keyPill
        objectName: "studio-keystrokes"
        readonly property var now: frame.inputNow ? frame.inputNow.keys : null
        readonly property var look: frame.keysLook || ({})
        readonly property real k: (look.size || 1) * frame.unit
        readonly property real margin: Math.min(48 * frame.unit, frame.outer.h * 0.08)
        readonly property string at: look.position || "bottom-center"
        readonly property bool dark: look.look !== "light"
        visible: !!look.show && now !== null
        opacity: now ? now.opacity : 0
        x: at.endsWith("left") ? margin : at.endsWith("right") ? frame.outer.w - width - margin : (frame.outer.w - width) / 2
        y: at.startsWith("top") ? margin : at.startsWith("bottom") ? frame.outer.h - height - margin : (frame.outer.h - height) / 2
        width: keyText.implicitWidth + 32 * k
        height: keyText.implicitHeight + 16 * k
        radius: 12 * k
        color: dark ? Qt.rgba(0, 0, 0, 0.72) : Qt.rgba(1, 1, 1, 0.88)
        Text {
            id: keyText
            anchors.centerIn: parent
            text: keyPill.now ? keyPill.now.text : ""
            color: keyPill.dark ? "white" : "black"
            font.family: Theme.fontUi
            font.pixelSize: Math.round(Math.max(14, Math.min(36, frame.inner.h / Math.max(0.1, frame.unit) * 0.04)) * keyPill.k)
            font.weight: Font.DemiBold
        }
    }
}
