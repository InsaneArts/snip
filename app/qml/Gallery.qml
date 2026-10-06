import QtQuick
import QtQuick.Window
import Snip.Core

// Design gallery (design doc §6): every M1 surface in its states, for one
// theme at a time; `snipd --gallery` cycles through all installed themes.
Window {
    id: g
    required property var controller
    property var themes: []
    property int index: 0
    visible: true
    width: 1280
    height: 820
    color: Theme.background
    title: "Snip gallery"

    Text {
        x: 24; y: 18
        text: Theme.name + " · " + (Theme.light ? "light" : "dark") + " · radius " + Theme.radius
        color: Theme.text
        font.family: Theme.fontUi
        font.pixelSize: 15
        font.weight: Font.DemiBold
    }
}
