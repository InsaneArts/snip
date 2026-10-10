.pragma library
// Tests' probe into a window: its controls by objectName, and where they are.

// The shown item named `name` under `item`, or null.
function find(item, name) {
    if (item.objectName === name && item.visible) return item
    for (const c of item.children) {
        const f = c.visible ? find(c, name) : null
        if (f) return f
    }
    return null
}

// Where control `name` is in `win`: its centre in the window (logical px),
// laid out first and, in `flick` when given, scrolled into view as a hand
// would; x and y are -1 when it isn't shown.
function where(win, name, flick) {
    const it = find(win.contentItem, name)
    // laid out first: just after a change, rows and columns still hold old places
    const layOut = () => { for (let q = it; q; q = q.parent) if (q.forceLayout) q.forceLayout() }
    layOut()
    if (it && flick) {
        const top = it.mapToItem(flick.contentItem, 0, 0).y
        if (top < flick.contentY || top + it.height > flick.contentY + flick.height)
            flick.contentY = Math.max(0, Math.min(flick.contentHeight - flick.height, top - flick.height / 2))
        layOut()
    }
    const p = it ? it.mapToItem(win.contentItem, it.width / 2, it.height / 2) : null
    return { name: name, x: p ? p.x : -1, y: p ? p.y : -1 }
}
