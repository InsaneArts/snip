import QtQuick
import "editor.js" as Ed

// The editor's toolbar, laid out like CleanShot's (docs/design/editor.md):
// crop | every tool | colour, size | Save as…, Done.
Rectangle {
    id: bar
    required property var ed
    height: 52
    color: Theme.surface

    component Sep: Rectangle {
        width: 1; height: 14
        anchors.verticalCenter: parent ? parent.verticalCenter : undefined
        color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.14)
    }
    component Tool: ToolButton {
        required property string name
        selected: bar.ed.tool === name
        onClicked: bar.ed.setTool(name)
    }

    Rectangle { anchors { left: parent.left; right: parent.right; bottom: parent.bottom } height: 1; color: Theme.hairline }

    Row {
        anchors { left: parent.left; leftMargin: 12; verticalCenter: parent.verticalCenter }
        spacing: 8
        Capsule {
            ToolButton { icon: "crop"; tip: "Crop (K)"; selected: bar.ed.tool === "crop"
                         onClicked: bar.ed.tool === "crop" ? bar.ed.cropDone() : bar.ed.setTool("crop") }
        }
        Capsule {
            Tool { name: "select"; icon: "pointer"; tip: "Select (V)" }
            Tool { name: "rect"; icon: "square"; tip: "Rectangle (R)" }
            Sep {}
            Tool { name: "arrow"; icon: "arrow"; tip: "Arrow (A)" }
            Sep {}
            Tool { name: "text"; icon: "text"; tip: "Text (T)" }
            Sep {}
            Tool { name: "redact"; icon: "pixelate"; tip: "Pixelate (P)" }
            Sep {}
            Tool { name: "counter"; icon: "counter"; tip: "Counter (C)" }
        }
        Capsule {
            visible: bar.ed.tool !== "crop" && bar.ed.sizeKind !== "redact"
            ToolButton {
                id: swatch
                tip: "Colour"
                chevron: true
                Rectangle {
                    x: 9; anchors.verticalCenter: parent.verticalCenter
                    width: 16; height: 16; radius: 8
                    color: bar.ed.ink
                    border.width: 1; border.color: Qt.rgba(0, 0, 0, 0.2)
                }
                leftPad: 22
                onClicked: bar.ed.openMenuNamed("color", swatch)
            }
        }
        Capsule {
            visible: bar.ed.tool !== "crop" && (bar.ed.sizeKind === "redact" || !!Ed.sizes[bar.ed.sizeKind])
            ToolButton {
                id: styleButton
                visible: bar.ed.sizeKind === "redact"
                label: Ed.redactStyles.find(r => r.id === (bar.ed.selectedObj && bar.ed.selectedObj.kind === "redact"
                                                            ? bar.ed.selectedObj.style : bar.ed.redactStyle)).label
                chevron: true
                onClicked: bar.ed.openMenuNamed("style", styleButton)
            }
            ToolButton {
                id: sizeButton
                visible: !!Ed.sizes[bar.ed.sizeKind]
                tip: "Size"
                icon: bar.ed.sizeKind === "text" ? "" : bar.ed.sizeKind === "counter" ? "counter" : "stroke"
                label: bar.ed.sizeKind === "text" ? Ed.sizes.text[bar.ed.sizeIndex.text] + " pt" : ""
                chevron: true
                onClicked: bar.ed.openMenuNamed("size", sizeButton)
            }
        }
    }

    Row {
        anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
        spacing: 8
        TextButton { text: "Save as…"; tip: "Save as… (Ctrl+Shift+S)"; onClicked: bar.ed.saveAsDialog() }
        TextButton { text: "Done"; tip: "Done: keep the edits and close"; primary: true; onClicked: bar.ed.closeEditor() }
    }
}
