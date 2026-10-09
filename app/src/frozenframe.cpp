#include "frozenframe.h"

#include <QImage>
#include <QQuickWindow>
#include <QSGGeometryNode>
#include <QSGTextureMaterial>

#include "snip_bridge/src/bridge.cxxqt.h"

QPointF
snipBufferCoord(double u, double v, unsigned t)
{
  if (t >= 4)
    u = 1.0 - u; // flipped variants: flip around the vertical axis first
  switch (t % 4) {
    case 1:
      return { v, 1.0 - u }; // 90 ccw
    case 2:
      return { 1.0 - u, 1.0 - v }; // 180
    case 3:
      return { 1.0 - v, u }; // 270 ccw
    default:
      return { u, v };
  }
}

namespace {
void
releaseFrame(void* handle)
{
  snip::frame_release(reinterpret_cast<std::size_t>(handle));
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
  const unsigned t = fi.valid ? fi.transform : 0;
  const QSize s = !fi.valid ? QSize() : (t % 2 ? QSize(int(fi.height), int(fi.width)) : QSize(int(fi.width), int(fi.height)));
  if (s != m_frameSize) {
    m_frameSize = s; // upright size
    Q_EMIT frameSizeChanged();
  }
  Q_EMIT epochChanged();
  update();
}

QSGNode*
FrozenFrame::updatePaintNode(QSGNode* old, UpdatePaintNodeData*)
{
  auto* node = static_cast<QSGGeometryNode*>(old);
  if (m_epoch != m_uploadedEpoch) {
    const snip::FrameInfo fi = snip::frame_acquire(m_output);
    if (fi.valid) {
      // Zero-copy wrap; the QImage owns a frame reference until Qt has
      // uploaded it (released in the cleanup function).
      const QImage img(reinterpret_cast<const uchar*>(fi.ptr),
                       int(fi.width),
                       int(fi.height),
                       qsizetype(fi.stride),
                       fi.format == 0 ? QImage::Format_ARGB32_Premultiplied : QImage::Format_RGB32,
                       releaseFrame,
                       reinterpret_cast<void*>(fi.handle));
      QSGTexture* tex = window()->createTextureFromImage(img, QQuickWindow::TextureIsOpaque);
      tex->setFiltering(QSGTexture::Nearest);
      if (!node) {
        node = new QSGGeometryNode;
        auto* geom = new QSGGeometry(QSGGeometry::defaultAttributes_TexturedPoint2D(), 4);
        geom->setDrawingMode(QSGGeometry::DrawTriangleStrip);
        node->setGeometry(geom);
        node->setFlag(QSGNode::OwnsGeometry);
        node->setMaterial(new QSGOpaqueTextureMaterial);
        node->setFlag(QSGNode::OwnsMaterial);
      }
      auto* mat = static_cast<QSGOpaqueTextureMaterial*>(node->material());
      delete mat->texture();
      mat->setTexture(tex);
      mat->setFiltering(QSGTexture::Nearest);
      m_transform = fi.transform;
      m_uploadedEpoch = m_epoch;
      m_awaitSwapEpoch.store(m_epoch);
    }
  }
  if (!node)
    return nullptr;
  const double w = width(), h = height();
  const double us[4] = { 0, 0, 1, 1 }, vs[4] = { 0, 1, 0, 1 }; // TL, BL, TR, BR
  auto* v = node->geometry()->vertexDataAsTexturedPoint2D();
  for (int i = 0; i < 4; ++i) {
    const QPointF tc = snipBufferCoord(us[i], vs[i], m_transform);
    v[i].set(float(us[i] * w), float(vs[i] * h), float(tc.x()), float(tc.y()));
  }
  node->markDirty(QSGNode::DirtyGeometry | QSGNode::DirtyMaterial);
  return node;
}

void
FrozenFrame::itemChange(ItemChange change, const ItemChangeData& data)
{
  if (change == ItemSceneChange) {
    QObject::disconnect(m_swapConn);
    if (data.window) {
      // frameSwapped fires on the render thread right after the swap (the
      // wl_surface commit): the moment the frozen frame is on screen.
      m_swapConn = connect(
        data.window,
        &QQuickWindow::frameSwapped,
        this,
        [this]() {
          const int e = m_awaitSwapEpoch.exchange(0);
          if (e != 0) {
            snip::mark_time(QStringLiteral("overlay_presented"),
                            QStringLiteral("\"output\":\"%1\",\"epoch\":%2").arg(m_output).arg(e));
            QMetaObject::invokeMethod(this, [this, e]() { Q_EMIT presented(e); }, Qt::QueuedConnection);
          }
        },
        Qt::DirectConnection);
    }
  }
  QQuickItem::itemChange(change, data);
}
