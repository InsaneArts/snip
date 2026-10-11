import QtQuick
import QtQuick.Window
import QtQuick.Effects
import org.kde.layershell as LayerShell
import Snip.Core
import "probe.js" as Probe

// Capture history (HIS-01, 03, 05), CleanShot's: a band across the top of the
// screen with large thumbnails, newest first. ←/→ or the wheel move, Enter or
// a click restores a capture to Quick Access (still editable; the band stays
// for more), a double-click annotates it, Space previews, Ctrl+C copies, a
// drag takes the file out, typing searches the text in captures, Backspace
// deletes (HIS-03; asked first, unless you said not to), right-click opens a
// menu, Esc or a click below the band closes it.
Window {
    id: hist
    property var controller: null
    property string output: ""
    property var all: []                     // the most recent captures (or what the search found)
    property var items: []                   // ...those the filter shows
    property string filter: "all"            // HIS-01: all | screenshots | videos | gifs | projects (objects still editable)
    property int current: 0
    property string query: ""
    property var restored: ({})              // ids restored this time, marked on their thumbnails

    flags: Qt.FramelessWindowHint
    color: "transparent"
    visible: false
    LayerShell.Window.screen: Platform.screen(output !== "" ? output : Qt.application.screens[0].name)
    LayerShell.Window.scope: "snip-history"
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
                               | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
    LayerShell.Window.exclusionZone: -1
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityExclusive

    function open(out) {
        output = out
        query = ""
        filter = "all"
        restored = {}
        asking = ""
        current = -1
        reload()
        visible = true
        keys.forceActiveFocus()
    }
    function close() {
        controller.preview.close()
        visible = false
    }
    function reload() {
        all = JSON.parse(Engine.history(query, 200))
        const media = (i) => i.media || "image"
        items = all.filter(i => filter === "all" || (filter === "videos" ? media(i) === "video"
                                                    : filter === "gifs" ? media(i) === "gif"
                                                    : filter === "projects" ? !!i.project
                                                    : media(i) === "image" && !i.project))
        current = current < 0 ? -1 : Math.min(current, items.length - 1)
        list.positionViewAtIndex(current, ListView.Contain)
    }
    onQueryChanged: if (visible) { current = query === "" ? -1 : 0; searchTimer.restart() }
    onFilterChanged: if (visible) { current = -1; reload() }
    Timer { id: searchTimer; interval: 120; onTriggered: hist.reload() }

    readonly property var item: current >= 0 && current < items.length ? items[current] : null
    function restore(i) {
        const it = items[i]
        if (!it) return
        Engine.restoreCard(it.id)
        restored = Object.assign({}, restored, { [it.id]: true })
    }
    function annotate(i) {
        if (!items[i]) return
        Engine.annotate(items[i].id)
        close()
    }
    function copy(i) {
        if (!items[i]) return
        Engine.copy(items[i].id)
        controller.toast.show("Copied", output)
    }
    // HIS-03: delete from history (a copy saved in your folders stays), asked first
    property string asking: ""               // the id to delete, "all" for Clear History, or ""
    // the header's width with all of it shown: while a question doesn't fit
    // beside it, only the title stays
    readonly property real headerFull: titleText.implicitWidth + filterRow.implicitWidth + searchBox.width + countText.implicitWidth + 3 * 14
    readonly property bool crowded: asking !== "" && 24 + headerFull + 24 + question.implicitWidth + 24 > width
    property bool dontAsk: false
    function askDelete(i) {
        if (!items[i]) return
        if (JSON.parse(Engine.editorPrefs() || "{}").historyAskDelete === false) deleteNow(items[i].id)
        else { asking = items[i].id; dontAsk = false }
    }
    function confirm() {
        if (dontAsk) Engine.setEditorPrefs(JSON.stringify(Object.assign(JSON.parse(Engine.editorPrefs() || "{}"), { historyAskDelete: false })))
        if (asking === "all") {
            for (const it of all) controller.quickAccess.remove(it.id, false)
            const n = Engine.clearHistory()
            controller.toast.show("Cleared " + n + (n === 1 ? " capture" : " captures"), output)
            reload()
        } else if (asking !== "") deleteNow(asking)
        asking = ""
    }
    function deleteNow(id) {
        Engine.deleteCapture(id)
        controller.quickAccess.remove(id, false)
        reload()
    }
    function act(action, i) {
        menu.visible = false
        const it = items[i]
        if (!it) return
        switch (action) {
        case "restore": restore(i); break
        case "annotate": annotate(i); break
        case "pin": Engine.pin(it.id); break
        case "copy": copy(i); break
        case "open-with": { const path = Engine.dragFile(it.id); if (path) Platform.openWith(path); break }
        case "show": Engine.showInFiles(it.id); break
        case "delete": askDelete(i); break
        }
    }
    function preview(i) {
        if (!items[i]) return
        if (controller.preview.card) controller.preview.close()
        else controller.preview.open(Object.assign({}, items[i], { output: output }))
    }

    function ago(secs) {
        const d = Math.max(0, Date.now() / 1000 - secs)
        if (d < 60) return "Just now"
        if (d < 3600) return Math.floor(d / 60) + " min ago"
        if (d < 86400) return Math.floor(d / 3600) + " h ago"
        const t = new Date(secs * 1000)
        return Qt.formatDateTime(t, d < 2 * 86400 ? "'Yesterday' hh:mm" : "d MMM, hh:mm")
    }

    // below the band: a click closes (or closes the menu first)
    MouseArea { anchors.fill: parent; onClicked: menu.visible ? menu.visible = false : hist.close() }

    Rectangle {
        id: band
        anchors { left: parent.left; right: parent.right; top: parent.top }
        height: 292
        color: Qt.rgba(Theme.surface.r, Theme.surface.g, Theme.surface.b, 0.97)
        MouseArea { anchors.fill: parent }        // clicks on the band stay here
        Rectangle { anchors { left: parent.left; right: parent.right; bottom: parent.bottom } height: 1; color: Theme.hairline }
        RectangularShadow {
            z: -1
            anchors.fill: parent
            offset.y: 6; blur: 24
            color: Theme.shadow
        }

        // the title, the search (type to search) and the count
        Row {
            id: header
            x: 24; y: 16
            spacing: 14
            Text { id: titleText; text: "Capture History"; color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 15; font.weight: Font.DemiBold
                   anchors.verticalCenter: parent.verticalCenter }
            // HIS-01
            Row {
                id: filterRow
                visible: !hist.crowded
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2
                Repeater {
                    model: [["all", "All"], ["screenshots", "Screenshots"], ["videos", "Videos"], ["gifs", "GIFs"], ["projects", "Projects"]]
                    Rectangle {
                        required property var modelData
                        readonly property bool on: hist.filter === modelData[0]
                        objectName: "filter-" + modelData[0]
                        width: filterLabel.implicitWidth + 20
                        height: 26
                        radius: 13
                        color: on ? Theme.accent : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, filterHover.hovered ? 0.12 : 0.06)
                        Text {
                            id: filterLabel
                            anchors.centerIn: parent
                            text: parent.modelData[1]
                            color: parent.on ? Theme.onAccent : Theme.text
                            font.family: Theme.fontUi
                            font.pixelSize: 12
                            font.weight: parent.on ? Font.DemiBold : Font.Normal
                        }
                        HoverHandler { id: filterHover; cursorShape: Qt.PointingHandCursor }
                        TapHandler { onTapped: hist.filter = parent.modelData[0] }
                    }
                }
            }
            Rectangle {
                id: searchBox
                visible: !hist.crowded
                anchors.verticalCenter: parent.verticalCenter
                width: Math.min(320, Math.max(180, searchText.implicitWidth + 44)); height: 28; radius: 14
                color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08)
                border.width: hist.query !== "" ? 1 : 0; border.color: Theme.accent
                Icon { x: 10; anchors.verticalCenter: parent.verticalCenter; name: "search"; size: 14; color: Theme.textSecondary }
                Text {
                    id: searchText
                    x: 30; anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - 40
                    elide: hist.query !== "" ? Text.ElideLeft : Text.ElideRight     // the end of what's typed
                    text: hist.query !== "" ? hist.query : "Type to search the text in captures"
                    color: hist.query !== "" ? Theme.text : Theme.textSecondary
                    font.family: Theme.fontUi; font.pixelSize: 12
                }
            }
            Text {
                id: countText
                visible: !hist.crowded
                anchors.verticalCenter: parent.verticalCenter
                text: hist.items.length === 0 ? (hist.query !== "" || hist.filter !== "all" ? "Nothing matches" : "No captures yet")
                      : hist.items.length + (hist.items.length === 1 ? " capture" : " captures")
                color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 12
            }
        }
        Row {
            id: actions
            visible: hist.asking === ""
            anchors { right: parent.right; rightMargin: 24; verticalCenter: header.verticalCenter }
            spacing: 16
            Text { anchors.verticalCenter: parent.verticalCenter
                   visible: 24 + hist.headerFull + 24 + implicitWidth + 16 + clearButton.width + 24 <= hist.width   // only with room
                   text: "←→ Move   ⏎ Restore   Space Preview   ⌫ Delete   Esc Close"
                   color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 11 }
            ToolButton { id: clearButton; label: "Clear History…"; enabled: hist.all.length > 0
                         onClicked: { hist.asking = "all"; hist.dontAsk = false } }
        }
        // the question before deleting
        Row {
            id: question
            visible: hist.asking !== ""
            anchors { right: parent.right; rightMargin: 24; verticalCenter: header.verticalCenter }
            spacing: 12
            Text { anchors.verticalCenter: parent.verticalCenter
                   text: hist.asking === "all" ? "Delete all " + hist.all.length + " captures from history?"
                                               : "Delete this capture from history?"
                   color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 13; font.weight: Font.DemiBold }
            Text { anchors.verticalCenter: parent.verticalCenter
                   text: "Copies saved in your folders stay."
                   color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 12 }
            Row {
                visible: hist.asking !== "all"
                anchors.verticalCenter: parent.verticalCenter
                spacing: 6
                Rectangle {
                    width: 16; height: 16; radius: 4
                    color: hist.dontAsk ? Theme.accent : "transparent"
                    border.width: hist.dontAsk ? 0 : 1.5; border.color: Theme.textSecondary
                    Icon { visible: hist.dontAsk; anchors.centerIn: parent; name: "check"; size: 12; stroke: 2.5; color: Theme.background }
                    MouseArea { anchors.fill: parent; anchors.margins: -4; cursorShape: Qt.PointingHandCursor; onClicked: hist.dontAsk = !hist.dontAsk }
                }
                Text { text: "Don't ask again"; color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 12 }
            }
            ToolButton { label: "Cancel"; onClicked: hist.asking = "" }
            ToolButton { label: hist.asking === "all" ? "Delete All" : "Delete"; selected: true; onClicked: hist.confirm() }
        }

        ListView {
            id: list
            anchors { left: parent.left; right: parent.right; top: header.bottom; topMargin: 18; bottom: parent.bottom; bottomMargin: 14 }
            leftMargin: 24; rightMargin: 24
            orientation: ListView.Horizontal
            spacing: 16
            clip: true
            model: hist.items
            currentIndex: hist.current
            highlightFollowsCurrentItem: false
            boundsBehavior: Flickable.StopAtBounds
            onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)
            delegate: Item {
                id: entry
                required property var modelData
                required property int index
                readonly property bool focused: index === hist.current
                readonly property real th: 168
                width: Math.max(110, Math.min(320, th * modelData.width / Math.max(1, modelData.height)))
                height: list.height

                Item {
                    id: thumb
                    objectName: "thumb:" + entry.index
                    y: 4                                  // room for the ring: the list clips at its edge
                    width: parent.width; height: entry.th
                    layer.enabled: true
                    layer.effect: MultiEffect { shadowEnabled: true; shadowBlur: 0.5; shadowOpacity: 0.3; shadowVerticalOffset: 3 }
                    Image {
                        id: picture
                        anchors.fill: parent
                        source: "file://" + (entry.modelData.thumb || entry.modelData.path) + "?" + (entry.modelData.rev || 0)
                        sourceSize.height: entry.th * 2
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                        visible: false
                    }
                    Item {
                        id: roundMask
                        anchors.fill: parent
                        visible: false
                        layer.enabled: true
                        Rectangle { anchors.fill: parent; radius: 10; color: "white" }
                    }
                    MultiEffect { anchors.fill: parent; source: picture; maskEnabled: true; maskSource: roundMask }
                }
                Rectangle {
                    visible: entry.focused
                    x: -3; y: thumb.y - 3; width: thumb.width + 6; height: thumb.height + 6
                    radius: 13; color: "transparent"
                    border.width: 2; border.color: Theme.accent
                }
                // restored this time
                Rectangle {
                    visible: !!hist.restored[entry.modelData.id]
                    x: 8; y: thumb.y + 8; width: 22; height: 22; radius: 11; color: Theme.accent
                    Icon { anchors.centerIn: parent; name: "check"; size: 13; stroke: 2.6; color: "white" }
                }
                Column {
                    y: thumb.y + entry.th + 8; width: parent.width
                    spacing: 2
                    Text {
                        width: parent.width; elide: Text.ElideRight
                        text: hist.ago(entry.modelData.created)
                        color: Theme.text
                        font.family: Theme.fontUi; font.pixelSize: 12
                    }
                    Text {
                        width: parent.width; elide: Text.ElideRight
                        text: entry.modelData.app || entry.modelData.title || (entry.modelData.width + " × " + entry.modelData.height)
                        color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 11
                    }
                }
                MouseArea {
                    anchors.fill: thumb
                    cursorShape: Qt.PointingHandCursor
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    property point at
                    property bool dragged: false
                    property bool wasChosen: false
                    onPressed: (m) => {
                        at = Qt.point(m.x, m.y); dragged = false; wasChosen = entry.focused
                        if (m.button === Qt.RightButton) {
                            hist.current = entry.index
                            const p = mapToItem(null, m.x, m.y)
                            menu.x = Math.min(hist.width - menu.width - 8, p.x); menu.y = p.y
                            menu.visible = true
                        }
                    }
                    onPositionChanged: (m) => {
                        if (!pressed || dragged || Math.hypot(m.x - at.x, m.y - at.y) < 6) return
                        dragged = true
                        const path = Engine.dragFile(entry.modelData.id)
                        if (path) Platform.startDrag(thumb, path, entry.modelData.path)
                    }
                    // a click chooses it (again: lets it go); only Restore brings its card back
                    onClicked: (m) => { if (!dragged && m.button === Qt.LeftButton) hist.current = wasChosen ? -1 : entry.index }
                    onDoubleClicked: hist.annotate(entry.index)
                }
                TextButton {
                    objectName: "restore:" + entry.index
                    visible: entry.focused
                    anchors.horizontalCenter: thumb.horizontalCenter
                    anchors.bottom: thumb.bottom
                    anchors.bottomMargin: 10
                    primary: true
                    text: "Restore"
                    tip: "Back to Quick Access (Enter)"
                    onClicked: hist.restore(entry.index)
                }
            }
        }
    }

    CardMenu {
        id: menu
        z: 10
        visible: false
        items: [
            { label: "Restore", key: "⏎", action: "restore", icon: "layers" },
            { label: "Annotate", key: "Ctrl+E", action: "annotate", icon: "pencil" },
            { label: "Pin to Screen", key: "", action: "pin", icon: "pin" },
            { label: "Copy", key: "Ctrl+C", action: "copy", icon: "copy" },
            { label: "Open With…", key: "", action: "open-with", icon: "share" },
            { label: "Show in Files", key: "", action: "show", icon: "folder" },
            { sep: true },
            { label: "Delete", key: "⌫", action: "delete", icon: "trash", danger: true }
        ]
        onTriggered: (action) => hist.act(action, hist.current)
    }

    Item {
        id: keys
        focus: true
        Keys.onPressed: (e) => {
            const ctrl = e.modifiers & Qt.ControlModifier
            if (hist.asking !== "") {                          // the question takes the keys
                if (e.key === Qt.Key_Return || e.key === Qt.Key_Enter) hist.confirm()
                else if (e.key === Qt.Key_Escape) hist.asking = ""
                e.accepted = true
                return
            }
            if (menu.visible && e.key === Qt.Key_Escape) { menu.visible = false; e.accepted = true; return }
            if (e.key === Qt.Key_Escape) {
                if (hist.query !== "") hist.query = ""
                else hist.close()
            } else if (e.key === Qt.Key_Right || e.key === Qt.Key_Down) hist.current = Math.min(hist.items.length - 1, hist.current + 1)
            else if (e.key === Qt.Key_Left || e.key === Qt.Key_Up) hist.current = Math.max(0, hist.current - 1)
            else if (e.key === Qt.Key_Home) hist.current = 0
            else if (e.key === Qt.Key_End) hist.current = hist.items.length - 1
            else if (e.key === Qt.Key_Return || e.key === Qt.Key_Enter) hist.restore(hist.current)
            else if (e.key === Qt.Key_Space && hist.query === "") hist.preview(hist.current)
            else if (ctrl && e.key === Qt.Key_C) hist.copy(hist.current)
            else if (ctrl && e.key === Qt.Key_E) hist.annotate(hist.current)
            else if (e.key === Qt.Key_Backspace && hist.query !== "") hist.query = hist.query.slice(0, -1)
            else if (e.key === Qt.Key_Backspace || e.key === Qt.Key_Delete) hist.askDelete(hist.current)
            else if (!ctrl && e.text.length === 1 && e.text >= " ") hist.query += e.text   // typing searches
            else return
            e.accepted = true
        }
    }
    WheelHandler {
        target: null
        onWheel: (w) => {
            const step = (w.angleDelta.y || w.angleDelta.x) < 0 ? 1 : -1
            hist.current = Math.max(0, Math.min(hist.items.length - 1, hist.current + step))
        }
    }

    // test hooks (snip debug ui history ...)
    function debug(a) {
        switch (a[0]) {
        case "keys": Platform.sendKeys(hist, a.slice(1).join(" ")); break
        case "annotate": annotate(current); break
        case "act": act(a[1], current); break
        case "clear": asking = "all"; break
        case "where": Engine.log("history_where", JSON.stringify(Probe.where(hist, a[1])).slice(1, -1)); break
        case "state":
            Engine.log("history_state", JSON.stringify({ open: visible, items: items.map(i => i.id), current: current, query: query, filter: filter,
                                                        restored: Object.keys(restored), asking: asking, menu: menu.visible,
                                                        // tests: the header, and the actions or question at its right
                                                        header: [header.x, header.y, header.width, header.height],
                                                        right: (r => [r.x, r.y, r.width, r.height])(asking === "" ? actions : question) }).slice(1, -1))
            break
        }
    }
}
