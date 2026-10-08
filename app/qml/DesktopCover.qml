import QtQuick
import QtQuick.Window
import org.kde.layershell as LayerShell
import Snip.Core

// CAP-12, the live toggle: on each output, the wallpaper over the space bars
// reserve, as omarchy-shell draws it (scaled to cover the output, centred,
// mipmapped: spike S03). The bar keeps its space, so nothing moves.
// Click-through, never takes focus.
Instantiator {
    id: covers
    property var desk: ({ hidden: false, wallpaper: "", outputs: [] })
    model: Qt.application.screens
    delegate: Window {
        id: cover
        required property var modelData
        readonly property var bands: (covers.desk.outputs.find(o => o.output === modelData.name) || { bands: [] }).bands
        visible: covers.desk.hidden && bands.length > 0 && !!covers.desk.wallpaper
        color: "transparent"
        flags: Qt.FramelessWindowHint
        title: "snip-cover"
        LayerShell.Window.screen: Platform.screen(modelData.name)
        LayerShell.Window.scope: "snip-cover"
        LayerShell.Window.layer: LayerShell.Window.LayerOverlay
        LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
                                   | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
        LayerShell.Window.exclusionZone: -1
        LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
        onVisibleChanged: if (visible) Platform.setInputRects(cover, [])

        Repeater {
            model: cover.bands
            Item {
                required property var modelData
                x: modelData[0]
                y: modelData[1]
                width: modelData[2]
                height: modelData[3]
                clip: true
                Image {
                    x: -parent.x
                    y: -parent.y
                    width: cover.width
                    height: cover.height
                    source: covers.desk.wallpaper ? "file://" + covers.desk.wallpaper : ""
                    fillMode: Image.PreserveAspectCrop
                    mipmap: true
                }
            }
        }
    }
}
