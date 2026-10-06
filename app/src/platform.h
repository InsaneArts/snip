#pragma once

#include <QObject>
#include <QQuickItem>
#include <QScreen>
#include <QVariantList>
#include <QQuickWindow>
#include <QWindow>
#include <QtQml/qqmlregistration.h>

// Small platform services QML can't do itself.
class Platform : public QObject
{
  Q_OBJECT
  QML_ELEMENT
  QML_SINGLETON

public:
  using QObject::QObject;

  // The real QScreen for an output name. LayerShellQt's `screen` needs a
  // QScreen*, not QML's ScreenInfo wrapper (spike S01: every overlay landed
  // on the first monitor otherwise).
  Q_INVOKABLE QScreen* screen(const QString& name) const;

  // Drag a file out (QAO-04): text/uri-list + the portal file-transfer type
  // for Flatpak targets, never raw image/png (Nautilus would save it
  // nameless), COPY only. Returns "copy" or "ignore".
  Q_INVOKABLE QString startDrag(QQuickItem* source, const QString& path, const QString& thumbnail);

  // Input region = union of `rects` (logical, window coordinates); clicks
  // elsewhere fall through to the windows below.
  Q_INVOKABLE void setInputRects(QWindow* w, const QVariantList& rects);

  // Position a layer-shell window at logical (x, y) of its output (margins).
  Q_INVOKABLE void placeLayer(QWindow* w, int x, int y);
  Q_INVOKABLE void setMargins(QWindow* w, int left, int top, int right, int bottom);

  Q_INVOKABLE QString hex(const QColor& c) const;

  // Emit framePresented(w) once a frame that reflects the scene as it is
  // *now* has been swapped (committed to the compositor). Used to take
  // Snip's own UI off screen before a capture.
  Q_INVOKABLE void notifyNextFrame(QQuickWindow* w);

Q_SIGNALS:
  void framePresented(QQuickWindow* window);
};
