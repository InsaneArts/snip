import QtQuick
import QtQuick.Effects
import Snip.Core

// QAO-09 context menu, drawn by us so it follows the theme. Items whose
// feature lands in a later milestone are not shown yet. Resize… turns it
// into the size form.
Rectangle {
    id: menu
    property var card: null
    signal triggered(string action)

    property string mode: "menu"            // "resize": the W × H form
    onCardChanged: mode = "menu"
    // a recording (video or GIF): only what applies to a file
    readonly property bool isMedia: !!card && (card.data_.media || "image") !== "image"
    readonly property int imgW: card ? card.data_.width : 1
    readonly property int imgH: card ? card.data_.height : 1

    property var items: [
        { label: "Copy", key: "Ctrl+C", action: "copy", icon: "copy" },
        { label: card && card.saved ? "Show in Files" : "Save", key: card && card.saved ? "" : "Ctrl+S", action: card && card.saved ? "show" : "save", icon: card && card.saved ? "folder" : "save" },
        { label: "Save As…", key: "Ctrl+Shift+S", action: "save-as", icon: "save" },
        { label: "Annotate", key: "Ctrl+E", action: "annotate", icon: "pencil", hidden: menu.isMedia },
        { label: "Pin to Screen", key: "Ctrl+P", action: "pin", icon: "pin", hidden: menu.isMedia },
        { label: "Open With…", key: "", action: "open-with", icon: "share" },
        { label: "Extract Text", key: "", action: "text", icon: "scan-text", hidden: menu.isMedia },
        { label: "Rotate Left", key: "", action: "rotate-left", icon: "rotate-ccw", hidden: menu.isMedia },
        { label: "Flip Horizontal", key: "", action: "flip-horizontal", icon: "flip-horizontal", hidden: menu.isMedia },
        { label: "Scale to 1×", key: "", action: "scale-1x", icon: "minimize", hidden: menu.isMedia || !(card && card.data_.scale > 1) },
        { label: "Resize…", key: "", action: "resize", icon: "scaling", hidden: menu.isMedia },
        { label: "Preview", key: "Space", action: "preview", icon: "eye" },
        { label: "Print…", key: "", action: "print", icon: "printer", hidden: menu.isMedia },
        { sep: true },
        { label: "Move to Trash", key: "", action: "trash", icon: "trash", danger: true },
        { sep: true },
        { label: "Close", key: "Ctrl+W", action: "close", icon: "x" },
        { label: "Close All", key: "", action: "close-all", icon: "layers" },
        { label: "Save All", key: "", action: "save-all", icon: "save" },
        { label: "Hide", key: "", action: "hide", icon: "eye-off" }
    ]

    width: 220
    height: (mode === "resize" ? form.implicitHeight : col.implicitHeight) + 12
    radius: Math.min(Theme.radius, 12)
    color: Qt.rgba(Theme.surface.r, Theme.surface.g, Theme.surface.b, 0.98)
    border.width: 1
    border.color: Theme.hairline

    RectangularShadow {
        z: -1
        anchors.fill: parent
        radius: parent.radius
        offset.y: 8
        blur: 28
        color: Theme.shadow
    }

    Column {
        id: col
        visible: menu.mode === "menu"
        x: 6
        y: 6
        width: parent.width - 12
        Repeater {
            model: menu.items.filter(i => !i.hidden)
            delegate: Item {
                required property var modelData
                objectName: modelData.action ? "menu:" + modelData.action : ""
                width: col.width
                height: modelData.sep ? 9 : 30
                Rectangle {
                    visible: !!parent.modelData.sep
                    anchors.verticalCenter: parent.verticalCenter
                    x: 8
                    width: parent.width - 16
                    height: 1
                    color: Theme.hairline
                }
                Rectangle {
                    visible: !parent.modelData.sep
                    anchors.fill: parent
                    radius: Math.min(Theme.radiusControl, 6)
                    color: ma.pressed ? Theme.controlPressed : ma.containsMouse ? Theme.controlHover : "transparent"
                    Row {
                        x: 10
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 10
                        Icon {
                            visible: !!modelData.icon
                            name: modelData.icon || "x"
                            size: 15
                            stroke: 1.9
                            color: modelData.danger ? Theme.danger : Theme.textSecondary
                            anchors.verticalCenter: parent.verticalCenter
                        }
                        Text {
                            text: modelData.label || ""
                            color: modelData.danger ? Theme.danger : Theme.text
                            font.family: Theme.fontUi
                            font.pixelSize: 13
                            anchors.verticalCenter: parent.verticalCenter
                        }
                    }
                    Text {
                        anchors.right: parent.right
                        anchors.rightMargin: 10
                        anchors.verticalCenter: parent.verticalCenter
                        text: modelData.key || ""
                        color: Theme.textSecondary
                        font.family: Theme.fontUi
                        font.pixelSize: 11
                    }
                    MouseArea {
                        id: ma
                        anchors.fill: parent
                        hoverEnabled: true
                        enabled: !modelData.sep
                        onClicked: modelData.action === "resize" ? menu.openResize() : menu.triggered(modelData.action)
                    }
                }
            }
        }
    }

    // Resize…: width and height in pixels, proportions locked unless unlocked
    property bool lock: true
    property int rw: 1
    property int rh: 1
    function openResize() { rw = imgW; rh = imgH; lock = true; mode = "resize" }
    function setSize(which, text) {
        const v = parseInt(text)
        if (!(v > 0)) return
        if (which === "w") { rw = v; if (lock) rh = Math.max(1, Math.round(v * imgH / imgW)) }
        else { rh = v; if (lock) rw = Math.max(1, Math.round(v * imgW / imgH)) }
    }
    function applyResize() { if (rw !== imgW || rh !== imgH) triggered("resize " + rw + " " + rh); else triggered("") }
    Column {
        id: form
        visible: menu.mode === "resize"
        x: 12
        y: 12
        width: parent.width - 24
        spacing: 10
        Text {
            text: "Resize"
            color: Theme.text
            font.family: Theme.fontUi
            font.pixelSize: 13
            font.weight: Font.DemiBold
        }
        Row {
            spacing: 6
            SizeField { objectName: "resize-w"; text: menu.rw; onEdited: (t) => menu.setSize("w", t) }
            Text { anchors.verticalCenter: parent.verticalCenter; text: "×"; color: Theme.textSecondary; font.pixelSize: 13 }
            SizeField { objectName: "resize-h"; text: menu.rh; onEdited: (t) => menu.setSize("h", t) }
            Text { anchors.verticalCenter: parent.verticalCenter; text: "px"; color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 12 }
        }
        Item {
            objectName: "resize-lock"
            width: parent.width
            height: 20
            Icon { id: lockIcon; anchors.verticalCenter: parent.verticalCenter; name: "lock"; size: 14; stroke: 1.9
                   color: menu.lock ? Theme.accent : Theme.textSecondary }
            Text {
                x: lockIcon.width + 6
                anchors.verticalCenter: parent.verticalCenter
                text: menu.lock ? "Proportions locked" : "Proportions free"
                color: Theme.text
                font.family: Theme.fontUi
                font.pixelSize: 12
            }
            TapHandler { onTapped: menu.lock = !menu.lock }
        }
        Row {
            anchors.right: parent.right
            spacing: 8
            TextButton { objectName: "resize-cancel"; text: "Cancel"; onClicked: menu.triggered("") }
            TextButton { objectName: "resize-apply"; text: "Resize"; primary: true; onClicked: menu.applyResize() }
        }
    }

    component SizeField: Rectangle {
        property alias text: input.text
        signal edited(string text)
        width: 72
        height: 28
        radius: Math.min(Theme.radiusControl, 8)
        color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08)
        border.width: input.activeFocus ? 1 : 0
        border.color: Theme.accent
        TextInput {
            id: input
            anchors.fill: parent
            anchors.leftMargin: 8
            anchors.rightMargin: 8
            verticalAlignment: TextInput.AlignVCenter
            horizontalAlignment: TextInput.AlignRight
            color: Theme.text
            font.family: Theme.fontUi
            font.pixelSize: 13
            font.features: { "tnum": 1 }
            selectByMouse: true
            validator: IntValidator { bottom: 1; top: 20000 }
            onTextEdited: parent.edited(text)
            onAccepted: menu.applyResize()
        }
    }
}
