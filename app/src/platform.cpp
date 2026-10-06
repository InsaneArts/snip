#include "platform.h"

#include <KUrlMimeData>
#include <LayerShellQt/window.h>
#include <QDrag>
#include <QGuiApplication>
#include <QImage>
#include <QMimeData>
#include <QPixmap>
#include <QQmlEngine>
#include <QQuickWindow>
#include <QRegion>
#include <QUrl>

#include "snip_bridge/src/bridge.cxxqt.h"

QScreen*
Platform::screen(const QString& name) const
{
  for (QScreen* s : QGuiApplication::screens()) {
    if (s->name() == name) {
      // Returned to QML without a parent, a QObject becomes JS-owned and the
      // GC would deleteLater() Qt's real QScreen (spike S17 segfault).
      QQmlEngine::setObjectOwnership(s, QQmlEngine::CppOwnership);
      return s;
    }
  }
  return nullptr;
}

QString
Platform::startDrag(QQuickItem* source, const QString& path, const QString& thumbnail)
{
  if (path.isEmpty())
    return QStringLiteral("ignore");
  auto* mime = new QMimeData;
  mime->setUrls({ QUrl::fromLocalFile(path) });
  KUrlMimeData::exportUrlsToPortal(mime);

  auto* drag = new QDrag(source);
  drag->setMimeData(mime);
  const QImage thumb(thumbnail.isEmpty() ? path : thumbnail);
  if (!thumb.isNull()) {
    const qreal dpr = source && source->window() ? source->window()->devicePixelRatio() : 1.0;
    QImage t = thumb.scaledToWidth(int(180 * dpr), Qt::SmoothTransformation);
    t.setDevicePixelRatio(dpr);
    drag->setPixmap(QPixmap::fromImage(t));
    drag->setHotSpot(QPoint(int(t.width() / dpr / 2), int(t.height() / dpr / 2)));
  }
  // COPY only: Hyprland picks MOVE whenever the source allows it, and a file
  // manager would then move the capture away.
  const Qt::DropAction r = drag->exec(Qt::CopyAction, Qt::CopyAction);
  const QString res = r == Qt::CopyAction ? QStringLiteral("copy") : QStringLiteral("ignore");
  snip::mark_time(QStringLiteral("drag_end"), QStringLiteral("\"result\":\"%1\"").arg(res));
  return res;
}

void
Platform::setInputRects(QWindow* w, const QVariantList& rects)
{
  if (!w)
    return;
  QRegion region;
  for (const QVariant& v : rects) {
    const QRectF r = v.toRectF();
    if (r.width() > 0 && r.height() > 0)
      region += r.toAlignedRect();
  }
  // An empty QRegion means "no mask" (all input) to Qt, so keep a 1 px
  // corner instead; the window is hidden whenever nothing is shown anyway.
  if (region.isEmpty())
    region = QRegion(0, 0, 1, 1);
  w->setMask(region);
}

void
Platform::placeLayer(QWindow* w, int x, int y)
{
  if (auto* lw = LayerShellQt::Window::get(w))
    lw->setMargins(QMargins(x, y, 0, 0));
}

void
Platform::setMargins(QWindow* w, int left, int top, int right, int bottom)
{
  if (auto* lw = LayerShellQt::Window::get(w))
    lw->setMargins(QMargins(left, top, right, bottom));
}

QString
Platform::hex(const QColor& c) const
{
  return c.name(QColor::HexRgb).toUpper();
}
