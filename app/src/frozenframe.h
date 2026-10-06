#pragma once

#include <QQuickItem>
#include <QRect>
#include <QSize>
#include <QtQml/qqmlregistration.h>

// One output's frozen frame (Rust-owned shm), shown 1:1 with nearest
// sampling and the output transform undone in texture coordinates.
class FrozenFrame : public QQuickItem
{
  Q_OBJECT
  QML_ELEMENT
  Q_PROPERTY(QString output READ output WRITE setOutput NOTIFY outputChanged)
  Q_PROPERTY(int epoch READ epoch WRITE setEpoch NOTIFY epochChanged)
  Q_PROPERTY(QSize frameSize READ frameSize NOTIFY frameSizeChanged)
  Q_PROPERTY(bool hasFrame READ hasFrame NOTIFY frameSizeChanged)

public:
  explicit FrozenFrame(QQuickItem* parent = nullptr);

  QString output() const { return m_output; }
  void setOutput(const QString& o);
  int epoch() const { return m_epoch; }
  void setEpoch(int e);
  QSize frameSize() const { return m_frameSize; }
  bool hasFrame() const { return !m_frameSize.isEmpty(); }

  // Physical pixels per logical pixel (the frame's, else the window's).
  Q_INVOKABLE qreal pixelScale() const;
  // Logical item rect -> physical frame rect, edges rounded independently
  // (like Hyprland's CBox::round) so adjacent selections tile exactly.
  Q_INVOKABLE QRect toPhysical(const QRectF& logical) const;
  // Physical rect -> logical item rect (for drawing exactly what is captured).
  Q_INVOKABLE QRectF toLogical(const QRect& physical) const;

Q_SIGNALS:
  void outputChanged();
  void epochChanged();
  void frameSizeChanged();
  void presented(int epoch);

protected:
  QSGNode* updatePaintNode(QSGNode* old, UpdatePaintNodeData*) override;
  void itemChange(ItemChange change, const ItemChangeData& data) override;

private:
  QString m_output;
  int m_epoch = 0;
  int m_uploadedEpoch = 0;
  unsigned m_transform = 0;
  QSize m_frameSize;
  std::atomic<int> m_awaitSwapEpoch{ 0 };
  QMetaObject::Connection m_swapConn;
};

// Display corner (u, v) in [0,1]^2 -> buffer texture coordinate for a buffer
// whose content had wl_output transform t applied:
// buffer = rotate_ccw^k(flip?(content)).
QPointF snipBufferCoord(double u, double v, unsigned t);
