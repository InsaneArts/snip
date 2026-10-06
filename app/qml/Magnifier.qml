import QtQuick
import QtQuick.Effects
import Snip.Core

// SEL-05: loupe with a pixel grid and the centre pixel's hex value.
Item {
    id: m
    property string output: ""
    property int px: 0
    property int py: 0
    property int epoch: 0
    property int cells: 15
    property real cell: 9
    readonly property real lens: cells * cell
    readonly property color centre: loupe.color
    readonly property string hex: Platform.hex(loupe.color)

    width: lens
    height: lens + 8 + readout.height

    RectangularShadow {
        anchors.fill: lensBox
        radius: lensBox.radius
        offset.y: 6
        blur: 24
        color: Theme.shadow
    }

    Item {
        id: lensBox
        width: m.lens
        height: m.lens
        readonly property real radius: Math.min(Theme.radius, 12)

        Item {
            id: content
            anchors.fill: parent
            layer.enabled: lensBox.radius > 0
            layer.effect: MultiEffect {
                maskEnabled: true
                maskSource: lensMask
                maskThresholdMin: 0.5
                maskSpreadAtMin: 1.0
            }
            Loupe {
                id: loupe
                anchors.fill: parent
                output: m.output
                px: m.px
                py: m.py
                cells: m.cells
                epoch: m.epoch
            }
            // pixel grid
            Repeater {
                model: m.cells - 1
                Rectangle { x: (index + 1) * m.cell; y: 0; width: 1; height: m.lens; color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.15) }
            }
            Repeater {
                model: m.cells - 1
                Rectangle { y: (index + 1) * m.cell; x: 0; height: 1; width: m.lens; color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.15) }
            }
            // centre pixel: dark + light ring, visible on any colour
            Rectangle {
                x: Math.floor(m.cells / 2) * m.cell - 1
                y: x
                width: m.cell + 2
                height: width
                color: "transparent"
                border.width: 1
                border.color: "black"
                Rectangle { anchors.fill: parent; anchors.margins: 1; color: "transparent"; border.width: 1; border.color: "white" }
            }
        }
        Item {
            id: lensMask
            anchors.fill: parent
            visible: false
            layer.enabled: true
            Rectangle { anchors.fill: parent; radius: lensBox.radius; color: "black" }
        }
        Rectangle {
            anchors.fill: parent
            radius: lensBox.radius
            color: "transparent"
            border.width: 1
            border.color: Theme.hairline
        }
    }

    Rectangle {
        id: readout
        anchors.top: lensBox.bottom
        anchors.topMargin: 8
        anchors.horizontalCenter: lensBox.horizontalCenter
        height: 24
        width: row.implicitWidth + 16
        radius: Math.min(Theme.radiusControl, 12)
        color: Theme.pillBg
        border.width: 1
        border.color: Theme.hairline
        Row {
            id: row
            anchors.centerIn: parent
            spacing: 6
            Rectangle {
                width: 12; height: 12; radius: 3
                anchors.verticalCenter: parent.verticalCenter
                color: m.centre
                border.width: 1
                border.color: Theme.hairline
            }
            Text {
                text: m.hex
                color: Theme.text
                font.family: Theme.fontMono
                font.pixelSize: 12
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                text: "C"
                color: Theme.textSecondary
                font.family: Theme.fontUi
                font.pixelSize: 10
                font.weight: Font.DemiBold
                anchors.verticalCenter: parent.verticalCenter
                Rectangle { anchors.fill: parent; anchors.margins: -3; radius: 3; color: "transparent"; border.width: 1; border.color: Theme.hairline }
            }
        }
    }
}
