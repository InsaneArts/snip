#pragma once

#include <QObject>
#include <QPointer>
#include <QVideoSink>
#include <QtQml/qqmlregistration.h>

// The time of the frame a VideoOutput shows (Studio Mode, D55): QML can't
// read a QVideoFrame's startTime, and MediaPlayer's position moves only
// about 20 times a second, so zooms animated from it would step. Set `sink`
// to the VideoOutput's videoSink; `ms` follows every frame shown, played or
// sought to.
class FrameTime : public QObject
{
  Q_OBJECT
  QML_ELEMENT
  Q_PROPERTY(QVideoSink* sink READ sink WRITE setSink NOTIFY sinkChanged)
  Q_PROPERTY(qreal ms READ ms NOTIFY msChanged)

public:
  using QObject::QObject;

  QVideoSink* sink() const { return m_sink; }
  void setSink(QVideoSink* s);
  qreal ms() const { return m_ms; }

Q_SIGNALS:
  void sinkChanged();
  void msChanged();

private:
  QPointer<QVideoSink> m_sink;
  QMetaObject::Connection m_conn;
  qreal m_ms = 0;
};
