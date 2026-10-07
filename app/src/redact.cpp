#include "redact.h"

#include <QUrl>
#include <cstring>

#include "snip_bridge/src/bridge.cxxqt.h"

QImage
RedactProvider::requestImage(const QString& id, QSize* size, const QSize&)
{
  const QStringList parts = id.split(QLatin1Char('/'));
  if (parts.size() < 5)
    return {};
  const QStringList r = parts[3].split(QLatin1Char(','));
  if (r.size() != 4)
    return {};
  const QString path = QUrl::fromPercentEncoding(parts.mid(4).join(QLatin1Char('/')).toUtf8());
  const snip::RedactTiles t = snip::redact_tiles(path,
                                                 r[0].toInt(),
                                                 r[1].toInt(),
                                                 r[2].toInt(),
                                                 r[3].toInt(),
                                                 parts[0].toUInt(),
                                                 parts[2].toUInt(),
                                                 parts[1].toFloat());
  if (!t.valid || t.width == 0 || t.height == 0)
    return {};
  QImage img(int(t.width), int(t.height), QImage::Format_RGBA8888);
  for (int y = 0; y < img.height(); ++y)
    std::memcpy(img.scanLine(y), t.rgba.data() + size_t(y) * t.width * 4, size_t(t.width) * 4);
  if (size)
    *size = img.size();
  return img;
}
