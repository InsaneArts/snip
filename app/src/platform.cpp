#include "platform.h"

#include <KUrlMimeData>
#include <LayerShellQt/window.h>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDrag>
#include <QFile>
#include <QGuiApplication>
#include <QImage>
#include <QKeyEvent>
#include <QKeySequence>
#include <QMimeData>
#include <QPixmap>
#include <QQmlEngine>
#include <QQuickItemGrabResult>
#include <QQuickWindow>
#include <QRegion>
#include <QUrl>
#include <atomic>
#include <cmath>
#include <memory>

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
Platform::clearInputRects(QWindow* w)
{
  if (w)
    w->setMask(QRegion());
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

void
Platform::notifyNextFrame(QQuickWindow* w)
{
  if (!w)
    return;
  // beforeSynchronizing runs with the GUI thread blocked, so a sync seen
  // after this call carries the current state; its frameSwapped is ours.
  auto synced = std::make_shared<std::atomic<bool>>(false);
  auto conns = std::make_shared<QList<QMetaObject::Connection>>();
  conns->append(connect(
    w, &QQuickWindow::beforeSynchronizing, this, [synced]() { synced->store(true); }, Qt::DirectConnection));
  conns->append(connect(
    w,
    &QQuickWindow::frameSwapped,
    this,
    [this, w, synced, conns]() {
      if (!synced->load())
        return;
      for (const auto& c : *conns)
        QObject::disconnect(c);
      QMetaObject::invokeMethod(this, [this, w]() { Q_EMIT framePresented(w); }, Qt::QueuedConnection);
    },
    Qt::DirectConnection));
  w->update();
}

QString
Platform::hex(const QColor& c) const
{
  return c.name(QColor::HexRgb).toUpper();
}

void
Platform::grabExact(QQuickItem* item, int width, int height, int token)
{
  QQuickWindow* win = item ? item->window() : nullptr;
  if (!win || width < 1 || height < 1) {
    Q_EMIT grabbed(token, false);
    return;
  }
  // Qt renders a grab of logical size s into round(s * dpr) texels, mapping
  // the item's rect onto them. Pick an s with at least `width` texels, then
  // make the item exactly that many units wide: one unit per texel.
  const qreal dpr = win->effectiveDevicePixelRatio();
  auto fit = [dpr](int px) {
    int s = std::max(1, int(std::ceil(px / dpr)));
    while (qRound(s * dpr) < px)
      ++s;
    return s;
  };
  const QSize logical(fit(width), fit(height));
  const QSize texels = logical * dpr;
  item->setSize(QSizeF(texels));
  QSharedPointer<QQuickItemGrabResult> result = item->grabToImage(logical);
  if (!result) {
    Q_EMIT grabbed(token, false);
    return;
  }
  connect(result.data(), &QQuickItemGrabResult::ready, this, [this, result, width, height, token]() {
    QImage img = result->image();
    if (img.width() < width || img.height() < height) {
      Q_EMIT grabbed(token, false);
      return;
    }
    img = img.copy(0, 0, width, height).convertToFormat(QImage::Format_RGBA8888);
    snip::edit_put(uint32_t(token),
                   rust::Slice<const uint8_t>(img.constBits(), size_t(img.sizeInBytes())),
                   uint32_t(width),
                   uint32_t(height),
                   uint32_t(img.bytesPerLine()));
    Q_EMIT grabbed(token, true);
  });
}

void
Platform::sendKeys(QWindow* w, const QString& keys)
{
  if (!w)
    return;
  auto send = [w](int key, Qt::KeyboardModifiers mods, const QString& text) {
    QKeyEvent press(QEvent::KeyPress, key, mods, text);
    QGuiApplication::sendEvent(w, &press);
    QKeyEvent release(QEvent::KeyRelease, key, mods, text);
    QGuiApplication::sendEvent(w, &release);
  };
  for (int i = 0; i < keys.size(); ++i) {
    if (keys[i] == QLatin1Char('{')) {
      const int end = keys.indexOf(QLatin1Char('}'), i);
      if (end > i) {
        // "ctrl+shift+z" -> a key sequence Qt can parse
        const QKeySequence seq = QKeySequence::fromString(keys.mid(i + 1, end - i - 1), QKeySequence::PortableText);
        if (!seq.isEmpty()) {
          const QKeyCombination k = seq[0];
          const bool plain = k.keyboardModifiers() == Qt::NoModifier;
          QString text;
          if (k.key() == Qt::Key_Return)
            text = QStringLiteral("\r");
          else if (plain && k.key() >= Qt::Key_Space && k.key() <= Qt::Key_AsciiTilde)
            text = QString(QChar(char16_t(k.key()))).toLower();
          send(k.key(), k.keyboardModifiers(), text);
        }
        i = end;
        continue;
      }
    }
    const QChar c = keys[i];
    const int key = c.isLetter() ? Qt::Key_A + (c.toUpper().unicode() - 'A') : c.unicode();
    send(key, c.isUpper() ? Qt::ShiftModifier : Qt::NoModifier, QString(c));
  }
}

void
Platform::saveFileDialog(int token, const QString& title, const QString& name, const QString& folder)
{
  QDBusConnection bus = QDBusConnection::sessionBus();
  const QString service = QStringLiteral("org.freedesktop.portal.Desktop");
  const QString handle = QStringLiteral("snip%1").arg(token);
  // Listen on the request path before asking, so the answer can't be missed.
  QString sender = bus.baseService().mid(1);
  sender.replace(QLatin1Char('.'), QLatin1Char('_'));
  auto* req = new PortalRequest(this);
  req->path = QStringLiteral("/org/freedesktop/portal/desktop/request/%1/%2").arg(sender, handle);
  bus.connect(service, req->path, QStringLiteral("org.freedesktop.portal.Request"), QStringLiteral("Response"), req,
              SLOT(response(uint, QVariantMap)));
  connect(req, &PortalRequest::done, this, [this, req, token](uint code, const QVariantMap& results) {
    const QStringList uris = results.value(QStringLiteral("uris")).toStringList();
    const QString path = code == 0 && !uris.isEmpty() ? QUrl(uris.first()).toLocalFile() : QString();
    req->deleteLater();
    Q_EMIT fileChosen(token, path);
  });

  QVariantMap opts;
  opts[QStringLiteral("handle_token")] = handle;
  opts[QStringLiteral("current_name")] = name;
  opts[QStringLiteral("modal")] = true;
  QByteArray dir = QFile::encodeName(folder);
  dir.append('\0');
  opts[QStringLiteral("current_folder")] = dir;
  QDBusMessage call = QDBusMessage::createMethodCall(service, QStringLiteral("/org/freedesktop/portal/desktop"),
                                                     QStringLiteral("org.freedesktop.portal.FileChooser"),
                                                     QStringLiteral("SaveFile"));
  call << QString() << title << opts;
  const QDBusMessage reply = bus.call(call, QDBus::Block, 5000);
  if (reply.type() == QDBusMessage::ErrorMessage) {
    req->deleteLater();
    Q_EMIT fileChosen(token, QString());
  }
}

void
Platform::closeFileDialogs()
{
  for (PortalRequest* req : findChildren<PortalRequest*>()) {
    QDBusMessage close = QDBusMessage::createMethodCall(QStringLiteral("org.freedesktop.portal.Desktop"), req->path,
                                                        QStringLiteral("org.freedesktop.portal.Request"),
                                                        QStringLiteral("Close"));
    QDBusConnection::sessionBus().call(close, QDBus::NoBlock);
    // Close ends the dialog without a Response
    Q_EMIT req->done(1, {});
  }
}
