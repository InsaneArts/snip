#pragma once

#include <QQuickImageProvider>

// image://snip-redact/<style>/<level>/<seed>/<x>,<y>,<w>,<h>/<percent-encoded path>:
// the editor's Pixelate (style 0), Blur (1), Text shape (2) and Natural blur
// (3, not secure) for a region of an image at an intensity, built in Rust
// (ANN-06, D26, D30).
class RedactProvider : public QQuickImageProvider
{
public:
  RedactProvider()
    : QQuickImageProvider(QQuickImageProvider::Image)
  {
  }
  QImage requestImage(const QString& id, QSize* size, const QSize& requestedSize) override;
};
