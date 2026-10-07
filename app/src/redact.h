#pragma once

#include <QQuickImageProvider>

// image://snip-redact/<style>/<seed>/<x>,<y>,<w>,<h>/<percent-encoded path>:
// the editor's Pixelate (style 0), Blur (1) and Text shape (2) tiles for a
// region of an image, built in Rust (ANN-06, D26, D30).
class RedactProvider : public QQuickImageProvider
{
public:
  RedactProvider()
    : QQuickImageProvider(QQuickImageProvider::Image)
  {
  }
  QImage requestImage(const QString& id, QSize* size, const QSize& requestedSize) override;
};
