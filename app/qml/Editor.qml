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
    // the image being drawn on: the capture's, or one rotated, flipped or
    // resized from it (doc.base, a file beside it; undo goes back with the doc)
    readonly property int imgW: doc.w || capture.width || 1
    readonly property int imgH: doc.h || capture.height || 1
    readonly property string basePath: doc.base ? String(capture.image || "").replace(/[^/]*$/, doc.base) : (capture.image || "")
    onBasePathChanged: words = null
    // Rotate / Flip / Resize the whole document, objects included (ANN-11)
    function transformImage(op, nw, nh) {
        const r = JSON.parse(Engine.editorTransform(basePath, op, nw || 0, nh || 0) || "null")
        if (!r) { toast("Couldn't change the image"); return }
        const next = Ed.transformDoc(doc, op, imgW, imgH, r.width, r.height)
        next.base = r.file; next.w = r.width; next.h = r.height
        if (next.fill === undefined) delete next.fill
        selected = -1
        commit(next)
    }
    readonly property real dpr: devicePixelRatio > 0 ? devicePixelRatio : 1
    // image px per logical px: sizes look the same on any scale
    readonly property real unit: dpr
    // Settings › Annotate (config [annotate])
    readonly property var annotateCfg: (controller && controller.settings && controller.settings.annotate) || ({})

    title: "Snip Editor — " + (capture.title || "Capture")
    color: Theme.background
    minimumWidth: Math.max(640, toolbar.needed)
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
    function setItems(items) { commit(expanded(Object.assign({}, doc, { items: items }))) }
    // "Auto-expand canvas" (ANN-17): a new or changed object past the canvas
    // grows it in the same undo step, filled like an outward crop
    function expanded(next) {
        if (!annotateCfg.auto_expand) return next
        const c = next.crop || full
        let x0 = c.x, y0 = c.y, x1 = c.x + c.w, y1 = c.y + c.h
        next.items.forEach((o, i) => {
            if (o.kind === "redact" || o.kind === "spotlight" || JSON.stringify(o) === JSON.stringify(doc.items[i])) return
            measure.o = o
            const e = Ed.extent(o, measure.box), m = Math.round(Math.max(imgW, imgH) * 0.02)
            if (e.x < x0) x0 = e.x - m
            if (e.y < y0) y0 = e.y - m
            if (e.x + e.w > x1) x1 = e.x + e.w + m
            if (e.y + e.h > y1) y1 = e.y + e.h + m
        })
        measure.o = null
        if (x0 === c.x && y0 === c.y && x1 === c.x + c.w && y1 === c.y + c.h) return next
        const r = clampCrop({ x: Math.floor(x0), y: Math.floor(y0), w: Math.ceil(x1) - Math.floor(x0), h: Math.ceil(y1) - Math.floor(y0) })
        return Object.assign({}, next, { crop: r, fill: next.fill || edgeFill || Engine.edgeColor(basePath) })
    }
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
    property real redactLevel: 1
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
    onToolChanged: {
        if (tool === "highlight" && words === null) { words = []; Engine.requestWords(basePath) }
        if (tool === "crop" && !edgeFill) edgeFill = Engine.edgeColor(basePath)
    }
    Connections {
        target: Engine
        function onEditorWords(path, json) { if (path === ed.basePath) ed.words = JSON.parse(json) }
    }
    property string spotShape: "rect"
    property real spotDim: 0.55
    property string toolBeforeCrop: "arrow"

    function setTool(t) {
        finishText()
        if (t === "crop" && tool !== "crop") { toolBeforeCrop = tool; cropStart = { crop: doc.crop, fill: doc.fill } }
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
    function setRedactLevel(v) {
        redactLevel = v
        if (selectedObj && selectedObj.kind === "redact" && (selectedObj.level || 1) !== v)
            replaceItem(selected, Object.assign(Ed.copy(selectedObj), { level: v }))
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
    // the image or its crop, and around it the Background tool's backdrop (BG-01…06)
    readonly property var innerRect: doc.crop || full
    property var bgPreview: null              // a slider being dragged: shown, not yet in the document
    readonly property var bgLive: doc.bg ? Object.assign({}, doc.bg, bgPreview || {}) : null
    readonly property bool framed: !!bgLive && tool !== "crop"
    readonly property var exportRect: bgLive ? Ed.bgRect(bgLive, balancedRect) : innerRect
    // Auto-balance (BG-02): the plain border around the content, evened out
    // to its widest side in the border's own colour
    readonly property var balance: bgLive && bgLive.balance && basePath
        ? JSON.parse(Engine.contentMargins(basePath, innerRect.x, innerRect.y, innerRect.w, innerRect.h)) : null
    readonly property var balancedRect: {
        const r = innerRect, b = balance
        if (!b) return r
        const m = Math.max(b.l, b.t, b.r, b.b)
        return { x: r.x - (m - b.l), y: r.y - (m - b.t), w: r.w + 2 * m - b.l - b.r, h: r.h + 2 * m - b.t - b.b }
    }
    function setBg(patch) {
        bgPreview = null
        const next = Object.assign({}, doc)
        if (patch === null) delete next.bg
        else next.bg = Object.assign({}, doc.bg || Ed.defaultBackground(unit), patch)
        if (JSON.stringify(next) === JSON.stringify(doc)) return
        commit(next)
        if (next.bg) storeBg({ previous: bgPoints(next.bg) })
    }

    // Presets and "Apply Previous Settings" (BG-03), shared by every editor
    // through backgrounds.json; sizes are kept in points to suit any scale
    property var bgStore: JSON.parse(Engine.backgrounds() || "{}")
    readonly property var bgPresets: bgStore.presets || []
    property string bgPreset: ""                    // the preset applied last here
    readonly property bool bgPresetChanged: {
        const p = bgPresets.find(p => p.name === bgPreset)
        return !!p && JSON.stringify(doc.bg ? bgPoints(doc.bg) : null) !== JSON.stringify(p.bg)
    }
    function bgScaled(bg, f) {
        const o = Object.assign({}, bg)
        for (const key of ["padding", "inset", "radius"]) if (o[key] !== undefined) o[key] = Math.round(o[key] * f * 100) / 100
        return o
    }
    function bgPoints(bg) { return bgScaled(bg, 1 / unit) }
    function bgPixels(bg) { const o = bgScaled(bg, unit); for (const key of ["padding", "inset", "radius"]) if (o[key] !== undefined) o[key] = Math.round(o[key]); return o }
    function storeBg(patch) {
        // read again first: another editor may have changed it
        bgStore = Object.assign({}, JSON.parse(Engine.backgrounds() || "{}"), patch)
        Engine.setBackgrounds(JSON.stringify(bgStore))
    }
    function applyBg(points) { setBg(Object.assign(Ed.defaultBackground(unit), bgPixels(points))) }
    function applyPreviousBg() { if (bgStore.previous) { applyBg(bgStore.previous); bgPreset = "" } }
    function applyBgPreset(name) {
        const p = bgPresets.find(p => p.name === name)
        if (p) { applyBg(p.bg); bgPreset = name }
    }
    function saveBgPreset(name) {
        name = name.trim()
        if (!name) return
        if (!doc.bg) setBg({})
        const kept = JSON.parse(Engine.backgrounds() || "{}").presets || []
        const i = kept.findIndex(p => p.name === name), entry = { name: name, bg: bgPoints(doc.bg) }
        storeBg({ presets: i >= 0 ? kept.map((p, j) => j === i ? entry : p) : kept.concat([entry]) })
        bgPreset = name
    }
    function deleteBgPreset(name) {
        storeBg({ presets: (JSON.parse(Engine.backgrounds() || "{}").presets || []).filter(p => p.name !== name) })
        if (bgPreset === name) bgPreset = ""
    }
    function previewBg(patch) {
        if (!doc.bg) setBg({})
        bgPreview = Object.assign({}, bgPreview || {}, patch)
    }
    // cropping shows room around the image, to crop outward into
    readonly property var cropView: {
        const c = doc.crop || full, m = Math.max(imgW, imgH) * 0.12
        const x0 = Math.min(0, c.x) - m, y0 = Math.min(0, c.y) - m
        return { x: x0, y: y0, w: Math.max(imgW, c.x + c.w) + m - x0, h: Math.max(imgH, c.y + c.h) + m - y0 }
    }
    readonly property var shown: tool === "crop" ? cropView : framed ? exportRect : innerRect
    property var cropDraft: null
    readonly property var cropShown: cropDraft || doc.crop || full
    // an outward crop grows the canvas, up to three times the image (ANN-11)
    function clampCrop(r) { return Ed.clampTo(r, -imgW, -imgH, 2 * imgW, 2 * imgH) }
    property real cropRatio: 0                       // 0 free, -1 the image's own, else width / height
    readonly property real ratioValue: cropRatio === -1 ? imgW / imgH : cropRatio
    function setCropRatio(v) {
        cropRatio = v
        if (ratioValue) setCrop(Ed.fitRatio(cropShown, ratioValue))
    }
    // the canvas beyond the image takes the colour around its edge
    property string edgeFill: ""
    function setCrop(r) {
        r = clampCrop(r)
        if (r.w < 4 || r.h < 4) return
        const c = r.x === 0 && r.y === 0 && r.w === imgW && r.h === imgH ? null : r
        const outward = c && (c.x < 0 || c.y < 0 || c.x + c.w > imgW || c.y + c.h > imgH)
        const next = Object.assign({}, doc, { crop: c })
        if (outward && !doc.fill) next.fill = edgeFill || Engine.edgeColor(basePath)
        if (JSON.stringify(next) !== JSON.stringify(doc)) commit(next)
    }
    function cropDone() { setTool(toolBeforeCrop === "crop" ? "arrow" : toolBeforeCrop) }
    // CleanShot's crop bar: Cancel goes back to the crop (and fill) it started with
    property var cropStart: null
    function cropCancel() {
        const was = cropStart || { crop: null, fill: undefined }
        if (JSON.stringify([doc.crop, doc.fill]) !== JSON.stringify([was.crop, was.fill])) {
            const next = Object.assign({}, doc, { crop: was.crop })
            if (was.fill === undefined) delete next.fill; else next.fill = was.fill
            commit(next)
        }
        cropDone()
    }
    // what the canvas shows beyond the image: the edge colour (automatic),
    // transparent, or any colour
    function setFill(c) { if (doc.fill !== c) commit(Object.assign({}, doc, { fill: c })) }
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
        if (d && d.items) doc = Object.assign({ v: 1, crop: null, counterStart: 1 }, d)
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

    // Omarchy's emoji list, read when the picker first opens
    property var emojiList: []
    function openEmoji() {
        if (!emojiList.length) emojiList = JSON.parse(Engine.emojis())
        emojiSearch.text = ""
        openMenu(emojiMenu, emojiButton)
        emojiSearch.forceActiveFocus()
    }
    function insertEmoji(e) {
        openedMenu = null
        textEdit.insert(textEdit.cursorPosition, e)
        textEdit.forceActiveFocus()
    }

    function focusKeys() { keys.forceActiveFocus() }

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
                // objects copied in an editor first, else the clipboard's image
                case Qt.Key_V: (controller.objectClipboard || []).length ? ed.pasteObjects() : ed.pasteImage(); break
                case Qt.Key_I: shift ? ed.takeScreenshot() : ed.chooseImage(); break
                case Qt.Key_D: ed.duplicateSelected(); break
                case Qt.Key_P: ed.flush("pin"); break
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
                if (e.key === Qt.Key_Escape) ed.cropCancel(); else ed.cropDone()
                e.accepted = true
                return
            }
            switch (e.key) {
            case Qt.Key_Escape:
                if (ed.openedMenu) ed.openedMenu = null
                else if (ed.selected >= 0) ed.selected = -1
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
            case Qt.Key_B: ed.setTool("background"); break
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
    // Add image (ANN-12): a file, the clipboard's image or a dropped file,
    // copied beside the base image and placed at most half the canvas big
    property int imageToken: 0
    function addImageFile(file, at) {
        if (!file) return false
        const sz = Platform.imageSize(basePath.replace(/[^/]*$/, "") + file)
        if (!(sz.width > 0)) return false
        const r = exportRect, f = Math.min(1, r.w * 0.5 / sz.width, r.h * 0.5 / sz.height)
        const w = sz.width * f, h = sz.height * f, c = at || { x: r.x + r.w / 2, y: r.y + r.h / 2 }
        addItem({ kind: "image", x: c.x - w / 2, y: c.y - h / 2, w: w, h: h, src: file })
        return true
    }
    // Add image > Take Screenshot (ANN-12): the editor steps aside while you
    // select an area or a window, then the shot lands in the canvas
    property int shotToken: 0
    function takeScreenshot() {
        openedMenu = null
        shotToken = controller.nextToken()
        visible = false
        shotTimer.start()
    }
    Timer {
        id: shotTimer
        interval: 300           // Hyprland fades the window out first
        onTriggered: Engine.takeScreenshotForEditor(ed.shotToken)
    }
    Connections {
        target: Engine
        function onEditorScreenshot(token, path) {
            if (token !== ed.shotToken) return
            ed.shotToken = 0
            ed.visible = true
            ed.requestActivate()
            keys.forceActiveFocus()
            if (path) ed.addImageFile(Engine.editorImportImage(ed.basePath, path))
        }
    }
    // Drop zones (ANN-12, CleanShot's "combine images"): an image dropped
    // near an edge of the canvas joins it on that side, as wide (above,
    // below) or as tall (left, right) as the canvas, and the canvas grows to
    // hold it; dropped further in, it lands on top where it was dropped.
    function dropSide(vx, vy) {
        const a = toView(shown.x, shown.y), b = toView(shown.x + shown.w, shown.y + shown.h)
        const u = (vx - a.x) / Math.max(1, b.x - a.x), v = (vy - a.y) / Math.max(1, b.y - a.y)
        const d = { left: u, right: 1 - u, top: v, bottom: 1 - v }
        const side = Object.keys(d).reduce((m, k) => d[k] < d[m] ? k : m, "left")
        return d[side] < 0.25 ? side : ""
    }
    function appendImage(file, side) {
        if (!file) return false
        const sz = Platform.imageSize(basePath.replace(/[^/]*$/, "") + file)
        if (!(sz.width > 0)) return false
        const c = innerRect, across = side === "top" || side === "bottom"
        const w = across ? c.w : sz.width * c.h / sz.height, h = across ? sz.height * c.w / sz.width : c.h
        const x = side === "left" ? c.x - w : side === "right" ? c.x + c.w : c.x
        const y = side === "top" ? c.y - h : side === "bottom" ? c.y + c.h : c.y
        const x0 = Math.floor(Math.min(c.x, x)), y0 = Math.floor(Math.min(c.y, y))
        const crop = { x: x0, y: y0, w: Math.ceil(Math.max(c.x + c.w, x + w)) - x0, h: Math.ceil(Math.max(c.y + c.h, y + h)) - y0 }
        commit(Object.assign({}, doc, { items: doc.items.concat([{ kind: "image", x: x, y: y, w: w, h: h, src: file }]), crop: crop,
                                        fill: doc.fill || edgeFill || Engine.edgeColor(basePath) }))
        selected = doc.items.length - 1
        return true
    }
    function dropFiles(paths, vx, vy) {
        const side = dropSide(vx, vy)
        for (const path of paths) {
            const file = Engine.editorImportImage(basePath, path)
            if (side) appendImage(file, side)
            else addImageFile(file, toImage(vx, vy))
        }
    }

    function chooseImage() {
        imageToken = controller.nextToken()
        Platform.openImageDialog(imageToken, "Add Image")
    }
    function pasteImage() {
        if (!addImageFile(Engine.editorPasteImage(basePath))) toast("No image on the clipboard")
    }
    Connections {
        target: Platform
        function onFileChosen(token, path) {
            if (token === ed.imageToken && path !== "") ed.addImageFile(Engine.editorImportImage(ed.basePath, path))
        }
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
        anchors { left: parent.left; right: bgPanel.visible ? bgPanel.left : parent.right; top: toolbar.bottom; bottom: bottomBar.top }
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

            // a checkerboard under transparent areas (ANN-01); not exported
            Canvas {
                anchors.fill: parent
                onWidthChanged: requestPaint()
                onHeightChanged: requestPaint()
                onPaint: {
                    const g = getContext("2d")
                    for (let y = 0; y < height; y += 8)
                        for (let x = 0; x < width; x += 8) {
                            g.fillStyle = ((x + y) / 8) % 2 ? "#d8d8d8" : "#f4f4f4"
                            g.fillRect(x, y, 8, 8)
                        }
                }
            }

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
                    BackgroundLayer {
                        visible: ed.framed
                        bg: ed.bgLive
                        rect: ed.exportRect
                        inner: ed.balancedRect
                        frameColor: ed.balance ? ed.balance.color : ""
                        imagePath: ed.basePath
                    }
                    // the image (and a grown canvas), cut to rounded corners on a background;
                    // one texel per image px, so its pixels stay exact
                    Item {
                        id: imageFrame
                        readonly property var f: ({ x: Math.min(0, ed.innerRect.x), y: Math.min(0, ed.innerRect.y),
                                                    w: Math.max(ed.imgW, ed.innerRect.x + ed.innerRect.w) - Math.min(0, ed.innerRect.x),
                                                    h: Math.max(ed.imgH, ed.innerRect.y + ed.innerRect.h) - Math.min(0, ed.innerRect.y) })
                        x: f.x; y: f.y; width: f.w; height: f.h
                        layer.enabled: ed.framed
                        layer.textureSize: Qt.size(Math.ceil(f.w), Math.ceil(f.h))
                        layer.effect: MultiEffect { maskEnabled: true; maskSource: imageMask; maskThresholdMin: 0.5; maskSpreadAtMin: 1.0 }
                        Rectangle {
                            // the grown canvas (outward crop)
                            visible: !!(ed.doc.fill || (ed.tool === "crop" && ed.edgeFill))
                            x: -ed.imgW - imageFrame.f.x; y: -ed.imgH - imageFrame.f.y; width: 3 * ed.imgW; height: 3 * ed.imgH
                            color: ed.doc.fill || ed.edgeFill || "white"
                        }
                        Image {
                            id: base
                            x: -imageFrame.f.x; y: -imageFrame.f.y
                            width: ed.imgW; height: ed.imgH
                            source: ed.basePath ? "file://" + ed.basePath : ""
                            cache: false
                            smooth: true
                            mipmap: true
                            fillMode: Image.Stretch
                        }
                    }
                    Item {
                        id: imageMask
                        visible: false
                        x: imageFrame.x; y: imageFrame.y; width: imageFrame.width; height: imageFrame.height
                        layer.enabled: ed.framed
                        layer.textureSize: Qt.size(Math.ceil(width), Math.ceil(height))
                        Rectangle {
                            x: ed.innerRect.x - imageFrame.f.x; y: ed.innerRect.y - imageFrame.f.y
                            width: ed.innerRect.w; height: ed.innerRect.h
                            radius: ed.bgLive ? ed.bgLive.radius : 0
                            color: "white"
                        }
                    }
                    SpotlightLayer {
                        width: ed.imgW; height: ed.imgH
                        items: ed.liveItems
                    }
                    // "Draw shadow on objects": a hidden copy of the objects casts the
                    // shadow under them, so the objects themselves stay crisp
                    Item {
                        id: shadowCaster
                        visible: false
                        x: ed.exportRect.x; y: ed.exportRect.y
                        width: ed.exportRect.w; height: ed.exportRect.h
                        layer.enabled: shadowed.visible
                        Item {
                            x: -ed.exportRect.x; y: -ed.exportRect.y
                            Repeater {
                                model: shadowed.visible ? ed.liveItems.filter(o => o.kind !== "highlight" && o.kind !== "spotlight") : []
                                delegate: EditorItem {
                                    required property var modelData
                                    o: modelData
                                    base: ed.basePath
                                }
                            }
                        }
                    }
                    MultiEffect {
                        id: shadowed
                        visible: ed.annotateCfg.object_shadows !== false && ed.doc.items.length + (ed.draft ? 1 : 0) > 0
                        anchors.fill: shadowCaster
                        source: shadowCaster
                        shadowEnabled: true
                        shadowBlur: 0.35
                        shadowOpacity: 0.35
                        shadowVerticalOffset: 2 * ed.unit
                        shadowHorizontalOffset: 0
                    }
                    Item {
                        id: objectsGroup
                        x: ed.exportRect.x; y: ed.exportRect.y
                        width: ed.exportRect.w; height: ed.exportRect.h
                        Item {
                            x: -ed.exportRect.x; y: -ed.exportRect.y
                            Repeater {
                                id: objects
                                model: ed.doc.items
                                delegate: EditorItem {
                                    required property var modelData
                                    required property int index
                                    o: modelData
                                    base: ed.basePath
                                    label: Ed.counterLabel(ed.doc, Ed.counterNumber(ed.doc.items, index), modelData.style)
                                    z: modelData.kind === "counter" ? 1 : 0    // counters stay on top (CleanShot)
                                    visible: index !== ed.draftIndex
                                }
                            }
                            EditorItem {
                                o: ed.draft
                                base: ed.basePath
                                visible: ed.draft !== null && !ed.editingText
                                z: 2
                                label: ed.draft && ed.draft.kind === "counter"
                                       ? Ed.counterLabel(ed.doc, ed.draftIndex >= 0 ? Ed.counterNumber(ed.doc.items, ed.draftIndex)
                                                                                   : Ed.counterNumber(ed.doc.items, ed.doc.items.length, ed.draft.style), ed.draft.style) : "1"
                            }
                        }
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
                // measures text for auto-expand
                EditorItem { id: measure; visible: false; o: null }
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
                    base: ed.basePath
                    label: ed.selected >= 0 && ed.shownSelection ? Ed.counterLabel(ed.doc, Ed.counterNumber(ed.doc.items, ed.selected), ed.shownSelection.style) : "1"
                }
                // text being typed, exactly where it will be drawn
                Rectangle {
                    id: textBox
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

        // CleanShot's blue smiley above the text being typed: emoji (ANN-05)
        Rectangle {
            id: emojiButton
            z: 1                    // above the canvas's pointer area
            visible: ed.editingText
            readonly property var at: ed.toView(textBox.x + textBox.width / 2, textBox.y)
            x: at.x - width / 2; y: at.y - height - 8
            width: 22; height: 22; radius: 11
            color: emojiMa.containsMouse ? Qt.lighter("#0a7cff", 1.15) : "#0a7cff"
            border.width: 2; border.color: "white"
            Icon { anchors.centerIn: parent; name: "smile"; size: 14; stroke: 2; color: "white" }
            MouseArea { id: emojiMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                        onClicked: ed.openEmoji() }
        }

        // crop: the rest dimmed, a frame with handles, size and Done
        Item {
            id: cropUi
            z: 1                    // the crop bar's buttons sit above the canvas's pointer area
            anchors.fill: parent
            visible: ed.tool === "crop"
            readonly property var a: ed.toView(ed.cropShown.x, ed.cropShown.y)
            readonly property var b: ed.toView(ed.cropShown.x + ed.cropShown.w, ed.cropShown.y + ed.cropShown.h)
            readonly property var i0: ed.toView(ed.shown.x, ed.shown.y)
            readonly property var i1: ed.toView(ed.shown.x + ed.shown.w, ed.shown.y + ed.shown.h)
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
                    ToolButton {
                        id: ratioButton
                        label: (Ed.cropRatios.find(r => r.r === ed.cropRatio) || Ed.cropRatios[0]).label
                        chevron: true
                        onClicked: ed.openMenuNamed("ratio", ratioButton)
                    }
                    // the crop's size, typed (CleanShot's W ↔ H)
                    component SizeField: Rectangle {
                        id: sizeField
                        property int value: 0
                        signal typed(int v)
                        anchors.verticalCenter: parent ? parent.verticalCenter : undefined
                        width: 58; height: 26; radius: 7
                        color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08)
                        border.width: input.activeFocus ? 1 : 0; border.color: Theme.accent
                        TextInput {
                            id: input
                            anchors.fill: parent; anchors.leftMargin: 8; anchors.rightMargin: 8
                            verticalAlignment: TextInput.AlignVCenter; horizontalAlignment: TextInput.AlignHCenter
                            text: sizeField.value
                            color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 13; font.features: { "tnum": 1 }
                            validator: IntValidator { bottom: 4; top: 100000 }
                            selectByMouse: true
                            onAccepted: { sizeField.typed(parseInt(text)); ed.focusKeys() }
                            Keys.onTabPressed: (e) => { sizeField.typed(parseInt(text)); e.accepted = false }
                            Keys.onEscapePressed: { text = sizeField.value; ed.focusKeys() }
                        }
                    }
                    SizeField {
                        value: Math.round(ed.cropShown.w)
                        onTyped: (v) => { const c = ed.cropShown; ed.setCrop({ x: c.x, y: c.y, w: v, h: ed.ratioValue ? Math.round(v / ed.ratioValue) : c.h }) }
                    }
                    Text { anchors.verticalCenter: parent.verticalCenter; text: "×"; color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 13 }
                    SizeField {
                        value: Math.round(ed.cropShown.h)
                        onTyped: (v) => { const c = ed.cropShown; ed.setCrop({ x: c.x, y: c.y, w: ed.ratioValue ? Math.round(v * ed.ratioValue) : c.w, h: v }) }
                    }
                    // the canvas beyond the image
                    ToolButton {
                        id: fillButton
                        tip: "Canvas fill"
                        chevron: true
                        leftPad: 18
                        Rectangle {
                            x: 9; anchors.verticalCenter: parent.verticalCenter
                            width: 16; height: 16; radius: 8
                            readonly property string f: ed.doc.fill || ed.edgeFill || "#ffffff"
                            color: f === "transparent" ? "white" : f
                            border.width: 1; border.color: Theme.hairline
                            Rectangle { visible: parent.f === "transparent"; anchors.centerIn: parent; width: 14; height: 1.5; rotation: -45; color: Theme.danger }
                        }
                        onClicked: ed.openMenuNamed("fill", fillButton)
                    }
                    ToolButton { icon: "rotate-ccw"; tip: "Rotate left"; onClicked: ed.transformImage("rotate-left") }
                    ToolButton { icon: "rotate-cw"; tip: "Rotate right"; onClicked: ed.transformImage("rotate-right") }
                    ToolButton { icon: "flip-horizontal"; tip: "Flip horizontal"; onClicked: ed.transformImage("flip-horizontal") }
                    ToolButton { icon: "flip-vertical"; tip: "Flip vertical"; onClicked: ed.transformImage("flip-vertical") }
                    ToolButton { id: resizeButton; label: "Resize"; chevron: true; onClicked: { resizeMenu.load(); ed.openMenuNamed("resize", resizeButton) } }
                    ToolButton { label: "Revert to Original"; onClicked: { ed.cropRatio = 0; ed.setCrop(ed.full) } }
                    ToolButton { id: cropCancelButton; label: "Cancel"; onClicked: ed.cropCancel() }
                    ToolButton { label: "Crop"; selected: true; onClicked: ed.cropDone() }
                }
            }
        }

        // drop image files onto the canvas
        DropArea {
            id: dropArea
            anchors.fill: parent
            keys: ["text/uri-list"]
            property string side: ""
            onPositionChanged: (d) => side = ed.dropSide(d.x, d.y)
            onExited: side = ""
            onDropped: (drop) => {
                side = ""
                ed.dropFiles(drop.urls.map(u => decodeURIComponent(String(u).replace(/^file:\/\//, ""))), drop.x, drop.y)
                drop.accept()
            }
        }
        // where a drop would join the canvas: its edge, lit
        Rectangle {
            visible: dropArea.containsDrag && dropArea.side !== ""
            readonly property var a: ed.toView(ed.shown.x, ed.shown.y)
            readonly property var b: ed.toView(ed.shown.x + ed.shown.w, ed.shown.y + ed.shown.h)
            readonly property bool across: dropArea.side === "top" || dropArea.side === "bottom"
            x: dropArea.side === "right" ? b.x - 2 : a.x - (across ? 0 : 2)
            y: dropArea.side === "bottom" ? b.y - 2 : a.y - (across ? 2 : 0)
            width: across ? b.x - a.x : 4
            height: across ? 4 : b.y - a.y
            radius: 2
            color: Theme.accent
        }

        MouseArea {
            id: pointer
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.LeftButton | Qt.MiddleButton
            cursorShape: gesture === "pan" || keys.spaceHeld ? Qt.ClosedHandCursor
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
                                 seed: Math.floor(Math.random() * 1e9), level: ed.redactLevel }
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
                    const snap = !(m.modifiers & Qt.ControlModifier), near = 10 / ed.k   // Ctrl: no snapping
                    let r
                    if (gesture === "crop-handle") {
                        r = Ed.withHandle(pressObj, handle, p.x, p.y)
                        if (snap) r = Ed.snapEdges(r, ed.imgW, ed.imgH, near)
                        r = Ed.withRatio(r, ed.ratioValue, handle)
                    } else if (gesture === "crop-move") {
                        r = Ed.moved(pressObj, p.x - pressAt.x, p.y - pressAt.y)
                        if (snap) {   // the whole frame snaps by its nearest edges
                            const s2 = Ed.snapEdges(r, ed.imgW, ed.imgH, near)
                            r.x += (s2.x !== r.x ? s2.x - r.x : (s2.x + s2.w) - (r.x + r.w))
                            r.y += (s2.y !== r.y ? s2.y - r.y : (s2.y + s2.h) - (r.y + r.h))
                        }
                    } else {
                        r = Ed.normRect(pressAt.x, pressAt.y, p.x, p.y)
                        if (ed.ratioValue) { r.h = r.w / ed.ratioValue; if (p.y < pressAt.y) r.y = pressAt.y - r.h }
                        else if (snap) r = Ed.snapEdges(r, ed.imgW, ed.imgH, near)
                    }
                    ed.cropDraft = ed.clampCrop(r)
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
                    // "Inverse arrow direction", and Alt while drawing, point the arrow the other way (CleanShot)
                    if (d.kind === "arrow" && (!!(m.modifiers & Qt.AltModifier) !== !!ed.annotateCfg.inverse_arrows))
                        Object.assign(d, { x1: d.x2, y1: d.y2, x2: d.x1, y2: d.y1 })
                    if (d.kind === "pencil" && ed.annotateCfg.smooth_drawing !== false) d.pts = Ed.simplify(d.pts, 0.75 / ed.k)
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

    BackgroundPanel {
        id: bgPanel
        ed: ed
        visible: ed.tool === "background"
        z: 2
        width: 300
        anchors { right: parent.right; top: toolbar.bottom; bottom: bottomBar.top }
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
            IconButton { icon: "pin"; tip: "Pin to Screen (Ctrl+P)"; tipAbove: true; width: 32; height: 32; radius: 16; onClicked: ed.flush("pin") }
            IconButton { icon: "copy"; tip: "Copy (Ctrl+C)"; tipAbove: true; width: 32; height: 32; radius: 16; onClicked: ed.flush("copy") }
        }
    }

    // ---------------------------------------------------------------- dropdowns
    function openMenuNamed(name, anchorItem) {
        openMenu({ color: colorMenu, size: sizeMenu, style: styleMenu, level: levelMenu, presets: presetsMenu, bgColor: bgColorMenu, fill: fillMenu, zoom: zoomMenu, arrow: arrowMenu, text: textMenu, dim: dimMenu,
                   counter: counterMenu, opacity: opacityMenu, ratio: ratioMenu, resize: resizeMenu, addImage: addImageMenu }[name], anchorItem)
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
    // the eyedropper (ANN-13): the frozen screen with the magnifier, so it
    // picks anywhere, this canvas included; for the ink or the background
    property int pickToken: 0
    property string pickFor: "ink"      // ink | bg | fill
    function pickColor(target) {
        openedMenu = null
        pickFor = target
        pickToken = controller.nextToken()
        Engine.pickColorForEditor(pickToken)
    }
    Connections {
        target: Engine
        function onEditorColor(token, c) {
            if (token !== ed.pickToken) return
            ed.pickToken = 0
            ed.requestActivate()
            if (!c) return
            if (ed.pickFor === "bg") ed.setBg({ kind: "color", value: c })
            else if (ed.pickFor === "fill") ed.setFill(c)
            else ed.setColor(c)
        }
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
                onEyedropper: ed.pickColor("ink")
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
                width: 168; height: 30; radius: 8
                color: current ? Theme.controlPressed : stMa.containsMouse ? Theme.controlHover : "transparent"
                Text { x: 14; anchors.verticalCenter: parent.verticalCenter; text: parent.modelData.label; color: Theme.text
                       font.family: Theme.fontUi; font.pixelSize: 13 }
                // Natural blur reads what it hides: say so
                Text { anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
                       text: parent.modelData.note || ""; color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 11 }
                MouseArea { id: stMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: { ed.setRedactStyle(parent.modelData.id); ed.openedMenu = null } }
            }
        }
    }

    Menu {
        id: levelMenu
        Repeater {
            model: Ed.redactLevels
            delegate: Rectangle {
                required property var modelData
                readonly property real current: ed.selectedObj && ed.selectedObj.kind === "redact" ? (ed.selectedObj.level || 1) : ed.redactLevel
                width: 112; height: 30; radius: 8
                color: Math.abs(current - modelData.v) < 0.01 ? Theme.controlPressed : lvMa.containsMouse ? Theme.controlHover : "transparent"
                Text { x: 14; anchors.verticalCenter: parent.verticalCenter; text: parent.modelData.label; color: Theme.text
                       font.family: Theme.fontUi; font.pixelSize: 13 }
                MouseArea { id: lvMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: { ed.setRedactLevel(parent.modelData.v); ed.openedMenu = null } }
            }
        }
    }

    // Image size: width and height in pixels or percent, proportions locked
    Menu {
        id: resizeMenu
        z: 11
        property bool percent: false
        property bool lock: true
        property real rw: ed.imgW
        property real rh: ed.imgH
        function load() { rw = ed.imgW; rh = ed.imgH }
        function shown(v, full) { return percent ? Math.round(v / full * 100) : Math.round(v) }
        function set(which, text) {
            const v = Number(text); if (!(v > 0)) return
            const px = percent ? v / 100 * (which === "w" ? ed.imgW : ed.imgH) : v
            if (which === "w") { rw = px; if (lock) rh = px * ed.imgH / ed.imgW }
            else { rh = px; if (lock) rw = px * ed.imgW / ed.imgH }
        }
        component Field: Rectangle {
            property alias text: input.text
            signal edited(string text)
            width: 70; height: 28; radius: 8
            color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08)
            TextInput {
                id: input
                anchors.fill: parent; anchors.leftMargin: 8; anchors.rightMargin: 8
                verticalAlignment: TextInput.AlignVCenter; horizontalAlignment: TextInput.AlignRight
                color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 13; font.features: { "tnum": 1 }
                selectByMouse: true
                validator: IntValidator { bottom: 1; top: 20000 }
                onEditingFinished: parent.edited(text)
            }
        }
        Row {
            spacing: 6
            Field { text: resizeMenu.shown(resizeMenu.rw, ed.imgW); onEdited: (t) => resizeMenu.set("w", t) }
            Text { anchors.verticalCenter: parent.verticalCenter; text: "×"; color: Theme.textSecondary; font.pixelSize: 13 }
            Field { text: resizeMenu.shown(resizeMenu.rh, ed.imgH); onEdited: (t) => resizeMenu.set("h", t) }
            ToolButton { label: resizeMenu.percent ? "%" : "px"; onClicked: resizeMenu.percent = !resizeMenu.percent }
        }
        Row {
            spacing: 6
            ToolButton { label: resizeMenu.lock ? "Proportions locked" : "Proportions free"; onClicked: resizeMenu.lock = !resizeMenu.lock }
            ToolButton { label: "Resize"; selected: true
                         onClicked: { ed.openedMenu = null; ed.transformImage("resize", Math.round(resizeMenu.rw), Math.round(resizeMenu.rh)) } }
        }
    }

    Menu {
        id: addImageMenu
        Repeater {
            model: [{ label: "Choose From File…", key: "Ctrl+I", act: () => ed.chooseImage() },
                    { label: "Take Screenshot", key: "Ctrl+Shift+I", act: () => ed.takeScreenshot() },
                    { label: "Paste From Clipboard", key: "Ctrl+V", act: () => ed.pasteImage() }]
            delegate: Rectangle {
                required property var modelData
                width: 230; height: 30; radius: 8
                color: aiMa.containsMouse ? Theme.controlHover : "transparent"
                Text { x: 12; anchors.verticalCenter: parent.verticalCenter; text: parent.modelData.label; color: Theme.text
                       font.family: Theme.fontUi; font.pixelSize: 13 }
                Text { anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
                       text: parent.modelData.key; color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 12 }
                MouseArea { id: aiMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: { ed.openedMenu = null; parent.modelData.act() } }
            }
        }
    }

    Menu {
        id: ratioMenu
        z: 11      // above the crop bar
        Repeater {
            model: Ed.cropRatios
            delegate: Rectangle {
                required property var modelData
                width: 140; height: 30; radius: 8
                color: ed.cropRatio === modelData.r ? Theme.controlPressed : raMa.containsMouse ? Theme.controlHover : "transparent"
                Text { x: 14; anchors.verticalCenter: parent.verticalCenter; text: parent.modelData.label; color: Theme.text
                       font.family: Theme.fontUi; font.pixelSize: 13 }
                MouseArea { id: raMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: { ed.setCropRatio(parent.modelData.r); ed.openedMenu = null } }
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

    component MenuRow: Rectangle {
        id: menuRow
        property string text: ""
        property bool enabledRow: true
        signal activated()
        width: 270; height: 30; radius: 8
        color: enabledRow && rowMa.containsMouse ? Theme.controlHover : "transparent"
        Text { x: 12; anchors.verticalCenter: parent.verticalCenter; width: parent.width - 48; elide: Text.ElideRight
               text: menuRow.text; color: menuRow.enabledRow ? Theme.text : Theme.textSecondary
               font.family: Theme.fontUi; font.pixelSize: 13 }
        MouseArea { id: rowMa; anchors.fill: parent; hoverEnabled: true; enabled: menuRow.enabledRow
                    cursorShape: Qt.PointingHandCursor; onClicked: menuRow.activated() }
    }

    // the Background tool's own colour: shown while picking, one undo step when it closes
    Menu {
        id: bgColorMenu
        onVisibleChanged: if (!visible && ed.bgPreview) ed.setBg(ed.bgPreview)
        ColorPanel {
            value: ed.bgLive && ed.bgLive.kind === "color" && ed.bgLive.value.startsWith("#") ? ed.bgLive.value : "#3a7bd5"
            onPicked: (c) => ed.previewBg({ kind: "color", value: c })
            onSave: ed.saveColor(current)
            onEyedropper: ed.pickColor("bg")
        }
    }

    // Crop's canvas fill: Automatic (the edge colour), Transparent, White, Black, any colour
    Menu {
        id: fillMenu
        z: 11      // above the crop bar
        Repeater {
            model: [{ label: "Automatic", value: "auto" }, { label: "Transparent", value: "transparent" },
                    { label: "White", value: "#ffffff" }, { label: "Black", value: "#000000" }]
            delegate: MenuRow {
                required property var modelData
                width: 200
                text: modelData.label
                onActivated: { ed.setFill(modelData.value === "auto" ? (ed.edgeFill || Engine.edgeColor(ed.basePath)) : modelData.value); ed.openedMenu = null }
            }
        }
        Rectangle { width: 200; height: 1; color: Theme.hairline }
        ColorPanel {
            width: 200
            value: ed.doc.fill && ed.doc.fill.startsWith("#") ? ed.doc.fill : "#ffffff"
            onPicked: (c) => ed.setFill(c)
            onSave: ed.saveColor(current)
            onEyedropper: ed.pickColor("fill")
        }
    }

    // the Background tool's presets (BG-03)
    Menu {
        id: presetsMenu
        MenuRow { text: "Apply Previous Settings"; enabledRow: !!ed.bgStore.previous
                  onActivated: { ed.applyPreviousBg(); ed.openedMenu = null } }
        Rectangle { visible: ed.bgPresets.length > 0; width: 270; height: 1; color: Theme.hairline }
        Repeater {
            model: ed.bgPresets
            delegate: MenuRow {
                required property var modelData
                text: modelData.name
                onActivated: { ed.applyBgPreset(modelData.name); ed.openedMenu = null }
                Icon { visible: ed.bgPreset === parent.modelData.name; anchors { right: parent.right; rightMargin: 36; verticalCenter: parent.verticalCenter }
                       name: "check"; size: 14; color: Theme.accent }
                ToolButton { anchors { right: parent.right; rightMargin: 2; verticalCenter: parent.verticalCenter }
                             implicitWidth: 28; icon: "trash"; tip: "Delete preset"; onClicked: ed.deleteBgPreset(parent.modelData.name) }
            }
        }
        Rectangle { width: 270; height: 1; color: Theme.hairline }
        MenuRow { visible: ed.bgPresetChanged; text: "Save Changes to “" + ed.bgPreset + "”"
                  onActivated: { ed.saveBgPreset(ed.bgPreset); ed.openedMenu = null } }
        MenuRow { text: "Add New Preset…"; onActivated: { ed.openedMenu = null; bgPanel.naming = true } }
    }

    Menu {
        id: emojiMenu
        readonly property var found: {
            const q = emojiSearch.text.trim().toLowerCase()
            return q ? ed.emojiList.filter(i => i.k.indexOf(q) >= 0) : ed.emojiList
        }
        // closing it in any way goes back to typing
        onVisibleChanged: if (!visible && ed.editingText) textEdit.forceActiveFocus()
        Rectangle {
            width: 8 * 34; height: 30; radius: 8
            color: Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.08)
            TextInput {
                id: emojiSearch
                anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 10
                verticalAlignment: TextInput.AlignVCenter
                color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 13
                Text { visible: !parent.text; anchors.verticalCenter: parent.verticalCenter; text: "Search emoji"
                       color: Theme.textSecondary; font: parent.font }
                Keys.onEscapePressed: ed.openedMenu = null
                Keys.onReturnPressed: if (emojiMenu.found.length) ed.insertEmoji(emojiMenu.found[0].e)
                Keys.onEnterPressed: if (emojiMenu.found.length) ed.insertEmoji(emojiMenu.found[0].e)
            }
        }
        GridView {
            width: 8 * 34; height: 6 * 34
            cellWidth: 34; cellHeight: 34
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            model: emojiMenu.found
            delegate: Rectangle {
                required property var modelData
                width: 34; height: 34; radius: 8
                color: emMa.containsMouse ? Theme.controlHover : "transparent"
                Text { anchors.centerIn: parent; text: parent.modelData.e; font.pixelSize: 22 }
                MouseArea { id: emMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: ed.insertEmoji(parent.modelData.e) }
            }
            Text { visible: !emojiMenu.found.length; anchors.centerIn: parent; color: Theme.textSecondary
                   font.family: Theme.fontUi; font.pixelSize: 13
                   text: ed.emojiList.length ? "No emoji found" : "Emoji come from Omarchy" }
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
            // CleanShot's zoom menu
            model: [{ t: "Fit Canvas", k: "Ctrl+0", z: 0 }, { t: "50%", k: "", z: 0.5 }, { t: "100%", k: "Ctrl+1", z: 1 }, { t: "200%", k: "", z: 2 },
                    { t: "Zoom In", k: "Ctrl++", z: "in" }, { t: "Zoom Out", k: "Ctrl+−", z: "out" }]
            delegate: Rectangle {
                required property var modelData
                width: 150; height: 28; radius: 8
                color: zmMa.containsMouse ? Theme.controlHover : "transparent"
                Text { x: 12; anchors.verticalCenter: parent.verticalCenter; text: parent.modelData.t; color: Theme.text; font.family: Theme.fontUi; font.pixelSize: 13 }
                Text { anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
                       text: parent.modelData.k; color: Theme.textSecondary; font.family: Theme.fontUi; font.pixelSize: 12 }
                MouseArea { id: zmMa; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                const z = parent.modelData.z
                                z === "in" ? ed.zoomStep(1) : z === "out" ? ed.zoomStep(-1) : z ? ed.zoomTo(z) : ed.zoomFit()
                                ed.openedMenu = null
                            } }
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
        case "words": Engine.requestWords(basePath); break
        case "eyedropper": pickColor(a[1] || "ink"); break
        case "add-image": addImageFile(Engine.editorImportImage(basePath, a.slice(1).join(" "))); break
        case "paste-image": pasteImage(); break
        case "save-color": saveColor(a[1]); break
        case "counter-start": setCounterStart(n(1)); break
        case "text": addItem({ kind: "text", x: n(1), y: n(2), text: a.slice(3).join(" "), color: ink, size: sizeFor("text"), style: textStyle }); break
        case "text-style": setTextStyle(a[1]); break
        case "spotlight": addItem({ kind: "spotlight", x: n(1), y: n(2), w: n(3), h: n(4), shape: a[5] || spotShape, dim: a[6] ? n(6) : spotDim }); break
        case "text-width": { const o = Ed.copy(selectedObj); o.w = n(1); replaceItem(selected, o); break }
        case "redact": addItem({ kind: "redact", x: n(1), y: n(2), w: n(3), h: n(4), style: a[5] || redactStyle, seed: a[6] ? n(6) : 1,
                                 level: a[7] ? n(7) : redactLevel }); break
        case "crop": a[1] === "reset" ? setCrop(full) : setCrop({ x: n(1), y: n(2), w: n(3), h: n(4) }); break
        case "crop-ratio": setCropRatio(n(1)); break
        case "fill": setFill(a[1]); break
        case "crop-cancel": cropCancel(); break
        case "transform": transformImage(a[1], n(2), n(3)); break
        case "bg": setBg(a[1] === "none" ? null : JSON.parse(a.slice(1).join(" "))); break
        case "bg-preset": applyBgPreset(a.slice(1).join(" ")); break
        case "bg-preset-save": saveBgPreset(a.slice(1).join(" ")); break
        case "bg-preset-delete": deleteBgPreset(a.slice(1).join(" ")); break
        case "bg-previous": applyPreviousBg(); break
        case "take-screenshot": takeScreenshot(); break
        case "drop": { const v = toView(n(2), n(3)); dropFiles([a[1]], v.x, v.y); break }   // at image px
        case "add-wallpaper": bgPanel.addWallpaper(a.slice(1).join(" ")); break
        case "remove-wallpaper": bgPanel.removeWallpaper(a.slice(1).join(" ")); break
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
                ready: base.status === Image.Ready, editingText: editingText, typed: textEdit.text, active: ed.active, menuOpen: openedMenu !== null, toolbarNeeds: toolbar.needed,
                cropCancelAt: tool === "crop" ? cropCancelButton.mapToItem(null, cropCancelButton.width / 2, cropCancelButton.height / 2) : null,
                emojiButton: emojiButton.visible ? emojiButton.mapToItem(null, 11, 11) : null,
                selectionBox: { x: selectionBox.x, y: selectionBox.y, w: selectionBox.width, h: selectionBox.height },
                words: words ? words.length : -1, fill: doc.fill || "", shownX: shown.x, shownY: shown.y,
                imgW: imgW, imgH: imgH, base: doc.base || "", bg: doc.bg || null, exportRect: exportRect, balance: balance, paddingValue: bgPanel.visible ? bgPanel.paddingSlider.valueBox.mapToItem(null, 14, 9) : null,
                bgPresets: bgPresets.map(p => p.name), bgPreset: bgPreset, bgPresetChanged: bgPresetChanged, ownWalls: bgPanel.ownWalls,
                shadows: annotateCfg.object_shadows !== false, ink: ink, savedColors: savedColors, picking: pickToken !== 0,
                counterLabels: doc.items.map((o, i) => o.kind === "counter" ? Ed.counterLabel(doc, Ed.counterNumber(doc.items, i), o.style) : null).filter(l => l !== null)
            }).slice(1, -1))
            break
        }
    }
}
