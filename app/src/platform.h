#pragma once

#include <QObject>
#include <QQuickItem>
#include <QScreen>
#include <QSize>
#include <QVariantList>
#include <QQuickWindow>
#include <QWindow>
#include <QtQml/qqmlregistration.h>

// One pending xdg-desktop-portal request: forwards its Response.
class PortalRequest : public QObject
{
  Q_OBJECT
public:
  using QObject::QObject;
  QString path;
Q_SIGNALS:
  void done(uint code, const QVariantMap& results);
public Q_SLOTS:
  void response(uint code, const QVariantMap& results) { Q_EMIT done(code, results); }
};

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
  // Back to the whole window taking input.
  Q_INVOKABLE void clearInputRects(QWindow* w);

  // Position a layer-shell window at logical (x, y) of its output (margins).
  Q_INVOKABLE void placeLayer(QWindow* w, int x, int y);
  Q_INVOKABLE void setMargins(QWindow* w, int left, int top, int right, int bottom);

  Q_INVOKABLE QString hex(const QColor& c) const;

  // Emit framePresented(w) once a frame that reflects the scene as it is
  // *now* has been swapped (committed to the compositor). Used to take
  // Snip's own UI off screen before a capture.
  Q_INVOKABLE void notifyNextFrame(QQuickWindow* w);

  // The editor's export (ANN-15): render `item` in its own coordinates
  // (its transform ignored) at exactly width x height pixels, one item unit
  // per pixel, and hand the pixels to the engine under `token`. Resizes
  // `item` to make that exact at a fractional scale. Emits grabbed().
  Q_INVOKABLE void grabExact(QQuickItem* item, int width, int height, int token);

  // Tests: deliver key events to `w` inside this process (never through the
  // seat, so nothing can reach another app). Plain characters type
  // themselves; {Return}, {Escape}, {Backspace}, {Delete}, {ctrl+z},
  // {ctrl+shift+z}, {ctrl+s} and the like name keys.
  Q_INVOKABLE void sendKeys(QWindow* w, const QString& keys);

  // Save As (QAO-09, ANN-15): the desktop's own save dialog, through the
  // xdg-desktop-portal FileChooser. Answers with fileChosen(token, path);
  // the path is empty when cancelled or when there is no portal.
  Q_INVOKABLE void saveFileDialog(int token, const QString& title, const QString& name, const QString& folder);
  // "Open With…" (CleanShot's share): the desktop's app chooser for a file,
  // through the OpenURI portal. Nothing happens with SNIP_NO_PORTALS set
  // (tests: no dialog on the user's screen).
  Q_INVOKABLE void openWith(const QString& path);
  // Add image (ANN-12): the desktop's open dialog for images. Answers with
  // fileChosen(token, path) like saveFileDialog.
  Q_INVOKABLE void openImageDialog(int token, const QString& title);
  // Settings › Export location: the desktop's folder chooser, starting in
  // `folder`. Answers with fileChosen(token, path) like saveFileDialog.
  Q_INVOKABLE void openFolderDialog(int token, const QString& title, const QString& folder);
  // An image file's size without decoding it ({width, height}; 0 x 0 if unreadable).
  Q_INVOKABLE QSize imageSize(const QString& path) const;
  // Tests: cancel every open dialog (portal Request.Close).
  Q_INVOKABLE void closeFileDialogs();

Q_SIGNALS:
  void fileChosen(int token, const QString& path);
  void framePresented(QQuickWindow* window);
  void grabbed(int token, bool ok);
};
