import QtQuick
import QtQuick.Window
import QtQuick.Effects
import QtQuick.Shapes
import Snip.Core
import "editor.js" as Ed

// Snip's editor (ANN-01…19), CleanShot's Annotate window: a toolbar, the
// canvas, a bottom bar. The document is a list of plain objects in image
// pixels (editor.js); edits go back to the capture on their own (ANN-15),
// as the same scene rendered offscreen at native size (Platform.grabExact).
Window {
    id: ed
    property var capture: ({})          // editor_open: {id, image, width, height, doc, title, output, scale}
    property var controller: null
    signal finished()

    readonly property string captureId: capture.id || ""
    readonly property int imgW: capture.width || 1
    readonly property int imgH: capture.height || 1
    readonly property real dpr: devicePixelRatio > 0 ? devicePixelRatio : 1
    // image px per logical px: sizes look the same on any scale
    readonly property real unit: dpr

    title: "Snip Editor — " + (capture.title || "Capture")
    color: Theme.background
    minimumWidth: 640
    minimumHeight: 420

    // ---------------------------------------------------------------- document
    property var doc: ({ v: 1, items: [], crop: null })
    property var undoStack: []
    property var redoStack: []
    property int revision: 0
    property int savedRevision: 0

    function commit(next) {
        undoStack = undoStack.concat([doc])
        redoStack = []
        doc = next
        changed()
    }
    function changed() {
        revision++
        saveTimer.restart()
        Engine.log("editor_changed", JSON.stringify({ id: captureId, items: doc.items.length, revision: revision }).slice(1, -1))
    }
    function undo() {
        finishText()
        if (!undoStack.length) return
        redoStack = redoStack.concat([doc])
        doc = undoStack[undoStack.length - 1]
        undoStack = undoStack.slice(0, -1)
        selected = -1
        changed()
    }
    function redo() {
        finishText()
        if (!redoStack.length) return
        undoStack = undoStack.concat([doc])
        doc = redoStack[redoStack.length - 1]
        redoStack = redoStack.slice(0, -1)
        selected = -1
        changed()
    }
    function setItems(items) { commit(Object.assign({}, doc, { items: items })) }
    function addItem(o) {
        setItems(doc.items.concat([o]))
        selected = doc.items.length - 1
    }
    function replaceItem(i, o) {
        const items = doc.items.slice()
        items[i] = o
        setItems(items)
    }
    function removeSelected() {
        if (selected < 0) return
        const items = doc.items.slice()
        items.splice(selected, 1)
        selected = -1
        setItems(items)
    }

    // ---------------------------------------------------------------- tools
    property string tool: "arrow"
    property string ink: Ed.palette[8]     // the colour new objects get
    property var sizeIndex: Object.assign({}, Ed.defaultSize)
    readonly property string sizeKind: tool !== "select" ? tool
                                     : !selectedObj ? "arrow" : selectedObj.filled ? "frect" : selectedObj.kind
    function sizeFor(kind) { return Math.round(Ed.sizes[kind][sizeIndex[kind]] * unit) }
    property string redactStyle: "pixelate"   // CleanShot's default (D26)
    property string arrowStyle: "standard"
    property string textStyle: "standard"
    property string counterStyle: "num"
    // the highlighter keeps its own colour (yellow) and opacity
    property string highlightInk: "#ffcc00"
    property real highlightOpacity: 0.35
    readonly property string toolInk: sizeKind === "highlight" ? highlightInk : ink
    // OCR word boxes for the highlighter, fetched once it is first used
    property var words: null
    readonly property var textLines: words ? Ed.textLines(words) : []
    onToolChanged: if (tool === "highlight" && words === null) { words = []; Engine.requestWords(capture.image || "") }
    Connections {
        target: Engine
        function onEditorWords(path, json) { if (path === (ed.capture.image || "")) ed.words = JSON.parse(json) }
    }
    property string spotShape: "rect"
    property real spotDim: 0.55
    property string toolBeforeCrop: "arrow"

    function setTool(t) {
        finishText()
        if (t === "crop" && tool !== "crop") toolBeforeCrop = tool
        tool = t
        if (t !== "select") selected = -1
    }
    function setHighlightOpacity(v) {
        highlightOpacity = v
        if (selectedObj && selectedObj.kind === "highlight" && selectedObj.opacity !== v)
            replaceItem(selected, Object.assign(Ed.copy(selectedObj), { opacity: v }))
    }
    function setCounterStyle(st) {
        counterStyle = st
        if (selectedObj && selectedObj.kind === "counter" && (selectedObj.style || "num") !== st)
            replaceItem(selected, Object.assign(Ed.copy(selectedObj), { style: st }))
    }
    function setCounterStart(v) {
        v = Math.max(0, Math.min(99, v))
        if (v !== (doc.counterStart === undefined ? 1 : doc.counterStart)) commit(Object.assign({}, doc, { counterStart: v }))
    }
    // spotlight options apply to the selected spotlight too
    function setSpot(shape, dim) {
        spotShape = shape; spotDim = dim
        if (selectedObj && selectedObj.kind === "spotlight" && (selectedObj.shape !== shape || selectedObj.dim !== dim))
            replaceItem(selected, Object.assign(Ed.copy(selectedObj), { shape: shape, dim: dim }))
    }
    function setTextStyle(st) {
        textStyle = st
        if (editingText) { textObj = Object.assign(Ed.copy(textObj), { style: st }); return }
        if (selectedObj && selectedObj.kind === "text" && (selectedObj.style || "standard") !== st) {
            const o = Ed.copy(selectedObj); o.style = st; replaceItem(selected, o)
        }
    }
    function setArrowStyle(st) {
        arrowStyle = st
        if (selectedObj && selectedObj.kind === "arrow" && (selectedObj.style || "standard") !== st)
            replaceItem(selected, Ed.withArrowStyle(selectedObj, st))
    }
    function setRedactStyle(st) {
        redactStyle = st
        if (selectedObj && selectedObj.kind === "redact" && selectedObj.style !== st) {
            const o = Ed.copy(selectedObj); o.style = st; replaceItem(selected, o)
        }
    }
    // colour and size apply to the selection too, as in CleanShot
    function setColor(c) {
        if (sizeKind === "highlight") highlightInk = c; else ink = c
        if (selectedObj && selectedObj.color !== c) {
            const o = Ed.copy(selectedObj); o.color = c; replaceItem(selected, o)
        }
    }
    function setSize(i) {
        const kind = sizeKind
        if (!Ed.sizes[kind]) return
        const s = Object.assign({}, sizeIndex); s[kind] = i; sizeIndex = s
        if (selectedObj && selectedObj.kind === kind) {
            const o = Ed.copy(selectedObj); o.size = sizeFor(kind); replaceItem(selected, o)
        }
    }

    property int selected: -1
    readonly property var selectedObj: selected >= 0 && selected < doc.items.length ? doc.items[selected] : null
    // The object under the pointer, drawn on top while it changes; it
    // replaces document object `draftIndex` (-1: a new one).
    property var draft: null
    property int draftIndex: -1
    readonly property var shownSelection: draft && draftIndex === selected ? draft : selectedObj
    // the document as drawn right now, with the object being changed
    readonly property var liveItems: !draft ? doc.items
                                   : draftIndex >= 0 ? doc.items.map((o, i) => i === draftIndex ? draft : o) : doc.items.concat([draft])
    readonly property rect selectionBox: selectionHalo.box

    // ---------------------------------------------------------------- view
    readonly property var full: ({ x: 0, y: 0, w: imgW, h: imgH })
    // what is exported (the crop) and what is on screen (all of it while cropping)
    readonly property var exportRect: doc.crop || full
    readonly property var shown: tool === "crop" ? full : exportRect
    property var cropDraft: null
    readonly property var cropShown: cropDraft || doc.crop || full
    function setCrop(r) {
        r = Ed.clampRect(r, imgW, imgH)
        if (r.w < 4 || r.h < 4) return
        const c = r.x === 0 && r.y === 0 && r.w === imgW && r.h === imgH ? null : r
        if (JSON.stringify(c) !== JSON.stringify(doc.crop)) commit(Object.assign({}, doc, { crop: c }))
    }
    function cropDone() { setTool(toolBeforeCrop === "crop" ? "arrow" : toolBeforeCrop) }
    property real zoom: 1               // 1 = 100 %: one image px per device px
    property bool fit: true
    readonly property real k: zoom / dpr // view px per image px
    readonly property real fitZoom: Math.min(1, Math.max(0.05, (view.width - 64) * dpr / shown.w), Math.max(0.05, (view.height - 64) * dpr / shown.h))
    onFitZoomChanged: if (fit) zoom = fitZoom
    property real panX: 0
    property real panY: 0
    // whole device pixels, so 100 % is crisp
    readonly property real originX: Math.round((view.width / 2 - shown.w * k / 2 + panX) * dpr) / dpr
    readonly property real originY: Math.round((view.height / 2 - shown.h * k / 2 + panY) * dpr) / dpr

    function toImage(vx, vy) { return { x: (vx - originX) / k + shown.x, y: (vy - originY) / k + shown.y } }
    function toView(ix, iy) { return { x: originX + (ix - shown.x) * k, y: originY + (iy - shown.y) * k } }

    function clampPan() {
        const mx = Math.max(0, (shown.w * k - view.width) / 2 + 32), my = Math.max(0, (shown.h * k - view.height) / 2 + 32)
        panX = Math.max(-mx, Math.min(mx, panX))
        panY = Math.max(-my, Math.min(my, panY))
    }
    // zoom about a view point, keeping the image pixel under it in place
    function zoomTo(z, vx, vy) {
        if (vx === undefined) { vx = view.width / 2; vy = view.height / 2 }
        const p = toImage(vx, vy)
        fit = false
        zoom = Math.max(0.05, Math.min(8, z))
        panX = vx - view.width / 2 + shown.w * k / 2 - (p.x - shown.x) * k
        panY = vy - view.height / 2 + shown.h * k / 2 - (p.y - shown.y) * k
        clampPan()
    }
    function zoomFit() { fit = true; zoom = fitZoom; panX = 0; panY = 0 }
    readonly property var zoomSteps: [0.1, 0.25, 0.5, 0.75, 1, 1.5, 2, 3, 4, 6, 8]
    function zoomStep(dir) {
        const z = dir > 0 ? zoomSteps.find(s => s > zoom * 1.001) : zoomSteps.slice().reverse().find(s => s < zoom * 0.999)
        if (z !== undefined) zoomTo(z)
    }

    // ---------------------------------------------------------------- write back
    property bool committing: false
    property int commitRevision: 0
    property string commitAction: ""
    property string commitDoc: ""
    property int commitToken: 0
    property bool queued: false
    property string queuedAction: ""
    property bool closing: false
    Timer { id: saveTimer; interval: 700; onTriggered: ed.flush("") }

    // Write the document back to the capture, then do `action`
    // ("" | copy | save | close). One commit at a time, in order.
    function flush(action) {
        finishText()
        if (committing) {
            queued = true
            if (action) queuedAction = action
            return
        }
        saveTimer.stop()
        if (revision === savedRevision) { act(action, true, ""); return }
        committing = true
        commitRevision = revision
        commitAction = action
        commitDoc = JSON.stringify(doc)
        commitToken = controller.nextToken()
        Platform.grabExact(exportRoot, exportRect.w, exportRect.h, commitToken)
    }
    Connections {
        target: Platform
        function onGrabbed(token, ok) {
            if (token !== ed.commitToken || !ed.committing) return
            if (ok) Engine.editorCommit(ed.captureId, token, ed.commitDoc, ed.commitAction === "close" ? "" : ed.commitAction)
            else ed.committed(false, "Couldn't render the image")
        }
    }
    Connections {
        target: Engine
        function onEditorDone(id, action, ok, detail) {
            if (id !== ed.captureId || !ed.committing) return
            ed.committed(ok, detail)
        }
    }
    function committed(ok, detail) {
        committing = false
        if (ok) savedRevision = commitRevision
        const action = commitAction
        if (queued) {
            queued = false
            const next = queuedAction || action
            queuedAction = ""
            flush(next)
            return
        }
        // copy and save already ran on the written capture
        act(action === "close" ? "close" : "", ok, detail, action)
    }
    // `ran`: the action the commit already performed in the engine
    function act(action, ok, detail, ran) {
        if (!ok) {
            closing = false
            toast(detail || "Couldn't save the edit")
            return
        }
        if (pendingSaveAs !== "") {
            Engine.saveAs(captureId, pendingSaveAs)
            pendingSaveAs = ""
            toast("Saved")
            close_()
            return
        }
        const what = ran || action
        if (what === "copy" && !ran) Engine.copy(captureId)
        if (what === "save" && !ran) Engine.save(captureId)
        if (what === "copy" || what === "save") {
            toast(what === "copy" ? "Copied" : "Saved")
            close_()
        } else if (action === "close") {
            close_()
        }
    }
    // Save As (ANN-15): pick a path, write the edits back, then save there
    property int saveAsToken: 0
    property string pendingSaveAs: ""
    function saveAsDialog() {
        const sug = JSON.parse(Engine.saveAsSuggestion(captureId) || "{}")
        saveAsToken = controller.nextToken()
        Platform.saveFileDialog(saveAsToken, "Save As", sug.name || "", sug.folder || "")
    }
    Connections {
        target: Platform
        function onFileChosen(token, path) {
            if (token !== ed.saveAsToken || path === "") return
            ed.pendingSaveAs = path
            ed.flush("")
        }
    }
    function toast(t) { controller.toast.show(t, capture.output || Qt.application.screens[0].name) }
    function close_() {
        Engine.log("editor_closed", JSON.stringify({ id: captureId, revision: revision }).slice(1, -1))
        visible = false
        finished()
    }
    function closeEditor() {
        if (closing) return
        closing = true
        flush("close")
    }
    onClosing: (close) => { close.accepted = false; closeEditor() }

    Component.onCompleted: {
        const d = capture.doc
        if (d && d.items) doc = { v: 1, items: d.items, crop: d.crop || null, counterStart: d.counterStart === undefined ? 1 : d.counterStart }
        const scr = Platform.screen(capture.output || "")
        const aw = scr ? scr.availableGeometry.width : 1600, ah = scr ? scr.availableGeometry.height : 1000
        const s = capture.scale || 1
        width = Math.round(Math.max(minimumWidth, Math.min(aw * 0.86, imgW / s + 96)))
        height = Math.round(Math.max(minimumHeight, Math.min(ah * 0.86, imgH / s + 96 + toolbar.height + bottomBar.height)))
        visible = true
        requestActivate()
        keys.forceActiveFocus()
        Engine.log("editor_shown", JSON.stringify({ id: captureId, w: imgW, h: imgH, winW: width, winH: height, aw: aw, ah: ah, s: s }).slice(1, -1))
    }

    // ---------------------------------------------------------------- text
    property bool editingText: false
    property var textObj: null
    function startText(p, index) {
        finishText()
        textObj = index >= 0 ? Ed.copy(doc.items[index])
                             : { kind: "text", x: p.x, y: p.y - sizeFor("text") * 0.65, text: "", color: ink, size: sizeFor("text"), style: textStyle }
        draftIndex = index
        selected = index
        editingText = true
        textEdit.text = textObj.text
        textEdit.cursorPosition = textEdit.text.length
        textEdit.forceActiveFocus()
    }
    function finishText() {
        if (!editingText) return
        editingText = false
        const t = textEdit.text.replace(/\s+$/, "")
        const i = draftIndex
        draftIndex = -1
        if (i >= 0) {
            if (t === "") { selected = i; removeSelected() }
            else if (t !== doc.items[i].text) { const o = Ed.copy(textObj); o.text = t; replaceItem(i, o) }
        } else if (t !== "") {
            const o = Ed.copy(textObj); o.text = t; addItem(o)
        }
        keys.forceActiveFocus()
    }

    // ---------------------------------------------------------------- keys
    FocusScope {
        id: keys
        anchors.fill: parent
        focus: true
        property bool spaceHeld: false

        Keys.onPressed: (e) => {
            const ctrl = (e.modifiers & Qt.ControlModifier) !== 0, shift = (e.modifiers & Qt.ShiftModifier) !== 0
            if (ctrl) {
                switch (e.key) {
                case Qt.Key_Z: shift ? ed.redo() : ed.undo(); break
                case Qt.Key_Y: ed.redo(); break
                case Qt.Key_S: shift ? ed.saveAsDialog() : ed.flush("save"); break
                // with an object selected Ctrl+C copies it; Ctrl+Shift+C always copies the image
                case Qt.Key_C: (!shift && ed.selectedObj) ? ed.copyObjects() : ed.flush("copy"); break
                case Qt.Key_V: ed.pasteObjects(); break
                case Qt.Key_D: ed.duplicateSelected(); break
                case Qt.Key_0: ed.zoomFit(); break
                case Qt.Key_1: ed.zoomTo(1); break
                case Qt.Key_Plus: case Qt.Key_Equal: ed.zoomStep(1); break
                case Qt.Key_Minus: ed.zoomStep(-1); break
                default: return
                }
                e.accepted = true
                return
            }
            const nudge = shift ? 10 : 1
            if (ed.tool === "crop" && (e.key === Qt.Key_Escape || e.key === Qt.Key_Return || e.key === Qt.Key_Enter)) {
                ed.cropDone()
                e.accepted = true
                return
            }
            switch (e.key) {
            case Qt.Key_Escape:
                if (ed.selected >= 0) ed.selected = -1
                else ed.closeEditor()
                break
            case Qt.Key_Delete: case Qt.Key_Backspace: ed.removeSelected(); break
            case Qt.Key_Space: if (!e.isAutoRepeat) spaceHeld = true; break
            case Qt.Key_V: ed.setTool("select"); break
            case Qt.Key_A: ed.setTool("arrow"); break
            case Qt.Key_R: ed.setTool("rect"); break
            case Qt.Key_F: ed.setTool("frect"); break
            case Qt.Key_E: ed.setTool("ellipse"); break
            case Qt.Key_L: ed.setTool("line"); break
            case Qt.Key_H: ed.setTool("spotlight"); break
            case Qt.Key_D: ed.setTool("pencil"); break
            case Qt.Key_M: ed.setTool("highlight"); break
            case Qt.Key_T: ed.setTool("text"); break
            case Qt.Key_C: ed.setTool("counter"); break
            case Qt.Key_P: ed.setTool("redact"); break
            case Qt.Key_K: ed.setTool("crop"); break
            case Qt.Key_Left: ed.nudge(-nudge, 0); break
            case Qt.Key_Right: ed.nudge(nudge, 0); break
            case Qt.Key_Up: ed.nudge(0, -nudge); break
            case Qt.Key_Down: ed.nudge(0, nudge); break
            default: return
            }
            e.accepted = true
        }
        Keys.onReleased: (e) => { if (e.key === Qt.Key_Space && !e.isAutoRepeat) spaceHeld = false }
    }
    // Select's commands (CleanShot): Ctrl+C / Ctrl+V objects, Ctrl+D duplicates
    property int pasteCount: 0
    function copyObjects() {
        controller.objectClipboard = [Ed.copy(selectedObj)]
        pasteCount = 0
    }
    function pasteObjects() {
        const objs = controller.objectClipboard || []
        if (!objs.length) return
        pasteCount++
        const off = 16 * pasteCount * unit
        setItems(doc.items.concat(objs.map(o => Ed.moved(o, off, off))))
        selected = doc.items.length - 1
    }
    function duplicateSelected() {
        if (selectedObj) addItem(Ed.moved(selectedObj, 16 * unit, 16 * unit))
    }
    function nudge(dx, dy) {
        if (selectedObj) replaceItem(selected, Ed.moved(selectedObj, dx, dy))
    }

    // ---------------------------------------------------------------- toolbar
    EditorToolbar {
        id: toolbar
        ed: ed
        z: 2
        anchors { left: parent.left; right: parent.right; top: parent.top }
    }

    // ---------------------------------------------------------------- canvas
    Item {
        id: view
        anchors { left: parent.left; right: parent.right; top: toolbar.bottom; bottom: bottomBar.top }
        clip: true

        RectangularShadow {
            anchors.fill: clipper
            blur: 24; offset.y: 6
            color: Qt.rgba(0, 0, 0, Theme.light ? 0.16 : 0.4)
        }
        Item {
            id: clipper
            x: ed.originX; y: ed.originY
            width: ed.shown.w * ed.k; height: ed.shown.h * ed.k
            clip: true

            // what gets exported: only this subtree, in image px (the grab
            // ignores its scale)
            Item {
                id: exportRoot
                // the grab ignores this position and scale: it renders the crop
                x: (ed.exportRect.x - ed.shown.x) * ed.k; y: (ed.exportRect.y - ed.shown.y) * ed.k
                scale: ed.k
                transformOrigin: Item.TopLeft
                Item {
                    id: content
                    x: -ed.exportRect.x; y: -ed.exportRect.y
                    Image {
                        id: base
                        width: ed.imgW; height: ed.imgH
                        source: ed.capture.image ? "file://" + ed.capture.image : ""
                        cache: false
                        smooth: true
                        mipmap: true
                        fillMode: Image.Stretch
                    }
                    SpotlightLayer {
                        width: ed.imgW; height: ed.imgH
                        items: ed.liveItems
                    }
                    Repeater {
                        id: objects
                        model: ed.doc.items
                        delegate: EditorItem {
                            required property var modelData
                            required property int index
                            o: modelData
                            base: ed.capture.image || ""
                            label: Ed.counterLabel(ed.doc, Ed.counterNumber(ed.doc.items, index), modelData.style)
                            z: modelData.kind === "counter" ? 1 : 0    // counters stay on top (CleanShot)
                            visible: index !== ed.draftIndex
                        }
                    }
                    EditorItem {
                        o: ed.draft
                        base: ed.capture.image || ""
                        visible: ed.draft !== null && !ed.editingText
                        z: 2
                        label: ed.draft && ed.draft.kind === "counter"
                               ? Ed.counterLabel(ed.doc, ed.draftIndex >= 0 ? Ed.counterNumber(ed.doc.items, ed.draftIndex)
                                                                           : Ed.counterNumber(ed.doc.items, ed.doc.items.length, ed.draft.style), ed.draft.style) : "1"
                    }
                }
            }
        }

        // selection, outside the exported scene: halo under a copy of the object
        Item {
            x: ed.originX; y: ed.originY
            scale: ed.k
            transformOrigin: Item.TopLeft
            Item {
                x: -ed.shown.x; y: -ed.shown.y
                EditorItem {
                    // also measures the selection: text handles sit on its box
                    id: selectionHalo
                    visible: ed.shownSelection !== null && !ed.editingText
                    o: ed.shownSelection
                    halo: true
                    pad: 4 / ed.k
                }
                EditorItem {
                    visible: ed.shownSelection !== null && !ed.editingText && ed.shownSelection.kind !== "text"
                    o: ed.shownSelection
                    base: ed.capture.image || ""
                    label: ed.selected >= 0 && ed.shownSelection ? Ed.counterLabel(ed.doc, Ed.counterNumber(ed.doc.items, ed.selected), ed.shownSelection.style) : "1"
                }
                // text being typed, exactly where it will be drawn
                Rectangle {
                    visible: ed.editingText
                    x: ed.textObj ? ed.textObj.x - 8 / ed.k : 0
                    y: ed.textObj ? ed.textObj.y - 4 / ed.k : 0
                    width: Math.max(textEdit.contentWidth, ed.textObj ? ed.textObj.size * 0.6 : 0) + 16 / ed.k
                    height: textEdit.contentHeight + 8 / ed.k
                    radius: 6 / ed.k
                    color: ed.textObj && Ed.textBoxed(ed.textObj.style) ? Ed.textTint(ed.textObj.color) : Qt.rgba(0.04, 0.47, 1, 0.06)
                    border.width: 1.5 / ed.k
                    border.color: Qt.rgba(0.04, 0.47, 1, 0.75)
                }
                TextEdit {
                    id: textEdit
                    visible: ed.editingText
                    x: ed.textObj ? ed.textObj.x : 0
                    y: ed.textObj ? ed.textObj.y : 0
                    color: ed.textObj ? ed.textObj.color : "black"
                    readonly property string family: ed.textObj ? Ed.textFamily(ed.textObj.style) : "ui"
                    font.family: family === "rounded" ? Theme.fontRounded : family === "mono" ? Theme.fontMono : Theme.fontUi
                    font.pixelSize: ed.textObj ? ed.textObj.size : 20
                    font.weight: family === "mono" ? Font.DemiBold : Font.Bold
                    width: ed.textObj && ed.textObj.w > 0 ? ed.textObj.w : implicitWidth
                    wrapMode: ed.textObj && ed.textObj.w > 0 ? TextEdit.Wrap : TextEdit.NoWrap
                    textFormat: TextEdit.PlainText
                    selectByMouse: true
                    cursorDelegate: Rectangle { width: 2 / ed.k; color: Qt.rgba(0.04, 0.47, 1, 1) }
                    Keys.onPressed: (e) => {
                        if (e.key === Qt.Key_Escape || ((e.key === Qt.Key_Return || e.key === Qt.Key_Enter) && !(e.modifiers & Qt.ShiftModifier))) {
                            ed.finishText()
                            e.accepted = true
                        }
                    }
                }
            }
        }

        // handles, in view px so they keep their size at any zoom
        Repeater {
            model: ed.shownSelection && !ed.editingText ? Ed.handles(ed.shownSelection, ed.selectionBox) : []
            delegate: Rectangle {
                required property var modelData
                readonly property var at: ed.toView(modelData.x, modelData.y)
                x: at.x - width / 2; y: at.y - height / 2
                width: modelData.square ? 10 : 13; height: width; radius: modelData.square ? 2 : 6.5
                color: "#0a7cff"
                border.width: 2; border.color: "white"
                layer.enabled: true
                layer.effect: MultiEffect { shadowEnabled: true; shadowBlur: 0.4; shadowOpacity: 0.35; shadowVerticalOffset: 1 }
            }
        }

        // crop: the rest dimmed, a frame with handles, size and Done
        Item {
            id: cropUi
            anchors.fill: parent
            visible: ed.tool === "crop"
            readonly property var a: ed.toView(ed.cropShown.x, ed.cropShown.y)
            readonly property var b: ed.toView(ed.cropShown.x + ed.cropShown.w, ed.cropShown.y + ed.cropShown.h)
            readonly property var i0: ed.toView(0, 0)
            readonly property var i1: ed.toView(ed.imgW, ed.imgH)
            readonly property color dim: Qt.rgba(0, 0, 0, 0.55)
            Rectangle { x: cropUi.i0.x; y: cropUi.i0.y; width: cropUi.i1.x - cropUi.i0.x; height: cropUi.a.y - cropUi.i0.y; color: cropUi.dim }
            Rectangle { x: cropUi.i0.x; y: cropUi.b.y; width: cropUi.i1.x - cropUi.i0.x; height: cropUi.i1.y - cropUi.b.y; color: cropUi.dim }
            Rectangle { x: cropUi.i0.x; y: cropUi.a.y; width: cropUi.a.x - cropUi.i0.x; height: cropUi.b.y - cropUi.a.y; color: cropUi.dim }
            Rectangle { x: cropUi.b.x; y: cropUi.a.y; width: cropUi.i1.x - cropUi.b.x; height: cropUi.b.y - cropUi.a.y; color: cropUi.dim }
            Rectangle {
                x: cropUi.a.x - 1; y: cropUi.a.y - 1
                width: cropUi.b.x - cropUi.a.x + 2; height: cropUi.b.y - cropUi.a.y + 2
                color: "transparent"
                border.width: 1.5; border.color: "white"
                // thirds
                Repeater {
                    model: 2
                    Rectangle { required property int index; x: parent.width * (index + 1) / 3; y: 1; width: 1; height: parent.height - 2; color: Qt.rgba(1, 1, 1, 0.35) }
                }
                Repeater {
                    model: 2
                    Rectangle { required property int index; y: parent.height * (index + 1) / 3; x: 1; height: 1; width: parent.width - 2; color: Qt.rgba(1, 1, 1, 0.35) }
                }
            }
            Repeater {
                model: Ed.cropHandles(ed.cropShown)
                delegate: Rectangle {
                    required property var modelData
                    readonly property var at: ed.toView(modelData.x, modelData.y)
                    readonly property bool corner: modelData.name.length === 2
                    x: at.x - width / 2; y: at.y - height / 2
                    width: corner ? 14 : (modelData.name === "t" || modelData.name === "b" ? 22 : 6)
                    height: corner ? 14 : (modelData.name === "l" || modelData.name === "r" ? 22 : 6)
                    radius: 3
                    color: "white"
                    border.width: 1; border.color: Qt.rgba(0, 0, 0, 0.25)
                }
            }
            Rectangle {
                id: cropBar
                anchors.horizontalCenter: parent.horizontalCenter
                y: 12
                width: cropRow.implicitWidth + 16; height: 36
                radius: height / 2
                color: Theme.light ? "#ffffff" : Qt.lighter(Theme.surface, 1.2)
                border.width: 1; border.color: Theme.hairline
                Row {
                    id: cropRow
                    anchors.centerIn: parent
                    spacing: 6
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        leftPadding: 8; rightPadding: 4
                        text: Math.round(ed.cropShown.w) + " × " + Math.round(ed.cropShown.h)
                        color: Theme.textSecondary
                        font.family: Theme.fontUi; font.pixelSize: 13; font.features: { "tnum": 1 }
                    }
                    ToolButton { label: "Reset"; onClicked: { ed.setCrop(ed.full) } }
                    ToolButton { label: "Done"; selected: true; onClicked: ed.cropDone() }
                }
            }
        }

        MouseArea {
            id: pointer
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.LeftButton | Qt.MiddleButton
            cursorShape: ed.picking ? Qt.CrossCursor
                       : gesture === "pan" || keys.spaceHeld ? Qt.ClosedHandCursor
                       : ed.tool === "crop" ? Qt.CrossCursor
                       : hoverHandle !== "" ? Qt.SizeAllCursor
                       : hoverItem >= 0 ? Qt.OpenHandCursor
                       : ed.tool === "text" ? Qt.IBeamCursor : ed.tool === "select" ? Qt.ArrowCursor : Qt.CrossCursor

            property string gesture: ""     // draw | move | handle | pan
            property string handle: ""
            property var pressAt: null       // image px
            property var pressObj: null
            property rect pressBox
            property real pressVX: 0
            property real pressVY: 0
            property real pressPanX: 0
            property real pressPanY: 0
            property string hoverHandle: ""
            property int hoverItem: -1

            function cropHandleAt(vx, vy) {
                for (const h of Ed.cropHandles(ed.cropShown)) {
                    const v = ed.toView(h.x, h.y)
                    if (Math.hypot(v.x - vx, v.y - vy) <= 12) return h.name
                }
                return ""
            }
            function handleAt(vx, vy) {
                if (!ed.selectedObj) return ""
                for (const h of Ed.handles(ed.selectedObj, ed.selectionBox)) {
                    const v = ed.toView(h.x, h.y)
                    if (Math.hypot(v.x - vx, v.y - vy) <= 10) return h.name
                }
                return ""
            }
            function itemAt(p) {
                const tol = 6 / ed.k
                for (let i = ed.doc.items.length - 1; i >= 0; i--) {
                    const d = objects.itemAt(i)
                    if (Ed.hits(ed.doc.items[i], p.x, p.y, tol, d ? d.box : null)) return i
                }
                return -1
            }

            onPressed: (m) => {
                keys.forceActiveFocus()
                pressVX = m.x; pressVY = m.y
                if (ed.picking) {
                    const ip = ed.toImage(m.x, m.y)
                    ed.pickAt(ip.x, ip.y)
                    return
                }
                if (keys.spaceHeld || m.button === Qt.MiddleButton) {
                    gesture = "pan"; pressPanX = ed.panX; pressPanY = ed.panY
                    return
                }
                const p = ed.toImage(m.x, m.y)
                pressAt = p
                if (ed.editingText) ed.finishText()
                if (ed.tool === "crop") {
                    const r = ed.cropShown
                    handle = cropHandleAt(m.x, m.y)
                    gesture = handle !== "" ? "crop-handle"
                            : p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h ? "crop-move" : "crop-new"
                    pressObj = Object.assign({ kind: "rect" }, r)
                    ed.cropDraft = Ed.copy(r)
                    return
                }
                const h = handleAt(m.x, m.y)
                if (h !== "") {
                    gesture = "handle"; handle = h
                    pressBox = Qt.rect(ed.selectionBox.x, ed.selectionBox.y, ed.selectionBox.width, ed.selectionBox.height)
                    pressObj = ed.selectedObj; ed.draftIndex = ed.selected; ed.draft = Ed.copy(pressObj)
                    return
                }
                const i = itemAt(p)
                if (i >= 0) {
                    ed.selected = i
                    pressObj = ed.doc.items[i]
                    if (m.modifiers & Qt.AltModifier) {    // Alt-drag leaves the original and moves a copy
                        gesture = "copy"; ed.draftIndex = -1
                    } else {
                        gesture = "move"; ed.draftIndex = i
                    }
                    ed.draft = Ed.copy(pressObj)
                    return
                }
                ed.selected = -1
                ed.draftIndex = -1
                const c = ed.ink
                switch (ed.tool) {
                case "arrow":
                    ed.draft = { kind: "arrow", x1: p.x, y1: p.y, x2: p.x, y2: p.y, cx: p.x, cy: p.y, color: c, size: ed.sizeFor("arrow"),
                                 style: ed.arrowStyle }
                    gesture = "draw"; break
                case "rect": case "frect":
                    ed.draft = { kind: "rect", x: p.x, y: p.y, w: 0, h: 0, color: c, size: ed.sizeFor("rect"), filled: ed.tool === "frect" }
                    gesture = "draw"; break
                case "highlight": {
                    const sh = Ed.snapHighlight(ed.textLines, p.x, p.y, p.x, 4)
                    const hh = ed.sizeFor("highlight")
                    ed.draft = Object.assign({ kind: "highlight", color: ed.highlightInk, opacity: ed.highlightOpacity, snapped: !!sh },
                                             sh || { x: p.x, y: p.y - hh / 2, w: 1, h: hh })
                    gesture = "highlight"; break
                }
                case "pencil":
                    ed.draft = { kind: "pencil", pts: [[p.x, p.y]], color: c, size: ed.sizeFor("pencil") }
                    gesture = "draw"; break
                case "spotlight":
                    ed.draft = { kind: "spotlight", x: p.x, y: p.y, w: 0, h: 0, shape: ed.spotShape, dim: ed.spotDim }
                    gesture = "draw"; break
                case "ellipse":
                    ed.draft = { kind: "ellipse", x: p.x, y: p.y, w: 0, h: 0, color: c, size: ed.sizeFor("ellipse") }
                    gesture = "draw"; break
                case "line":
                    ed.draft = { kind: "line", x1: p.x, y1: p.y, x2: p.x, y2: p.y, color: c, size: ed.sizeFor("line") }
                    gesture = "draw"; break
                case "redact":
                    ed.draft = { kind: "redact", x: Math.round(p.x), y: Math.round(p.y), w: 0, h: 0, style: ed.redactStyle,
                                 seed: Math.floor(Math.random() * 1e9) }
                    gesture = "draw"; break
                case "counter":
                    ed.draft = { kind: "counter", x: p.x, y: p.y, color: c, size: ed.sizeFor("counter"), style: ed.counterStyle }
                    pressObj = ed.draft
                    gesture = "place"; break
                case "text":
                    ed.startText(p, -1); break
                }
            }
            onPositionChanged: (m) => {
                if (gesture === "") {
                    hoverHandle = handleAt(m.x, m.y)
                    hoverItem = hoverHandle === "" ? itemAt(ed.toImage(m.x, m.y)) : -1
                    return
                }
                if (gesture === "pan") {
                    ed.panX = pressPanX + m.x - pressVX; ed.panY = pressPanY + m.y - pressVY
                    ed.clampPan()
                    return
                }
                const p = ed.toImage(m.x, m.y)
                const shift = (m.modifiers & Qt.ShiftModifier) !== 0
                if (gesture.startsWith("crop")) {
                    let r
                    if (gesture === "crop-handle") r = Ed.withHandle(pressObj, handle, p.x, p.y)
                    else if (gesture === "crop-move") {
                        const dx = Math.max(-pressObj.x, Math.min(ed.imgW - pressObj.x - pressObj.w, p.x - pressAt.x))
                        const dy = Math.max(-pressObj.y, Math.min(ed.imgH - pressObj.y - pressObj.h, p.y - pressAt.y))
                        r = Ed.moved(pressObj, dx, dy)
                    } else r = Ed.normRect(pressAt.x, pressAt.y, p.x, p.y)
                    ed.cropDraft = Ed.clampRect(r, ed.imgW, ed.imgH)
                    return
                }
                if (gesture === "move" || gesture === "copy" || gesture === "place") {
                    let dx = p.x - pressAt.x, dy = p.y - pressAt.y
                    if (shift && gesture !== "place") { if (Math.abs(dx) > Math.abs(dy)) dy = 0; else dx = 0 }   // Shift locks an axis
                    ed.draft = Ed.moved(pressObj, dx, dy)
                } else if (gesture === "handle") {
                    ed.draft = Ed.withHandle(pressObj, handle, p.x, p.y, pressBox)
                } else if (gesture === "highlight") {
                    const d = ed.draft, hh = ed.sizeFor("highlight")
                    const sh = d.snapped ? Ed.snapHighlight(ed.textLines, pressAt.x, pressAt.y, p.x, 4) : null
                    ed.draft = Object.assign(Ed.copy(d), sh || { x: Math.min(pressAt.x, p.x), y: pressAt.y - hh / 2, w: Math.abs(p.x - pressAt.x), h: hh })
                } else if (gesture === "draw" && ed.draft.kind === "pencil") {
                    const last = ed.draft.pts[ed.draft.pts.length - 1]
                    if (Math.hypot(p.x - last[0], p.y - last[1]) * ed.k >= 2)       // a point every 2 view px
                        ed.draft = Object.assign(Ed.copy(ed.draft), { pts: ed.draft.pts.concat([[p.x, p.y]]) })
                } else if (gesture === "draw") {
                    const d = Ed.copy(ed.draft)
                    let x = p.x, y = p.y
                    if (d.kind === "arrow" || d.kind === "line") {
                        if (shift) {   // 45° steps
                            const a = Math.round(Math.atan2(y - pressAt.y, x - pressAt.x) / (Math.PI / 4)) * (Math.PI / 4)
                            const r = Math.hypot(x - pressAt.x, y - pressAt.y)
                            x = pressAt.x + Math.cos(a) * r; y = pressAt.y + Math.sin(a) * r
                        }
                        d.x2 = x; d.y2 = y
                        if (d.kind === "arrow") {
                            d.cx = (d.x1 + x) / 2; d.cy = (d.y1 + y) / 2
                            if (d.style === "curved") d = Ed.withArrowStyle(d, "curved")
                        }
                    } else {
                        if (shift) {   // square
                            const s = Math.max(Math.abs(x - pressAt.x), Math.abs(y - pressAt.y))
                            x = pressAt.x + Math.sign(x - pressAt.x || 1) * s; y = pressAt.y + Math.sign(y - pressAt.y || 1) * s
                        }
                        const r = Ed.normRect(pressAt.x, pressAt.y, x, y)
                        d.x = r.x; d.y = r.y; d.w = r.w; d.h = r.h
                    }
                    ed.draft = d.kind === "redact" ? Ed.whole(d) : d
                }
            }
            onReleased: (m) => {
                const g = gesture
                gesture = ""
                if (g === "pan" || g === "") return
                if (g.startsWith("crop")) {
                    const r = ed.cropDraft
                    ed.cropDraft = null
                    if (r) ed.setCrop(r)
                    return
                }
                const d = ed.draft, i = ed.draftIndex
                ed.draft = null
                ed.draftIndex = -1
                if (!d) return
                if (g === "highlight") {
                    delete d.snapped
                    if (d.w >= 3) ed.addItem(d)
                } else if (g === "draw") {
                    // Alt while drawing points the arrow the other way (CleanShot)
                    if (d.kind === "arrow" && (m.modifiers & Qt.AltModifier))
                        Object.assign(d, { x1: d.x2, y1: d.y2, x2: d.x1, y2: d.y1 })
                    if (d.kind === "pencil") d.pts = Ed.simplify(d.pts, 0.75 / ed.k)
                    if (!Ed.tooSmall(d)) ed.addItem(d)
                } else if (g === "place") {
                    ed.addItem(d)
                } else if (g === "copy") {
                    if (JSON.stringify(d) !== JSON.stringify(pressObj)) ed.addItem(d)
                } else if (i >= 0 && JSON.stringify(d) !== JSON.stringify(ed.doc.items[i])) {
                    ed.replaceItem(i, d)
                }
            }
            onDoubleClicked: (m) => {
                const i = itemAt(ed.toImage(m.x, m.y))
                if (i >= 0 && ed.doc.items[i].kind === "text") ed.startText(null, i)
            }
            onWheel: (w) => {
                if (w.modifiers & Qt.ControlModifier) {
                    ed.zoomTo(ed.zoom * Math.pow(1.0015, w.angleDelta.y), w.x, w.y)
                } else {
                    const dx = w.pixelDelta.x || w.angleDelta.x / 2, dy = w.pixelDelta.y || w.angleDelta.y / 2
                    ed.panX += (w.modifiers & Qt.ShiftModifier) ? dy : dx
                    ed.panY += (w.modifiers & Qt.ShiftModifier) ? 0 : dy
                    ed.clampPan()
                }
            }
        }
    }

    // ---------------------------------------------------------------- bottom bar
    Rectangle {
        id: bottomBar
        z: 2
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: 50
        color: Theme.surface
        Rectangle { anchors { left: parent.left; right: parent.right; top: parent.top } height: 1; color: Theme.hairline }

        Capsule {
            anchors { left: parent.left; leftMargin: 12; verticalCenter: parent.verticalCenter }
            ToolButton { id: zoomButton; label: Math.round(ed.zoom * 100) + "%"; chevron: true; tip: "Zoom (Ctrl+scroll)"; tipAbove: true
                         onClicked: ed.openMenu(zoomMenu, zoomButton) }
        }

        // drag the result out (QAO-04 style): edits are written back first
        Rectangle {
            id: dragMe
            anchors.centerIn: parent
            width: dragRow.implicitWidth + 36; height: 34
            radius: height / 2
            color: dragMa.containsMouse ? Qt.lighter(Theme.light ? "#ffffff" : Theme.surface, 1.15) : (Theme.light ? Qt.rgba(1, 1, 1, 0.9) : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.07))
            border.width: 1; border.color: Theme.hairline
            Row {
                id: dragRow
                anchors.centerIn: parent
                spacing: 8
                Icon { name: "grip"; size: 14; stroke: 2; color: Theme.textSecondary; anchors.verticalCenter: parent.verticalCenter }
                Text { text: "Drag Me"; color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 13; font.weight: Font.Medium
                       anchors.verticalCenter: parent.verticalCenter }
                Icon { name: "grip"; size: 14; stroke: 2; color: Theme.textSecondary; anchors.verticalCenter: parent.verticalCenter }
            }
            MouseArea {
                id: dragMa
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.OpenHandCursor
                onContainsMouseChanged: if (containsMouse) ed.flush("")
                property real px: 0
                property real py: 0
                onPressed: (m) => { px = m.x; py = m.y }
                onPositionChanged: (m) => {
                    if (!pressed || Math.hypot(m.x - px, m.y - py) < 6) return
                    const path = Engine.dragFile(ed.captureId)
                    if (path) Platform.startDrag(dragMe, path, path)
                }
            }
        }

        Row {
            anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
            spacing: 8
            IconButton { icon: "copy"; tip: "Copy (Ctrl+C)"; tipAbove: true; width: 32; height: 32; radius: 16; onClicked: ed.flush("copy") }
        }
    }

    // ---------------------------------------------------------------- dropdowns
    function openMenuNamed(name, anchorItem) {
        openMenu({ color: colorMenu, size: sizeMenu, style: styleMenu, zoom: zoomMenu, arrow: arrowMenu, text: textMenu, dim: dimMenu,
                   counter: counterMenu, opacity: opacityMenu }[name], anchorItem)
    }
    property Item openedMenu: null
    function openMenu(menu, anchorItem) {
        if (openedMenu === menu) { openedMenu = null; return }
        const p = anchorItem.mapToItem(null, 0, 0)
        menu.x = Math.round(Math.min(width - menu.width - 8, Math.max(8, p.x + anchorItem.width / 2 - menu.width / 2)))
        menu.y = anchorItem.mapToItem(null, 0, 0).y > height / 2 ? Math.round(p.y - menu.height - 8) : Math.round(p.y + anchorItem.height + 8)
        openedMenu = menu
    }
    MouseArea {
        z: 9
        anchors.fill: parent
        visible: ed.openedMenu !== null
        onPressed: ed.openedMenu = null
    }

    component Menu: Rectangle {
        id: menu
        default property alias content: menuCol.data
        z: 10
        visible: opacity > 0
        opacity: ed.openedMenu === menu ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 110; easing.type: Easing.OutCubic } }
        width: menuCol.implicitWidth + 12
        height: menuCol.implicitHeight + 12
        radius: Math.min(18, width / 2)
        color: Theme.light ? "#ffffff" : Qt.lighter(Theme.surface, 1.2)
        border.width: 1; border.color: Theme.hairline
        layer.enabled: true
        layer.effect: MultiEffect { shadowEnabled: true; shadowBlur: 0.6; shadowOpacity: 0.3; shadowVerticalOffset: 4 }
        Column { id: menuCol; anchors.centerIn: parent; spacing: 4 }
    }

    component Swatch: Item {
        required property string modelData
        width: 30; height: 30
        Rectangle {
            anchors.centerIn: parent
            width: 30; height: 30; radius: 15
            color: "transparent"
            border.width: 2
            border.color: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.6)
            visible: Qt.colorEqual(ed.toolInk, parent.modelData)
        }
        Rectangle {
            anchors.centerIn: parent
            width: 22; height: 22; radius: 11
            color: parent.modelData
            border.width: 1; border.color: Qt.rgba(0, 0, 0, 0.15)
            scale: swMa.containsMouse ? 1.12 : 1
            Behavior on scale { NumberAnimation { duration: 90 } }
        }
        MouseArea { id: swMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                    onClicked: { ed.setColor(parent.modelData); ed.openedMenu = null } }
    }

    // My Colors (ANN-13): up to ten, kept across sessions
    property var savedColors: { try { return JSON.parse(Engine.savedColors() || "[]") } catch (e) { return [] } }
    function saveColor(c) {
        const list = [c].concat(savedColors.filter(x => x.toLowerCase() !== c.toLowerCase())).slice(0, 10)
        savedColors = list
        Engine.setSavedColors(JSON.stringify(list))
    }
    property bool customOpen: false
    // the eyedropper: the next click on the image picks its colour
    property bool picking: false
    function pickAt(ix, iy) {
        picking = false
        const c = Engine.pixelColor(capture.image || "", Math.floor(ix), Math.floor(iy))
        if (c) setColor(c)
    }

    Menu {
        id: colorMenu
        onVisibleChanged: if (!visible) ed.customOpen = false
        Row {
            spacing: 10
            Column {
                spacing: 4
                Repeater { model: Ed.palette; delegate: Swatch {} }
                Rectangle { visible: ed.savedColors.length > 0; width: 30; height: 1; color: Theme.hairline }
                Repeater { model: ed.savedColors; delegate: Swatch {} }
                Rectangle { width: 30; height: 1; color: Theme.hairline }
                // the rainbow: the custom panel
                Item {
                    width: 30; height: 30
                    Shape {
                        anchors.centerIn: parent
                        width: 22; height: 22
                        scale: rbMa.containsMouse || ed.customOpen ? 1.12 : 1
                        preferredRendererType: Shape.CurveRenderer
                        ShapePath {
                            strokeColor: "transparent"
                            fillGradient: ConicalGradient {
                                centerX: 11; centerY: 11
                                GradientStop { position: 0.0; color: "#ff0000" }
                                GradientStop { position: 0.17; color: "#ff00ff" }
                                GradientStop { position: 0.33; color: "#0000ff" }
                                GradientStop { position: 0.5; color: "#00ffff" }
                                GradientStop { position: 0.67; color: "#00ff00" }
                                GradientStop { position: 0.83; color: "#ffff00" }
                                GradientStop { position: 1.0; color: "#ff0000" }
                            }
                            PathAngleArc { centerX: 11; centerY: 11; radiusX: 11; radiusY: 11; sweepAngle: 360 }
                        }
                    }
                    MouseArea { id: rbMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                                onClicked: ed.customOpen = !ed.customOpen }
                }
            }
            ColorPanel {
                id: colorPanel
                visible: ed.customOpen
                value: ed.toolInk
                onPicked: (c) => ed.setColor(c)
                onSave: ed.saveColor(current)
                onEyedropper: { ed.picking = true; ed.openedMenu = null }
            }
        }
    }

    Menu {
        id: sizeMenu
        Repeater {
            model: Ed.sizes[ed.sizeKind] || []
            delegate: Rectangle {
                required property int index
                required property real modelData
                width: 64; height: 30; radius: 15
                color: ed.sizeIndex[ed.sizeKind] === index ? Theme.controlPressed : szMa.containsMouse ? Theme.controlHover : "transparent"
                Rectangle {
                    visible: ed.sizeKind !== "text"
                    anchors.centerIn: parent
                    width: ed.sizeKind === "counter" ? parent.modelData * 0.6 : 34
                    height: ed.sizeKind === "counter" ? width : Math.max(1.5, parent.modelData * 0.9)
                    radius: height / 2
                    color: Theme.text
                }
                Text { visible: ed.sizeKind === "text"; anchors.centerIn: parent; text: parent.modelData + " pt"; color: Theme.text
                       font.family: Theme.fontUi; font.pixelSize: 13 }
                MouseArea { id: szMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: { ed.setSize(parent.index); ed.openedMenu = null } }
            }
        }
    }

    Menu {
        id: styleMenu
        Repeater {
            model: Ed.redactStyles
            delegate: Rectangle {
                required property var modelData
                readonly property bool current: (ed.selectedObj && ed.selectedObj.kind === "redact" ? ed.selectedObj.style : ed.redactStyle) === modelData.id
                width: 112; height: 30; radius: 15
                color: current ? Theme.controlPressed : stMa.containsMouse ? Theme.controlHover : "transparent"
                Text { anchors.centerIn: parent; text: parent.modelData.label; color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 13 }
                MouseArea { id: stMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: { ed.setRedactStyle(parent.modelData.id); ed.openedMenu = null } }
            }
        }
    }

    Menu {
        id: opacityMenu
        Repeater {
            model: Ed.highlightOpacities
            delegate: Rectangle {
                required property real modelData
                readonly property real current: ed.selectedObj && ed.selectedObj.kind === "highlight" ? ed.selectedObj.opacity : ed.highlightOpacity
                width: 92; height: 30; radius: 8
                color: Math.abs(current - modelData) < 0.01 ? Theme.controlPressed : opMa.containsMouse ? Theme.controlHover : "transparent"
                Rectangle { x: 10; anchors.verticalCenter: parent.verticalCenter; width: 16; height: 16; radius: 4
                            color: Qt.rgba(Qt.color(ed.highlightInk).r, Qt.color(ed.highlightInk).g, Qt.color(ed.highlightInk).b, parent.modelData) }
                Text { x: 36; anchors.verticalCenter: parent.verticalCenter; text: Math.round(parent.modelData * 100) + "%"; color: Theme.text
                       font.family: Theme.fontUi; font.pixelSize: 13 }
                MouseArea { id: opMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: { ed.setHighlightOpacity(parent.modelData); ed.openedMenu = null } }
            }
        }
    }

    Menu {
        id: counterMenu
        Repeater {
            model: Ed.counterStyles
            delegate: Rectangle {
                required property var modelData
                readonly property string current: ed.selectedObj && ed.selectedObj.kind === "counter" ? (ed.selectedObj.style || "num") : ed.counterStyle
                width: 150; height: 30; radius: 8
                color: current === modelData.id ? Theme.controlPressed : csMa.containsMouse ? Theme.controlHover : "transparent"
                Text { x: 14; anchors.verticalCenter: parent.verticalCenter; text: parent.modelData.label; color: Theme.text
                       font.family: Theme.fontUi; font.pixelSize: 13 }
                MouseArea { id: csMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: { ed.setCounterStyle(parent.modelData.id); ed.openedMenu = null } }
            }
        }
        Rectangle { width: 150; height: 1; color: Theme.hairline }
        Item {
            // where numbering starts (0 or higher)
            width: 150; height: 32
            readonly property int start: ed.doc.counterStart === undefined ? 1 : ed.doc.counterStart
            Text { x: 14; anchors.verticalCenter: parent.verticalCenter; text: "Start at"; color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 13 }
            Row {
                anchors { right: parent.right; rightMargin: 6; verticalCenter: parent.verticalCenter }
                spacing: 2
                ToolButton { label: "−"; implicitWidth: 24; onClicked: ed.setCounterStart(parent.parent.start - 1) }
                Text { width: 22; horizontalAlignment: Text.AlignHCenter; anchors.verticalCenter: parent.verticalCenter
                       text: parent.parent.start; color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 13; font.features: { "tnum": 1 } }
                ToolButton { label: "+"; implicitWidth: 24; onClicked: ed.setCounterStart(parent.parent.start + 1) }
            }
        }
    }

    Menu {
        id: dimMenu
        Repeater {
            model: Ed.spotlightDims
            delegate: Rectangle {
                required property real modelData
                readonly property real current: ed.selectedObj && ed.selectedObj.kind === "spotlight" ? ed.selectedObj.dim : ed.spotDim
                width: 92; height: 30; radius: 8
                color: Math.abs(current - modelData) < 0.01 ? Theme.controlPressed : dmMa.containsMouse ? Theme.controlHover : "transparent"
                Rectangle { x: 10; anchors.verticalCenter: parent.verticalCenter; width: 16; height: 16; radius: 4
                            color: Qt.rgba(0, 0, 0, parent.modelData); border.width: 1; border.color: Theme.hairline }
                Text { x: 36; anchors.verticalCenter: parent.verticalCenter; text: Math.round(parent.modelData * 100) + "%"; color: Theme.text
                       font.family: Theme.fontUi; font.pixelSize: 13 }
                MouseArea { id: dmMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: { ed.setSpot(ed.spotShape, parent.modelData); ed.openedMenu = null } }
            }
        }
    }

    Menu {
        id: textMenu
        Repeater {
            model: Ed.textStyles
            delegate: Rectangle {
                required property var modelData
                readonly property bool current: (ed.selectedObj && ed.selectedObj.kind === "text" ? (ed.selectedObj.style || "standard")
                                                 : ed.editingText && ed.textObj ? (ed.textObj.style || "standard") : ed.textStyle) === modelData.id
                width: 168; height: 34; radius: 8
                color: current ? Theme.controlPressed : txMa.containsMouse ? Theme.controlHover : "transparent"
                EditorItem {
                    o: ({ kind: "text", x: 14, y: 6, text: "Aa", color: ed.ink, size: 16, style: parent.modelData.id })
                }
                Text { x: 64; anchors.verticalCenter: parent.verticalCenter; text: parent.modelData.label; color: Theme.text
                       font.family: Theme.fontUi; font.pixelSize: 13 }
                MouseArea { id: txMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: { ed.setTextStyle(parent.modelData.id); ed.openedMenu = null } }
            }
        }
    }

    Menu {
        id: arrowMenu
        Repeater {
            model: Ed.arrowStyles
            delegate: Rectangle {
                required property var modelData
                readonly property bool current: (ed.selectedObj && ed.selectedObj.kind === "arrow" ? (ed.selectedObj.style || "standard") : ed.arrowStyle) === modelData.id
                width: 132; height: 32; radius: 8
                color: current ? Theme.controlPressed : arMa.containsMouse ? Theme.controlHover : "transparent"
                EditorItem {
                    x: 10; y: 4
                    o: Ed.withArrowStyle({ kind: "arrow", x1: 2, y1: 21, x2: 34, y2: 4, cx: 18, cy: 12.5, color: Theme.text.toString(), size: 2.5 }, parent.modelData.id)
                }
                Text { x: 54; anchors.verticalCenter: parent.verticalCenter; text: parent.modelData.label; color: Theme.text
                       font.family: Theme.fontUi; font.pixelSize: 13 }
                MouseArea { id: arMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: { ed.setArrowStyle(parent.modelData.id); ed.openedMenu = null } }
            }
        }
    }

    Menu {
        id: zoomMenu
        Repeater {
            model: [{ t: "Fit", z: 0 }, { t: "50%", z: 0.5 }, { t: "100%", z: 1 }, { t: "200%", z: 2 }]
            delegate: Rectangle {
                required property var modelData
                width: 76; height: 28; radius: 14
                color: zmMa.containsMouse ? Theme.controlHover : "transparent"
                Text { anchors.centerIn: parent; text: parent.modelData.t; color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 13 }
                MouseArea { id: zmMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: { parent.modelData.z ? ed.zoomTo(parent.modelData.z) : ed.zoomFit(); ed.openedMenu = null } }
            }
        }
    }

    // ---------------------------------------------------------------- test hooks
    // snip debug ui editor <op> ...: drawing without the pointer, and state
    function debug(a) {
        const n = (i) => Number(a[i])
        switch (a[0]) {
        case "arrow": addItem(Ed.withArrowStyle({ kind: "arrow", x1: n(1), y1: n(2), x2: n(3), y2: n(4), cx: (n(1) + n(3)) / 2, cy: (n(2) + n(4)) / 2,
                                                  color: ink, size: sizeFor("arrow") }, a[5] || arrowStyle)); break
        case "arrow-style": setArrowStyle(a[1]); break
        case "rect": addItem({ kind: "rect", x: n(1), y: n(2), w: n(3), h: n(4), color: ink, size: sizeFor("rect") }); break
        case "frect": addItem({ kind: "rect", x: n(1), y: n(2), w: n(3), h: n(4), color: ink, size: sizeFor("rect"), filled: true }); break
        case "ellipse": addItem({ kind: "ellipse", x: n(1), y: n(2), w: n(3), h: n(4), color: ink, size: sizeFor("ellipse") }); break
        case "line": addItem({ kind: "line", x1: n(1), y1: n(2), x2: n(3), y2: n(4), color: ink, size: sizeFor("line") }); break
        case "counter": addItem({ kind: "counter", x: n(1), y: n(2), color: ink, size: sizeFor("counter"), style: a[3] || counterStyle }); break
        case "counter-style": setCounterStyle(a[1]); break
        case "words": Engine.requestWords(capture.image || ""); break
        case "eyedropper": picking = true; break
        case "save-color": saveColor(a[1]); break
        case "counter-start": setCounterStart(n(1)); break
        case "text": addItem({ kind: "text", x: n(1), y: n(2), text: a.slice(3).join(" "), color: ink, size: sizeFor("text"), style: textStyle }); break
        case "text-style": setTextStyle(a[1]); break
        case "spotlight": addItem({ kind: "spotlight", x: n(1), y: n(2), w: n(3), h: n(4), shape: a[5] || spotShape, dim: a[6] ? n(6) : spotDim }); break
        case "text-width": { const o = Ed.copy(selectedObj); o.w = n(1); replaceItem(selected, o); break }
        case "redact": addItem({ kind: "redact", x: n(1), y: n(2), w: n(3), h: n(4), style: a[5] || redactStyle, seed: a[6] ? n(6) : 1 }); break
        case "crop": a[1] === "reset" ? setCrop(full) : setCrop({ x: n(1), y: n(2), w: n(3), h: n(4) }); break
        case "color": setColor(a[1]); break
        case "tool": setTool(a[1]); break
        case "undo": undo(); break
        case "redo": redo(); break
        case "delete": selected = n(1); removeSelected(); break
        case "zoom": a[1] === "fit" ? zoomFit() : zoomTo(n(1)); break
        case "flush": flush(a[1] || ""); break
        case "keys": Platform.sendKeys(ed, a.slice(1).join(" ")); break
        case "close": closeEditor(); break
        case "save-as": saveAsDialog(); break
        case "cancel-dialogs": Platform.closeFileDialogs(); break
        case "state":
            Engine.log("editor_state", JSON.stringify({
                id: captureId, items: doc.items, crop: doc.crop, selected: selected, tool: tool, zoom: zoom, dpr: dpr,
                revision: revision, saved: savedRevision, undo: undoStack.length, redo: redoStack.length,
                // where image px (0,0) is, in window-logical px, and logical px per image px
                originX: originX, originY: originY + view.y, k: k, winW: width, winH: height,
                ready: base.status === Image.Ready, editingText: editingText, typed: textEdit.text, active: ed.active,
                selectionBox: { x: selectionBox.x, y: selectionBox.y, w: selectionBox.width, h: selectionBox.height },
                words: words ? words.length : -1, ink: ink, savedColors: savedColors, picking: picking,
                counterLabels: doc.items.map((o, i) => o.kind === "counter" ? Ed.counterLabel(doc, Ed.counterNumber(doc.items, i), o.style) : null).filter(l => l !== null)
            }).slice(1, -1))
            break
        }
    }
}
