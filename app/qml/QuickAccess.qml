import QtQuick
import QtQuick.Window
import org.kde.layershell as LayerShell
import Snip.Core

// The Quick Access stack (QAO-*): one layer surface in a bottom corner of
// the monitor that was captured. The surface is a tall transparent column;
// the input region covers only the cards, so the rest stays click-through.
Window {
    id: qa
    required property var controller
    readonly property var c: controller
    readonly property var st: c.settings.quickAccess || ({})
    readonly property bool right: st.position === "right"
    readonly property real cardWidth: st.size || 280
    readonly property int maxVisible: Math.max(1, st.maxVisible || 5)
    readonly property real margin: 20
    readonly property real spacing: 14
    readonly property real menuSpace: 236

    property string output: ""
    property bool hidden: false
    property bool remap: false
    property bool capturing: false     // off screen while Snip captures the screen
    property bool dragging: false
    property bool expanded: false
    property var menuCard: null
    property real menuX: 0
    property real menuY: 0
    property var heights: []
    readonly property bool hovering: stackHover.hovered
    readonly property int count: cards.count
    readonly property int firstVisible: expanded ? 0 : Math.max(0, count - maxVisible)

    visible: count > 0 && !hidden && !remap && output !== ""
    color: "transparent"
    flags: Qt.FramelessWindowHint
    width: cardWidth + 2 * margin + menuSpace
    title: "snip-qao"

    LayerShell.Window.screen: Platform.screen(output)
    LayerShell.Window.scope: "snip-qao"
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
                               | (qa.right ? LayerShell.Window.AnchorRight : LayerShell.Window.AnchorLeft)
    LayerShell.Window.exclusionZone: 0
    // QAO-03: keys reach the cards only while the pointer rests on them.
    LayerShell.Window.keyboardInteractivity: (keyDwell.ready && qa.hovering) || qa.menuCard
                                             ? LayerShell.Window.KeyboardInteractivityExclusive
                                             : LayerShell.Window.KeyboardInteractivityNone

    ListModel { id: cards }   // { cardId, json }, oldest first

    // ---------------------------------------------------------------- API (Main / Engine)
    function add(card) {
        if (card.output && card.output !== output) moveTo(card.output)
        for (let i = 0; i < cards.count; i++)
            if (cards.get(i).cardId === card.id) { cards.remove(i); break }
        cards.append({ cardId: card.id, json: JSON.stringify(card) })
        hidden = false
        relayout()
    }

    function update(id, card) {
        for (let i = 0; i < cards.count; i++)
            if (cards.get(i).cardId === id) cards.setProperty(i, "json", JSON.stringify(card))
    }

    function command(cmd) {
        if (cmd === "close-all") closeAll()
        else if (cmd === "hide") { menuCard = null; hidden = true }
        else if (cmd === "show") hidden = false
        else if (cmd === "restore") { if (hidden && count > 0) hidden = false; else Engine.restoreLast() }
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
        relayout()
    }

    function closeAll() {
        menuCard = null
        const ids = []
        for (let i = 0; i < cards.count; i++) ids.push(cards.get(i).cardId)
        for (const id of ids) Engine.cardClosed(id)
        cards.clear()
        relayout()
    }

    function saveAll() {
        for (let i = 0; i < cards.count; i++) Engine.save(cards.get(i).cardId)
    }

    // Card actions (buttons, menu, keys).
    function act(action, card) {
        if (!card) return
        const id = card.cardId
        menuCard = null
        switch (action) {
        case "copy":
            if (Engine.copy(id)) {
                card.flashCopied()
                // CleanShot: Copy closes the card; Alt+Copy keeps it.
                if (!c.altHeld) closeLater.schedule(card)
            }
            break
        case "save": Engine.save(id); break
        case "show": Engine.showInFiles(id); break
        case "annotate": Engine.annotate(id); break
        case "text": Engine.extractText(id); break
        case "preview": c.preview.open(card.data_); break
        case "trash": Engine.trash(id); card.closeRequested(false); remove(id, false); break
        case "close": card.closeRequested(true); break
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

    QtObject {
        id: closeLater
        property var pending: null
        function schedule(card) { pending = card; t.restart() }
        property Timer t: Timer { interval: 450; onTriggered: if (closeLater.pending) closeLater.pending.closeRequested(true) }
    }

    // ---------------------------------------------------------------- layout
    function heightFor(json) {
        const d = JSON.parse(json)
        const a = d.width > 0 ? d.height / d.width : 0.5625
        return Math.round(cardWidth * Math.max(0.42, Math.min(1.25, a)))
    }

    function relayout() {
        const h = []
        for (let i = 0; i < cards.count; i++) h.push(heightFor(cards.get(i).json))
        heights = h
        Qt.callLater(updateMask)
        Engine.log("qa_layout", "\"w\":" + width + ",\"h\":" + height + ",\"heights\":" + JSON.stringify(h) + ",\"ys\":" + JSON.stringify(h.map((_, i) => yFor(i))) + ",\"x\":" + cardX)
    }

    // Newest at the bottom (nearest the corner), older ones stacked above.
    function yFor(i) {
        let y = qa.height - margin
        for (let j = cards.count - 1; j >= i; j--) {
            if (j < firstVisible) break
            y -= (heights[j] || 0)
            if (j > i) y -= spacing
        }
        return y
    }

    readonly property real cardX: right ? width - margin - cardWidth : margin
    readonly property real topY: count > 0 ? yFor(firstVisible) : height

    function updateMask() {
        const r = []
        if (menuCard) {
            r.push(Qt.rect(0, 0, width, height))      // a click anywhere in the column closes the menu
        } else {
            for (let i = firstVisible; i < cards.count; i++)
                r.push(Qt.rect(cardX - 4, yFor(i) - 4, cardWidth + 8, (heights[i] || 0) + 8))
            if (stackBar.visible || moreBadge.visible)
                r.push(Qt.rect(cardX, topY - 44, cardWidth, 40))
        }
        Platform.setInputRects(qa, r)
    }
    onHeightChanged: relayout()
    onMenuCardChanged: Qt.callLater(updateMask)
    onFirstVisibleChanged: Qt.callLater(updateMask)
    onVisibleChanged: if (visible) Qt.callLater(updateMask)

    // ---------------------------------------------------------------- content
    Item {
        id: stack
        anchors.fill: parent
        opacity: qa.capturing ? 0 : 1
        enabled: !qa.capturing

        HoverHandler { id: stackHover; enabled: !qa.menuCard }

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

        // click outside the menu closes it
        MouseArea {
            anchors.fill: parent
            enabled: qa.menuCard !== null
            acceptedButtons: Qt.AllButtons
            onPressed: qa.menuCard = null
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
                visible: index >= qa.firstVisible
                y: qa.yFor(index)
                x: entered ? qa.cardX : (qa.right ? qa.width + 8 : -qa.cardWidth - 8)
                property bool entered: false
                property bool leaving: false
                Behavior on y { enabled: cardItem.entered; NumberAnimation { duration: 240; easing.type: Easing.OutCubic } }
                Behavior on x { NumberAnimation { duration: cardItem.leaving ? 200 : 300; easing.type: cardItem.leaving ? Easing.InCubic : Easing.OutBack; easing.overshoot: 1.05 } }
                Component.onCompleted: Qt.callLater(() => entered = true)

                // QAO-07: auto-close, paused while hovered, dragging or in a menu.
                Timer {
                    interval: Math.max(2, qa.st.autoCloseSeconds || 10) * 1000
                    running: qa.st.autoClose !== false && !qa.hovering && !qa.dragging && !qa.menuCard && !qa.hidden && qa.visible
                    onTriggered: {
                        if (qa.st.autoCloseAction === "save_and_close") Engine.save(cardItem.cardId)
                        cardItem.closeRequested(true)
                    }
                }

                onCloseRequested: (animated) => {
                    if (leaving) return
                    if (!animated) { qa.remove(cardId, true); return }
                    leaving = true
                    entered = false       // slides back out to the edge
                    leaveTimer.start()
                }
                Timer { id: leaveTimer; interval: 200; onTriggered: qa.remove(cardItem.cardId, true) }
                onMenuRequested: (mx, my) => {
                    qa.menuX = qa.right ? qa.cardX - menu.width - 8 : qa.cardX + qa.cardWidth + 8
                    qa.menuY = Math.max(8, Math.min(qa.height - menu.height - 8, cardItem.y + my - 12))
                    qa.menuCard = cardItem
                }
            }
        }

        // "+k" for collapsed older cards (QAO-06)
        Rectangle {
            id: moreBadge
            visible: qa.count > qa.maxVisible && !qa.expanded
            x: qa.cardX + (qa.cardWidth - width) / 2
            y: qa.topY - height - 10
            height: 26
            width: moreText.implicitWidth + 20
            radius: 13
            color: Theme.pillBg
            border.width: 1
            border.color: Theme.hairline
            Text {
                id: moreText
                anchors.centerIn: parent
                text: "+" + (qa.count - qa.maxVisible) + " more"
                color: Theme.text
                font.family: Theme.fontUi
                font.pixelSize: 12
                font.weight: Font.DemiBold
                font.features: { "tnum": 1 }
            }
            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { qa.expanded = true; qa.relayout() } }
        }

        // Close All / Save All above the stack while it is hovered
        Row {
            id: stackBar
            visible: qa.count > 1 && qa.hovering && !moreBadge.visible && !qa.menuCard
            x: qa.cardX + (qa.cardWidth - width) / 2
            y: qa.topY - height - 10
            spacing: 8
            Pill { label: "Close All"; implicitHeight: 26; onClicked: qa.closeAll() }
            Pill { label: "Save All"; implicitHeight: 26; onClicked: qa.saveAll() }
            onVisibleChanged: Qt.callLater(qa.updateMask)
        }

        CardMenu {
            id: menu
            visible: qa.menuCard !== null
            x: qa.menuX
            y: qa.menuY
            card: qa.menuCard
            onTriggered: (action) => qa.act(action, qa.menuCard)
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
