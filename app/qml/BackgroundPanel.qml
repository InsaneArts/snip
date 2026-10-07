import QtQuick
import Snip.Core
import "editor.js" as Ed

// The Background tool's panel (BG-01…06), CleanShot's sidebar: presets and
// None, sources (gradients, the Omarchy theme's wallpapers, blurred, plain
// colours), then padding, inset, shadow, corners, ratio and alignment.
Rectangle {
    id: panel
    required property var ed
    readonly property var bg: ed.bgLive
    readonly property var walls: JSON.parse(Engine.wallpapers() || "[]")
    property bool naming: false
    onNamingChanged: if (naming) { presetName.text = ""; presetName.forceActiveFocus() }
    function finishNaming(save) {
        if (save) ed.saveBgPreset(presetName.text)
        naming = false
        ed.focusKeys()
    }
    color: Theme.surface
    Rectangle { anchors { left: parent.left; top: parent.top; bottom: parent.bottom } width: 1; color: Theme.hairline }

    component Heading: Text { color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 12; font.weight: Font.DemiBold; topPadding: 6 }
    component Tile: Rectangle {
        property bool current: false
        signal picked()
        width: 50; height: 34; radius: 7
        border.width: current ? 2 : 1
        border.color: current ? Theme.accent : Theme.hairline
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: parent.picked() }
    }

    Flickable {
        anchors.fill: parent
        anchors.margins: 16
        contentHeight: col.implicitHeight
        clip: true
        Column {
            id: col
            width: parent.width
            spacing: 10

            // presets (BG-03): a dropdown and +, as in CleanShot
            Row {
                visible: !panel.naming
                width: parent.width; spacing: 6
                Rectangle {
                    id: presetsButton
                    width: parent.width - addPreset.width - 6; height: 30; radius: 8
                    color: presetsMa.containsMouse ? Theme.controlHover : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08)
                    Text { x: 10; anchors.verticalCenter: parent.verticalCenter; width: parent.width - 34; elide: Text.ElideRight
                           text: panel.ed.bgPreset ? panel.ed.bgPreset + (panel.ed.bgPresetChanged ? " (edited)" : "") : "Presets…"
                           color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 13 }
                    Icon { anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                           name: "chevron-down"; size: 14; color: Theme.textSecondary }
                    MouseArea { id: presetsMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                                onClicked: panel.ed.openMenuNamed("presets", presetsButton) }
                }
                ToolButton { id: addPreset; icon: "plus"; tip: "Add New Preset…"; onClicked: panel.naming = true }
            }
            // the new preset's name
            Rectangle {
                visible: panel.naming
                width: parent.width; height: 30; radius: 8
                color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08)
                border.width: 1; border.color: Theme.accent
                TextInput {
                    id: presetName
                    anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 10
                    verticalAlignment: TextInput.AlignVCenter
                    color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 13
                    Text { visible: !parent.text; anchors.verticalCenter: parent.verticalCenter; text: "Preset name, then Enter"
                           color: Theme.textSecondary; font: parent.font }
                    Keys.onReturnPressed: panel.finishNaming(true)
                    Keys.onEnterPressed: panel.finishNaming(true)
                    Keys.onEscapePressed: panel.finishNaming(false)
                }
            }
            Rectangle {
                width: parent.width; height: 30; radius: 8
                color: noneMa.containsMouse ? Theme.controlHover : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08)
                border.width: panel.bg ? 0 : 2; border.color: Theme.accent
                Text { anchors.centerIn: parent; text: "None"; color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 13; font.weight: Font.DemiBold }
                MouseArea { id: noneMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: { panel.ed.setBg(null); panel.ed.bgPreset = "" } }
            }

            Heading { text: "Gradients" }
            Grid {
                columns: 5; spacing: 6
                Repeater {
                    model: Ed.gradients.length
                    delegate: Tile {
                        required property int index
                        current: !!panel.bg && panel.bg.kind === "gradient" && panel.bg.value === index
                        BackgroundLayer {
                            bg: ({ kind: "gradient", value: parent.index, padding: 0, inset: 0, radius: 0, shadow: 0 })
                            rect: ({ x: 2, y: 2, w: 46, h: 30 })
                            inner: ({ x: 0, y: 0, w: 0, h: 0 })
                        }
                        onPicked: panel.ed.setBg({ kind: "gradient", value: index })
                    }
                }
            }

            Heading { text: "Wallpapers"; visible: panel.walls.length > 0 }
            Grid {
                columns: 5; spacing: 6
                Repeater {
                    model: panel.walls
                    delegate: Tile {
                        required property string modelData
                        current: !!panel.bg && panel.bg.kind === "wallpaper" && panel.bg.value === modelData
                        clip: true
                        Image { anchors.fill: parent; anchors.margins: 2; source: "file://" + parent.modelData; fillMode: Image.PreserveAspectCrop
                                sourceSize: Qt.size(120, 80); asynchronous: true }
                        onPicked: panel.ed.setBg({ kind: "wallpaper", value: modelData })
                    }
                }
            }

            Heading { text: "Blurred" }
            Row {
                spacing: 6
                Tile {
                    visible: panel.walls.length > 0
                    current: !!panel.bg && panel.bg.kind === "blur-wallpaper"
                    clip: true
                    Image { anchors.fill: parent; anchors.margins: 2; source: panel.walls.length ? "file://" + panel.walls[0] : ""
                            fillMode: Image.PreserveAspectCrop; sourceSize: Qt.size(60, 40); smooth: true }
                    onPicked: panel.ed.setBg({ kind: "blur-wallpaper", value: panel.walls[0] })
                }
                Tile {
                    current: !!panel.bg && panel.bg.kind === "blur-image"
                    clip: true
                    Image { anchors.fill: parent; anchors.margins: 2; source: "file://" + panel.ed.basePath
                            fillMode: Image.PreserveAspectCrop; sourceSize: Qt.size(40, 26); smooth: true }
                    onPicked: panel.ed.setBg({ kind: "blur-image", value: "" })
                }
            }

            Heading { text: "Plain color" }
            Grid {
                columns: 7; spacing: 6
                Repeater {
                    model: ["theme-a", "theme-b"].concat(Ed.palette).concat(["transparent"])
                    delegate: Rectangle {
                        required property string modelData
                        width: 28; height: 28; radius: 14
                        color: modelData === "theme-a" ? Theme.accent : modelData === "theme-b" ? (Theme.signature.length > 1 ? Theme.signature[1] : Theme.background) : modelData
                        border.width: !!panel.bg && panel.bg.kind === "color" && panel.bg.value === modelData ? 2 : 1
                        border.color: !!panel.bg && panel.bg.kind === "color" && panel.bg.value === modelData ? Theme.accent : Theme.hairline
                        Rectangle { visible: parent.modelData === "transparent"; anchors.centerIn: parent; width: 18; height: 2; rotation: -45; color: Theme.danger }
                        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor
                                    onClicked: panel.ed.setBg({ kind: "color", value: parent.modelData }) }
                    }
                }
            }

            Slider { width: parent.width; label: "Padding"; from: 0; to: 240; unit: " px"
                     value: panel.bg ? panel.bg.padding / panel.ed.unit : 0
                     onMoved: (v) => panel.ed.previewBg({ padding: Math.round(v * panel.ed.unit) })
                     onReleased: (v) => panel.ed.setBg({ padding: Math.round(v * panel.ed.unit) }) }
            Slider { width: parent.width; label: "Inset"; from: 0; to: 60; unit: " px"
                     value: panel.bg ? panel.bg.inset / panel.ed.unit : 0
                     onMoved: (v) => panel.ed.previewBg({ inset: Math.round(v * panel.ed.unit) })
                     onReleased: (v) => panel.ed.setBg({ inset: Math.round(v * panel.ed.unit) }) }
            Slider { width: parent.width; label: "Shadow"; from: 0; to: 100; unit: "%"
                     value: panel.bg ? panel.bg.shadow * 100 : 0
                     onMoved: (v) => panel.ed.previewBg({ shadow: v / 100 })
                     onReleased: (v) => panel.ed.setBg({ shadow: v / 100 }) }
            Slider { width: parent.width; label: "Corners"; from: 0; to: 60; unit: " px"
                     value: panel.bg ? panel.bg.radius / panel.ed.unit : 0
                     onMoved: (v) => panel.ed.previewBg({ radius: Math.round(v * panel.ed.unit) })
                     onReleased: (v) => panel.ed.setBg({ radius: Math.round(v * panel.ed.unit) }) }

            Heading { text: "Aspect ratio" }
            Flow {
                width: parent.width; spacing: 4
                Repeater {
                    model: Ed.bgRatios
                    delegate: ToolButton {
                        required property var modelData
                        label: modelData.label
                        selected: (panel.bg ? panel.bg.ratio : 0) === modelData.r
                        onClicked: panel.ed.setBg({ ratio: modelData.r })
                    }
                }
            }

            Heading { text: "Alignment" }
            Grid {
                columns: 3; spacing: 4
                Repeater {
                    model: ["tl", "t", "tr", "l", "c", "r", "bl", "b", "br"]
                    delegate: Rectangle {
                        required property string modelData
                        width: 26; height: 20; radius: 5
                        readonly property bool current: (panel.bg ? panel.bg.align : "c") === modelData
                        color: current ? Theme.accent : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08)
                        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: panel.ed.setBg({ align: parent.modelData }) }
                    }
                }
            }
        }
    }
}
