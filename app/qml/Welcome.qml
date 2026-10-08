import QtQuick
import QtQuick.Window
import Snip.Core

// First-run setup (OMA-02): Snip's keys, start at login, the bar icon. It
// opens by itself on Snip's first start (until Get Started) and with `snip
// welcome`. The Hyprland config changes only when you click Use these keys.
Window {
    id: wel
    property var controller: null
    property var st: ({})
    property string error: ""

    title: "Welcome to Snip"
    color: Theme.background
    width: 540
    height: 640
    minimumWidth: 500
    minimumHeight: 600

    function refresh() { st = JSON.parse(Engine.welcomeState() || "{}") }
    function open() {
        error = ""
        refresh()
        show()
        raise()
        requestActivate()
    }
    function useKeys() {
        error = Engine.useKeys()
        refresh()
    }
    function done() {
        Engine.welcomeDone()
        close()
    }
    Shortcut { sequences: ["Escape", StandardKey.Close]; onActivated: wel.close() }

    Column {
        id: page
        x: 32
        y: 28
        width: parent.width - 64
        spacing: 16

        Image {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 84
            height: 84
            source: "../icons/snip.png"
            sourceSize: Qt.size(168, 168)
            mipmap: true
        }
        Column {
            width: parent.width
            spacing: 4
            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: "Welcome to Snip"
                color: Theme.text
                font.family: Theme.fontUi
                font.pixelSize: 22
                font.weight: Font.DemiBold
            }
            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: "CleanShot X's workflow for Omarchy."
                color: Theme.textSecondary
                font.family: Theme.fontUi
                font.pixelSize: 13
            }
        }

        Card {
            title: "Snip's keys"
            Grid {
                columns: 2
                columnSpacing: 14
                rowSpacing: 6
                Repeater {
                    model: ["Super+Shift+3", "Capture fullscreen", "Super+Shift+4", "Capture area (hold Space for a window)",
                            "Super+Shift+5", "All-In-One", "Super+Shift+6", "Edit the image on the clipboard"]
                    Text {
                        required property string modelData
                        required property int index
                        text: modelData
                        color: index % 2 ? Theme.text : Theme.accent
                        font.family: Theme.fontUi
                        font.pixelSize: 13
                        font.weight: index % 2 ? Font.Normal : Font.DemiBold
                    }
                }
            }
            Text {
                visible: wel.st.keysInConfig === true
                width: parent.width
                wrapMode: Text.Wrap
                text: "✓ Your Hyprland config loads Snip's keys."
                color: Theme.success
                font.family: Theme.fontUi
                font.pixelSize: 13
            }
            Text {
                visible: wel.st.keysInConfig !== true
                width: parent.width
                wrapMode: Text.Wrap
                text: "They replace Omarchy's “move window to workspace 3–6”. Use these keys adds one line to hyprland.lua; delete it to undo."
                color: Theme.textSecondary
                font.family: Theme.fontUi
                font.pixelSize: 12
            }
            Row {
                visible: wel.st.keysInConfig !== true
                spacing: 8
                TextButton { objectName: "use-keys"; text: "Use these keys"; primary: true; onClicked: wel.useKeys() }
                TextButton { objectName: "own-keys"; text: "Choose my own"; onClicked: wel.controller.openSettings("shortcuts") }
            }
            Text {
                visible: wel.error !== ""
                width: parent.width
                wrapMode: Text.Wrap
                text: wel.error
                color: Theme.danger
                font.family: Theme.fontUi
                font.pixelSize: 12
            }
        }

        Card {
            title: "Start at login"
            Item {
                visible: wel.st.autostart === true || wel.st.autostart === false
                objectName: "autostart"
                width: parent.width
                height: 24
                Rectangle {
                    id: box
                    anchors.verticalCenter: parent.verticalCenter
                    width: 16
                    height: 16
                    radius: Math.min(4, Theme.radiusControl)
                    color: wel.st.autostart ? Theme.accent : "transparent"
                    border.width: wel.st.autostart ? 0 : 1
                    border.color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.3)
                    Icon { anchors.centerIn: parent; visible: wel.st.autostart === true; name: "check"; size: 12; stroke: 3; color: Theme.onAccent }
                }
                Text {
                    x: box.width + 8
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Start Snip when you log in"
                    color: Theme.text
                    font.family: Theme.fontUi
                    font.pixelSize: 13
                }
                TapHandler {
                    onTapped: {
                        wel.error = Engine.setAutostart(!wel.st.autostart)
                        wel.refresh()
                    }
                }
            }
            Text {
                visible: wel.st.autostart !== true && wel.st.autostart !== false
                width: parent.width
                wrapMode: Text.Wrap
                text: "Snip isn't run by systemd here, so it starts when you open it."
                color: Theme.textSecondary
                font.family: Theme.fontUi
                font.pixelSize: 12
            }
        }

        Card {
            title: "In your bar"
            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: "Click Snip's icon in the bar for every capture, History and Settings."
                color: Theme.text
                font.family: Theme.fontUi
                font.pixelSize: 13
            }
        }

        TextButton {
            objectName: "welcome-done"
            anchors.right: parent.right
            text: "Get Started"
            primary: true
            onClicked: wel.done()
        }
    }

    // A step: its title, then what it holds.
    component Card: Rectangle {
        id: card
        property string title: ""
        default property alias content: inner.data
        width: page.width
        height: col.implicitHeight + 28
        radius: Math.min(12, Theme.radius + 4)
        color: Theme.surface
        border.width: 1
        border.color: Theme.hairline
        Column {
            id: col
            x: 14
            y: 14
            width: parent.width - 28
            spacing: 10
            Text {
                text: card.title
                color: Theme.text
                font.family: Theme.fontUi
                font.pixelSize: 14
                font.weight: Font.DemiBold
            }
            Column { id: inner; width: parent.width; spacing: 8 }
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
        case "close": close(); break
        case "where": {
            const it = find(contentItem, a[1])
            const p = it ? it.mapToItem(contentItem, it.width / 2, it.height / 2) : null
            Engine.log("welcome_where", JSON.stringify({ name: a[1], x: p ? p.x : -1, y: p ? p.y : -1 }).slice(1, -1))
            break
        }
        case "state":
            Engine.log("welcome_state", JSON.stringify({ open: visible, st: st, error: error }).slice(1, -1))
            break
        }
    }
}
