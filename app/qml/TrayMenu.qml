import QtQuick
import QtQuick.Window
import org.kde.layershell as LayerShell
import Snip.Core
import "probe.js" as Probe

// Snip's menu in the bar (OMA-08, D35): CleanShot's, in its order (from its
// MainMenu.nib), less what Snip hasn't got yet (Check for Updates); Hide
// Desktop Icons is Hide Desktop Clutter (CAP-12). A click on
// the bar icon opens it under the icon.
// A full-output layer, so a click anywhere else (the icon too) closes it, as
// Esc does. An entry runs once the menu is off screen, so it's never in a
// capture.
Window {
    id: tray
    property var controller: null
    property string output: ""
    property point at: Qt.point(0, 0)          // the click on the output (logical px)
    property int barTop: 0                      // the bar's band (top or bottom)
    property int barBottom: 0
    property var keys: ({})                     // action → its key, as Settings shows it
    // REC-06: Snip's menu stops a recording that runs, and pauses it (D52)
    readonly property string recordPhase: controller ? controller.recordHud.phase : "closed"
    readonly property bool recording: ["countdown", "starting", "launching", "recording", "paused"].includes(recordPhase)
    readonly property bool pausable: recordPhase === "recording" || recordPhase === "paused"

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
        ["all-in-one", "All-In-One"], ["area", "Capture Area"], ["previous", "Capture Previous Area"],
        ["fullscreen", "Capture Fullscreen"], ["window", "Capture Window"], ["timer-area", "Self-Timer"],
        ["scroll", "Scrolling Capture"], ["text", "Capture Text"],
        ["record", tray.recording ? "Stop Recording" : "Record Screen"]
    ].concat(tray.pausable ? [["record-pause", tray.recordPhase === "paused" ? "Resume Recording" : "Pause Recording"]] : [], [null,
        ["open", "Open…"], ["edit-clipboard", "Open from Clipboard"], ["pin", "Pin to the Screen…"],
        ["history", "Capture History…"], ["restore", "Restore Recently Closed"], null,
        ["desktop-toggle", tray.controller && tray.controller.quickAccess.clutterHidden ? "Show Desktop Clutter" : "Hide Desktop Clutter"], null,
        ["settings", "Settings…"], ["about", "About Snip"], ["quit", "Quit Snip"]
    ])

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
        menu.opacity = 1
        visible = true
        keyCatcher.forceActiveFocus()
    }
    function close() { visible = false }
    // an entry runs once the menu is off screen, so it's never in a capture
    function run(a) {
        menu.opacity = 0
        Platform.afterFrames([tray], () => {
            tray.close()
            if (a === "quit") Qt.quit()
            else if (a === "about") tray.controller.openSettings("about")
            else if (a === "open" || a === "pin") {
                Platform.openImageDialog(a === "open" ? "Open" : "Pin to the Screen", (path) => {
                    if (path === "") return
                    if (a === "open") Engine.editFile(path)
                    else Engine.pinFile(path)
                })
            } else Engine.runAction(a)
        })
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
    function debug(a) {
        switch (a[0]) {
        case "open": open(); break
        case "keys": Platform.sendKeys(tray, a.slice(1).join(" ")); break
        case "where": Engine.log("tray_where", JSON.stringify(Probe.where(tray, a[1])).slice(1, -1)); break
        case "state":
            Engine.log("tray_state", JSON.stringify({ open: visible, output: output,
                                                     menu: [menu.x, menu.y, menu.width, menu.height],
                                                     items: menu.items.filter(i => !i.sep).map(i => [i.action, i.key]),
                                                     labels: menu.items.filter(i => !i.sep).map(i => i.label) }).slice(1, -1))
            break
        }
    }
}
