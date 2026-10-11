#include "frametime.h"

#include <QVideoFrame>

void
FrameTime::setSink(QVideoSink* s)
{
  if (s == m_sink)
    return;
  disconnect(m_conn);
  m_sink = s;
  if (s)
    m_conn = connect(s, &QVideoSink::videoFrameChanged, this, [this](const QVideoFrame& f) {
      if (f.startTime() < 0)
        return;
      m_ms = f.startTime() / 1000.0;
      Q_EMIT msChanged();
    });
  Q_EMIT sinkChanged();
}
