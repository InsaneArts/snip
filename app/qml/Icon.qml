import QtQuick
import QtQuick.Shapes

// Line icons (Lucide geometry, ISC licence; 24-unit grid, 1.75 stroke),
// drawn as vector shapes so they stay crisp at every scale and take the
// theme colour directly.
Item {
    id: root
    property string name: "x"
    property color color: "white"
    property real size: 16
    property real stroke: 1.75
    property bool filled: false         // fill the first path (CleanShot's filled-rectangle tool)
    width: size
    height: size

    readonly property var paths: ({
        "x": ["M18 6 6 18", "M6 6l12 12"],
        "copy": ["M10 8h10a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H10a2 2 0 0 1-2-2V10a2 2 0 0 1 2-2z",
                 "M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"],
        "save": ["M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4", "M7 10l5 5 5-5", "M12 15V3"],
        "check": ["M20 6 9 17l-5-5"],
        "pencil": ["M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z",
                   "M15 5l4 4"],
        "pin": ["M12 17v5",
                "M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"],
        "folder": ["M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2z"],
        "more": ["M12 12h.01", "M19 12h.01", "M5 12h.01"],
        "text": ["M4 7V4h16v3", "M9 20h6", "M12 4v16"],
        "trash": ["M3 6h18", "M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6", "M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"],
        "eye": ["M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0",
                "M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0"],
        "eye-off": ["M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49",
                    "M14.084 14.158a3 3 0 0 1-4.242-4.242",
                    "M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143",
                    "M2 2l20 20"],
        "layers": ["M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83z",
                   "M2 12a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 12",
                   "M2 17a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 17"],
        "window": ["M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z", "M3 9h18"],
        "rotate-ccw": ["M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8", "M3 3v5h5"],
        "rotate-cw": ["M21 12a9 9 0 1 1-9-9 9.75 9.75 0 0 1 6.74 2.74L21 8", "M21 3v5h-5"],
        "flip-vertical": ["M7 3l5 5 5-5H7", "M7 21l5-5 5 5H7", "M2 12h2", "M8 12h2", "M14 12h2", "M20 12h2"],
        "flip-horizontal": ["M3 7l5 5-5 5V7", "M21 7l-5 5 5 5V7", "M12 20v2", "M12 14v2", "M12 8v2", "M12 2v2"],
        "pointer": ["M4.037 4.688a.495.495 0 0 1 .651-.651l16 6.5a.5.5 0 0 1-.063.947l-6.124 1.58a2 2 0 0 0-1.438 1.435l-1.579 6.126a.5.5 0 0 1-.947.063z"],
        "arrow": ["M7 7h10v10", "M7 17 17 7"],
        "square": ["M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"],
        "pixelate": ["M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z", "M3 9h18", "M3 15h18", "M9 3v18", "M15 3v18"],
        "counter": ["M12 2a10 10 0 1 0 0 20a10 10 0 1 0 0-20z", "M10.5 9.5 12.5 8v8"],
        "undo": ["M9 14 4 9l5-5", "M4 9h10.5a5.5 5.5 0 0 1 5.5 5.5a5.5 5.5 0 0 1-5.5 5.5H11"],
        "redo": ["M15 14l5-5-5-5", "M20 9H9.5A5.5 5.5 0 0 0 4 14.5A5.5 5.5 0 0 0 9.5 20H13"],
        "chevron-down": ["M6 9l6 6 6-6"],
        "grip": ["M5 9h14", "M5 15h14"],
        "stroke": ["M5 19 19 5"],
        "image-plus": ["M16 5h6", "M19 2v6", "M21 11.5V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7.5", "M21 15l-3.1-3.1a2 2 0 0 0-2.8 0L6 21", "M9 7a2 2 0 1 0 0 4a2 2 0 1 0 0-4z"],
        "image": ["M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z", "M9 7a2 2 0 1 0 0 4a2 2 0 1 0 0-4z", "M21 15l-3.1-3.1a2 2 0 0 0-2.8 0L6 21"],
        "pipette": ["M2 22l1-1h3l9-9", "M3 21v-3l9-9", "M15 6l3.4-3.4a2.1 2.1 0 1 1 3 3L18 9l.4.4a2.1 2.1 0 1 1-3 3l-3.8-3.8a2.1 2.1 0 1 1 3-3l.4.4z"],
        "plus": ["M5 12h14", "M12 5v14"],
        "search": ["M19 11a8 8 0 1 1-16 0 8 8 0 0 1 16 0z", "M21 21l-4.3-4.3"],
        "share": ["M4 12v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8", "M16 6l-4-4-4 4", "M12 2v13"],
        "lock": ["M5 11h14a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2z", "M7 11V7a5 5 0 0 1 10 0v4"],
        "smile": ["M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0z", "M8 14s1.5 2 4 2 4-2 4-2", "M9 9h.01", "M15 9h.01"],
        "highlighter": ["M9 11l-6 6v3h9l3-3", "M22 12l-4.6 4.6a2 2 0 0 1-2.8 0l-5.2-5.2a2 2 0 0 1 0-2.8L14 4"],
        "spotlight": ["M4 4h16a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z", "M8 9h8v6H8z"],
        "ellipse": ["M12 3.5a8.5 8.5 0 1 0 0 17a8.5 8.5 0 1 0 0-17z"],
        "line": ["M5 19 19 5"],
        "crop": ["M6 2v14a2 2 0 0 0 2 2h14", "M18 22V8a2 2 0 0 0-2-2H2"],
        "play": ["M7 4l13 8-13 8z"],
        "skip-back": ["M19 20L9 12l10-8v16z", "M5 19V5"],
        "skip-forward": ["M5 4l10 8-10 8V4z", "M19 5v14"],
        "minus": ["M5 12h14"],
        "scissors": ["M6 9a3 3 0 1 0 0-6 3 3 0 0 0 0 6z", "M6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6z", "M20 4L8.12 15.88", "M14.47 14.48L20 20", "M8.12 8.12L12 12"],
        "pause": ["M7 4h3v16H7z", "M14 4h3v16h-3z"],
        "video": ["M4 6h10a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2z", "M16 10.5l5-3.2v9.4l-5-3.2"],
        "mic": ["M12 2a3 3 0 0 1 3 3v7a3 3 0 0 1-6 0V5a3 3 0 0 1 3-3z", "M19 10v2a7 7 0 0 1-14 0v-2", "M12 19v3"],
        "expand": ["M15 3h6v6", "M9 21H3v-6", "M21 3l-7 7", "M3 21l7-7"],
        "command": ["M15 6v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3"],
        "click": ["M14 4.1 12 6", "M5.1 8l-2.9-.8", "M6 12l-1.9 2", "M7.2 2.2 8 5.1", "M9.037 9.69a.498.498 0 0 1 .653-.653l11 4.5a.5.5 0 0 1-.074.949l-4.349 1.041a1 1 0 0 0-.74.739l-1.04 4.35a.5.5 0 0 1-.95.074z"],
        "webcam": ["M4 10a8 8 0 1 0 16 0a8 8 0 1 0-16 0", "M9 10a3 3 0 1 0 6 0a3 3 0 1 0-6 0", "M7 22h10", "M12 22v-4"],
        "volume": ["M11 5L6 9H2v6h4l5 4z", "M16 9a5 5 0 0 1 0 6", "M19.4 5.6a9 9 0 0 1 0 12.8"],
        "volume-x": ["M11 5L6 9H2v6h4l5 4z", "M22 9l-6 6", "M16 9l6 6"],
        "minimize": ["M4 14h6v6", "M20 10h-6V4", "M14 10l7-7", "M3 21l7-7"],
        "scaling": ["M21 3L9 15", "M12 3H3v18h18v-9", "M16 3h5v5", "M14 15H9v-5"],
        "printer": ["M6 9V2h12v7", "M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2", "M6 14h12v8H6z"],
        "scan-text": ["M3 7V5a2 2 0 0 1 2-2h2", "M17 3h2a2 2 0 0 1 2 2v2", "M21 17v2a2 2 0 0 1-2 2h-2",
                      "M7 21H5a2 2 0 0 1-2-2v-2", "M7 8h8", "M7 12h10", "M7 16h6"],
        "settings": ["M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z",
                     "M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0"],
        "camera": ["M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3l-2.5-3z",
                   "M15 13a3 3 0 1 1-6 0 3 3 0 0 1 6 0"],
        "sliders": ["M21 4h-7", "M10 4H3", "M21 12h-9", "M8 12H3", "M21 20h-5", "M12 20H3", "M14 2v4", "M8 10v4", "M16 18v4"],
        "keyboard": ["M4 4h16a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z", "M10 8h.01", "M12 12h.01", "M14 8h.01",
                     "M16 12h.01", "M18 8h.01", "M6 8h.01", "M7 16h10", "M8 12h.01"],
        "info": ["M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0", "M12 16v-4", "M12 8h.01"]
    })

    readonly property var current: paths[name] || []

    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer
        transform: Scale { xScale: root.size / 24; yScale: root.size / 24 }
        ShapePath { strokeColor: root.color; strokeWidth: root.stroke; fillColor: root.filled ? root.color : "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: root.current[0] || "" } }
        ShapePath { strokeColor: root.color; strokeWidth: root.stroke; fillColor: "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: root.current[1] || "" } }
        ShapePath { strokeColor: root.color; strokeWidth: root.stroke; fillColor: "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: root.current[2] || "" } }
        ShapePath { strokeColor: root.color; strokeWidth: root.stroke; fillColor: "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: root.current[3] || "" } }
        ShapePath { strokeColor: root.color; strokeWidth: root.stroke; fillColor: "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: root.current[4] || "" } }
        ShapePath { strokeColor: root.color; strokeWidth: root.stroke; fillColor: "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: root.current[5] || "" } }
        ShapePath { strokeColor: root.color; strokeWidth: root.stroke; fillColor: "transparent"; capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin; PathSvg { path: root.current[6] || "" } }
    }
}
