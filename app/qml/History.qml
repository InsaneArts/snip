import QtQuick
import QtQuick.Window
import QtQuick.Effects
import org.kde.layershell as LayerShell
import Snip.Core

// Capture history (HIS-01, 03, 05), CleanShot's: a band across the top of the
// screen with large thumbnails, newest first. ←/→ or the wheel move, Enter or
// a click restores a capture to Quick Access (still editable; the band stays
// for more), a double-click annotates it, Space previews, Ctrl+C copies, a
// drag takes the file out, typing searches the text in captures, Esc or a
// click below the band closes it.
Window {
    id: hist
    property var controller: null
    property string output: ""
    property var items: []
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
        restored = {}
        current = 0
        reload()
        visible = true
        keys.forceActiveFocus()
    }
    function close() {
        controller.preview.close()
        visible = false
    }
    function reload() {
        items = JSON.parse(Engine.history(query, 200))
        current = Math.max(0, Math.min(current, items.length - 1))
        list.positionViewAtIndex(current, ListView.Contain)
    }
    onQueryChanged: if (visible) { current = 0; searchTimer.restart() }
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

    // below the band: a click closes
    MouseArea { anchors.fill: parent; onClicked: hist.close() }

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
            Text { text: "Capture History"; color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 15; font.weight: Font.DemiBold
                   anchors.verticalCenter: parent.verticalCenter }
            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: Math.max(180, searchText.implicitWidth + 44); height: 28; radius: 14
                color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08)
                border.width: hist.query !== "" ? 1 : 0; border.color: Theme.accent
                Icon { x: 10; anchors.verticalCenter: parent.verticalCenter; name: "search"; size: 14; color: Theme.textSecondary }
                Text {
                    id: searchText
                    x: 30; anchors.verticalCenter: parent.verticalCenter
                    text: hist.query !== "" ? hist.query : "Type to search the text in captures"
                    color: hist.query !== "" ? Theme.text : Theme.textSecondary
                    font.family: Theme.fontUi; font.pixelSize: 12
                }
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: hist.items.length === 0 ? (hist.query !== "" ? "Nothing matches" : "No captures yet")
                      : hist.items.length + (hist.items.length === 1 ? " capture" : " captures")
                color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 12
            }
        }
        Text {
            anchors { right: parent.right; rightMargin: 24; verticalCenter: header.verticalCenter }
            text: "←→ Move   ⏎ Restore   Space Preview   Esc Close"
            color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 11
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
                    width: parent.width; height: entry.th
                    layer.enabled: true
                    layer.effect: MultiEffect { shadowEnabled: true; shadowBlur: 0.5; shadowOpacity: 0.3; shadowVerticalOffset: 3 }
                    Image {
                        id: picture
                        anchors.fill: parent
                        source: "file://" + entry.modelData.path + "?" + (entry.modelData.rev || 0)
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
                    x: -3; y: -3; width: thumb.width + 6; height: thumb.height + 6
                    radius: 13; color: "transparent"
                    border.width: 2; border.color: Theme.accent
                }
                // restored this time
                Rectangle {
                    visible: !!hist.restored[entry.modelData.id]
                    x: 8; y: 8; width: 22; height: 22; radius: 11; color: Theme.accent
                    Icon { anchors.centerIn: parent; name: "check"; size: 13; stroke: 2.6; color: "white" }
                }
                Column {
                    y: entry.th + 8; width: parent.width
                    spacing: 2
                    Text {
                        width: parent.width; elide: Text.ElideRight
                        text: entry.focused ? "⏎ Restore" : hist.ago(entry.modelData.created)
                        color: entry.focused ? Theme.accent : Theme.text
                        font.family: Theme.fontUi; font.pixelSize: 12; font.weight: entry.focused ? Font.DemiBold : Font.Normal
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
                    property point at
                    property bool dragged: false
                    onPressed: (m) => { at = Qt.point(m.x, m.y); dragged = false; hist.current = entry.index }
                    onPositionChanged: (m) => {
                        if (!pressed || dragged || Math.hypot(m.x - at.x, m.y - at.y) < 6) return
                        dragged = true
                        const path = Engine.dragFile(entry.modelData.id)
                        if (path) Platform.startDrag(thumb, path, entry.modelData.path)
                    }
                    onClicked: if (!dragged) hist.restore(entry.index)
                    onDoubleClicked: hist.annotate(entry.index)
                }
            }
        }
    }

    Item {
        id: keys
        focus: true
        Keys.onPressed: (e) => {
            const ctrl = e.modifiers & Qt.ControlModifier
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
        case "state":
            Engine.log("history_state", JSON.stringify({ open: visible, items: items.map(i => i.id), current: current, query: query,
                                                        restored: Object.keys(restored) }).slice(1, -1))
            break
        }
    }
}
