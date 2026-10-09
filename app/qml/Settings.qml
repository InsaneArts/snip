import QtQuick
import QtQuick.Window
import Snip.Core

// Snip's settings (SET-02), CleanShot's Settings window: a tab bar and a
// form for each tab. Every change goes straight into config.toml with the
// file's comments kept (SET-01), and the form shows the file as it is, so a
// hand edit shows up here at once. Recording gets its tab with M4.
Window {
    id: win
    property var controller: null
    readonly property var cfg: (controller && controller.settings.config) || ({})
    readonly property string cfgError: (controller && controller.settings.configError) || ""
    property string tab: "general"
    property bool confirmClear: false
    property real historyBytes: 0
    property int folderToken: 0                 // the Export location chooser that's open

    Connections {
        target: Platform
        function onFileChosen(token, path) {
            if (token === win.folderToken && path) win.set("general.export_dir", path)
        }
    }

    readonly property var tabs: [
        { id: "general", label: "General", icon: "settings" },
        { id: "shortcuts", label: "Shortcuts", icon: "keyboard" },
        { id: "quick-access", label: "Quick Access", icon: "layers" },
        { id: "screenshots", label: "Screenshots", icon: "camera" },
        { id: "recording", label: "Recording", icon: "video" },
        { id: "annotate", label: "Annotate", icon: "pencil" },
        { id: "advanced", label: "Advanced", icon: "sliders" },
        { id: "about", label: "About", icon: "info" },
    ]

    title: "Snip Settings"
    color: Theme.background
    width: 720
    height: 640
    minimumWidth: 660
    minimumHeight: 420

    function open(t) {
        if (tabs.some(x => x.id === t)) tab = t
        historyBytes = Engine.historyBytes()
        show()
        raise()
        requestActivate()
    }
    onTabChanged: {
        flick.contentY = 0
        confirmClear = false
        historyBytes = Engine.historyBytes()
        stopRecording()
        pending = null
        if (tab === "shortcuts") refreshShortcuts()
    }
    onCfgChanged: if (tab === "shortcuts") refreshShortcuts()
    onVisibleChanged: if (!visible) stopRecording()
    onActiveChanged: if (!active) stopRecording()

    // Shortcuts (SET-08). Hyprland runs the bound keys, so a key is recorded
    // with Hyprland's binds held off (Engine.recordKeys) and written as a
    // keycode ("SUPER + SHIFT + code:13"), which works in every layout.
    property var shortcutList: []                // [{id, name, keys: [label], custom}]
    property string recording: ""                // the action whose key is being recorded
    property string recordHint: ""
    property var pending: null                   // {action, spec, label, takes}: a key that's someone else's
    function refreshShortcuts() { shortcutList = JSON.parse(Engine.shortcuts() || "[]") }
    function startRecording(action) {
        pending = null
        recordHint = ""
        recording = action
        Engine.recordKeys(true)
        keyCatcher.forceActiveFocus()
    }
    function stopRecording() {
        if (recording === "") return
        recording = ""
        Engine.recordKeys(false)
    }
    function setShortcut(action, spec) {
        pending = null
        set("shortcuts." + action, spec)
        refreshShortcuts()
    }
    // Qt's modifiers and the key's xkb keycode
    function recorded(mods, code) {
        const parts = []
        if (mods & Qt.MetaModifier) parts.push("SUPER")
        if (mods & Qt.ControlModifier) parts.push("CTRL")
        if (mods & Qt.AltModifier) parts.push("ALT")
        if (mods & Qt.ShiftModifier) parts.push("SHIFT")
        parts.push("code:" + code)
        const spec = parts.join(" + ")
        const action = recording
        stopRecording()
        const c = JSON.parse(Engine.shortcutConflict(action, spec))
        if (c.takes) pending = { action: action, spec: spec, label: c.label, takes: c.takes }
        else setShortcut(action, spec)
    }
    function restoreDefaults() {
        const e = Engine.restoreShortcuts()
        if (e) controller.toast.show("Settings: " + e, screen ? screen.name : "")
        refreshShortcuts()
    }
    Item {
        id: keyCatcher
        Keys.onPressed: (e) => {
            e.accepted = true
            if (win.recording === "" || [Qt.Key_Shift, Qt.Key_Control, Qt.Key_Alt, Qt.Key_AltGr, Qt.Key_Meta, Qt.Key_Super_L,
                                         Qt.Key_Super_R, Qt.Key_CapsLock].indexOf(e.key) >= 0) return
            const mods = e.modifiers & (Qt.MetaModifier | Qt.ControlModifier | Qt.AltModifier | Qt.ShiftModifier)
            if (!mods && e.key === Qt.Key_Escape) { win.stopRecording(); return }
            if (!mods && (e.key === Qt.Key_Backspace || e.key === Qt.Key_Delete)) {
                const a = win.recording
                win.stopRecording()
                win.setShortcut(a, "")
                return
            }
            const alone = (e.key >= Qt.Key_F1 && e.key <= Qt.Key_F35) || e.key === Qt.Key_Print || e.key === Qt.Key_Pause
            if (!mods && !alone) { win.recordHint = "Hold Super, Ctrl, Alt or Shift with the key."; return }
            win.recorded(mods, e.nativeScanCode)
        }
        onActiveFocusChanged: if (!activeFocus) win.stopRecording()
    }

    // "general.sounds" → its value in config.toml (or the default)
    function value(key) {
        const p = key.split(".")
        const t = cfg[p[0]]
        return t ? t[p[1]] : undefined
    }
    function set(key, v) {
        const e = Engine.setSetting(key, JSON.stringify(v))
        Engine.log("setting", JSON.stringify({ key: key, value: v, error: e }).slice(1, -1))
        if (e) controller.toast.show("Settings: " + e, screen ? screen.name : "")
    }
    // After Capture (SET-03): a list of rules; one at least stays on
    function rule(r) { return (value("after_capture.screenshot") || []).indexOf(r) >= 0 }
    function toggleRule(r) {
        const now = value("after_capture.screenshot") || []
        set("after_capture.screenshot", rule(r) ? now.filter(x => x !== r) : now.concat([r]))
    }
    function size(n) {
        if (n < 1e6) return Math.round(n / 1e3) + " KB"
        if (n < 1e9) return (n / 1e6).toFixed(1) + " MB"
        return (n / 1e9).toFixed(2) + " GB"
    }
    function clearHistory() {
        controller.quickAccess.closeAll()
        const n = Engine.clearHistory()
        controller.toast.show("Cleared " + n + (n === 1 ? " capture" : " captures"), screen ? screen.name : "")
        confirmClear = false
        historyBytes = Engine.historyBytes()
    }

    Shortcut { enabled: win.recording === ""; sequences: ["Escape", StandardKey.Close]; onActivated: win.close() }

    // ---------------------------------------------------------------- tab bar
    Rectangle {
        id: header
        width: parent.width
        height: 68
        color: Theme.surface
        Row {
            anchors.centerIn: parent
            spacing: 2
            Repeater {
                model: win.tabs
                Rectangle {
                    required property var modelData
                    readonly property bool on: win.tab === modelData.id
                    objectName: "tab:" + modelData.id
                    width: Math.max(76, tabLabel.implicitWidth + 20)
                    height: 54
                    radius: Theme.radiusControl
                    color: on ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.16) : tabHover.hovered ? Theme.controlHover : "transparent"
                    Icon {
                        y: 8
                        anchors.horizontalCenter: parent.horizontalCenter
                        name: modelData.icon
                        size: 20
                        color: parent.on ? Theme.accent : Theme.text
                    }
                    Text {
                        id: tabLabel
                        y: 33
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: modelData.label
                        color: parent.on ? Theme.accent : Theme.text
                        font.family: Theme.fontUi
                        font.pixelSize: 12
                    }
                    HoverHandler { id: tabHover; cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: win.tab = modelData.id }
                }
            }
        }
        Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.hairline }
    }

    // ---------------------------------------------------------------- forms
    Flickable {
        id: flick
        anchors { top: header.bottom; left: parent.left; right: parent.right; bottom: parent.bottom }
        contentHeight: page.implicitHeight + 64
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        acceptedButtons: Qt.NoButton            // the wheel scrolls; drags stay with the controls

        Column {
            id: page
            x: Math.max(20, (flick.width - 600) / 2)
            y: 30
            spacing: 22

            Rectangle {
                visible: win.cfgError !== ""
                width: 600
                height: errorText.implicitHeight + 20
                radius: Theme.radiusControl
                color: Qt.rgba(Theme.danger.r, Theme.danger.g, Theme.danger.b, 0.14)
                Text {
                    id: errorText
                    x: 12; y: 10; width: parent.width - 24
                    wrapMode: Text.Wrap
                    text: "config.toml has an error, so Snip uses the defaults until it's fixed: " + win.cfgError
                    color: Theme.text
                    font.family: Theme.fontUi
                    font.pixelSize: 12
                }
            }

            // General
            Column {
                visible: win.tab === "general"
                spacing: 22
                Section {
                    label: "Sounds"
                    Check { key: "general.sounds"; text: "Play sounds" }
                }
                Section {
                    label: "After capture"
                    Repeater {
                        model: [["show_qao", "Show Quick Access Overlay"], ["copy", "Copy to clipboard"], ["save", "Save"],
                                ["annotate", "Open Annotate tool"], ["pin", "Pin to the screen"]]
                        Check {
                            required property var modelData
                            objectName: "after_capture." + modelData[0]
                            text: modelData[1]
                            checked: win.rule(modelData[0])
                            locked: checked && (win.value("after_capture.screenshot") || []).length === 1
                            onToggled: win.toggleRule(modelData[0])
                        }
                    }
                    Note { text: "Every capture also goes to history." }
                    Check { key: "after_capture.variants_ignore_rules"; text: "Capture Area & Copy, & Save and & Annotate skip these" }
                }
                Section {
                    label: "Save"
                    Check { key: "general.save_also_copies"; text: "Also copy the saved file to the clipboard" }
                }
                Section {
                    label: "Export location"
                    Row {
                        spacing: 8
                        Field { width: 300; key: "general.export_dir"; placeholder: "~/Pictures/Screenshots" }
                        TextButton {
                            text: "Choose…"
                            implicitHeight: 26
                            onClicked: {
                                win.folderToken = controller.nextToken()
                                Platform.openFolderDialog(win.folderToken, "Export location", win.value("general.export_dir") || "")
                            }
                        }
                    }
                    Note { text: "Where Save writes. ~ is your home folder." }
                }
            }

            // Shortcuts
            Column {
                visible: win.tab === "shortcuts"
                spacing: 8
                Repeater {
                    model: win.shortcutList
                    Column {
                        required property var modelData
                        spacing: 6
                        Row {
                            spacing: 16
                            Text {
                                width: 230
                                height: 28
                                horizontalAlignment: Text.AlignRight
                                verticalAlignment: Text.AlignVCenter
                                text: modelData.name + ":"
                                color: Theme.text
                                font.family: Theme.fontUi
                                font.pixelSize: Theme.fontSize
                            }
                            KeyField { action: modelData.id; keys: modelData.keys }
                        }
                        Rectangle {
                            readonly property var p: win.pending
                            visible: p !== null && p.action === modelData.id
                            x: 246
                            width: 340
                            height: takesText.implicitHeight + 50
                            radius: Theme.radiusControl
                            color: Qt.rgba(Theme.warning.r, Theme.warning.g, Theme.warning.b, 0.14)
                            Text {
                                id: takesText
                                x: 10; y: 8; width: parent.width - 20
                                wrapMode: Text.Wrap
                                text: parent.p ? parent.p.label + " already runs “" + parent.p.takes + "”. Use it for this instead?" : ""
                                color: Theme.text
                                font.family: Theme.fontUi
                                font.pixelSize: 12
                            }
                            Row {
                                anchors.right: parent.right; anchors.rightMargin: 8
                                anchors.bottom: parent.bottom; anchors.bottomMargin: 8
                                spacing: 6
                                TextButton { text: "Cancel"; implicitHeight: 26; onClicked: win.pending = null }
                                TextButton {
                                    objectName: "use-anyway"
                                    text: "Use Anyway"; primary: true; implicitHeight: 26
                                    onClicked: win.setShortcut(win.pending.action, win.pending.spec)
                                }
                            }
                        }
                    }
                }
                Item { width: 1; height: 8 }
                Row {
                    x: 246
                    spacing: 12
                    TextButton { objectName: "restore-defaults"; text: "Restore Defaults"; onClicked: win.restoreDefaults() }
                    Note {
                        anchors.verticalCenter: parent.verticalCenter
                        text: win.recordHint || "Click a shortcut and press keys. Esc cancels, Backspace clears."
                        color: win.recordHint ? Theme.warning : Theme.textSecondary
                    }
                }
            }

            // Quick Access
            Column {
                visible: win.tab === "quick-access"
                spacing: 22
                Section {
                    label: "Position on screen"
                    Choice { key: "quick_access.position"; options: [["left", "Left"], ["right", "Right"]] }
                }
                Section {
                    label: "Overlay size"
                    Item {
                        width: 300
                        height: 26
                        Slider {
                            property real live: -1
                            objectName: "quick_access.size"
                            y: -16
                            width: parent.width
                            from: 200
                            to: 420
                            unit: " px"
                            value: live >= 0 ? live : (win.value("quick_access.size") || 280)
                            onMoved: (v) => live = v
                            onReleased: (v) => { live = -1; win.set("quick_access.size", Math.round(v)) }
                        }
                    }
                }
                Section {
                    label: "Cards"
                    NumberBox { key: "quick_access.max_visible"; from: 1; to: 20; prefix: "Show at most"; suffix: "cards, then stack them" }
                }
                Section {
                    label: "Auto close"
                    Check { key: "quick_access.auto_close"; text: "Close cards automatically" }
                    Column {
                        spacing: 10
                        enabled: win.value("quick_access.auto_close") === true
                        opacity: enabled ? 1 : 0.45
                        NumberBox { key: "quick_access.auto_close_seconds"; from: 1; to: 600; prefix: "After"; suffix: "seconds" }
                        Choice { key: "quick_access.auto_close_action"; options: [["close", "Close"], ["save_and_close", "Save and close"]] }
                    }
                    Note { text: "A card that closes stays in history." }
                }
                Section {
                    label: "Drag and drop"
                    Check { key: "quick_access.close_after_drag"; text: "Close after dragging" }
                    Note { text: "Hold Alt while dragging to keep the card." }
                }
            }

            // Screenshots
            Column {
                visible: win.tab === "screenshots"
                spacing: 22
                Section {
                    label: "File format"
                    Choice { key: "general.format"; options: [["png", "PNG"], ["jpg", "JPG"], ["webp", "WebP"]] }
                    NumberBox {
                        visible: win.value("general.format") === "jpg"
                        key: "general.jpg_quality"; from: 1; to: 100; prefix: "Quality"; suffix: "%"
                    }
                }
                Section {
                    label: "Selection"
                    Check { key: "capture.freeze"; text: "Freeze the screen while selecting" }
                    Check { key: "capture.magnifier"; text: "Show magnifier" }
                    Check { key: "capture.logical_size"; text: "Show the size in logical pixels" }
                }
                Section {
                    label: "Crosshair"
                    Choice { key: "capture.crosshair"; options: [["off", "Off"], ["ctrl", "While Ctrl is held"], ["always", "Always"]] }
                }
                Section {
                    label: "Cursor"
                    Check { key: "capture.show_cursor"; text: "Show the pointer in screenshots" }
                }
                Section {
                    label: "Desktop"
                    Check { key: "capture.hide_desktop"; text: "Hide desktop clutter while capturing" }
                    Note { text: "Screenshots show the wallpaper where the bar is, and leave pins out." }
                }
                Section {
                    label: "Window capture"
                    Choice { key: "capture.window_desktop"; options: [[false, "Window only"], [true, "With the desktop"]] }
                    Note { text: win.value("capture.window_desktop") ? "The window as it is on screen, with what's around it: the wallpaper, other windows, the bar."
                                                                      : "The window alone, from its own buffer." }
                    Column {
                        spacing: 8
                        enabled: win.value("capture.window_desktop") !== true
                        opacity: enabled ? 1 : 0.45
                        Check { key: "capture.window_shadow"; text: "Keep the transparent background" }
                        Check { key: "capture.window_rounding"; text: "Keep the window's rounded corners" }
                    }
                    Note { text: "Hold Shift when you click a window to do the opposite." }
                }
                Section {
                    label: "Self-timer"
                    NumberBox { key: "capture.timer_seconds"; from: 1; to: 60; suffix: "seconds" }
                }
            }

            // Recording (REC)
            Column {
                visible: win.tab === "recording"
                spacing: 22
                Section {
                    label: "Video"
                    Choice { key: "recording.fps"; options: [[24, "24 fps"], [30, "30 fps"], [60, "60 fps"]] }
                    Choice { key: "recording.quality"; options: [["medium", "Medium"], ["high", "High"], ["very_high", "Very high"], ["ultra", "Ultra"]] }
                    Choice { key: "recording.max_size"; options: [["", "Original size"], ["2160p", "2160p"], ["1440p", "1440p"], ["1080p", "1080p"], ["720p", "720p"]] }
                    Note { text: "MP4 (H.264), encoded on the graphics card." }
                }
                Section {
                    label: "While recording"
                    Check { key: "recording.show_cursor"; text: "Show the pointer" }
                    Check { key: "recording.dim"; text: "Dim the screen outside the area" }
                    Check { key: "recording.dnd"; text: "Turn on Do Not Disturb" }
                    NumberBox { key: "recording.countdown"; from: 0; to: 10; prefix: "Count down"; suffix: "seconds first (0: none)" }
                    Check { key: "recording.highlight_clicks"; text: "Highlight clicks" }
                    Check { key: "recording.show_keystrokes"; text: "Show keystrokes" }
                    Note { text: "Only command keys show (Ctrl+C, Esc, arrows…), never typing." }
                }
                Section {
                    label: "Camera"
                    Choice { key: "recording.camera_shape"; options: [["circle", "Circle"], ["square", "Square"], ["rectangle", "Rectangle"], ["vertical", "Vertical"]] }
                    Choice { key: "recording.camera_size"; options: [["small", "Small"], ["medium", "Medium"], ["large", "Large"], ["huge", "Huge"]] }
                    Check { key: "recording.camera_mirror"; text: "Mirror the camera" }
                    Note { text: "Choose the camera in the recording toolbar." }
                }
                Section {
                    label: "Audio"
                    Check { key: "recording.separate_tracks"; text: "The microphone and the computer's sound on separate tracks" }
                    Note { text: "For other video editors. Choose the sources in the recording toolbar." }
                }
                Section {
                    label: "GIF"
                    NumberBox { key: "recording.gif_fps"; from: 5; to: 30; suffix: "frames a second" }
                    NumberBox { key: "recording.gif_width"; from: 200; to: 2000; prefix: "At most"; suffix: "px wide" }
                }
                Section {
                    label: "Save to"
                    Field { key: "recording.export_dir"; placeholder: "~/Videos/Screencasts" }
                    Note { text: "Where Save writes recordings. Screenshots go to their own folder." }
                }
            }

            // Annotate
            Column {
                visible: win.tab === "annotate"
                spacing: 22
                Section {
                    label: "Objects"
                    Check { key: "annotate.object_shadows"; text: "Draw shadow on objects" }
                    Check { key: "annotate.inverse_arrows"; text: "Inverse arrow direction" }
                    Note { text: "Hold Alt while drawing an arrow to flip it." }
                    Check { key: "annotate.smooth_drawing"; text: "Smooth drawing" }
                }
                Section {
                    label: "Canvas"
                    Check { key: "annotate.auto_expand"; text: "Expand the canvas when drawing past its edge" }
                }
                Section {
                    label: "Window"
                    Check { key: "annotate.always_on_top"; text: "Always on top" }
                    Check { key: "annotate.remember_background_tool"; text: "Remember if the Background tool was open" }
                }
                Section {
                    label: "Annotate with"
                    Field { key: "general.external_editor"; placeholder: "Snip's editor" }
                    Note { text: "Another editor's command, such as satty --filename.\nThe image's path goes at the end." }
                }
            }

            // Advanced
            Column {
                visible: win.tab === "advanced"
                spacing: 22
                Section {
                    label: "File name"
                    Field { id: nameField; key: "general.file_name"; placeholder: "Snip %y-%m-%d at %H.%M.%S" }
                    Note {
                        readonly property string ext: ({ png: ".png", jpg: ".jpg", webp: ".webp" })[win.value("general.format")] || ".png"
                        text: Engine.fileNamePreview(nameField.text, win.value("general.utc_time") === true, win.value("general.scale_suffix") === true) + ext
                        color: Theme.text
                    }
                    Note { text: "%y %m %d date, %H %M %S time, %n month, %w weekday, %p AM/PM,\n%t window title, %a app, %r random, %i counter" }
                    Check { key: "general.utc_time"; text: "Use UTC time" }
                    Check { key: "general.scale_suffix"; text: "Add @2x to HiDPI captures (their scale: @1.25x, @2x…)" }
                }
                Section {
                    label: "Clipboard"
                    Choice { key: "clipboard.mode"; options: [["image", "Image"], ["file", "File"], ["image_and_file", "Image and file"]] }
                    Check { key: "clipboard.text_path"; text: "Also offer the file's path as text" }
                    Note { text: "Terminals and plain text fields paste the path." }
                    Check { key: "clipboard.sensitive"; text: "Keep copies out of clipboard history" }
                }
                Section {
                    label: "Pinned screenshots"
                    NumberBox { key: "pins.corner_radius"; from: 0; to: 60; prefix: "Corner radius"; suffix: "px" }
                    Check { key: "pins.shadow"; text: "Shadow" }
                    Check { key: "pins.border"; text: "Border" }
                }
                Section {
                    label: "Keep history"
                    Choice {
                        key: "history.retention"
                        options: [["never", "Never"], ["1d", "1 day"], ["3d", "3 days"], ["1w", "1 week"], ["1m", "1 month"], ["forever", "Forever"]]
                    }
                    Row {
                        spacing: 12
                        visible: !win.confirmClear
                        Note { anchors.verticalCenter: parent.verticalCenter; text: "History takes " + win.size(win.historyBytes) + "." }
                        TextButton { objectName: "clear-history"; text: "Clear History…"; onClicked: win.confirmClear = true }
                    }
                    Row {
                        spacing: 8
                        visible: win.confirmClear
                        Note { anchors.verticalCenter: parent.verticalCenter; text: "Delete every capture in history?"; color: Theme.text }
                        TextButton { text: "Cancel"; onClicked: win.confirmClear = false }
                        TextButton { objectName: "clear-history-confirm"; text: "Delete All"; primary: true; onClicked: win.clearHistory() }
                    }
                }
                Section {
                    label: "Text recognition"
                    Field { key: "ocr.languages"; placeholder: "English and the installed languages" }
                    Note { text: "Tesseract languages, such as eng+deu." }
                    Check { key: "ocr.index_history"; text: "Read the text of every capture, for search" }
                    Check { key: "ocr.keep_line_breaks"; text: "Keep line breaks" }
                    Check { key: "ocr.barcodes"; text: "Read barcodes too (EAN, UPC, Code 128…)" }
                    Check { key: "ocr.detect_links"; text: "Find links in the text" }
                    Row {
                        spacing: 10
                        Note { anchors.verticalCenter: parent.verticalCenter; text: "Open links"; color: Theme.text }
                        Choice { key: "ocr.open_links"; options: [["ask", "Ask"], ["always", "Always"], ["never", "Never"]] }
                    }
                }
                Section {
                    label: "Speed"
                    NumberBox { key: "capture.keep_warm_minutes"; from: 0; to: 240; prefix: "Stay ready for"; suffix: "minutes after a capture" }
                }
                Section {
                    label: "Settings file"
                    Note { text: (controller && controller.settings.configPath) || ""; color: Theme.text }
                    TextButton { text: "Open"; onClicked: Qt.openUrlExternally("file://" + controller.settings.configPath) }
                }
            }

            // About
            Column {
                visible: win.tab === "about"
                width: 600
                spacing: 10
                Item { width: 1; height: 20 }
                Image {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: 96
                    height: 96
                    source: "../icons/snip.png"
                    sourceSize: Qt.size(192, 192)
                    smooth: true
                    mipmap: true
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "Snip"
                    color: Theme.text
                    font.family: Theme.fontUi
                    font.pixelSize: 22
                    font.weight: Font.DemiBold
                }
                Text {
                    objectName: "version"
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "Version " + ((controller && controller.settings.version) || "")
                    color: Theme.textSecondary
                    font.family: Theme.fontUi
                    font.pixelSize: 13
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "CleanShot X's workflow for Omarchy and Hyprland."
                    color: Theme.textSecondary
                    font.family: Theme.fontUi
                    font.pixelSize: 13
                }
            }
        }
    }

    // ---------------------------------------------------------------- controls

    // A row of the form, as in CleanShot: the label right-aligned in its
    // column, the controls stacked to its right.
    component Section: Row {
        id: sec
        property string label: ""
        default property alias content: controls.data
        spacing: 16
        Text {
            width: 170
            height: 26
            horizontalAlignment: Text.AlignRight
            verticalAlignment: Text.AlignVCenter
            text: sec.label + ":"
            color: Theme.text
            font.family: Theme.fontUi
            font.pixelSize: Theme.fontSize
        }
        Column { id: controls; spacing: 8 }
    }

    // A line of help under a control.
    component Note: Text {
        color: Theme.textSecondary
        font.family: Theme.fontUi
        font.pixelSize: 12
    }

    // A checkbox for a bool in config.toml (or toggled() when it has no key).
    component Check: Item {
        id: chk
        property string key: ""
        property string text: ""
        property bool checked: key !== "" && win.value(key) === true
        property bool locked: false             // the last After Capture rule
        signal toggled()
        objectName: key
        implicitWidth: box.width + 8 + checkLabel.implicitWidth
        implicitHeight: 26
        Rectangle {
            id: box
            anchors.verticalCenter: parent.verticalCenter
            width: 16
            height: 16
            radius: Math.min(4, Theme.radiusControl)
            color: chk.checked ? Theme.accent : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, checkHover.hovered ? 0.12 : 0.05)
            border.width: chk.checked ? 0 : 1
            border.color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.3)
            opacity: chk.locked ? 0.55 : 1
            Icon { anchors.centerIn: parent; visible: chk.checked; name: "check"; size: 12; stroke: 3; color: Theme.onAccent }
        }
        Text {
            id: checkLabel
            x: box.width + 8
            anchors.verticalCenter: parent.verticalCenter
            text: chk.text
            color: Theme.text
            font.family: Theme.fontUi
            font.pixelSize: Theme.fontSize
        }
        HoverHandler { id: checkHover; cursorShape: chk.locked ? Qt.ArrowCursor : Qt.PointingHandCursor }
        TapHandler {
            enabled: !chk.locked
            onTapped: chk.key !== "" ? win.set(chk.key, !chk.checked) : chk.toggled()
        }
    }

    // One of a few values, as segments.
    component Choice: Row {
        id: cho
        property string key: ""
        property var options: []                // [value, label] pairs
        readonly property var current: win.value(key)
        objectName: key
        spacing: 2
        Repeater {
            model: cho.options
            Rectangle {
                required property var modelData
                readonly property bool on: cho.current === modelData[0]
                objectName: cho.key + "=" + modelData[0]
                width: segLabel.implicitWidth + 22
                height: 26
                radius: Theme.radiusControl
                color: on ? Theme.accent : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, segHover.hovered ? 0.12 : 0.06)
                Text {
                    id: segLabel
                    anchors.centerIn: parent
                    text: modelData[1]
                    color: parent.on ? Theme.onAccent : Theme.text
                    font.family: Theme.fontUi
                    font.pixelSize: 12
                    font.weight: parent.on ? Font.DemiBold : Font.Normal
                }
                HoverHandler { id: segHover; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: win.set(cho.key, modelData[0]) }
            }
        }
    }

    // A whole number, typed (↑/↓ step it), between words.
    component NumberBox: Row {
        id: num
        property string key: ""
        property int from: 0
        property int to: 9999
        property string prefix: ""
        property string suffix: ""
        objectName: key
        spacing: 8
        Note { visible: num.prefix !== ""; anchors.verticalCenter: parent.verticalCenter; text: num.prefix; color: Theme.text; font.pixelSize: Theme.fontSize }
        Rectangle {
            width: 56
            height: 26
            radius: Theme.radiusControl
            color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.06)
            border.width: 1
            border.color: numInput.activeFocus ? Theme.accent : Theme.hairline
            TextInput {
                id: numInput
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 8
                verticalAlignment: TextInput.AlignVCenter
                horizontalAlignment: TextInput.AlignRight
                text: String(win.value(num.key) ?? "")
                validator: IntValidator { bottom: num.from; top: num.to }
                selectByMouse: true
                color: Theme.text
                selectionColor: Theme.accent
                selectedTextColor: Theme.onAccent
                font.family: Theme.fontUi
                font.pixelSize: Theme.fontSize
                font.features: { "tnum": 1 }
                function step(d) {
                    win.set(num.key, Math.max(num.from, Math.min(num.to, (parseInt(text) || 0) + d)))
                }
                Keys.onUpPressed: step(1)
                Keys.onDownPressed: step(-1)
                onEditingFinished: if (acceptableInput && parseInt(text) !== win.value(num.key)) win.set(num.key, parseInt(text))
            }
        }
        Note { visible: num.suffix !== ""; anchors.verticalCenter: parent.verticalCenter; text: num.suffix; color: Theme.text; font.pixelSize: Theme.fontSize }
    }

    // An action's keys, CleanShot's recorder: click, then press the keys.
    component KeyField: Row {
        id: kf
        property string action: ""
        property var keys: []
        readonly property bool listening: win.recording === action
        spacing: 4
        Rectangle {
            objectName: "keys:" + kf.action
            width: 200
            height: 28
            radius: Theme.radiusControl
            color: kf.listening ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.14)
                                : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, keyHover.hovered ? 0.1 : 0.06)
            border.width: 1
            border.color: kf.listening ? Theme.accent : Theme.hairline
            Text {
                anchors.centerIn: parent
                width: parent.width - 16
                horizontalAlignment: Text.AlignHCenter
                elide: Text.ElideRight
                text: kf.listening ? "Type shortcut…" : kf.keys.length ? kf.keys.join(", ") : "Record Shortcut"
                color: kf.listening || kf.keys.length ? Theme.text : Theme.textSecondary
                font.family: Theme.fontUi
                font.pixelSize: Theme.fontSize
                font.weight: kf.keys.length && !kf.listening ? Font.DemiBold : Font.Normal
            }
            HoverHandler { id: keyHover; cursorShape: Qt.PointingHandCursor }
            TapHandler { onTapped: kf.listening ? win.stopRecording() : win.startRecording(kf.action) }
        }
        Item {
            objectName: "clear:" + kf.action
            visible: kf.keys.length > 0 && !kf.listening
            width: 24
            height: 28
            Icon {
                anchors.centerIn: parent
                name: "x"
                size: 14
                color: clearHover.hovered ? Theme.text : Theme.textSecondary
            }
            HoverHandler { id: clearHover; cursorShape: Qt.PointingHandCursor }
            TapHandler { onTapped: win.setShortcut(kf.action, "") }
        }
    }

    // A line of text, written when Enter is pressed or it loses focus.
    component Field: Rectangle {
        id: fld
        property string key: ""
        property string placeholder: ""
        readonly property alias text: fieldInput.text
        objectName: key
        width: 380
        height: 26
        radius: Theme.radiusControl
        color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.06)
        border.width: 1
        border.color: fieldInput.activeFocus ? Theme.accent : Theme.hairline
        TextInput {
            id: fieldInput
            anchors.fill: parent
            anchors.leftMargin: 8
            anchors.rightMargin: 8
            verticalAlignment: TextInput.AlignVCenter
            clip: true
            text: win.value(fld.key) || ""
            selectByMouse: true
            color: Theme.text
            selectionColor: Theme.accent
            selectedTextColor: Theme.onAccent
            font.family: Theme.fontUi
            font.pixelSize: Theme.fontSize
            onEditingFinished: if (text !== (win.value(fld.key) || "")) win.set(fld.key, text)
            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: !fieldInput.text
                text: fld.placeholder
                color: Theme.textSecondary
                font: fieldInput.font
            }
        }
    }

    // ---------------------------------------------------------------- tests
    function find(item, name) {
        if (item.objectName === name && item.visible) return item
        for (const c of item.children) {
            if (!c.visible) continue
            const f = find(c, name)
            if (f) return f
        }
        return null
    }
    function debug(a) {
        switch (a[0]) {
        case "tab": tab = a[1]; break
        case "keys": Platform.sendKeys(win, a.slice(1).join(" ")); break
        case "close": close(); break
        case "record": startRecording(a[1]); break
        case "press": recorded(parseInt(a[1]), parseInt(a[2])); break      // Qt modifiers, keycode
        case "use": setShortcut(pending.action, pending.spec); break
        case "restore": restoreDefaults(); break
        case "where": {
            // the centre of a control in the window (logical px), for a click;
            // positioners lay out at the next frame: now, so it's where it'll be
            const it = find(contentItem, a[1])
            for (let p = it; p; p = p.parent) if (p.forceLayout) p.forceLayout()
            if (it) {
                // scrolled into view, as a hand would
                const top = it.mapToItem(flick.contentItem, 0, 0).y
                if (top < flick.contentY || top + it.height > flick.contentY + flick.height)
                    flick.contentY = Math.max(0, Math.min(flick.contentHeight - flick.height, top - flick.height / 2))
            }
            // laid out first: just after a change, rows and columns still hold old places
            for (let q = it; q; q = q.parent) if (q.forceLayout) q.forceLayout()
            const p = it ? it.mapToItem(contentItem, it.width / 2, it.height / 2) : null
            Engine.log("settings_where", JSON.stringify({ name: a[1], x: p ? p.x : -1, y: p ? p.y : -1 }).slice(1, -1))
            break
        }
        case "state":
            Engine.log("settings_state", JSON.stringify({ open: visible, tab: tab, error: cfgError, confirmClear: confirmClear,
                                                         version: controller.settings.version, value: a[1] ? value(a[1]) : null,
                                                         recording: recording, pending: pending, shortcuts: shortcutList }).slice(1, -1))
            break
        }
    }
}
