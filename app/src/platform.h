#pragma once

#include <QHash>
#include <QJSValue>
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
  // *now* has been swapped (committed to the compositor).
  Q_INVOKABLE void notifyNextFrame(QQuickWindow* w);
  // Snip's surfaces step aside (D18: Snip is never in a capture): each
  // draws itself transparent, then done() once every visible window in
  // `windows` has swapped a frame of that, or after 250 ms if one draws
  // nothing. Hidden windows aren't waited for.
  Q_INVOKABLE void afterFrames(const QVariantList& windows, const QJSValue& done);
  // Hand freed memory back to the system (malloc_trim): after a big window
  // (an editor, the video editor) is gone, glibc keeps what its threads
  // freed in their arenas, and snipd's memory only ever grows.
  Q_INVOKABLE void trimMemory();

  // The editor's export (ANN-15): render `item` in its own coordinates
  // (its transform ignored) at exactly width x height pixels, one item unit
  // per pixel, and hand the pixels to the engine under a token. Resizes
  // `item` to make that exact at a fractional scale. Then done(token, ok):
  // the engine takes the pixels by that token.
  Q_INVOKABLE void grabExact(QQuickItem* item, int width, int height, const QJSValue& done);

  // Tests: deliver key events to `w` inside this process (never through the
  // seat, so nothing can reach another app). Plain characters type
  // themselves; {Return}, {Escape}, {Backspace}, {Delete}, {ctrl+z},
  // {ctrl+shift+z}, {ctrl+s} and the like name keys.
  Q_INVOKABLE void sendKeys(QWindow* w, const QString& keys);

  // QAO-03: Alt is held now (known while one of Snip's windows has the keyboard)
  Q_INVOKABLE bool altHeld() const;

  // Save As (QAO-09, ANN-15): the desktop's own save dialog, through the
  // xdg-desktop-portal FileChooser. Then done(path): empty when cancelled or
  // when there is no portal. With SNIP_NO_PORTALS set (tests) every file
  // dialog answers cancelled at once.
  Q_INVOKABLE void saveFileDialog(const QString& title, const QString& name, const QString& folder, const QJSValue& done);
  // "Open With…" (CleanShot's share): the desktop's app chooser for a file,
  // through the OpenURI portal. Nothing happens with SNIP_NO_PORTALS set
  // (tests: no dialog on the user's screen).
  Q_INVOKABLE void openWith(const QString& path);
  // Print… (QAO-09, ANN-15): the desktop's print dialog (the Print portal),
  // then the image as a PDF, fitted to the page chosen there. With
  // SNIP_NO_PORTALS set (tests) it only writes that PDF, for A4, and returns
  // its path; otherwise it returns "".
  Q_INVOKABLE QString printImage(const QString& path, const QString& title);
  // Add image (ANN-12): the desktop's open dialog for images. Then
  // done(path), like saveFileDialog.
  Q_INVOKABLE void openImageDialog(const QString& title, const QJSValue& done);
  // Settings › Export location: the desktop's folder chooser, starting in
  // `folder`. Then done(path), like saveFileDialog.
  Q_INVOKABLE void openFolderDialog(const QString& title, const QString& folder, const QJSValue& done);
  // An image file's size without decoding it ({width, height}; 0 x 0 if unreadable).
  Q_INVOKABLE QSize imageSize(const QString& path) const;
  // Tests: cancel every open dialog (portal Request.Close).
  Q_INVOKABLE void closeFileDialogs();
  // Tests (SNIP_NO_PORTALS): the next file dialog answers `path`, not cancelled.
  Q_INVOKABLE void answerNextFileDialog(const QString& path);
  // Snip in the bar (OMA-08): the tray icon, from snip-tray-symbolic.svg in
  // `iconDir`; trayActivated when it's clicked, trayMenuRequested when it's
  // right-clicked. Not with SNIP_NO_TRAY set (tests: no second icon in the
  // user's bar).
  Q_INVOKABLE void showTray(const QString& iconDir);
  // REC-06: the tray icon turns red while Snip records.
  Q_INVOKABLE void setTrayRecording(bool on);

Q_SIGNALS:
  void trayActivated(int x, int y);
  void trayMenuRequested(int x, int y);
  void framePresented(QQuickWindow* window);

private:
  // the engine's handle for a grab's pixels
  int m_grabs = 0;
  // afterFrames' callbacks, by request: only the GUI thread touches them
  QHash<int, QJSValue> m_waiting;
  int m_waits = 0;
};
