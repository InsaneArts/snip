.pragma library

// The editor's objects: plain JSON in image pixels, so the document is
// saved as-is (edit.json) and undo is a list of snapshots.
//
//   arrow   {kind, x1, y1, x2, y2, cx, cy, color, size, style}
//           style: standard | fancy | curved | double; only curved bends at (cx, cy)
//   rect    {kind, x, y, w, h, color, size, filled}
//   ellipse {kind, x, y, w, h, color, size}
//   line    {kind, x1, y1, x2, y2, color, size}
//   text    {kind, x, y, text, color, size, style, w}     size = font px; w wraps it (0: one line)
//   counter {kind, x, y, color, size, style}              centre; size = diameter; num | alpha | roman
//   redact  {kind, x, y, w, h, style, seed, level}        pixelate | text | blur | black | natural
//   pencil  {kind, pts: [[x, y], …], color, size}         free hand, smoothed
//   highlight {kind, x, y, w, h, color, opacity}          a translucent band
//   image   {kind, x, y, w, h, src}                       src: a file beside the base image
//   spotlight {kind, x, y, w, h, shape, dim}              shape: rect | ellipse; dims the rest
//
// Counters are numbered by their order in the document, each style on its
// own, so deleting one renumbers the rest (ANN-08), from the document's
// counterStart (default 1).

// CleanShot's ten swatches; pink is its default.
var palette = ["#000000", "#ff3b30", "#ff9500", "#ffcc00", "#34c759",
               "#30c0b5", "#0a7cff", "#8e5cf7", "#f1285f", "#ffffff"]

// Size presets per tool, in logical px (× the image's scale).
var sizes = {
    arrow: [2, 3, 5, 8, 12],
    rect: [2, 3, 5, 8, 12],
    ellipse: [2, 3, 5, 8, 12],
    line: [2, 3, 5, 8, 12],
    pencil: [2, 3, 5, 8, 12],
    highlight: [12, 16, 20, 28, 36],
    text: [14, 20, 28, 40, 56],
    counter: [20, 26, 32, 40, 52]
}
var defaultSize = { arrow: 2, rect: 2, ellipse: 2, line: 2, pencil: 2, highlight: 2, text: 1, counter: 2 }

// Objects with a box (x, y, w, h) and corner handles.
function isBox(o) { return ["rect", "ellipse", "redact", "spotlight", "highlight", "image"].indexOf(o.kind) >= 0 }

var highlightOpacities = [0.25, 0.35, 0.45, 0.6]

// Text lines from OCR word boxes ([{line, x, y, w, h}]): their extent and words.
function textLines(words) {
    var lines = {}
    for (var i = 0; i < words.length; i++) {
        var w = words[i], l = lines[w.line] || (lines[w.line] = { y0: 1e9, y1: -1e9, words: [] })
        l.y0 = Math.min(l.y0, w.y); l.y1 = Math.max(l.y1, w.y + w.h); l.words.push(w)
    }
    return Object.keys(lines).map(function (k) { return lines[k] })
}

// CleanShot's smart highlighter (ANN-10): pressed on a text line, the band
// takes the line's height and runs from the first to the last word the drag
// covers. Null when the press isn't on text.
function snapHighlight(lines, x0, y0, x, pad) {
    for (var i = 0; i < lines.length; i++) {
        var l = lines[i]
        if (y0 < l.y0 - pad || y0 > l.y1 + pad) continue
        var lo = Math.min(x0, x), hi = Math.max(x0, x)
        var hit = l.words.filter(function (w) { return w.x + w.w >= lo - pad && w.x <= hi + pad })
        if (!hit.length) continue
        var a = Math.min.apply(null, hit.map(function (w) { return w.x })), b = Math.max.apply(null, hit.map(function (w) { return w.x + w.w }))
        return { x: a - 3, y: l.y0 - 3, w: b - a + 6, h: l.y1 - l.y0 + 6 }
    }
    return null
}

var spotlightDims = [0.25, 0.4, 0.55, 0.7, 0.85]

// A spotlight's outline: a rounded rectangle or an ellipse, as an SVG path.
function spotPath(o) {
    var x = o.x, y = o.y, w = Math.max(1, o.w), h = Math.max(1, o.h)
    if (o.shape === "ellipse") {
        var rx = w / 2, ry = h / 2, cy = y + ry
        return "M " + x + " " + cy + " A " + rx + " " + ry + " 0 1 0 " + (x + w) + " " + cy
             + " A " + rx + " " + ry + " 0 1 0 " + x + " " + cy + " Z"
    }
    var r = Math.min(w, h) * 0.12
    return "M " + (x + r) + " " + y + " H " + (x + w - r) + " A " + r + " " + r + " 0 0 1 " + (x + w) + " " + (y + r)
         + " V " + (y + h - r) + " A " + r + " " + r + " 0 0 1 " + (x + w - r) + " " + (y + h)
         + " H " + (x + r) + " A " + r + " " + r + " 0 0 1 " + x + " " + (y + h - r)
         + " V " + (y + r) + " A " + r + " " + r + " 0 0 1 " + (x + r) + " " + y + " Z"
}

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
    } else if (n.kind === "pencil") {
        n.pts = n.pts.map(function (p) { return [p[0] + dx, p[1] + dy] })
    } else {
        n.x += dx; n.y += dy
    }
    return n
}

// CleanShot's seven text styles: a font (system, rounded, mono), a box
// tinted with the text colour, or an outline.
var textStyles = [{ id: "standard", label: "Standard" }, { id: "box", label: "Box" }, { id: "rounded", label: "Rounded" },
                  { id: "rounded-box", label: "Rounded Box" }, { id: "outline", label: "Outline" },
                  { id: "mono", label: "Monospaced" }, { id: "mono-box", label: "Monospaced Box" }]
function textBoxed(style) { return style === "box" || style === "rounded-box" || style === "mono-box" }
function textFamily(style) { return style === "rounded" || style === "rounded-box" ? "rounded" : style === "mono" || style === "mono-box" ? "mono" : "ui" }

function luminance(hex) {
    var c = parseInt(String(hex).replace("#", "").slice(-6), 16)
    return (0.299 * ((c >> 16) & 255) + 0.587 * ((c >> 8) & 255) + 0.114 * (c & 255)) / 255
}
// A box's background: the text colour mixed into white, or a dark glass for light text.
function textTint(hex) {
    if (luminance(hex) > 0.8) return "#cc1d1d1f"
    var c = parseInt(String(hex).replace("#", "").slice(-6), 16), out = "#"
    for (var sh = 16; sh >= 0; sh -= 8) {
        var v = Math.round(((c >> sh) & 255) * 0.16 + 255 * 0.84)
        out += (v < 16 ? "0" : "") + v.toString(16)
    }
    return out
}

var arrowStyles = [{ id: "standard", label: "Standard" }, { id: "fancy", label: "Fancy" },
                   { id: "curved", label: "Curved" }, { id: "double", label: "Double" }]

// A curved arrow starts with a gentle bend; the others are straight.
function withArrowStyle(o, style) {
    var n = copy(o)
    n.style = style
    var mx = (n.x1 + n.x2) / 2, my = (n.y1 + n.y2) / 2
    if (style === "curved") {
        if (dist(n.cx, n.cy, mx, my) < 1) {
            var l = dist(n.x1, n.y1, n.x2, n.y2)
            var px = -(n.y2 - n.y1) / (l || 1), py = (n.x2 - n.x1) / (l || 1)
            n.cx = mx + px * l * 0.35; n.cy = my + py * l * 0.35
        }
    } else {
        n.cx = mx; n.cy = my
    }
    return n
}

function headPath(tx, ty, ang, len, half) {
    var c = Math.cos(ang), s = Math.sin(ang)
    return "M " + tx + " " + ty
         + " L " + (tx - c * len + s * half) + " " + (ty - s * len - c * half)
         + " L " + (tx - c * len - s * half) + " " + (ty - s * len + c * half) + " Z"
}

// An arrow as SVG paths: the shaft (stroked), the heads (filled) and, for
// Fancy, a tapered body (filled) instead of a shaft.
function arrowGeometry(o) {
    var style = o.style || "standard", s = o.size
    var len = s * 3.2 + 4, half = len * 0.62
    if (style === "fancy") {
        var a = Math.atan2(o.y2 - o.y1, o.x2 - o.x1), c = Math.cos(a), sn = Math.sin(a)
        var flen = s * 4.2 + 6, fhalf = flen * 0.62
        var bx = o.x2 - c * flen * 0.75, by = o.y2 - sn * flen * 0.75
        var w0 = s * 0.18, w1 = s * 0.95
        var pts = [[o.x1 - sn * w0, o.y1 + c * w0], [bx - sn * w1, by + c * w1], [bx - sn * fhalf, by + c * fhalf], [o.x2, o.y2],
                   [bx + sn * fhalf, by - c * fhalf], [bx + sn * w1, by - c * w1], [o.x1 + sn * w0, o.y1 - c * w0]]
        return { shaft: "", heads: "", body: "M " + pts.map(function (p) { return p[0] + " " + p[1] }).join(" L ") + " Z" }
    }
    // heads follow the curve's direction at each end
    var endA = dist(o.cx, o.cy, o.x2, o.y2) > 0.5 ? Math.atan2(o.y2 - o.cy, o.x2 - o.cx) : Math.atan2(o.y2 - o.y1, o.x2 - o.x1)
    var startA = dist(o.cx, o.cy, o.x1, o.y1) > 0.5 ? Math.atan2(o.y1 - o.cy, o.x1 - o.cx) : Math.atan2(o.y1 - o.y2, o.x1 - o.x2)
    var ex = o.x2 - Math.cos(endA) * len * 0.8, ey = o.y2 - Math.sin(endA) * len * 0.8
    var sx = o.x1, sy = o.y1, heads = headPath(o.x2, o.y2, endA, len, half)
    if (style === "double") {
        sx = o.x1 - Math.cos(startA) * len * 0.8; sy = o.y1 - Math.sin(startA) * len * 0.8
        heads += " " + headPath(o.x1, o.y1, startA, len, half)
    }
    return { shaft: "M " + sx + " " + sy + " Q " + o.cx + " " + o.cy + " " + ex + " " + ey, heads: heads, body: "" }
}

// Handles of a selected object, in image px. Text needs its drawn box.
function handles(o, box) {
    if (o.kind === "text" && box)
        return [{ name: "l", x: box.x, y: box.y + box.height / 2 }, { name: "r", x: box.x + box.width, y: box.y + box.height / 2 },
                { name: "br", x: box.x + box.width, y: box.y + box.height, square: true }]
    if (o.kind === "arrow") {
        var ends = [{ name: "start", x: o.x1, y: o.y1 }, { name: "end", x: o.x2, y: o.y2 }]
        if (o.style !== "curved") return ends
        var m = arrowAt(o, 0.5)
        return ends.concat([{ name: "mid", x: m.x, y: m.y }])
    }
    if (o.kind === "line")
        return [{ name: "start", x: o.x1, y: o.y1 }, { name: "end", x: o.x2, y: o.y2 }]
    if (isBox(o))
        return [{ name: "tl", x: o.x, y: o.y }, { name: "tr", x: o.x + o.w, y: o.y },
                { name: "bl", x: o.x, y: o.y + o.h }, { name: "br", x: o.x + o.w, y: o.y + o.h }]
    return []
}

// The object with handle `h` dragged to (x, y).
function withHandle(o, h, x, y, box) {
    var n = copy(o)
    if (n.kind === "text" && box) {
        var pad = o.x - box.x, right = box.x + box.width
        if (h === "r") n.w = Math.max(o.size * 2, x - o.x - pad)
        else if (h === "l") { n.x = Math.min(x + pad, right - o.size * 2 - pad); n.w = Math.max(o.size * 2, right - pad - n.x) }
        else if (h === "br") {     // the corner square scales the text
            var f = Math.max(0.2, (y - box.y) / Math.max(1, box.height))
            n.size = Math.max(6, Math.round(o.size * f))
            if (o.w) n.w = o.w * n.size / o.size
        }
        return n
    }
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
        if (n.kind === "image" && h.length === 2) {   // pictures keep their shape: the opposite corner stays
            var ratio = o.w / o.h
            r.h = r.w / ratio
            if (h[0] === "t") r.y = o.y + o.h - r.h
        }
        n.x = r.x; n.y = r.y; n.w = r.w; n.h = r.h
        if (n.kind === "redact") n = whole(n)
    }
    return n
}

// Redaction regions sit on whole pixels, so their tiles line up with the
// image (and so does a crop).
// Where an object draws, in image px (text needs its rendered `textBox`).
function extent(o, textBox) {
    var pts, pad = 0
    if (o.kind === "arrow") { pts = [[o.x1, o.y1], [o.x2, o.y2], [o.cx === undefined ? o.x1 : o.cx, o.cy === undefined ? o.y1 : o.cy]]; pad = o.size * 2 }
    else if (o.kind === "line") { pts = [[o.x1, o.y1], [o.x2, o.y2]]; pad = o.size / 2 }
    else if (o.kind === "pencil") { pts = o.pts; pad = o.size / 2 }
    else if (o.kind === "counter") { pts = [[o.x, o.y]]; pad = o.size / 2 }
    else if (o.kind === "text") return { x: textBox.x, y: textBox.y, w: textBox.width, h: textBox.height }
    else return { x: o.x, y: o.y, w: o.w, h: o.h }
    var xs = pts.map(function (p) { return p[0] }), ys = pts.map(function (p) { return p[1] })
    var x0 = Math.min.apply(null, xs) - pad, y0 = Math.min.apply(null, ys) - pad
    return { x: x0, y: y0, w: Math.max.apply(null, xs) + pad - x0, h: Math.max.apply(null, ys) + pad - y0 }
}

// Snapping guides (ANN-12): the moved box's edges or middle snap to the
// nearest target line within `tol`, per axis; gx / gy are the lines it met.
function snapBox(e, xs, ys, tol) {
    function best(cands, ts) {
        var r = null
        for (var i = 0; i < cands.length; i++)
            for (var j = 0; j < ts.length; j++) {
                var d = ts[j] - cands[i]
                if (Math.abs(d) <= tol && (r === null || Math.abs(d) < Math.abs(r.d))) r = { d: d, at: ts[j] }
            }
        return r
    }
    var bx = best([e.x, e.x + e.w / 2, e.x + e.w], xs), by = best([e.y, e.y + e.h / 2, e.y + e.h], ys)
    return { dx: bx ? bx.d : 0, dy: by ? by.d : 0, gx: bx ? bx.at : null, gy: by ? by.at : null }
}

function whole(r) {
    var n = copy(r)
    var x2 = Math.round(r.x + r.w), y2 = Math.round(r.y + r.h)
    n.x = Math.round(r.x); n.y = Math.round(r.y); n.w = x2 - n.x; n.h = y2 - n.y
    return n
}

function clampRect(r, w, h) { return clampTo(r, 0, 0, w, h) }
function clampTo(r, x0, y0, x1, y1) {
    var a = Math.max(x0, Math.min(x1, r.x)), b = Math.max(y0, Math.min(y1, r.y))
    var c = Math.max(x0, Math.min(x1, r.x + r.w)), d = Math.max(y0, Math.min(y1, r.y + r.h))
    return whole({ x: a, y: b, w: c - a, h: d - b })
}

// Crop & Resize (ANN-11). Ratios are width / height; 0 is free, -1 the image's own.
var cropRatios = [{ label: "Freeform", r: 0 }, { label: "Original Ratio", r: -1 }, { label: "1:1 (Square)", r: 1 },
                  { label: "4:3", r: 4 / 3 }, { label: "3:2", r: 3 / 2 }, { label: "16:9", r: 16 / 9 },
                  { label: "5:4", r: 5 / 4 }, { label: "9:16", r: 9 / 16 }]

// A crop kept at `ratio` while handle `h` moves (the opposite side stays put).
function withRatio(r, ratio, h) {
    if (!ratio || r.w <= 0 || r.h <= 0) return r
    var n = copy(r)
    if (h === "t" || h === "b") { var cx = r.x + r.w / 2; n.w = r.h * ratio; n.x = cx - n.w / 2 }
    else if (h === "l" || h === "r") { var cy = r.y + r.h / 2; n.h = r.w / ratio; n.y = cy - n.h / 2 }
    else { n.h = r.w / ratio; if (h === "tl" || h === "tr") n.y = r.y + r.h - n.h }
    return n
}

// The largest crop at `ratio` inside r, centred.
function fitRatio(r, ratio) {
    if (!ratio) return r
    var w = r.w, h = r.h
    if (w / h > ratio) w = h * ratio; else h = w / ratio
    return { x: r.x + (r.w - w) / 2, y: r.y + (r.h - h) / 2, w: w, h: h }
}

// Rotate, flip or resize a whole document with its image (ANN-11): every
// object's geometry follows; text stays upright. W, H: the image before;
// nw, nh: after a resize.
function transformDoc(doc, op, W, H, nw, nh) {
    var sx = op === "resize" ? nw / W : 1, sy = op === "resize" ? nh / H : 1, sc = (sx + sy) / 2
    var pt = function (x, y) {
        if (op === "rotate-right") return [H - y, x]
        if (op === "rotate-left") return [y, W - x]
        if (op === "flip-horizontal") return [W - x, y]
        if (op === "flip-vertical") return [x, H - y]
        return [x * sx, y * sy]
    }
    var box = function (r) {
        var a = pt(r.x, r.y), b = pt(r.x + r.w, r.y + r.h), n = normRect(a[0], a[1], b[0], b[1])
        return { x: n.x, y: n.y, w: n.w, h: n.h }
    }
    var items = doc.items.map(function (o) {
        var n = copy(o), a
        if (o.kind === "arrow" || o.kind === "line") {
            a = pt(o.x1, o.y1); n.x1 = a[0]; n.y1 = a[1]
            a = pt(o.x2, o.y2); n.x2 = a[0]; n.y2 = a[1]
            if (o.kind === "arrow") { a = pt(o.cx, o.cy); n.cx = a[0]; n.cy = a[1] }
        } else if (o.kind === "pencil") {
            n.pts = o.pts.map(function (q) { return pt(q[0], q[1]) })
        } else if (o.kind === "counter" || o.kind === "text") {
            a = pt(o.x, o.y); n.x = a[0]; n.y = a[1]
            if (o.w) n.w = o.w * sc
        } else {
            var b = box(o); n.x = b.x; n.y = b.y; n.w = b.w; n.h = b.h
        }
        if (n.size !== undefined) n.size = o.size * sc
        if (o.kind === "redact") n = whole(n)
        return n
    })
    var out = Object.assign({}, doc, { items: items })
    if (doc.crop) out.crop = whole(box(doc.crop))
    return out
}

// Edges within `d` of the image's edges snap onto them.
function snapEdges(r, iw, ih, d) {
    var x1 = r.x, y1 = r.y, x2 = r.x + r.w, y2 = r.y + r.h
    var snap = function (v, t) { return Math.abs(v - t) <= d ? t : v }
    x1 = snap(snap(x1, 0), iw); x2 = snap(snap(x2, 0), iw); y1 = snap(snap(y1, 0), ih); y2 = snap(snap(y2, 0), ih)
    return { x: x1, y: y1, w: x2 - x1, h: y2 - y1 }
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
    if (o.kind === "pencil") {
        for (var k = 1; k < o.pts.length; k++)
            if (segmentDist(x, y, o.pts[k - 1][0], o.pts[k - 1][1], o.pts[k][0], o.pts[k][1]) <= o.size + tol) return true
        return o.pts.length === 1 && dist(x, y, o.pts[0][0], o.pts[0][1]) <= o.size + tol
    }
    if (o.kind === "counter")
        return dist(x, y, o.x, o.y) <= o.size / 2 + tol
    if (o.kind === "redact" || o.kind === "spotlight" || o.kind === "highlight" || o.kind === "image")
        return x >= o.x - tol && x <= o.x + o.w + tol && y >= o.y - tol && y <= o.y + o.h + tol
    if (o.kind === "text" && box)
        return x >= box.x - tol && x <= box.x + box.width + tol && y >= box.y - tol && y <= box.y + box.height + tol
    return false
}

// Pencil: drop points that add nothing (Ramer–Douglas–Peucker), then draw
// smooth curves through the midpoints, so a shaky hand gives a calm line.
function simplify(pts, eps) {
    if (pts.length < 3) return pts
    var a = pts[0], b = pts[pts.length - 1], far = 0, at = 0
    for (var i = 1; i < pts.length - 1; i++) {
        var d = segmentDist(pts[i][0], pts[i][1], a[0], a[1], b[0], b[1])
        if (d > far) { far = d; at = i }
    }
    if (far <= eps) return [a, b]
    return simplify(pts.slice(0, at + 1), eps).slice(0, -1).concat(simplify(pts.slice(at), eps))
}
function pencilPath(o) {
    var p = o.pts
    if (!p.length) return ""
    if (p.length < 3) return "M " + p[0][0] + " " + p[0][1] + " L " + p[p.length - 1][0] + " " + p[p.length - 1][1]
    var path = "M " + p[0][0] + " " + p[0][1] + " L " + (p[0][0] + p[1][0]) / 2 + " " + (p[0][1] + p[1][1]) / 2
    for (var i = 1; i < p.length - 1; i++)
        path += " Q " + p[i][0] + " " + p[i][1] + " " + (p[i][0] + p[i + 1][0]) / 2 + " " + (p[i][1] + p[i + 1][1]) / 2
    return path + " L " + p[p.length - 1][0] + " " + p[p.length - 1][1]
}

// A drag shorter than this (image px) makes no object.
function tooSmall(o) {
    if (o.kind === "pencil") return o.pts.length < 2
    if (o.kind === "arrow") return dist(o.x1, o.y1, o.x2, o.y2) < o.size * 2
    if (o.kind === "line") return dist(o.x1, o.y1, o.x2, o.y2) < o.size * 2
    if (isBox(o)) return o.w < 3 || o.h < 3
    return false
}

var counterStyles = [{ id: "num", label: "1, 2, 3", short: "123" }, { id: "alpha", label: "A, B, C", short: "ABC" },
                     { id: "roman", label: "I, II, III", short: "I II" }]

function roman(n) {
    var out = "", vals = [[1000, "M"], [900, "CM"], [500, "D"], [400, "CD"], [100, "C"], [90, "XC"],
                          [50, "L"], [40, "XL"], [10, "X"], [9, "IX"], [5, "V"], [4, "IV"], [1, "I"]]
    for (var i = 0; i < vals.length; i++) while (n >= vals[i][0]) { out += vals[i][1]; n -= vals[i][0] }
    return out
}
function alpha(n) {
    var out = ""
    for (; n > 0; n = Math.floor((n - 1) / 26)) out = String.fromCharCode(65 + (n - 1) % 26) + out
    return out
}
// The label of the n-th counter (1-based) in a document.
function counterLabel(doc, n, style) {
    var v = (doc.counterStart === undefined ? 1 : doc.counterStart) + n - 1
    if (style === "alpha") return alpha(Math.max(1, v))
    if (style === "roman") return roman(Math.max(1, v))
    return String(v)
}

// Each style counts on its own: 1, 2 and A, B can share a screenshot.
// `style` is for a counter not in the document yet (index = items.length).
function counterNumber(items, index, style) {
    var st = index < items.length ? (items[index].style || "num") : (style || "num"), n = index < items.length ? 0 : 1
    for (var i = 0; i <= Math.min(index, items.length - 1); i++)
        if (items[i].kind === "counter" && (items[i].style || "num") === st) n++
    return n
}

var redactStyles = [{ id: "pixelate", label: "Pixelate" }, { id: "text", label: "Text shape" },
                    { id: "blur", label: "Blur" }, { id: "black", label: "Black out" },
                    { id: "natural", label: "Natural blur", note: "Not secure" }]
// the tiles' style number for image://snip-redact
function redactIndex(style) { return style === "blur" ? 1 : style === "text" ? 2 : style === "natural" ? 3 : 0 }
// intensity: how much bigger the blocks (or the blur) are than usual
var redactLevels = [{ v: 0.6, label: "Low" }, { v: 1, label: "Medium" }, { v: 1.6, label: "High" }, { v: 2.4, label: "Maximum" }]

// Crop handles: corners and edge middles.
function cropHandles(r) {
    var cx = r.x + r.w / 2, cy = r.y + r.h / 2
    return [{ name: "tl", x: r.x, y: r.y }, { name: "t", x: cx, y: r.y }, { name: "tr", x: r.x + r.w, y: r.y },
            { name: "r", x: r.x + r.w, y: cy }, { name: "br", x: r.x + r.w, y: r.y + r.h }, { name: "b", x: cx, y: r.y + r.h },
            { name: "bl", x: r.x, y: r.y + r.h }, { name: "l", x: r.x, y: cy }]
}

// ---------------------------------------------------------------- Background tool (BG-01…06)
//
// doc.bg = { kind: gradient | wallpaper | blur-wallpaper | blur-image | color,
//            value: gradient index | file path | colour,
//            padding, inset, radius (image px), insetColor, shadow (0–1),
//            ratio (0 auto, else width / height), align (tl t tr l c r bl b br) }

// Twenty gradients in CleanShot's spirit; the first two follow the theme.
var gradients = [
    ["theme-a", "theme-b"], ["theme-b", "theme-a"],
    ["#ff9a8b", "#ff6a88", "#ff99ac"], ["#fbc2eb", "#a6c1ee"], ["#84fab0", "#8fd3f4"], ["#a1c4fd", "#c2e9fb"],
    ["#f6d365", "#fda085"], ["#fccb90", "#d57eeb"], ["#e0c3fc", "#8ec5fc"], ["#43e97b", "#38f9d7"],
    ["#fa709a", "#fee140"], ["#30cfd0", "#330867"], ["#667eea", "#764ba2"], ["#ff0844", "#ffb199"],
    ["#4facfe", "#00f2fe"], ["#0ba360", "#3cba92"], ["#13547a", "#80d0c7"], ["#232526", "#414345"],
    ["#ee9ca7", "#ffdde1"], ["#c471f5", "#fa71cd"]
]
// CleanShot's list: landscape, then portrait
var bgRatios = [{ label: "Auto", r: 0 }, { label: "1:1", r: 1 }, { label: "5:4", r: 5 / 4 }, { label: "4:3", r: 4 / 3 },
                { label: "3:2", r: 3 / 2 }, { label: "16:9", r: 16 / 9 }, { label: "4:5", r: 4 / 5 }, { label: "3:4", r: 3 / 4 },
                { label: "9:16", r: 9 / 16 }]

function defaultBackground(unit) {
    return { kind: "gradient", value: 0, padding: Math.round(64 * unit), inset: 0, insetColor: "#59ffffff",
             radius: Math.round(10 * unit), shadow: 0.5, ratio: 0, align: "c" }
}

// The background's rectangle around `inner` (the image or its crop), in image px.
function bgRect(bg, inner) {
    var pad = bg.padding + bg.inset, cw = inner.w + 2 * pad, ch = inner.h + 2 * pad
    if (bg.ratio) { if (cw / ch < bg.ratio) cw = ch * bg.ratio; else ch = cw / bg.ratio }
    var fx = cw - inner.w, fy = ch - inner.h, a = bg.align || "c"
    var ox = a.indexOf("l") >= 0 ? pad : a.indexOf("r") >= 0 ? fx - pad : fx / 2
    var oy = a.indexOf("t") >= 0 ? pad : a.indexOf("b") >= 0 ? fy - pad : fy / 2
    return whole({ x: inner.x - ox, y: inner.y - oy, w: cw, h: ch })
}
