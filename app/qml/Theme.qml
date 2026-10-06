pragma Singleton
import QtQuick
import Snip.Core

// Design tokens (design doc §3), resolved in Rust from the current Omarchy
// theme and pushed live on every theme switch. Every token has a fallback.
QtObject {
    id: t

    readonly property var raw: {
        try { return JSON.parse(Engine.theme || "{}") } catch (e) { return {} }
    }
    function pick(k, d) { return raw[k] !== undefined && raw[k] !== null && raw[k] !== "" ? raw[k] : d }

    readonly property string name: pick("name", "default")
    readonly property bool light: pick("mode", "dark") === "light"

    readonly property color background: pick("background", "#121214")
    readonly property color surface: pick("surface", "#1d1d22")
    readonly property color text: pick("text", "#eeeeee")
    readonly property color textSecondary: pick("textSecondary", "#9a9aa2")
    readonly property color accent: pick("accent", "#7aa2f7")
    readonly property color onAccent: pick("onAccent", "#101012")
    readonly property color danger: pick("danger", "#e5484d")
    readonly property color success: pick("success", "#30a46c")
    readonly property color warning: pick("warning", "#f5a524")
    readonly property var signature: pick("signature", [accent])
    readonly property color scrim: pick("scrim", "#73000000")
    readonly property string wallpaper: pick("wallpaper", "")

    // Corners follow Hyprland's decoration:rounding (decision Q-D2).
    readonly property real radius: Number(pick("radius", 0))
    readonly property real radiusControl: Math.min(radius, 6)

    readonly property string fontUi: pick("fontUi", "sans-serif")
    readonly property string fontMono: pick("fontMono", "monospace")

    // Floating chrome
    readonly property color hairline: light ? Qt.rgba(0, 0, 0, 0.10) : Qt.rgba(1, 1, 1, 0.12)
    readonly property color shadow: light ? Qt.rgba(0, 0, 0, 0.18) : Qt.rgba(0, 0, 0, 0.45)
    readonly property real shadowBlur: light ? 30 : 40
    readonly property real shadowOffset: light ? 10 : 12
    readonly property color hoverScrim: Qt.rgba(0, 0, 0, light ? 0.22 : 0.38)
    readonly property color pillBg: Qt.rgba(surface.r, surface.g, surface.b, 0.94)
    readonly property color controlHover: Qt.rgba(text.r, text.g, text.b, 0.08)
    readonly property color controlPressed: Qt.rgba(text.r, text.g, text.b, 0.14)

    // Type scale
    readonly property int fontSize: 13
    readonly property int fontSmall: 11
}
