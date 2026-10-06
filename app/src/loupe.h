#pragma once

#include <QColor>
#include <QImage>
#include <QQuickItem>
#include <QtQml/qqmlregistration.h>

// SEL-05 magnifier: `cells` x `cells` physical pixels of the frozen frame
// around (px, py), drawn with nearest sampling. The pixel grid and the
// centre marker are QML on top. `color` is the centre pixel.
class Loupe : public QQuickItem
{
  Q_OBJECT
  QML_ELEMENT
  Q_PROPERTY(QString output READ output WRITE setOutput NOTIFY outputChanged)
  Q_PROPERTY(int px READ px WRITE setPx NOTIFY positionChanged)
  Q_PROPERTY(int py READ py WRITE setPy NOTIFY positionChanged)
  Q_PROPERTY(int cells READ cells WRITE setCells NOTIFY cellsChanged)
  Q_PROPERTY(int epoch READ epoch WRITE setEpoch NOTIFY epochChanged)
  Q_PROPERTY(QColor color READ color NOTIFY colorChanged)

public:
  explicit Loupe(QQuickItem* parent = nullptr);

  QString output() const { return m_output; }
  void setOutput(const QString& o);
  int px() const { return m_px; }
  void setPx(int v);
  int py() const { return m_py; }
  void setPy(int v);
  int cells() const { return m_cells; }
  void setCells(int c);
  int epoch() const { return m_epoch; }
  void setEpoch(int e);
  QColor color() const { return m_color; }

Q_SIGNALS:
  void outputChanged();
  void positionChanged();
  void cellsChanged();
  void epochChanged();
  void colorChanged();

protected:
  QSGNode* updatePaintNode(QSGNode* old, UpdatePaintNodeData*) override;

private:
  void sample();

  QString m_output;
  int m_px = 0, m_py = 0, m_cells = 15, m_epoch = 0;
  QColor m_color;
  QImage m_patch;
  bool m_dirty = false;
};
