#pragma once

#include <QQuickItem>
#include <QRect>
#include <QSize>
#include <atomic>

// Shows the Rust-owned frozen frame for one output, sampled 1:1 (nearest),
// and reports when the first frame containing it has been swapped.
class FrozenFrame : public QQuickItem
{
  Q_OBJECT
  Q_PROPERTY(QString output READ output WRITE setOutput NOTIFY outputChanged)
  Q_PROPERTY(int epoch READ epoch WRITE setEpoch NOTIFY epochChanged)
  Q_PROPERTY(QSize frameSize READ frameSize NOTIFY frameSizeChanged)
  Q_PROPERTY(qreal windowDpr READ windowDpr NOTIFY frameSizeChanged)

public:
  explicit FrozenFrame(QQuickItem* parent = nullptr);

  QString output() const { return m_output; }
  void setOutput(const QString& o);
  int epoch() const { return m_epoch; }
  void setEpoch(int e);
  QSize frameSize() const { return m_frameSize; }
  qreal windowDpr() const;

  // Logical item rect -> physical frame rect. Edges are rounded
  // independently (like Hyprland's CBox::round), so adjacent selections tile.
  Q_INVOKABLE QRect toPhysical(const QRectF& logical) const;

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
