import QtQuick

// CleanShot's custom colour panel (ANN-13): a saturation/brightness square,
// hue and alpha sliders, a hex field, the eyedropper and "Add to My Colors".
Item {
    id: panel
    property string value: "#ff3b30"           // #rrggbb or #aarrggbb
    signal picked(string color)
    signal eyedropper()
    signal save()

    // the colour being edited, as hue / saturation / value / alpha
    property real hue: 0
    property real sat: 1
    property real val: 1
    property real alpha: 1
    function load(c) {
        const q = Qt.color(c)
        hue = Math.max(0, q.hsvHue); sat = q.hsvSaturation; val = q.hsvValue; alpha = q.a
    }
    onValueChanged: if (value.toLowerCase() !== current.toLowerCase()) load(value)
    Component.onCompleted: load(value)

    readonly property color qcolor: Qt.hsva(hue, sat, val, alpha)
    readonly property string current: {
        const h = (v) => ("0" + Math.round(v * 255).toString(16)).slice(-2)
        return "#" + (alpha < 0.999 ? h(alpha) : "") + h(qcolor.r) + h(qcolor.g) + h(qcolor.b)
    }
    function emit() { picked(current) }

    implicitWidth: 196
    implicitHeight: col.implicitHeight

    component Checker: Canvas {
        onPaint: {
            const g = getContext("2d")
            for (let y = 0; y < height; y += 4)
                for (let x = 0; x < width; x += 4) {
                    g.fillStyle = ((x + y) / 4) % 2 ? "#cccccc" : "#ffffff"
                    g.fillRect(x, y, 4, 4)
                }
        }
    }

    Column {
        id: col
        width: parent.width
        spacing: 10

        // saturation → right, brightness → up
        Item {
            id: square
            width: parent.width; height: 140
            Rectangle {
                anchors.fill: parent; radius: 6
                gradient: Gradient { orientation: Gradient.Horizontal
                    GradientStop { position: 0; color: "white" }
                    GradientStop { position: 1; color: Qt.hsva(panel.hue, 1, 1, 1) } }
            }
            Rectangle {
                anchors.fill: parent; radius: 6
                gradient: Gradient {
                    GradientStop { position: 0; color: "transparent" }
                    GradientStop { position: 1; color: "black" } }
            }
            Rectangle {
                x: panel.sat * square.width - 7; y: (1 - panel.val) * square.height - 7
                width: 14; height: 14; radius: 7
                color: Qt.hsva(panel.hue, panel.sat, panel.val, 1)
                border.width: 2; border.color: "white"
            }
            MouseArea {
                anchors.fill: parent
                function set(m) {
                    panel.sat = Math.max(0, Math.min(1, m.x / width))
                    panel.val = Math.max(0, Math.min(1, 1 - m.y / height))
                    panel.emit()
                }
                onPressed: (m) => set(m)
                onPositionChanged: (m) => { if (pressed) set(m) }
            }
        }

        // hue
        Item {
            width: parent.width; height: 14
            Rectangle {
                anchors.fill: parent; radius: 7
                gradient: Gradient { orientation: Gradient.Horizontal
                    GradientStop { position: 0.0; color: "#ff0000" }
                    GradientStop { position: 0.17; color: "#ffff00" }
                    GradientStop { position: 0.33; color: "#00ff00" }
                    GradientStop { position: 0.5; color: "#00ffff" }
                    GradientStop { position: 0.67; color: "#0000ff" }
                    GradientStop { position: 0.83; color: "#ff00ff" }
                    GradientStop { position: 1.0; color: "#ff0000" } }
            }
            Rectangle { x: panel.hue * parent.width - 7; y: 0; width: 14; height: 14; radius: 7
                        color: Qt.hsva(panel.hue, 1, 1, 1); border.width: 2; border.color: "white" }
            MouseArea {
                anchors.fill: parent
                function set(m) { panel.hue = Math.max(0, Math.min(0.999, m.x / width)); panel.emit() }
                onPressed: (m) => set(m)
                onPositionChanged: (m) => { if (pressed) set(m) }
            }
        }

        // alpha, over a checkerboard
        Item {
            width: parent.width; height: 14
            Checker { anchors.fill: parent; opacity: 0.6 }
            Rectangle {
                anchors.fill: parent; radius: 7
                gradient: Gradient { orientation: Gradient.Horizontal
                    GradientStop { position: 0; color: "transparent" }
                    GradientStop { position: 1; color: Qt.hsva(panel.hue, panel.sat, panel.val, 1) } }
                border.width: 1; border.color: Theme.hairline
            }
            Rectangle { x: panel.alpha * parent.width - 7; y: 0; width: 14; height: 14; radius: 7
                        color: panel.qcolor; border.width: 2; border.color: "white" }
            MouseArea {
                anchors.fill: parent
                function set(m) { panel.alpha = Math.max(0, Math.min(1, m.x / width)); panel.emit() }
                onPressed: (m) => set(m)
                onPositionChanged: (m) => { if (pressed) set(m) }
            }
        }

        Row {
            spacing: 6
            // hex
            Rectangle {
                width: 104; height: 28; radius: 8
                color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08)
                TextInput {
                    id: hex
                    anchors.fill: parent; anchors.leftMargin: 10
                    verticalAlignment: TextInput.AlignVCenter
                    text: panel.current.toUpperCase()
                    color: Theme.text
                    font.family: Theme.fontMono; font.pixelSize: 12
                    selectByMouse: true
                    validator: RegularExpressionValidator { regularExpression: /#?([0-9a-fA-F]{6}|[0-9a-fA-F]{8})/ }
                    onAccepted: { const c = text.startsWith("#") ? text : "#" + text; panel.load(c); panel.emit() }
                }
            }
            ToolButton { icon: "pipette"; tip: "Pick a colour from the screen"; onClicked: panel.eyedropper() }
            ToolButton { icon: "plus"; tip: "Add to My Colors"; onClicked: panel.save() }
        }
    }
}
