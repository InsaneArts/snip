import QtQuick
import QtQuick.Window
import QtQuick.Effects
import org.kde.layershell as LayerShell
import Snip.Core

// The Quick Access stack (QAO-*), with CleanShot X's layout and motion
// (measured from its demo videos; docs/design/qao-motion.md):
//   - new cards appear on TOP of the stack; the oldest stays in the corner
//   - entrance: slide in from the screen edge by 20% of the card width with a
//     ~10% spring overshoot while fading in (settles at ~220 ms)
//   - exit: slide back toward the edge, accelerating, fading (~220 ms); the
//     cards above glide down into the gap
//   - hide all: the stack slides off the bottom edge, leaving a small tab
// One layer surface in a bottom corner of the captured monitor: a tall
// transparent column whose input region covers only the cards.
Window {
    id: qa
    required property var controller
    readonly property var c: controller
    readonly property var st: c.settings.quickAccess || ({})
    readonly property bool right: st.position === "right"
    readonly property real cardWidth: st.size || 280
    readonly property real cardHeight: Math.round(cardWidth * 0.705)
    readonly property int maxVisible: Math.max(1, st.maxVisible || 5)
    readonly property real margin: 24
    readonly property real spacing: 16
    readonly property real menuSpace: 236
    readonly property real travel: Math.round(cardWidth * 0.22)
    readonly property int outward: right ? 1 : -1

    property string output: ""
    property bool hidden: false
    property bool remap: false
    property bool capturing: false     // off screen while Snip captures the screen
    property bool dragging: false
    property bool expanded: false
    property var menuCard: null
    property real menuX: 0
    property real menuY: 0
    readonly property bool hovering: stackHover.hovered
    readonly property int count: cards.count
    readonly property int firstVisible: expanded ? 0 : Math.max(0, count - maxVisible)
    readonly property bool collapsed: count > maxVisible && !expanded

    // mapped once at start, empty, like the overlays (Main.warming): the
    // first card then appears as quickly as the rest
    readonly property bool warming: c.warming && count === 0
    visible: (count > 0 && !remap && output !== "") || warming
    color: "transparent"
    flags: Qt.FramelessWindowHint
    width: cardWidth + 2 * margin + menuSpace
    title: "snip-qao"

    LayerShell.Window.screen: Platform.screen(output !== "" ? output : Qt.application.screens[0].name)
    LayerShell.Window.scope: "snip-qao"
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
                               | (qa.right ? LayerShell.Window.AnchorRight : LayerShell.Window.AnchorLeft)
    LayerShell.Window.exclusionZone: 0
    // QAO-03: keys reach the cards only while the pointer rests on them. With
    // the menu open the keyboard is on-demand, not exclusive: Hyprland keeps
    // all pointer events on an exclusive layer, and the menu's outside-click
    // catcher would never see a click (found by e2e menu_closes_on_any_click).
    LayerShell.Window.keyboardInteractivity: keyDwell.ready && qa.hovering && !qa.menuCard
                                             ? LayerShell.Window.KeyboardInteractivityExclusive
                                             : qa.menuCard ? LayerShell.Window.KeyboardInteractivityOnDemand
                                                           : LayerShell.Window.KeyboardInteractivityNone

    ListModel { id: cards }   // { cardId, json }, oldest first

    // ---------------------------------------------------------------- API (Main / Engine)
    function add(card) {
        if (card.output && card.output !== output) moveTo(card.output)
        for (let i = 0; i < cards.count; i++)
            if (cards.get(i).cardId === card.id) { cards.remove(i); break }
        cards.append({ cardId: card.id, json: JSON.stringify(card) })
        hidden = false
        Qt.callLater(updateMask)
    }

    function update(id, card) {
        for (let i = 0; i < cards.count; i++)
            if (cards.get(i).cardId === id) cards.setProperty(i, "json", JSON.stringify(card))
    }

    function command(cmd) {
        if (cmd === "close-all") closeAll()
        else if (cmd === "hide") { menuCard = null; hidden = true }
        else if (cmd === "show") hidden = false
        else if (cmd === "dismiss" && stack.newestCard) dismiss(stack.newestCard, outward)   // test hook
        else if (cmd === "restore") { if (hidden && count > 0) hidden = false; else Engine.restoreLast() }
        Qt.callLater(updateMask)
    }

    // Engine.cardStatus: show a result on that card
    function cardStatus(id, status, detail) {
        for (let i = 0; i < stack.children.length; i++) {
            const ch = stack.children[i]
            if (ch.cardId === id) { ch.showResult(status, detail); return }
        }
    }

    function moveTo(o) {
        if (o === output) return
        remap = true          // a layer surface can't change outputs while mapped
        output = o
        Qt.callLater(() => remap = false)
    }

    function indexOf(id) {
        for (let i = 0; i < cards.count; i++) if (cards.get(i).cardId === id) return i
        return -1
    }

    function remove(id, toClosed) {
        const i = indexOf(id)
        if (i < 0) return
        cards.remove(i)
        if (toClosed) Engine.cardClosed(id)
        if (menuCard && menuCard.cardId === id) menuCard = null
        if (count <= maxVisible) expanded = false
        if (count === 0) hidden = false
        Qt.callLater(updateMask)
    }

    function closeAll() {
        menuCard = null
        const ids = []
        for (let i = 0; i < cards.count; i++) ids.push(cards.get(i).cardId)
        for (const id of ids) Engine.cardClosed(id)
        cards.clear()
        hidden = false
    }

    function saveAll() {
        for (let i = 0; i < cards.count; i++) Engine.save(cards.get(i).cardId)
    }

    // Dismiss with the exit animation, toward the edge (or the swipe's way).
    function dismiss(card, direction) {
        if (card && card.leave) card.leave(direction)
    }

    // Drag out (QAO-04): the card dims while dragging and closes after a
    // drop (CleanShot's "Close after dragging"; Alt keeps it).
    function drag(card) {
        dragging = true
        card.dragging = true
        const path = Engine.dragFile(card.cardId)
        const result = Platform.startDrag(card, path, card.data_.thumb || "")
        card.dragging = false
        dragging = false
        if (result === "copy" && st.closeAfterDrag !== false && !c.altHeld) dismiss(card, outward)
    }

    // Card actions (buttons, menu, keys).
    function act(action, card) {
        if (!card) return
        const id = card.cardId
        menuCard = null
        switch (action) {
        case "copy":
            if (Engine.copy(id)) {
                // CleanShot: Copy closes the card (Alt+Copy keeps it).
                if (c.altHeld) card.flash("copied", 900)
                else { card.flash("copied", 0); card.leaveAfter(420) }
            }
            break
        case "save": Engine.save(id); card.flash("saved", 900); break
        case "show": Engine.showInFiles(id); break
        case "annotate": Engine.annotate(id); break
        case "text": card.flash("text", 0); Engine.extractText(id); break   // the result arrives as cardStatus
        case "rotate-left": case "flip-horizontal": Engine.transformCapture(id, action); break
        case "preview": c.preview.open(card.data_); break
        case "trash": Engine.trash(id); remove(id, false); break
        case "close": dismiss(card, outward); break
        case "close-all": closeAll(); break
        case "save-all": saveAll(); break
        case "hide": command("hide"); break
        }
    }

    // Engine.prepareCapture: Snip's cards must not be in the capture. Hyprland
    // fades unmapped layers out (~150 ms, even with no_anim), so instead of
    // unmapping, draw the stack fully transparent and let the capture go
    // once that frame is on screen. The stack returns when the capture is
    // done (or, for a selection, when the overlay closes).
    property int captureRequest: 0
    property bool awaitSwap: false
    function prepareCapture(req) {
        captureRequest = req
        if (!visible) { Engine.captureReady(req); return }
        capturing = true
        awaitSwap = true
        Platform.notifyNextFrame(qa)
        captureGo.restart()     // fallback if no frame comes
    }
    function releaseCapture() {
        if (!awaitSwap) return
        awaitSwap = false
        captureGo.stop()
        Engine.captureReady(captureRequest)
    }
    Connections {
        target: Platform
        function onFramePresented(w) { if (w === qa) qa.releaseCapture() }
    }
    Timer { id: captureGo; interval: 40; onTriggered: qa.releaseCapture() }

    // The window has shown a frame since it was (re)mapped.
    property bool presented: false
    property var presentQueue: []
    function whenPresented(fn) { presentQueue.push(fn); Platform.notifyNextFrame(qa) }
    Connections {
        target: Platform
        function onFramePresented(w) {
            if (w !== qa) return
            if (qa.warming) qa.c.warmed(qa)
            qa.presented = true
            const q = qa.presentQueue
            qa.presentQueue = []
            for (const fn of q) fn()
        }
    }

    // ---------------------------------------------------------------- layout
    readonly property real cardX: right ? width - margin - cardWidth : margin
    readonly property real badgeSpace: collapsed ? 26 + 10 : 0
    readonly property real bottomY: height - margin - badgeSpace
    // oldest visible card in the corner slot; each newer one above it
    function yFor(i) {
        const slot = Math.max(0, i - firstVisible)
        return bottomY - cardHeight - slot * (cardHeight + spacing)
    }
    readonly property real topY: count > 0 ? yFor(count - 1) : height
    // hide all: everything below the bottom edge
    readonly property real hideShift: hidden ? height - topY + 40 : 0

    function updateMask() {
        const r = []
        if (menuCard) {
            r.push(Qt.rect(0, 0, width, height))      // a click anywhere in the column closes the menu
        } else if (hidden) {
            r.push(Qt.rect(tab.x - 6, tab.y - 6, tab.width + 12, tab.height + 12))
        } else {
            for (let i = firstVisible; i < cards.count; i++)
                r.push(Qt.rect(cardX - 6, yFor(i) - 6, cardWidth + 12, cardHeight + 12))
            if (collapsed)
                r.push(Qt.rect(cardX, height - margin - 26, cardWidth, 26))
        }
        Platform.setInputRects(qa, r)
    }
    onHeightChanged: Qt.callLater(updateMask)
    onMenuCardChanged: Qt.callLater(updateMask)
    onFirstVisibleChanged: Qt.callLater(updateMask)
    onHiddenChanged: Qt.callLater(updateMask)
    onVisibleChanged: {
        if (visible) Qt.callLater(updateMask)
        else presented = false
        if (visible && warming) Platform.notifyNextFrame(qa)
    }

    // ---------------------------------------------------------------- content
    Item {
        id: stack
        anchors.fill: parent
        opacity: qa.capturing ? 0 : 1
        enabled: !qa.capturing
        transform: Translate {
            y: qa.hideShift
            Behavior on y { NumberAnimation { duration: 340; easing.type: Easing.InOutCubic } }
        }

        HoverHandler { id: stackHover; enabled: !qa.menuCard && !qa.hidden }
        readonly property var newestCard: {
            let best = null
            for (let i = 0; i < children.length; i++) {
                const ch = children[i]
                if (ch.cardId !== undefined && !ch.leaving && (best === null || ch.index > best.index)) best = ch
            }
            return best
        }

        Timer {
            id: keyDwell
            property bool ready: false
            interval: 180
            running: qa.hovering && !ready
            onTriggered: ready = true
        }
        Connections {
            target: qa
            function onHoveringChanged() { if (!qa.hovering) keyDwell.ready = false }
        }

        Repeater {
            model: cards
            delegate: Card {
                id: cardItem
                required property int index
                required property string cardId
                required property string json
                data_: JSON.parse(json)
                host: qa
                cardWidth: qa.cardWidth
                newest: index === qa.count - 1 && qa.count > 1
                visible: index >= qa.firstVisible && opacity > 0.01

                // motion state: horizontal offset from the slot, and fade
                property real slide: qa.outward * qa.travel
                property real fade: 0
                property bool leaving: false
                property bool settled: false
                x: qa.cardX + slide
                y: qa.yFor(index)
                opacity: fade
                Behavior on y {
                    enabled: cardItem.settled
                    NumberAnimation { duration: 300; easing.type: Easing.OutCubic }
                }

                ParallelAnimation {
                    id: enterAnim
                    // fitted to CleanShot's entrance (rms 4.5 %): +10 % at ~150 ms, settled ~250 ms
                    NumberAnimation { target: cardItem; property: "slide"; to: 0; duration: 270
                                      easing.type: Easing.BezierSpline; easing.bezierCurve: [0.40, 1.40, 0.40, 1.10, 1, 1] }
                    NumberAnimation { target: cardItem; property: "fade"; to: 1; duration: 180 }   // linear, as measured
                    onFinished: cardItem.settled = true
                }
                // Start once the card can actually be seen: the first card maps the
                // window, and frames rendered before that would eat the motion.
                Component.onCompleted: {
                    if (qa.presented) enterAnim.start()
                    else qa.whenPresented(() => { if (enterAnim) enterAnim.start() })   // the card may be gone by then
                }
                // SNIP_TRACE: the rendered motion, one line per frame (docs/design/qao-motion.md)
                FrameAnimation {
                    running: enterAnim.running || leaveAnim.running
                    onTriggered: Engine.log("qao_motion", "\"id\":\"" + cardItem.cardId + "\",\"leaving\":" + cardItem.leaving
                                            + ",\"slide\":" + cardItem.slide.toFixed(2) + ",\"fade\":" + cardItem.fade.toFixed(3))
                }

                ParallelAnimation {
                    id: leaveAnim
                    property real to: qa.outward * qa.travel * 1.4
                    NumberAnimation { target: cardItem; property: "slide"; to: leaveAnim.to; duration: 220; easing.type: Easing.InCubic }
                    NumberAnimation { target: cardItem; property: "fade"; to: 0; duration: 200 }
                    onFinished: qa.remove(cardItem.cardId, true)
                }
                // direction < 0: toward the left, > 0: toward the right
                function leave(direction) {
                    if (leaving) return
                    leaving = true
                    enterAnim.stop()
                    const dir = direction === 0 ? qa.outward : Math.sign(direction)
                    // a swipe carries on from where the finger left the card
                    slide = slide + swipe
                    swipe = 0
                    leaveAnim.to = slide + dir * qa.travel * 1.4
                    leaveAnim.start()
                }
                Timer { id: leaveDelay; onTriggered: cardItem.leave(qa.outward) }
                function leaveAfter(ms) { leaveDelay.interval = ms; leaveDelay.restart() }

                // QAO-07: auto-close, paused while hovered, dragging, in a menu or hidden.
                Timer {
                    interval: Math.max(2, qa.st.autoCloseSeconds || 10) * 1000
                    running: qa.st.autoClose !== false && !qa.hovering && !qa.dragging && !qa.menuCard && !qa.hidden && qa.visible && !cardItem.leaving
                    onTriggered: {
                        if (qa.st.autoCloseAction === "save_and_close") Engine.save(cardItem.cardId)
                        cardItem.leave(qa.outward)
                    }
                }

                onCloseRequested: (animated) => animated ? leave(qa.outward) : qa.remove(cardId, true)
                onMenuRequested: (mx, my) => {
                    qa.menuX = qa.right ? qa.cardX - menu.width - 10 : qa.cardX + qa.cardWidth + 10
                    qa.menuY = Math.max(8, Math.min(qa.height - menu.height - 8, cardItem.y + my - 12))
                    qa.menuCard = cardItem
                }
            }
        }

        // "+k" for collapsed older cards (QAO-06), under the stack
        FrostButton {
            id: moreBadge
            visible: qa.collapsed
            x: qa.cardX + (qa.cardWidth - width) / 2
            y: qa.height - qa.margin - height
            implicitHeight: 26
            label: "+" + (qa.count - qa.maxVisible) + " more"
            onClicked: qa.expanded = true
        }

        // While the menu is open, a click anywhere else in the panel (cards
        // included) only closes it, like a native menu.
        MouseArea {
            z: 50
            anchors.fill: parent
            visible: qa.menuCard !== null
            acceptedButtons: Qt.AllButtons
            onPressed: qa.menuCard = null
            onWheel: (w) => { qa.menuCard = null; w.accepted = true }
        }

        CardMenu {
            id: menu
            z: 100
            visible: qa.menuCard !== null
            x: qa.menuX
            y: qa.menuY
            card: qa.menuCard
            onTriggered: (action) => qa.act(action, qa.menuCard)
        }
    }

    // ...and outside the panel: an invisible full-screen catcher on the layer
    // below ours, mapped only while the menu is open. A click anywhere on the
    // screen closes the menu instead of reaching the app underneath.
    Window {
        id: outsideCatcher
        visible: qa.menuCard !== null && qa.visible
        color: "transparent"
        flags: Qt.FramelessWindowHint
        title: "snip-dismiss"
        LayerShell.Window.screen: Platform.screen(qa.output)
        LayerShell.Window.scope: "snip-dismiss"
        LayerShell.Window.layer: LayerShell.Window.LayerTop
        LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
                                   | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
        LayerShell.Window.exclusionZone: -1
        LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.AllButtons
            onPressed: qa.menuCard = null
            onWheel: (w) => { qa.menuCard = null; w.accepted = true }
        }
    }

    // ---------------------------------------------------------------- hidden: a tab peeks from the bottom edge
    Rectangle {
        id: tab
        readonly property bool lifted: tabArea.containsMouse
        width: tabRow.implicitWidth + 24
        height: 30
        radius: 15
        x: qa.cardX + (qa.cardWidth - width) / 2
        y: qa.hidden ? qa.height - (lifted ? height - 2 : 14) : qa.height + 8
        Behavior on y { NumberAnimation { duration: 280; easing.type: Easing.OutCubic } }
        // parked below the window, its shadow would still reach into it
        visible: y < qa.height
        color: Qt.rgba(0.93, 0.93, 0.94, 0.95)
        RectangularShadow { z: -1; anchors.fill: parent; radius: parent.radius; blur: 16; offset.y: 2; color: Theme.shadow }
        Row {
            id: tabRow
            anchors.horizontalCenter: parent.horizontalCenter
            y: 5
            spacing: 5
            Icon { name: "layers"; size: 13; stroke: 2.2; color: "#1d1d1f"; anchors.verticalCenter: parent.verticalCenter }
            Text {
                text: qa.count
                color: "#1d1d1f"
                font.family: Theme.fontUi
                font.pixelSize: 12
                font.weight: Font.DemiBold
                font.features: { "tnum": 1 }
                anchors.verticalCenter: parent.verticalCenter
            }
        }
        MouseArea {
            id: tabArea
            anchors.fill: parent
            anchors.margins: -6
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: qa.command("show")
        }
    }

    // ---------------------------------------------------------------- keyboard (QAO-03)
    readonly property var hoveredCard: {
        if (!hovering) return null
        for (let i = 0; i < stack.children.length; i++) {
            const ch = stack.children[i]
            if (ch.cardId !== undefined && ch.hovered) return ch
        }
        return null
    }
    Item {
        focus: true
        Keys.onPressed: (e) => {
            const card = qa.menuCard || qa.hoveredCard
            if (e.key === Qt.Key_Escape) { qa.menuCard = null; e.accepted = true; return }
            if (!card) return
            const ctrl = e.modifiers & Qt.ControlModifier
            if (ctrl && e.key === Qt.Key_C) qa.act("copy", card)
            else if (ctrl && e.key === Qt.Key_S) qa.act("save", card)
            else if (ctrl && e.key === Qt.Key_W) qa.act("close", card)
            else if (ctrl && e.key === Qt.Key_E) qa.act("annotate", card)
            else if (e.key === Qt.Key_Space) qa.act("preview", card)
            else return
            e.accepted = true
        }
    }
}
