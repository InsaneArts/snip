import QtQuick
import Snip.Core

// The zoom selected (D55), over the timeline above it: its level (100–400 %),
// and where it looks: drag the spotlight on the frame, which snaps to the
// centre lines. The spotlight is what the export shows at the zoom's peak
// (snip_core::studio places it), and the preview follows as it moves.
Rectangle {
    id: pop
    property var ed: null                   // the video editor
    property var source: null               // the video, for the frame
    readonly property var zoom: ed && ed.zoomSelected >= 0 ? ed.studio.zooms[ed.zoomSelected] || null : null
    // the part of the recording it shows at its peak, 0–1: [x0, y0, x1, y1]
    readonly property var shown: {
        if (!zoom) return [0, 0, 1, 1]
        const o = ed.outer, i = ed.inner
        const r = JSON.parse(Engine.studioCard(JSON.stringify({ zooms: [zoom] }), (zoom.from + zoom.to) / 2, o.w, o.h, i.x - o.x, i.y - o.y, i.w, i.h))
        const at = (v, a, n) => Math.max(0, Math.min(1, (v - a) / n))
        return [at(0, r[0], r[2]), at(0, r[1], r[3]), at(o.w, r[0], r[2]), at(o.h, r[1], r[3])]
    }
    function setFocus(fx, fy) { ed.changeZoom(ed.zoomSelected, { focus: [fx, fy] }) }

    visible: zoom !== null
    width: 276
    height: column.implicitHeight + 24
    radius: 12
    color: Theme.surface
    border.width: 1
    border.color: Theme.hairline
    // what's under it doesn't take its clicks
    MouseArea { anchors.fill: parent; onPressed: pop.ed.focusKeys() }

    Column {
        id: column
        x: 12
        y: 12
        width: pop.width - 24
        spacing: 10
        Item {
            width: parent.width
            height: 22
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Zoom"
                color: Theme.text
                font.family: Theme.fontUi
                font.pixelSize: 13
                font.weight: Font.DemiBold
            }
            Rectangle {
                objectName: "studio-zoom-remove"
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                width: 24
                height: 24
                radius: 6
                color: removeHover.hovered ? Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08) : "transparent"
                Icon { anchors.centerIn: parent; name: "trash"; size: 14; color: Theme.textSecondary }
                HoverHandler { id: removeHover; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: pop.ed.removeZoom(pop.ed.zoomSelected) }
            }
        }
        Slider {
            objectName: "studio-zoom-level"
            width: parent.width
            label: "Level"
            from: 100
            to: 400
            unit: "%"
            value: pop.zoom ? pop.zoom.level * 100 : 200
            onMoved: (v) => { pop.ed.zoomLevel = Math.round(v) / 100; pop.ed.changeZoom(pop.ed.zoomSelected, { level: pop.ed.zoomLevel }) }
            onReleased: (v) => { pop.ed.zoomLevel = Math.round(v) / 100; pop.ed.changeZoom(pop.ed.zoomSelected, { level: pop.ed.zoomLevel }) }
            onCancelled: pop.ed.focusKeys()
        }
        // the frame, and the spotlight on what the zoom shows
        Item {
            id: frame
            objectName: "studio-zoom-frame"
            width: parent.width
            height: Math.round(width * pop.ed.inner.h / Math.max(1, pop.ed.inner.w))
            clip: true
            ShaderEffectSource {
                anchors.fill: parent
                sourceItem: pop.visible ? pop.source : null
                hideSource: false
                textureSize: Qt.size(Math.ceil(width * Screen.devicePixelRatio), Math.ceil(height * Screen.devicePixelRatio))
            }
            Rectangle {
                id: spot
                x: pop.shown[0] * frame.width
                y: pop.shown[1] * frame.height
                width: (pop.shown[2] - pop.shown[0]) * frame.width
                height: (pop.shown[3] - pop.shown[1]) * frame.height
                color: "transparent"
                border.width: 2
                border.color: "white"
                radius: 3
            }
            // the rest, dimmed
            Rectangle { width: frame.width; height: spot.y; color: Theme.scrim }
            Rectangle { y: spot.y + spot.height; width: frame.width; height: frame.height - y; color: Theme.scrim }
            Rectangle { y: spot.y; width: spot.x; height: spot.height; color: Theme.scrim }
            Rectangle { x: spot.x + spot.width; y: spot.y; width: frame.width - x; height: spot.height; color: Theme.scrim }
            // the centre lines, while the spotlight snaps to them
            Rectangle { visible: aim.pressed && aim.snapX; x: frame.width / 2 - 0.5; width: 1; height: frame.height; color: Theme.accent }
            Rectangle { visible: aim.pressed && aim.snapY; y: frame.height / 2 - 0.5; width: frame.width; height: 1; color: Theme.accent }
            MouseArea {
                id: aim
                anchors.fill: parent
                cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                preventStealing: true
                property real dx: 0                // from the pointer to the spotlight's centre
                property real dy: 0
                property bool snapX: false
                property bool snapY: false
                function aimAt(m) {
                    let fx = Math.max(0, Math.min(1, (m.x + dx) / frame.width))
                    let fy = Math.max(0, Math.min(1, (m.y + dy) / frame.height))
                    snapX = Math.abs(fx - 0.5) * frame.width < 6
                    snapY = Math.abs(fy - 0.5) * frame.height < 6
                    pop.setFocus(snapX ? 0.5 : fx, snapY ? 0.5 : fy)
                }
                onPressed: (m) => {
                    pop.ed.focusKeys()
                    // from inside it, the spotlight moves with the pointer; outside, it jumps there
                    const inside = m.x >= spot.x && m.x <= spot.x + spot.width && m.y >= spot.y && m.y <= spot.y + spot.height
                    dx = inside ? spot.x + spot.width / 2 - m.x : 0
                    dy = inside ? spot.y + spot.height / 2 - m.y : 0
                    aimAt(m)
                }
                onPositionChanged: (m) => { if (pressed) aimAt(m) }
            }
        }
        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            text: "Drag the spotlight to what the zoom should show."
            color: Theme.textSecondary
            font.family: Theme.fontUi
            font.pixelSize: 11
        }
    }
}
