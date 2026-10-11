import QtQuick
import Snip.Core

// Studio Mode's zoom lane (D55), between the timeline's ruler and its
// frames: each zoom a block from its start to its end, with its level. A
// click on the lane adds a zoom (2 s), a drag across it one that long; drag
// a zoom to move it, or its ends to change them; Delete removes the one
// selected. Zooms never overlap: one pushed against the next pans to it.
// Times are the recording's own ms, as the cuts' are.
Item {
    id: lane
    property var ed: null                   // the video editor
    property real k: 1                      // px per ms
    readonly property var zooms: ed ? ed.studio.zooms : []
    readonly property real least: ed ? ed.zoomLeast : 0
    signal menu(int index, real x, real y)   // a right-click on zoom `index`, at x, y here

    // the free time around `ms`, between the zooms other than zoom `skip`:
    // [from, to], or null inside one
    function room(ms, skip) {
        let lo = 0, hi = ed.duration
        for (let i = 0; i < zooms.length; i++) {
            if (i === skip) continue
            const z = zooms[i]
            if (ms >= z.from && ms < z.to) return null
            if (z.to <= ms) lo = Math.max(lo, z.to)
            else hi = Math.min(hi, z.from)
        }
        return [lo, hi]
    }
    // where a click at `ms` puts a zoom: 2 s from there, in the room there is
    function placed(ms) {
        const r = room(ms, -1)
        if (!r || r[1] - r[0] < least) return null
        const b = Math.min(ms + 2000, r[1])
        return [Math.max(r[0], b - 2000), b]
    }
    // the zoom under `x`, and which of its parts: its start, its end, or its middle
    function hit(x) {
        for (let i = 0; i < zooms.length; i++) {
            const a = zooms[i].from * k, b = zooms[i].to * k, edge = Math.min(6, (b - a) / 4)
            if (x >= a - 2 && x <= b + 2)
                return { index: i, part: x < a + edge ? "from" : x > b - edge ? "to" : "move" }
        }
        return null
    }

    Rectangle { anchors.fill: parent; radius: 6; color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.05) }
    Text {
        visible: lane.zooms.length === 0 && !area.containsMouse
        x: 8
        anchors.verticalCenter: parent.verticalCenter
        text: "Click or drag here to zoom in"
        color: Theme.textSecondary
        font.family: Theme.fontUi
        font.pixelSize: 11
    }
    // where a click would put one
    Rectangle {
        readonly property var at: area.containsMouse && !area.pressed && !area.over ? lane.placed(area.mouseX / lane.k) : null
        visible: at !== null
        x: at ? at[0] * lane.k : 0
        width: at ? (at[1] - at[0]) * lane.k : 0
        height: lane.height
        radius: 6
        color: "transparent"
        border.width: 1
        border.color: Theme.accent
        Text { anchors.centerIn: parent; visible: parent.width > 24; text: "+"; color: Theme.accent; font.family: Theme.fontUi; font.pixelSize: 14 }
    }
    Repeater {
        model: lane.zooms
        Rectangle {
            id: block
            required property var modelData
            required property int index
            readonly property bool chosen: lane.ed.zoomSelected === index
            objectName: "studio-zoom:" + index
            x: modelData.from * lane.k
            width: Math.max(4, (modelData.to - modelData.from) * lane.k)
            height: lane.height
            radius: 6
            color: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, chosen ? 1 : 0.7)
            border.width: chosen ? 2 : 0
            border.color: Theme.text
            Text {
                anchors.centerIn: parent
                visible: parent.width > 28
                text: Math.round(block.modelData.level * 10) / 10 + "×"
                color: Theme.onAccent
                font.family: Theme.fontUi
                font.pixelSize: 11
                font.weight: Font.DemiBold
            }
        }
    }
    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        preventStealing: true
        readonly property var over: containsMouse ? lane.hit(mouseX) : null
        // what the press took: {index, part ("draw", "move", "from", "to"), x,
        // from, to, moved}; a draw keeps the room it's in
        property var grab: null
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: grab && grab.part === "move" ? Qt.ClosedHandCursor
                   : over && over.part !== "move" ? Qt.SizeHorCursor
                   : over ? Qt.OpenHandCursor : Qt.PointingHandCursor
        onPressed: (m) => {
            lane.ed.focusKeys()
            const h = lane.hit(m.x), t = m.x / lane.k, r = lane.room(t, -1)
            if (m.button === Qt.RightButton) {
                grab = null
                if (h) { lane.ed.selectZoom(h.index); lane.menu(h.index, m.x, m.y) }
                return
            }
            if (h) {
                const z = lane.zooms[h.index]
                grab = { index: h.index, part: h.part, x: m.x, from: z.from, to: z.to, moved: false }
                lane.ed.selectZoom(h.index)
            } else {
                grab = r && r[1] - r[0] >= lane.least ? { index: -1, part: "draw", x: m.x, from: t, to: t, room: r, moved: false } : null
            }
        }
        onPositionChanged: (m) => {
            if (!pressed || !grab || (!grab.moved && Math.abs(m.x - grab.x) < 4)) return
            grab.moved = true
            const d = (m.x - grab.x) / lane.k
            let a = grab.from, b = grab.to
            if (grab.part === "draw") {
                // from the press to the pointer, at least the shortest zoom
                const r = grab.room, t = Math.max(r[0], Math.min(r[1], a + d))
                a = Math.min(grab.from, t)
                b = Math.max(grab.from, t)
                if (b - a < lane.least) {
                    if (t >= grab.from) { b = Math.min(r[1], a + lane.least); a = b - lane.least }
                    else { a = Math.max(r[0], b - lane.least); b = a + lane.least }
                }
                if (grab.index < 0) { grab.index = lane.ed.addZoom(Math.round(a), Math.round(b)); return }
            } else {
                const r = lane.room(grab.from, grab.index)
                if (grab.part === "move") {
                    const len = b - a
                    a = Math.max(r[0], Math.min(r[1] - len, a + d))
                    b = a + len
                } else if (grab.part === "from") {
                    a = Math.max(r[0], Math.min(b - lane.least, a + d))
                } else {
                    b = Math.min(r[1], Math.max(a + lane.least, b + d))
                }
            }
            lane.ed.changeZoom(grab.index, { from: Math.round(a), to: Math.round(b) })
        }
        onReleased: {
            const g = grab
            grab = null
            if (!g) return
            if (g.part === "draw" && !g.moved) {
                // a click: a zoom of 2 s there
                const p = lane.placed(g.from)
                if (p) lane.ed.showZoom(lane.ed.addZoom(Math.round(p[0]), Math.round(p[1])))
            } else if (g.part === "draw" || !g.moved) {
                // one just drawn, or a click on one: the preview shows it
                lane.ed.showZoom(g.index)
            }
        }
        onCanceled: grab = null
    }
}
