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
            ToolButton { icon: "crop"; tip: "Crop & Resize (K)"; selected: bar.ed.tool === "crop"
                         onClicked: bar.ed.tool === "crop" ? bar.ed.cropDone() : bar.ed.setTool("crop") }
        }
        Capsule {
            ToolButton { id: addImageButton; icon: "image-plus"; tip: "Add image (Ctrl+I)"; onClicked: bar.ed.openMenuNamed("addImage", addImageButton) }
        }
        Capsule {
            ToolButton { icon: "image"; tip: "Background (B)"; selected: bar.ed.tool === "background"
                         onClicked: bar.ed.setTool(bar.ed.tool === "background" ? "select" : "background") }
        }
        Capsule {
            Tool { name: "select"; icon: "pointer"; tip: "Select (V)" }
            Tool { name: "rect"; icon: "square"; tip: "Rectangle (R)" }
            Tool { name: "frect"; icon: "square"; iconFilled: true; tip: "Filled rectangle (F)" }
            Sep {}
            Tool { name: "ellipse"; icon: "ellipse"; tip: "Ellipse (E)" }
            Sep {}
            Tool { name: "line"; icon: "line"; tip: "Line (L)" }
            Sep {}
            Tool { name: "arrow"; icon: "arrow"; tip: "Arrow (A)" }
            Sep {}
            Tool { name: "text"; icon: "text"; tip: "Text (T)" }
            Sep {}
            Tool { name: "redact"; icon: "pixelate"; tip: "Pixelate (P)" }
            Sep {}
            Tool { name: "spotlight"; icon: "spotlight"; tip: "Spotlight (H)" }
            Sep {}
            Tool { name: "counter"; icon: "counter"; tip: "Counter (C)" }
            Sep {}
            Tool { name: "pencil"; icon: "pencil"; tip: "Pencil (D)" }
            Sep {}
            Tool { name: "highlight"; icon: "highlighter"; tip: "Highlighter (M)" }
        }
        Capsule {
            // Spotlight: its shape, and how dark the rest gets
            visible: bar.ed.sizeKind === "spotlight"
            readonly property var spot: bar.ed.selectedObj && bar.ed.selectedObj.kind === "spotlight" ? bar.ed.selectedObj : null
            ToolButton { icon: "square"; tip: "Rounded rectangle"; selected: (parent.parent.spot ? parent.parent.spot.shape : bar.ed.spotShape) === "rect"
                         onClicked: bar.ed.setSpot("rect", parent.parent.spot ? parent.parent.spot.dim : bar.ed.spotDim) }
            ToolButton { icon: "ellipse"; tip: "Ellipse"; selected: (parent.parent.spot ? parent.parent.spot.shape : bar.ed.spotShape) === "ellipse"
                         onClicked: bar.ed.setSpot("ellipse", parent.parent.spot ? parent.parent.spot.dim : bar.ed.spotDim) }
        }
        Capsule {
            visible: bar.ed.sizeKind === "spotlight"
            ToolButton {
                id: dimButton
                tip: "Dim"
                label: Math.round((bar.ed.selectedObj && bar.ed.selectedObj.kind === "spotlight" ? bar.ed.selectedObj.dim : bar.ed.spotDim) * 100) + "%"
                chevron: true
                onClicked: bar.ed.openMenuNamed("dim", dimButton)
            }
        }
        Capsule {
            visible: bar.ed.tool !== "crop" && bar.ed.tool !== "background" && bar.ed.sizeKind !== "redact" && bar.ed.sizeKind !== "spotlight"
            ToolButton {
                id: swatch
                tip: "Colour"
                chevron: true
                Rectangle {
                    x: 9; anchors.verticalCenter: parent.verticalCenter
                    width: 16; height: 16; radius: 8
                    color: bar.ed.toolInk
                    border.width: 1; border.color: Qt.rgba(0, 0, 0, 0.2)
                }
                leftPad: 22
                onClicked: bar.ed.openMenuNamed("color", swatch)
            }
        }
        Capsule {
            visible: bar.ed.tool !== "crop" && bar.ed.tool !== "background" && (bar.ed.sizeKind === "redact" || !!Ed.sizes[bar.ed.sizeKind])
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
        Capsule {
            visible: bar.ed.sizeKind === "text" && bar.ed.tool !== "crop"
            ToolButton {
                id: textStyleButton
                tip: "Text style"
                chevron: true
                leftPad: 18
                readonly property string style: bar.ed.editingText && bar.ed.textObj ? (bar.ed.textObj.style || "standard")
                                              : bar.ed.selectedObj && bar.ed.selectedObj.kind === "text" ? (bar.ed.selectedObj.style || "standard")
                                              : bar.ed.textStyle
                EditorItem {
                    x: 9; y: 3
                    o: ({ kind: "text", x: 0, y: 0, text: "A", color: Theme.text.toString(), size: 15, style: textStyleButton.style })
                }
                onClicked: bar.ed.openMenuNamed("text", textStyleButton)
            }
        }
        Capsule {
            visible: bar.ed.sizeKind === "highlight"
            ToolButton {
                id: opacityButton
                tip: "Opacity"
                label: Math.round((bar.ed.selectedObj && bar.ed.selectedObj.kind === "highlight" ? bar.ed.selectedObj.opacity : bar.ed.highlightOpacity) * 100) + "%"
                chevron: true
                onClicked: bar.ed.openMenuNamed("opacity", opacityButton)
            }
        }
        Capsule {
            visible: bar.ed.sizeKind === "counter"
            ToolButton {
                id: counterStyleButton
                tip: "Counter style"
                chevron: true
                label: Ed.counterStyles.find(c => c.id === (bar.ed.selectedObj && bar.ed.selectedObj.kind === "counter"
                                                              ? (bar.ed.selectedObj.style || "num") : bar.ed.counterStyle)).short
                onClicked: bar.ed.openMenuNamed("counter", counterStyleButton)
            }
        }
        Capsule {
            visible: bar.ed.sizeKind === "arrow" && bar.ed.tool !== "crop"
            ToolButton {
                id: arrowStyleButton
                tip: "Arrow style"
                chevron: true
                leftPad: 26
                EditorItem {
                    x: 7; y: 4
                    o: Ed.withArrowStyle({ kind: "arrow", x1: 2, y1: 15, x2: 22, y2: 2, cx: 12, cy: 8.5, color: Theme.text.toString(), size: 2 },
                                         bar.ed.selectedObj && bar.ed.selectedObj.kind === "arrow" ? (bar.ed.selectedObj.style || "standard") : bar.ed.arrowStyle)
                }
                onClicked: bar.ed.openMenuNamed("arrow", arrowStyleButton)
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
