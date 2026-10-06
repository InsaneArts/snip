#include "frozenframe.h"

#include <QImage>
#include <QQuickWindow>
#include <QSGSimpleTextureNode>
#include <cmath>
#include <time.h>

#include "snip_spike/src/bridge.cxxqt.h"

namespace {
quint64
nowNs()
{
  timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return quint64(ts.tv_sec) * 1000000000ull + quint64(ts.tv_nsec);
}

void
mark(const QString& ev, const QString& extra = {})
{
  snip::mark_time(ev, extra);
}
} // namespace

FrozenFrame::FrozenFrame(QQuickItem* parent)
  : QQuickItem(parent)
{
  setFlag(ItemHasContents, true);
}

void
FrozenFrame::setOutput(const QString& o)
{
  if (o == m_output)
    return;
  m_output = o;
  Q_EMIT outputChanged();
  update();
}

void
FrozenFrame::setEpoch(int e)
{
  if (e == m_epoch)
    return;
  m_epoch = e;
  const snip::FrameInfo fi = snip::frame_info(m_output);
  if (fi.valid) {
    const QSize s(int(fi.width), int(fi.height));
    if (s != m_frameSize) {
      m_frameSize = s;
      Q_EMIT frameSizeChanged();
    }
  }
  Q_EMIT epochChanged();
  update();
}

qreal
FrozenFrame::windowDpr() const
{
  return window() ? window()->devicePixelRatio() : 1.0;
}

QRect
FrozenFrame::toPhysical(const QRectF& l) const
{
  if (m_frameSize.isEmpty() || width() <= 0 || height() <= 0)
    return {};
  const double sx = m_frameSize.width() / width();
  const double sy = m_frameSize.height() / height();
  const int x0 = int(std::lround(l.left() * sx));
  const int y0 = int(std::lround(l.top() * sy));
  const int x1 = int(std::lround(l.right() * sx));
  const int y1 = int(std::lround(l.bottom() * sy));
  return QRect(x0, y0, std::max(0, x1 - x0), std::max(0, y1 - y0));
}

QSGNode*
FrozenFrame::updatePaintNode(QSGNode* old, UpdatePaintNodeData*)
{
  auto* node = static_cast<QSGSimpleTextureNode*>(old);
  if (m_epoch != m_uploadedEpoch) {
    const snip::FrameInfo fi = snip::frame_info(m_output);
    if (fi.valid) {
      const quint64 t0 = nowNs();
      // Zero-copy wrap of the memfd mapping; XRGB8888 LE == QImage::Format_RGB32.
      const QImage img(reinterpret_cast<const uchar*>(fi.ptr),
                       int(fi.width),
                       int(fi.height),
                       qsizetype(fi.stride),
                       QImage::Format_RGB32);
      QSGTexture* tex = window()->createTextureFromImage(img, QQuickWindow::TextureIsOpaque);
      if (!node) {
        node = new QSGSimpleTextureNode;
        node->setOwnsTexture(true);
      }
      node->setTexture(tex);
      node->setFiltering(QSGTexture::Nearest);
      m_uploadedEpoch = m_epoch;
      m_awaitSwapEpoch.store(m_epoch);
      mark(QStringLiteral("texture_uploaded"),
           QStringLiteral("\"output\":\"%1\",\"epoch\":%2,\"upload_ms\":%3,\"dpr\":%4,\"item\":[%5,%6]")
             .arg(m_output)
             .arg(m_epoch)
             .arg(double(nowNs() - t0) / 1e6, 0, 'f', 2)
             .arg(window()->effectiveDevicePixelRatio())
             .arg(width())
             .arg(height()));
    }
  }
  if (!node)
    return nullptr;
  node->setRect(boundingRect());
  return node;
}

void
FrozenFrame::itemChange(ItemChange change, const ItemChangeData& data)
{
  if (change == ItemSceneChange) {
    QObject::disconnect(m_swapConn);
    if (data.window) {
      // frameSwapped fires on the render thread right after the buffer swap
      // (i.e. the wl_surface commit). Record there, notify QML queued.
      m_swapConn = connect(
        data.window,
        &QQuickWindow::frameSwapped,
        this,
        [this]() {
          const int e = m_awaitSwapEpoch.exchange(0);
          if (e != 0) {
            mark(QStringLiteral("frame_swapped"),
                 QStringLiteral("\"output\":\"%1\",\"epoch\":%2").arg(m_output).arg(e));
            QMetaObject::invokeMethod(this, [this, e]() { Q_EMIT presented(e); }, Qt::QueuedConnection);
          }
        },
        Qt::DirectConnection);
    }
  }
  QQuickItem::itemChange(change, data);
}
