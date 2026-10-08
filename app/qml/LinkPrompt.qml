import QtQuick
import QtQuick.Effects
import QtQuick.Window
import org.kde.layershell as LayerShell
import Snip.Core

// "Open link?" (OCR-03/05): Capture Text found a link, in a code or the text.
// Above the toast, bottom centre of the monitor in use; only the card takes
// clicks and it never takes focus. Always and Never are remembered
// (ocr.open_links). It goes by itself after a while.
Window {
    id: prompt
    property string output: ""
    property string url: ""
    property bool shown: false

    function offer(link, out) {
        url = link
        if (out && out !== output) { shown = false; output = out }
        shown = true
        hideTimer.restart()
    }
    function answer(choice) {
        if (choice === "always" || choice === "never") Engine.setSetting("ocr.open_links", JSON.stringify(choice))
        if (choice !== "never") Engine.openLink(url)
        shown = false
    }

    visible: shown && output !== ""
    color: "transparent"
    flags: Qt.FramelessWindowHint
    title: "snip-link"

    LayerShell.Window.screen: Platform.screen(output)
    LayerShell.Window.scope: "snip-link"
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
                               | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
    LayerShell.Window.exclusionZone: -1
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
    onVisibleChanged: if (visible) fitInput()
    function fitInput() { Platform.setInputRects(prompt, [Qt.rect(card.x, card.y, card.width, card.height)]) }

    Timer { id: hideTimer; interval: 8000; onTriggered: prompt.shown = false }
    HoverHandler { onHoveredChanged: if (hovered) hideTimer.stop(); else hideTimer.restart() }

    RectangularShadow {
        anchors.fill: card
        radius: card.radius
        offset.y: 8
        blur: 28
        color: Theme.shadow
    }
    Rectangle {
        id: card
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 132                     // just above the toast
        width: Math.min(560, Math.max(320, row.implicitWidth + 32))
        height: 92
        radius: Math.min(Theme.radius + 4, 14)
        color: Theme.surface
        border.width: 1
        border.color: Theme.hairline
        onXChanged: prompt.fitInput()
        onYChanged: prompt.fitInput()
        onWidthChanged: prompt.fitInput()

        Column {
            x: 16
            y: 12
            width: parent.width - 32
            spacing: 2
            Text {
                text: "Open link?"
                color: Theme.text
                font.family: Theme.fontUi
                font.pixelSize: 13
                font.weight: Font.DemiBold
            }
            Text {
                width: parent.width
                elide: Text.ElideMiddle
                text: prompt.url
                color: Theme.textSecondary
                font.family: Theme.fontUi
                font.pixelSize: 12
            }
        }
        Row {
            id: row
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.margins: 12
            spacing: 8
            TextButton { objectName: "link-never"; text: "Never"; tip: "Don't offer links again"; onClicked: prompt.answer("never") }
            TextButton { objectName: "link-always"; text: "Always"; tip: "Open links without asking"; onClicked: prompt.answer("always") }
            TextButton { objectName: "link-open"; text: "Open"; primary: true; onClicked: prompt.answer("open") }
        }
    }

    // tests
    function find(item, name) {
        if (item.objectName === name && item.visible) return item
        for (const c of item.children) {
            const f = c.visible ? find(c, name) : null
            if (f) return f
        }
        return null
    }
    function debug(a) {
        switch (a[0]) {
        case "where": {
            const it = find(contentItem, a[1])
            // laid out first: just after a change, rows and columns still hold old places
            for (let q = it; q; q = q.parent) if (q.forceLayout) q.forceLayout()
            const p = it ? it.mapToItem(contentItem, it.width / 2, it.height / 2) : null
            Engine.log("link_where", JSON.stringify({ name: a[1], x: p ? p.x : -1, y: p ? p.y : -1 }).slice(1, -1))
            break
        }
        case "state":
            Engine.log("link_prompt", JSON.stringify({ open: visible, url: url, output: output }).slice(1, -1))
            break
        }
    }
}
