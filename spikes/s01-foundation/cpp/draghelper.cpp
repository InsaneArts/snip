#include "draghelper.h"

#include <KUrlMimeData>
#include <QDrag>
#include <QFile>
#include <QImage>
#include <QMimeData>
#include <QPixmap>
#include <QUrl>

#include "snip_spike/src/bridge.cxxqt.h"

QString
DragHelper::startDrag(QQuickItem* source, const QString& path, const QString& variant)
{
  auto* mime = new QMimeData;
  const QUrl url = QUrl::fromLocalFile(path);
  mime->setUrls({ url }); // text/uri-list

  if (variant == QLatin1String("png") || variant == QLatin1String("all")) {
    QFile f(path);
    if (f.open(QIODevice::ReadOnly))
      mime->setData(QStringLiteral("image/png"), f.readAll());
  }
  if (variant == QLatin1String("all")) {
    mime->setData(QStringLiteral("x-special/gnome-copied-files"), QByteArray("copy\n") + url.toEncoded());
    mime->setText(path);
  }
  if (variant == QLatin1String("utf8")) // GTK4 apps match the exact charset type
    mime->setData(QStringLiteral("text/plain;charset=utf-8"), path.toUtf8());
  bool portal = false;
  if (variant == QLatin1String("portal") || variant == QLatin1String("all"))
    portal = KUrlMimeData::exportUrlsToPortal(mime);

  auto* drag = new QDrag(source);
  drag->setMimeData(mime);
  const QImage thumb(path);
  if (!thumb.isNull())
    drag->setPixmap(QPixmap::fromImage(thumb.scaledToWidth(200, Qt::SmoothTransformation)));

  snip::mark_time(QStringLiteral("drag_start"),
                  QStringLiteral("\"variant\":\"%1\",\"formats\":\"%2\",\"portal\":%3")
                    .arg(variant, mime->formats().join(QLatin1Char(' ')), portal ? QStringLiteral("true") : QStringLiteral("false")));

  // COPY only: Hyprland picks MOVE whenever the source allows it, and a file
  // manager would then move the user's capture away.
  const Qt::DropAction result = drag->exec(Qt::CopyAction, Qt::CopyAction);
  const QString r = result == Qt::CopyAction   ? QStringLiteral("copy")
                    : result == Qt::MoveAction ? QStringLiteral("move")
                                               : QStringLiteral("ignore");
  snip::mark_time(QStringLiteral("drag_end"), QStringLiteral("\"variant\":\"%1\",\"result\":\"%2\"").arg(variant, r));
  return r;
}
