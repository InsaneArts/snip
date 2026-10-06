#pragma once

#include <QObject>
#include <QQuickItem>
#include <QString>

// Starts a Wayland drag of a stored capture with a configurable MIME set, so
// the S3 matrix can see which combination each receiver accepts.
//   uri     : text/uri-list only
//   png     : text/uri-list + image/png
//   portal  : text/uri-list + application/vnd.portal.filetransfer (Flatpak)
//   all     : uri + png + portal + x-special/gnome-copied-files + text/plain
class DragHelper : public QObject
{
  Q_OBJECT
public:
  using QObject::QObject;
  // Returns the drop action the target accepted ("copy", "move", "ignore").
  Q_INVOKABLE QString startDrag(QQuickItem* source, const QString& path, const QString& variant);
};
