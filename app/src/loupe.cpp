#include "loupe.h"

#include "frozenframe.h"

#include <QQuickWindow>
#include <QSGImageNode>
#include <cmath>

#include "snip_bridge/src/bridge.cxxqt.h"

Loupe::Loupe(QQuickItem* parent)
  : QQuickItem(parent)
{
  setFlag(ItemHasContents, true);
}

void
Loupe::setOutput(const QString& o)
{
  if (o == m_output)
    return;
  m_output = o;
  Q_EMIT outputChanged();
  sample();
}

void
Loupe::setPx(int v)
{
  if (v == m_px)
    return;
  m_px = v;
  Q_EMIT positionChanged();
  sample();
}

void
Loupe::setPy(int v)
{
  if (v == m_py)
    return;
  m_py = v;
  Q_EMIT positionChanged();
  sample();
}

void
Loupe::setCells(int c)
{
  c = std::max(3, c | 1); // odd, so there is a centre pixel
  if (c == m_cells)
    return;
  m_cells = c;
  Q_EMIT cellsChanged();
  sample();
}

void
Loupe::setEpoch(int e)
{
  if (e == m_epoch)
    return;
  m_epoch = e;
  Q_EMIT epochChanged();
  sample();
}

void
Loupe::sample()
{
  if (!isVisible() && m_epoch == 0)
    return;
  const snip::FrameInfo fi = snip::frame_acquire(m_output);
  if (!fi.valid)
    return;
  const unsigned t = fi.transform;
  const int bw = int(fi.width), bh = int(fi.height);
  const int uw = t % 2 ? bh : bw, uh = t % 2 ? bw : bh;
  const auto* base = reinterpret_cast<const uchar*>(fi.ptr);
  QImage patch(m_cells, m_cells, QImage::Format_RGB32);
  const int half = m_cells / 2;
  for (int j = 0; j < m_cells; ++j) {
    auto* line = reinterpret_cast<QRgb*>(patch.scanLine(j));
    for (int i = 0; i < m_cells; ++i) {
      const int ux = m_px - half + i, uy = m_py - half + j;
      if (ux < 0 || uy < 0 || ux >= uw || uy >= uh) {
        line[i] = qRgb(0, 0, 0);
        continue;
      }
      const QPointF tc = snipBufferCoord((ux + 0.5) / uw, (uy + 0.5) / uh, t);
      const int bx = std::clamp(int(tc.x() * bw), 0, bw - 1);
      const int by = std::clamp(int(tc.y() * bh), 0, bh - 1);
      const auto* p = base + qsizetype(by) * fi.stride + qsizetype(bx) * 4; // B G R X
      line[i] = qRgb(p[2], p[1], p[0]);
    }
  }
  snip::frame_release(fi.handle);
  m_patch = patch;
  const QColor c = QColor::fromRgb(patch.pixel(half, half));
  if (c != m_color) {
    m_color = c;
    Q_EMIT colorChanged();
  }
  m_dirty = true;
  update();
}

QSGNode*
Loupe::updatePaintNode(QSGNode* old, UpdatePaintNodeData*)
{
  auto* node = static_cast<QSGImageNode*>(old);
  if (m_patch.isNull()) {
    delete node;
    return nullptr;
  }
  if (!node) {
    node = window()->createImageNode();
    node->setOwnsTexture(true);
    node->setFiltering(QSGTexture::Nearest);
  }
  if (m_dirty) {
    node->setTexture(window()->createTextureFromImage(m_patch, QQuickWindow::TextureIsOpaque));
    node->setFiltering(QSGTexture::Nearest);
    m_dirty = false;
  }
  node->setRect(boundingRect());
  return node;
}
