.pragma library

// The editor's objects: plain JSON in image pixels, so the document is
// saved as-is (edit.json) and undo is a list of snapshots.
//
//   arrow   {kind, x1, y1, x2, y2, cx, cy, color, size}  (cx, cy) bends it
//   rect    {kind, x, y, w, h, color, size, filled}
//   ellipse {kind, x, y, w, h, color, size}
//   line    {kind, x1, y1, x2, y2, color, size}
//   text    {kind, x, y, text, color, size}               size = font px
//   counter {kind, x, y, color, size}                     centre; size = diameter
//   redact  {kind, x, y, w, h, style, seed}               pixelate | blur | black
//
// Counters are numbered by their order in the document, so deleting one
// renumbers the rest (ANN-08).

// CleanShot's ten swatches; pink is its default.
var palette = ["#000000", "#ff3b30", "#ff9500", "#ffcc00", "#34c759",
               "#30c0b5", "#0a7cff", "#8e5cf7", "#f1285f", "#ffffff"]

// Size presets per tool, in logical px (× the image's scale).
var sizes = {
    arrow: [2, 3, 5, 8, 12],
    rect: [2, 3, 5, 8, 12],
    ellipse: [2, 3, 5, 8, 12],
    line: [2, 3, 5, 8, 12],
    text: [14, 20, 28, 40, 56],
    counter: [20, 26, 32, 40, 52]
}
var defaultSize = { arrow: 2, rect: 2, ellipse: 2, line: 2, text: 1, counter: 2 }

// Objects with a box (x, y, w, h) and corner handles.
function isBox(o) { return o.kind === "rect" || o.kind === "ellipse" || o.kind === "redact" }

function dist(ax, ay, bx, by) { return Math.hypot(ax - bx, ay - by) }

function segmentDist(px, py, ax, ay, bx, by) {
    var dx = bx - ax, dy = by - ay, l2 = dx * dx + dy * dy
    var t = l2 > 0 ? Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / l2)) : 0
    return dist(px, py, ax + t * dx, ay + t * dy)
}

// Point on an arrow's curve at t (quadratic Bézier through its control).
function arrowAt(o, t) {
    var u = 1 - t
    return { x: u * u * o.x1 + 2 * u * t * o.cx + t * t * o.x2,
             y: u * u * o.y1 + 2 * u * t * o.cy + t * t * o.y2 }
}

// Control point that makes the curve pass through m at t = 0.5.
function controlThrough(o, mx, my) {
    return { cx: 2 * mx - (o.x1 + o.x2) / 2, cy: 2 * my - (o.y1 + o.y2) / 2 }
}

function normRect(x1, y1, x2, y2) {
    return { x: Math.min(x1, x2), y: Math.min(y1, y2), w: Math.abs(x2 - x1), h: Math.abs(y2 - y1) }
}

function copy(o) { return JSON.parse(JSON.stringify(o)) }

function moved(o, dx, dy) {
    var n = copy(o)
    if (n.kind === "arrow") {
        n.x1 += dx; n.y1 += dy; n.x2 += dx; n.y2 += dy; n.cx += dx; n.cy += dy
    } else if (n.kind === "line") {
        n.x1 += dx; n.y1 += dy; n.x2 += dx; n.y2 += dy
    } else {
        n.x += dx; n.y += dy
    }
    return n
}

// Handles of a selected object, in image px.
function handles(o) {
    if (o.kind === "arrow") {
        var m = arrowAt(o, 0.5)
        return [{ name: "start", x: o.x1, y: o.y1 }, { name: "end", x: o.x2, y: o.y2 }, { name: "mid", x: m.x, y: m.y }]
    }
    if (o.kind === "line")
        return [{ name: "start", x: o.x1, y: o.y1 }, { name: "end", x: o.x2, y: o.y2 }]
    if (isBox(o))
        return [{ name: "tl", x: o.x, y: o.y }, { name: "tr", x: o.x + o.w, y: o.y },
                { name: "bl", x: o.x, y: o.y + o.h }, { name: "br", x: o.x + o.w, y: o.y + o.h }]
    return []
}

// The object with handle `h` dragged to (x, y).
function withHandle(o, h, x, y) {
    var n = copy(o)
    if (n.kind === "arrow") {
        if (h === "mid") {
            var c = controlThrough(n, x, y)
            n.cx = c.cx; n.cy = c.cy
            return n
        }
        // an end drags the control half as far: a straight arrow stays straight
        var dx = x - (h === "start" ? o.x1 : o.x2), dy = y - (h === "start" ? o.y1 : o.y2)
        if (h === "start") { n.x1 = x; n.y1 = y } else { n.x2 = x; n.y2 = y }
        n.cx = o.cx + dx / 2; n.cy = o.cy + dy / 2
        return n
    }
    if (n.kind === "line") {
        if (h === "start") { n.x1 = x; n.y1 = y } else { n.x2 = x; n.y2 = y }
        return n
    }
    if (isBox(n)) {
        var x1 = o.x, y1 = o.y, x2 = o.x + o.w, y2 = o.y + o.h
        if (h === "tl") { x1 = x; y1 = y } else if (h === "tr") { x2 = x; y1 = y }
        else if (h === "bl") { x1 = x; y2 = y } else if (h === "br") { x2 = x; y2 = y }
        else if (h === "t") y1 = y; else if (h === "b") y2 = y
        else if (h === "l") x1 = x; else if (h === "r") x2 = x
        var r = normRect(x1, y1, x2, y2)
        n.x = r.x; n.y = r.y; n.w = r.w; n.h = r.h
        if (n.kind === "redact") n = whole(n)
    }
    return n
}

// Redaction regions sit on whole pixels, so their tiles line up with the
// image (and so does a crop).
function whole(r) {
    var n = copy(r)
    var x2 = Math.round(r.x + r.w), y2 = Math.round(r.y + r.h)
    n.x = Math.round(r.x); n.y = Math.round(r.y); n.w = x2 - n.x; n.h = y2 - n.y
    return n
}

function clampRect(r, w, h) {
    var x1 = Math.max(0, Math.min(w, r.x)), y1 = Math.max(0, Math.min(h, r.y))
    var x2 = Math.max(0, Math.min(w, r.x + r.w)), y2 = Math.max(0, Math.min(h, r.y + r.h))
    return whole({ x: x1, y: y1, w: x2 - x1, h: y2 - y1 })
}

// Does a click at (x, y) (image px) land on o? `tol` widens thin strokes;
// `box` is the rendered size of text.
function hits(o, x, y, tol, box) {
    if (o.kind === "arrow") {
        var best = 1e9
        for (var i = 0; i <= 24; i++) {
            var p = arrowAt(o, i / 24)
            best = Math.min(best, dist(x, y, p.x, p.y))
        }
        return best <= o.size * 1.6 + tol
    }
    if (o.kind === "rect") {
        var inX = x >= o.x - tol && x <= o.x + o.w + tol, inY = y >= o.y - tol && y <= o.y + o.h + tol
        var nearEdge = Math.min(Math.abs(x - o.x), Math.abs(x - o.x - o.w), Math.abs(y - o.y), Math.abs(y - o.y - o.h))
        return inX && inY && (o.filled || nearEdge <= o.size + tol)
    }
    if (o.kind === "ellipse") {
        // distance from the outline, measured along the smaller radius
        var rx = Math.max(o.w / 2, 0.5), ry = Math.max(o.h / 2, 0.5)
        var nx = (x - o.x - rx) / rx, ny = (y - o.y - ry) / ry
        return Math.abs(Math.hypot(nx, ny) - 1) * Math.min(rx, ry) <= o.size + tol
    }
    if (o.kind === "line")
        return segmentDist(x, y, o.x1, o.y1, o.x2, o.y2) <= o.size + tol
    if (o.kind === "counter")
        return dist(x, y, o.x, o.y) <= o.size / 2 + tol
    if (o.kind === "redact")
        return x >= o.x - tol && x <= o.x + o.w + tol && y >= o.y - tol && y <= o.y + o.h + tol
    if (o.kind === "text" && box)
        return x >= o.x - tol && x <= o.x + box.width + tol && y >= o.y - tol && y <= o.y + box.height + tol
    return false
}

// A drag shorter than this (image px) makes no object.
function tooSmall(o) {
    if (o.kind === "arrow") return dist(o.x1, o.y1, o.x2, o.y2) < o.size * 2
    if (o.kind === "line") return dist(o.x1, o.y1, o.x2, o.y2) < o.size * 2
    if (isBox(o)) return o.w < 3 || o.h < 3
    return false
}

function counterNumber(items, index) {
    var n = 0
    for (var i = 0; i <= index; i++) if (items[i].kind === "counter") n++
    return n
}

var redactStyles = [{ id: "pixelate", label: "Pixelate" }, { id: "blur", label: "Blur" }, { id: "black", label: "Black out" }]

// Crop handles: corners and edge middles.
function cropHandles(r) {
    var cx = r.x + r.w / 2, cy = r.y + r.h / 2
    return [{ name: "tl", x: r.x, y: r.y }, { name: "t", x: cx, y: r.y }, { name: "tr", x: r.x + r.w, y: r.y },
            { name: "r", x: r.x + r.w, y: cy }, { name: "br", x: r.x + r.w, y: r.y + r.h }, { name: "b", x: cx, y: r.y + r.h },
            { name: "bl", x: r.x, y: r.y + r.h }, { name: "l", x: r.x, y: cy }]
}
