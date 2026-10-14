import QtQuick
import QtQuick.Window
import Snip.Core
import "probe.js" as Probe

// First-run setup (OMA-02, OMA-10, D65), in three steps. Keys: Snip's keys,
// and Print Screen's if ticked (D66). Try it: a first capture, then what its
// card does. Ready: start at login, the bar icon, `snip`. It opens by itself
// on Snip's first start (until Get Started) and with `snip welcome`. The
// Hyprland config changes only when you click Use these keys.
Window {
    id: wel
    property var controller: null
    property var st: ({})
    property string error: ""
    // 0 Keys, 1 Try it, 2 Ready
    property int step: 0
    // D66: offered ticked, and written only with the keys
    property bool printToo: true
    // a capture's card showed since the Welcome opened
    property bool tried: false
    // Capture Area's keys now ("" with none)
    property string areaKey: ""

    title: "Welcome to Snip"
    color: Theme.background
    width: 540
    height: 580
    minimumWidth: 500
    minimumHeight: 560

    function refresh() {
        st = JSON.parse(Engine.welcomeState() || "{}")
        const area = JSON.parse(Engine.shortcuts() || "[]").find(s => s.id === "area")
        areaKey = area && area.keys.length ? area.keys[0] : ""
    }
    function open() {
        error = ""
        step = 0
        tried = false
        refresh()
        show()
        raise()
        requestActivate()
    }
    function useKeys(print) {
        error = Engine.useKeys(print)
        refresh()
    }
    // a capture's card showed (Main)
    function captured() { tried = true }
    function go(to) {
        error = ""
        step = to
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
        y: 24
        width: parent.width - 64
        spacing: 14

        Image {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 64
            height: 64
            source: "../icons/snip.png"
            sourceSize: Qt.size(128, 128)
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
        // the steps, the one you're on in the accent
        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 18
            Repeater {
                model: ["Keys", "Try it", "Ready"]
                Row {
                    required property string modelData
                    required property int index
                    spacing: 6
                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 8
                        height: 8
                        radius: 4
                        color: index === wel.step ? Theme.accent : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.25)
                    }
                    Text {
                        text: modelData
                        color: index === wel.step ? Theme.text : Theme.textSecondary
                        font.family: Theme.fontUi
                        font.pixelSize: 13
                        font.weight: index === wel.step ? Font.DemiBold : Font.Normal
                    }
                }
            }
        }

        // Keys
        Card {
            visible: wel.step === 0
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
            Check {
                objectName: "print-keys"
                visible: wel.st.keysInConfig !== true
                checked: wel.printToo
                text: "Print Screen too"
                onToggled: wel.printToo = !wel.printToo
            }
            Note {
                visible: wel.st.keysInConfig !== true
                leftPadding: 24
                text: "Print captures an area and Super+Ctrl+Print reads text, in place of Omarchy's screenshot and text keys."
            }
            Note {
                visible: wel.st.keysInConfig !== true
                text: "Snip's keys replace Omarchy's “move window to workspace 3–6”. Use these keys adds a few lines to hyprland.lua; delete them to undo."
            }
            Row {
                visible: wel.st.keysInConfig !== true
                spacing: 8
                TextButton { objectName: "use-keys"; text: "Use these keys"; primary: true; onClicked: wel.useKeys(wel.printToo) }
                TextButton { objectName: "own-keys"; text: "Choose my own"; onClicked: wel.controller.openSettings("shortcuts") }
            }
            Text {
                visible: wel.st.keysInConfig === true
                width: parent.width
                wrapMode: Text.Wrap
                text: wel.st.printKeys ? "✓ Your Hyprland config loads Snip's keys, Print Screen's too." : "✓ Your Hyprland config loads Snip's keys."
                color: Theme.success
                font.family: Theme.fontUi
                font.pixelSize: 13
            }
            // the keys are there, Print Screen isn't: offered on its own
            Note {
                visible: wel.st.keysInConfig === true && wel.st.printKeys !== true
                text: "Print can capture an area and Super+Ctrl+Print read text, in place of Omarchy's screenshot and text keys."
            }
            TextButton {
                objectName: "use-print"
                visible: wel.st.keysInConfig === true && wel.st.printKeys !== true
                text: "Use Print Screen too"
                onClicked: wel.useKeys(true)
            }
        }

        // Try it
        Card {
            visible: wel.step === 1
            title: wel.tried ? "That's your capture" : "Take your first capture"
            Text {
                visible: !wel.tried
                width: parent.width
                wrapMode: Text.Wrap
                textFormat: Text.StyledText
                text: (wel.areaKey ? "Press <b>" + wel.areaKey + "</b>" : "Click Capture Area")
                      + ", then drag over anything on screen. Hold Space to pick a window instead."
                color: Theme.text
                font.family: Theme.fontUi
                font.pixelSize: 13
            }
            TextButton {
                objectName: "try-capture"
                visible: !wel.tried
                text: "Capture Area"
                primary: !wel.areaKey
                onClicked: Engine.runAction("area")
            }
            Text {
                visible: wel.tried
                width: parent.width
                wrapMode: Text.Wrap
                text: "✓ It's the card at the bottom " + (wel.controller && wel.controller.quickAccess.right ? "right" : "left") + " of your screen."
                color: Theme.success
                font.family: Theme.fontUi
                font.pixelSize: 13
            }
            Grid {
                visible: wel.tried
                columns: 2
                columnSpacing: 14
                rowSpacing: 6
                Repeater {
                    model: ["Hover it", "Copy, Save and more", "Drag it", "into any app, chat or folder",
                            "Double-click", "to annotate it", "Close it", "History keeps it"]
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
            Note {
                visible: wel.tried
                text: "Nothing goes to your folders until you Save. Snip's icon › History has your captures."
            }
        }

        // Ready
        Card {
            visible: wel.step === 2
            title: "Start at login"
            Check {
                objectName: "autostart"
                visible: wel.st.autostart === true || wel.st.autostart === false
                checked: wel.st.autostart === true
                text: "Start Snip when you log in"
                onToggled: {
                    wel.error = Engine.setAutostart(!wel.st.autostart)
                    wel.refresh()
                }
            }
            Note {
                visible: wel.st.autostart !== true && wel.st.autostart !== false
                text: "Snip isn't run by systemd here, so it starts when you open it."
            }
        }
        Card {
            visible: wel.step === 2
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
        Card {
            visible: wel.step === 2
            title: "In a terminal"
            Text {
                text: "snip area   snip record   snip history"
                color: Theme.text
                font.family: Theme.fontMono
                font.pixelSize: 13
            }
            Note { text: "and snip alone lists every command." }
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

    Row {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: 28
        spacing: 8
        TextButton { objectName: "welcome-back"; visible: wel.step > 0; text: "Back"; onClicked: wel.go(wel.step - 1) }
        TextButton { objectName: "welcome-next"; visible: wel.step < 2; text: "Next"; primary: true; onClicked: wel.go(wel.step + 1) }
        TextButton { objectName: "welcome-done"; visible: wel.step === 2; text: "Get Started"; primary: true; onClicked: wel.done() }
    }

    // A step's card: its title, then what it holds.
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

    // A tick box and what it's for.
    component Check: Item {
        id: chk
        property bool checked: false
        property string text: ""
        signal toggled()
        width: parent ? parent.width : 0
        height: 24
        Rectangle {
            id: box
            anchors.verticalCenter: parent.verticalCenter
            width: 16
            height: 16
            radius: Math.min(4, Theme.radiusControl)
            color: chk.checked ? Theme.accent : "transparent"
            border.width: chk.checked ? 0 : 1
            border.color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.3)
            Icon { anchors.centerIn: parent; visible: chk.checked; name: "check"; size: 12; stroke: 3; color: Theme.onAccent }
        }
        Text {
            x: box.width + 8
            anchors.verticalCenter: parent.verticalCenter
            text: chk.text
            color: Theme.text
            font.family: Theme.fontUi
            font.pixelSize: 13
        }
        TapHandler { onTapped: chk.toggled() }
    }

    // Small print under a step's controls.
    component Note: Text {
        width: parent ? parent.width : 0
        wrapMode: Text.Wrap
        color: Theme.textSecondary
        font.family: Theme.fontUi
        font.pixelSize: 12
    }

    // tests
    function debug(a) {
        switch (a[0]) {
        case "close": close(); break
        case "where": Engine.log("welcome_where", JSON.stringify(Probe.where(wel, a[1])).slice(1, -1)); break
        case "state":
            Engine.log("welcome_state", JSON.stringify({ open: visible, step: step, tried: tried, areaKey: areaKey, printToo: printToo, st: st, error: error }).slice(1, -1))
            break
        }
    }
}
