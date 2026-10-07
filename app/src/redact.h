#pragma once

#include <QQuickImageProvider>

// image://snip-redact/<style>/<seed>/<x>,<y>,<w>,<h>/<percent-encoded path>:
// the editor's Pixelate (style 0) and Blur (1) tiles for a region of an
// image, built in Rust from pixels outside the region only (ANN-06, D26).
class RedactProvider : public QQuickImageProvider
{
public:
  RedactProvider()
    : QQuickImageProvider(QQuickImageProvider::Image)
  {
  }
  QImage requestImage(const QString& id, QSize* size, const QSize& requestedSize) override;
};
