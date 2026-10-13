#include "platform.h"

#include <KUrlMimeData>
#include <LayerShellQt/window.h>
#include <QDBusArgument>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusUnixFileDescriptor>
#include <QDir>
#include <QDrag>
#include <QFile>
#include <QGuiApplication>
#include <QImage>
#include <QImageReader>
#include <QKeyEvent>
#include <QKeySequence>
#include <QMimeData>
#include <QPageLayout>
#include <QPainter>
#include <QPdfWriter>
#include <QPixmap>
#include <QQmlEngine>
#include <QQuickItemGrabResult>
#include <QQuickWindow>
#include <QRegion>
#include <QTimer>
#include <QUrl>
#include <atomic>
#include <malloc.h>
#include <cmath>
#include <functional>
#include <memory>

#include "snip_bridge/src/bridge.cxxqt.h"
#include "tray.h"

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

// `then`, on ctx's thread, once `w` has swapped a frame that reflects its
// scene as it is now.
static void
onNextFrame(QObject* ctx, QQuickWindow* w, const std::function<void()>& then)
{
  // beforeSynchronizing runs with the GUI thread blocked, so a sync seen
  // after this call carries the current state; its frameSwapped is ours.
  auto synced = std::make_shared<std::atomic<bool>>(false);
  auto conns = std::make_shared<QList<QMetaObject::Connection>>();
  conns->append(QObject::connect(
    w, &QQuickWindow::beforeSynchronizing, ctx, [synced]() { synced->store(true); }, Qt::DirectConnection));
  conns->append(QObject::connect(
    w,
    &QQuickWindow::frameSwapped,
    ctx,
    [ctx, synced, conns, then]() {
      if (!synced->load())
        return;
      for (const auto& c : *conns)
        QObject::disconnect(c);
      QMetaObject::invokeMethod(ctx, then, Qt::QueuedConnection);
    },
    Qt::DirectConnection));
  w->update();
}

void
Platform::notifyNextFrame(QQuickWindow* w)
{
  if (!w)
    return;
  onNextFrame(this, w, [this, w]() { Q_EMIT framePresented(w); });
}

QString
Platform::hex(const QColor& c) const
{
  return c.name(QColor::HexRgb).toUpper();
}

// An answer, back to the JS function that asked (a request answers once).
static void
answer(QJSValue done, const QJSValueList& args)
{
  if (!done.isCallable())
    return;
  const QJSValue r = done.call(args);
  if (r.isError())
    qWarning("snip: an answer's callback failed: %s", qPrintable(r.toString()));
}

void
Platform::trimMemory()
{
  malloc_trim(0);
}

void
Platform::afterFrames(const QVariantList& windows, const QJSValue& done)
{
  // answered once: by the last window's frame, or by the timeout. `done`
  // stays here: a frame's lambdas are copied and freed on the render thread,
  // and a QJSValue copied or freed there corrupts the JS engine's own store
  // of them (snipd crashed in a later binding's PersistentValue)
  const int id = ++m_waits;
  m_waiting.insert(id, done);
  auto left = std::make_shared<int>(0);
  auto finish = [this, id]() { answer(m_waiting.take(id), {}); };
  for (const QVariant& v : windows) {
    auto* w = qobject_cast<QQuickWindow*>(v.value<QObject*>());
    if (!w || !w->isVisible())
      continue;
    ++*left;
    onNextFrame(this, w, [left, finish]() {
      if (--*left == 0)
        finish();
    });
  }
  if (*left == 0) {
    finish();
    return;
  }
  // a window that stopped drawing never swaps; a slow compositor (seen at
  // 15 fps, nested) still swaps well within this
  QTimer::singleShot(250, this, finish);
}

void
Platform::grabExact(QQuickItem* item, int width, int height, const QJSValue& done)
{
  // the engine finds the pixels by it; Snip's own count, so a token is never reused
  const int token = ++m_grabs;
  QQuickWindow* win = item ? item->window() : nullptr;
  if (!win || width < 1 || height < 1) {
    answer(done, { QJSValue(token), QJSValue(false) });
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
    answer(done, { QJSValue(token), QJSValue(false) });
    return;
  }
  connect(result.data(), &QQuickItemGrabResult::ready, this, [result, width, height, token, done]() {
    QImage img = result->image();
    if (img.width() < width || img.height() < height) {
      answer(done, { QJSValue(token), QJSValue(false) });
      return;
    }
    img = img.copy(0, 0, width, height).convertToFormat(QImage::Format_RGBA8888);
    snip::edit_put(uint32_t(token),
                   rust::Slice<const uint8_t>(img.constBits(), size_t(img.sizeInBytes())),
                   uint32_t(width),
                   uint32_t(height),
                   uint32_t(img.bytesPerLine()));
    answer(done, { QJSValue(token), QJSValue(true) });
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

// Tests (SNIP_NO_PORTALS): what the next dialog answers; empty is cancelled
static QString testAnswer;

static void
portalFileRequest(Platform* self, const QString& method, const QString& title, const QVariantMap& extra, const QJSValue& done)
{
  if (qEnvironmentVariableIsSet("SNIP_NO_PORTALS")) {
    const QString path = testAnswer;
    testAnswer.clear();
    QMetaObject::invokeMethod(self, [done, path] { answer(done, { QJSValue(path) }); }, Qt::QueuedConnection);
    return;
  }
  static int requests = 0;
  QDBusConnection bus = QDBusConnection::sessionBus();
  const QString service = QStringLiteral("org.freedesktop.portal.Desktop");
  const QString handle = QStringLiteral("snip%1").arg(++requests);
  // Listen on the request path before asking, so the answer can't be missed.
  QString sender = bus.baseService().mid(1);
  sender.replace(QLatin1Char('.'), QLatin1Char('_'));
  auto* req = new PortalRequest(self);
  req->path = QStringLiteral("/org/freedesktop/portal/desktop/request/%1/%2").arg(sender, handle);
  bus.connect(service, req->path, QStringLiteral("org.freedesktop.portal.Request"), QStringLiteral("Response"), req,
              SLOT(response(uint, QVariantMap)));
  QObject::connect(req, &PortalRequest::done, self, [req, done](uint code, const QVariantMap& results) {
    const QStringList uris = results.value(QStringLiteral("uris")).toStringList();
    const QString path = code == 0 && !uris.isEmpty() ? QUrl(uris.first()).toLocalFile() : QString();
    req->deleteLater();
    answer(done, { QJSValue(path) });
  });

  QVariantMap opts = extra;
  opts[QStringLiteral("handle_token")] = handle;
  opts[QStringLiteral("modal")] = true;
  QDBusMessage call = QDBusMessage::createMethodCall(service, QStringLiteral("/org/freedesktop/portal/desktop"),
                                                     QStringLiteral("org.freedesktop.portal.FileChooser"), method);
  call << QString() << title << opts;
  const QDBusMessage reply = bus.call(call, QDBus::Block, 5000);
  if (reply.type() == QDBusMessage::ErrorMessage) {
    req->deleteLater();
    answer(done, { QJSValue(QString()) });
  }
}

void
Platform::saveFileDialog(const QString& title, const QString& name, const QString& folder, const QJSValue& done)
{
  QVariantMap opts;
  opts[QStringLiteral("current_name")] = name;
  QByteArray dir = QFile::encodeName(folder);
  dir.append('\0');
  opts[QStringLiteral("current_folder")] = dir;
  portalFileRequest(this, QStringLiteral("SaveFile"), title, opts, done);
}

// The image on one page: as large as fits, but never past 144 dpi (about its
// size on screen), centred at the top. Sizes in mm, as the portal gives them.
static bool
writePrintPdf(const QString& image, const QString& out, const QVariantMap& page)
{
  const QImage img(image);
  if (img.isNull())
    return false;
  const auto mm = [&page](const char* key, double fallback) { return page.value(QLatin1String(key), fallback).toDouble(); };
  const bool landscape = page.value(QStringLiteral("Orientation")).toString().contains(QLatin1String("landscape"));
  QPdfWriter pdf(out);
  pdf.setCreator(QStringLiteral("Snip"));
  pdf.setResolution(300);
  pdf.setPageLayout(QPageLayout(QPageSize(QSizeF(mm("Width", 210), mm("Height", 297)), QPageSize::Millimeter),
                                landscape ? QPageLayout::Landscape : QPageLayout::Portrait,
                                QMarginsF(mm("MarginLeft", 10), mm("MarginTop", 10), mm("MarginRight", 10), mm("MarginBottom", 10)),
                                QPageLayout::Millimeter));
  QPainter p(&pdf);
  const double s = std::min({ pdf.width() / double(img.width()), pdf.height() / double(img.height()), 300.0 / 144.0 });
  p.setRenderHint(QPainter::SmoothPixmapTransform);
  p.drawImage(QRectF((pdf.width() - img.width() * s) / 2, 0, img.width() * s, img.height() * s), img);
  return p.end();
}

static QVariantMap
dbusMap(const QVariant& v)
{
  return v.canConvert<QDBusArgument>() ? qdbus_cast<QVariantMap>(v.value<QDBusArgument>()) : v.toMap();
}

// One portal call that answers through a Request object: `done` gets its
// Response (code 2 when the call itself failed).
static void
portalCall(Platform* self, const QString& iface, const QString& method, QVariantList args, QVariantMap opts,
           std::function<void(uint, const QVariantMap&)> done)
{
  static int n = 0;
  QDBusConnection bus = QDBusConnection::sessionBus();
  const QString service = QStringLiteral("org.freedesktop.portal.Desktop");
  const QString handle = QStringLiteral("snipcall%1").arg(++n);
  QString sender = bus.baseService().mid(1);
  sender.replace(QLatin1Char('.'), QLatin1Char('_'));
  auto* req = new PortalRequest(self);
  req->path = QStringLiteral("/org/freedesktop/portal/desktop/request/%1/%2").arg(sender, handle);
  bus.connect(service, req->path, QStringLiteral("org.freedesktop.portal.Request"), QStringLiteral("Response"), req,
              SLOT(response(uint, QVariantMap)));
  QObject::connect(req, &PortalRequest::done, self, [req, done](uint code, const QVariantMap& results) {
    req->deleteLater();
    done(code, results);
  });
  opts[QStringLiteral("handle_token")] = handle;
  opts[QStringLiteral("modal")] = true;
  QDBusMessage call = QDBusMessage::createMethodCall(service, QStringLiteral("/org/freedesktop/portal/desktop"), iface, method);
  args << opts;
  call.setArguments(args);
  if (bus.call(call, QDBus::Block, 5000).type() == QDBusMessage::ErrorMessage)
    Q_EMIT req->done(2, {});
}

QString
Platform::printImage(const QString& path, const QString& title)
{
  static int n = 0;
  const QString pdf = QStringLiteral("%1/snip-print-%2-%3.pdf").arg(QDir::tempPath()).arg(QCoreApplication::applicationPid()).arg(++n);
  if (qEnvironmentVariableIsSet("SNIP_NO_PORTALS"))
    return writePrintPdf(path, pdf, {}) ? pdf : QString();
  const QString iface = QStringLiteral("org.freedesktop.portal.Print");
  portalCall(this, iface, QStringLiteral("PreparePrint"), { QString(), title, QVariantMap(), QVariantMap() }, {},
             [this, iface, path, pdf, title](uint code, const QVariantMap& r) {
               if (code != 0 || !writePrintPdf(path, pdf, dbusMap(r.value(QStringLiteral("page-setup")))))
                 return;
               QFile f(pdf);
               if (!f.open(QIODevice::ReadOnly))
                 return;
               // the message holds its own copy of the descriptor: the file can go
               const QDBusUnixFileDescriptor fd(f.handle());
               f.remove();
               QVariantMap opts;
               opts[QStringLiteral("token")] = r.value(QStringLiteral("token"));
               portalCall(this, iface, QStringLiteral("Print"), { QString(), title, QVariant::fromValue(fd) }, opts,
                          [](uint, const QVariantMap&) {});
             });
  return QString();
}

bool
Platform::altHeld() const
{
  return QGuiApplication::queryKeyboardModifiers() & Qt::AltModifier;
}

void
Platform::openWith(const QString& path)
{
  if (qEnvironmentVariableIsSet("SNIP_NO_PORTALS"))
    return;
  QFile f(path);
  if (!f.open(QIODevice::ReadOnly))
    return;
  QVariantMap opts;
  opts[QStringLiteral("ask")] = true;
  QDBusMessage call = QDBusMessage::createMethodCall(QStringLiteral("org.freedesktop.portal.Desktop"),
                                                     QStringLiteral("/org/freedesktop/portal/desktop"),
                                                     QStringLiteral("org.freedesktop.portal.OpenURI"),
                                                     QStringLiteral("OpenFile"));
  // the descriptor is duplicated into the message, so the file can close
  call << QString() << QVariant::fromValue(QDBusUnixFileDescriptor(f.handle())) << opts;
  QDBusConnection::sessionBus().call(call, QDBus::NoBlock);
}

void
Platform::openImageDialog(const QString& title, const QJSValue& done)
{
  QVariantMap opts;
  opts[QStringLiteral("multiple")] = false;
  portalFileRequest(this, QStringLiteral("OpenFile"), title, opts, done);
}

void
Platform::openFolderDialog(const QString& title, const QString& folder, const QJSValue& done)
{
  QVariantMap opts;
  opts[QStringLiteral("directory")] = true;
  QByteArray dir = QFile::encodeName(folder);
  dir.append('\0');
  opts[QStringLiteral("current_folder")] = dir;
  portalFileRequest(this, QStringLiteral("OpenFile"), title, opts, done);
}

QSize
Platform::imageSize(const QString& path) const
{
  QImageReader r(path);
  return r.size().isValid() ? r.size() : QSize(0, 0);
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

void
Platform::answerNextFileDialog(const QString& path)
{
  testAnswer = path;
}

void
Platform::showTray(const QString& iconDir)
{
  if (qEnvironmentVariableIsSet("SNIP_NO_TRAY") || findChild<Tray*>())
    return;
  auto* tray = new Tray(iconDir, this);
  connect(tray, &Tray::activated, this, &Platform::trayActivated);
  connect(tray, &Tray::menuRequested, this, &Platform::trayMenuRequested);
}

void
Platform::setTrayRecording(bool on)
{
  if (auto* tray = findChild<Tray*>())
    tray->setRecording(on);
}
