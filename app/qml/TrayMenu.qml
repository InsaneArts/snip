import QtQuick
import QtQuick.Window
import org.kde.layershell as LayerShell
import Snip.Core

// Snip's menu in the bar (OMA-08, D35), CleanShot's: a click on the bar icon
// opens it under the icon, with the captures and their keys, History,
// Settings and Quit. A full-output layer, so a click anywhere else (the icon
// too) closes it, as Esc does. An entry runs once the menu is off screen, so
// it's never in a capture.
Window {
    id: tray
    property var controller: null
    property string output: ""
    property point at: Qt.point(0, 0)          // the click on the output (logical px)
    property int barTop: 0                      // the bar's band (top or bottom)
    property int barBottom: 0
    property var keys: ({})                     // action → its key, as Settings shows it
    property string pending: ""

    flags: Qt.FramelessWindowHint
    color: "transparent"
    visible: false
    LayerShell.Window.screen: Platform.screen(output !== "" ? output : Qt.application.screens[0].name)
    LayerShell.Window.scope: "snip-tray"
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
                               | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
    LayerShell.Window.exclusionZone: -1
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityExclusive

    readonly property var entries: [
        ["area", "Capture Area"], ["fullscreen", "Capture Fullscreen"], ["window", "Capture Window"],
        ["scroll", "Scrolling Capture"], ["text", "Capture Text"], ["all-in-one", "All-In-One"], null,
        ["history", "Open History…"], ["restore", "Restore Recently Closed"], null,
        ["settings", "Settings…"], ["quit", "Quit Snip"]
    ]

    function toggle() { if (visible) close(); else open() }
    function open() {
        const a = JSON.parse(Engine.trayAnchor() || "null")
        if (!a) return
        output = a.output
        at = Qt.point(a.x, a.y)
        barTop = a.top
        barBottom = a.bottom
        const k = {}
        for (const s of JSON.parse(Engine.shortcuts() || "[]")) if (s.keys.length) k[s.id] = s.keys[0]
        keys = k
        pending = ""
        menu.opacity = 1
        visible = true
        keyCatcher.forceActiveFocus()
    }
    function close() { visible = false }
    function run(action) {
        pending = action
        menu.opacity = 0
        Platform.notifyNextFrame(tray)
    }
    Connections {
        target: Platform
        function onFramePresented(w) {
            if (w !== tray || tray.pending === "") return
            const a = tray.pending
            tray.pending = ""
            tray.close()
            if (a === "quit") Qt.quit()
            else Engine.runAction(a)
        }
    }

    MouseArea { anchors.fill: parent; acceptedButtons: Qt.AllButtons; onPressed: tray.close() }
    Item { id: keyCatcher; Keys.onEscapePressed: tray.close() }

    CardMenu {
        id: menu
        width: 270
        // under the bar (or above one at the bottom), from the click
        x: Math.max(8, Math.min(tray.at.x - 18, tray.width - width - 8))
        y: tray.barTop === 0 && tray.barBottom > 0 ? tray.height - tray.barBottom - height - 6 : tray.barTop + 6
        items: tray.entries.map(e => e ? { label: e[1], key: tray.keys[e[0]] || "", action: e[0] } : { sep: true })
        onTriggered: (action) => tray.run(action)
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
        case "open": open(); break
        case "keys": Platform.sendKeys(tray, a.slice(1).join(" ")); break
        case "where": {
            const it = find(contentItem, a[1])
            const p = it ? it.mapToItem(contentItem, it.width / 2, it.height / 2) : null
            Engine.log("tray_where", JSON.stringify({ name: a[1], x: p ? p.x : -1, y: p ? p.y : -1 }).slice(1, -1))
            break
        }
        case "state":
            Engine.log("tray_state", JSON.stringify({ open: visible, output: output,
                                                     menu: [menu.x, menu.y, menu.width, menu.height],
                                                     items: menu.items.filter(i => !i.sep).map(i => [i.action, i.key]) }).slice(1, -1))
            break
        }
    }
}
