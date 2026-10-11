// A Studio export, drawn: the kept parts of a recording come from one
// ffmpeg (`decode`, raw 4:2:0 frames), each is drawn with StudioFrame.qml
// (the background still, the card where the zoom evaluator puts it), and
// what's drawn goes to a second ffmpeg (`encode`, raw RGBA in) that adds the
// sound. It prints "frame N" as it goes, and "error …" lines when it fails
// (its stderr is the daemon's). A child of snipd: its GPU work, and a crash,
// stay out of the daemon. About 75 fps at 2688×1568 (NVENC): most of the time
// is the encoder's RGBA to YUV conversion; the research prototype (the
// design's §8.1) drew 148–155 fps at 2560×1440.

#include "render.h"

#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcess>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickRenderControl>
#include <QQuickRenderTarget>
#include <QQuickWindow>
#include <QVideoFrame>
#include <QVideoFrameFormat>
#include <QVideoSink>
#include <cstdio>
#include <cstring>
#include <memory>
#include <rhi/qrhi.h>

namespace {

QStringList
strings(const QJsonValue& v)
{
  QStringList out;
  for (const auto& x : v.toArray())
    out << x.toString();
  return out;
}

// exactly `n` bytes from `p`; false at its end
bool
readExactly(QProcess& p, char* dst, qint64 n)
{
  qint64 got = 0;
  while (got < n) {
    if (p.bytesAvailable() == 0 && !p.waitForReadyRead(-1))
      return false;
    const qint64 r = p.read(dst + got, n - got);
    if (r < 0)
      return false;
    got += r;
  }
  return true;
}

// the recording's own time (ms) of output frame `n`: the kept parts, one
// after another
double
sourceMs(const QJsonArray& keep, double fps, qint64 n)
{
  double t = n / fps;
  for (const auto& part : keep) {
    const double a = part.toArray()[0].toDouble(), b = part.toArray()[1].toDouble();
    if (t < b - a)
      return (a + t) * 1000.0;
    t -= b - a;
  }
  return keep.isEmpty() ? 0 : keep.last().toArray()[1].toDouble() * 1000.0;
}

int
fail(const QString& why)
{
  for (const auto& line : why.trimmed().split('\n'))
    printf("error %s\n", qPrintable(line));
  return 1;
}

} // namespace

int
renderVideo(const QString& specPath)
{
  QFile file(specPath);
  if (!file.open(QIODevice::ReadOnly))
    return fail(QStringLiteral("can't read ") + specPath);
  const QJsonObject spec = QJsonDocument::fromJson(file.readAll()).object();
  const QJsonArray source = spec["source"].toArray(), size = spec["size"].toArray(), keep = spec["keep"].toArray();
  const int iw = source[0].toInt(), ih = source[1].toInt(), W = size[0].toInt(), H = size[1].toInt();
  const double fps = spec["fps"].toDouble(30), scale = spec["scale"].toDouble(1);
  const QJsonObject scene = spec["scene"].toObject();
  if (iw < 2 || ih < 2 || W < 2 || H < 2)
    return fail(QStringLiteral("no size to draw at"));

  // the scene, as the editor shows it, at the export's size
  QQuickWindow::setGraphicsApi(QSGRendererInterface::OpenGL);
  QQuickRenderControl control;
  QQuickWindow window(&control);
  QQmlEngine engine;
  // the background's theme colours, as the editor had them
  if (auto* core = engine.singletonInstance<QObject*>("Snip.Core", "Engine"))
    core->setProperty("theme", scene["theme"].toString());
  QQmlComponent component(&engine);
  component.loadFromModule("Snip.Ui", "StudioFrame");
  QObject* made = component.createWithInitialProperties({ { QStringLiteral("bg"), scene["bg"].toVariant() },
                                                          { QStringLiteral("outer"), scene["outer"].toVariant() },
                                                          { QStringLiteral("inner"), scene["inner"].toVariant() },
                                                          { QStringLiteral("studio"), scene["studio"].toVariant() },
                                                          { QStringLiteral("pixelScale"), scale } });
  std::unique_ptr<QQuickItem> root(qobject_cast<QQuickItem*>(made));
  if (!root) {
    QString why;
    for (const auto& e : component.errors())
      why += e.toString() + "\n";
    return fail(why.isEmpty() ? QStringLiteral("the scene didn't load") : why);
  }
  root->setParentItem(window.contentItem());
  root->setTransformOrigin(QQuickItem::TopLeft);
  root->setScale(scale);
  window.contentItem()->setSize(QSizeF(W, H));
  window.setGeometry(0, 0, W, H);
  if (!control.initialize())
    return fail(QStringLiteral("no GPU to draw with"));
  QRhi* rhi = control.rhi();
  std::unique_ptr<QRhiTexture> texture(rhi->newTexture(QRhiTexture::RGBA8, QSize(W, H), 1, QRhiTexture::RenderTarget | QRhiTexture::UsedAsTransferSource));
  texture->create();
  std::unique_ptr<QRhiRenderBuffer> depth(rhi->newRenderBuffer(QRhiRenderBuffer::DepthStencil, QSize(W, H), 1));
  depth->create();
  QRhiTextureRenderTargetDescription description{ QRhiColorAttachment(texture.get()) };
  description.setDepthStencilBuffer(depth.get());
  std::unique_ptr<QRhiTextureRenderTarget> target(rhi->newTextureRenderTarget(description));
  std::unique_ptr<QRhiRenderPassDescriptor> pass(target->newCompatibleRenderPassDescriptor());
  target->setRenderPassDescriptor(pass.get());
  target->create();
  QQuickRenderTarget rendered = QQuickRenderTarget::fromRhiRenderTarget(target.get());
  // read back top row first, as the encoder takes it
  rendered.setMirrorVertically(rhi->isYUpInFramebuffer());
  window.setRenderTarget(rendered);

  QVideoSink* sink = nullptr;
  if (auto* video = root->property("video").value<QObject*>())
    sink = video->property("videoSink").value<QVideoSink*>();
  if (!sink)
    return fail(QStringLiteral("the scene has no video"));

  QProcess decode, encode;
  decode.start(QStringLiteral("ffmpeg"), strings(spec["decode"]));
  encode.start(QStringLiteral("ffmpeg"), strings(spec["encode"]));
  if (!decode.waitForStarted() || !encode.waitForStarted())
    return fail(QStringLiteral("ffmpeg didn't start"));

  QVideoFrameFormat format(QSize(iw, ih), QVideoFrameFormat::Format_YUV420P);
  format.setColorSpace(QVideoFrameFormat::ColorSpace_BT709);
  format.setColorRange(QVideoFrameFormat::ColorRange_Video);
  format.setStreamFrameRate(fps);
  const int widths[3] = { iw, (iw + 1) / 2, (iw + 1) / 2 }, heights[3] = { ih, (ih + 1) / 2, (ih + 1) / 2 };
  QByteArray in(qint64(iw) * ih + 2 * qint64(widths[1]) * heights[1], Qt::Uninitialized);
  qint64 n = 0;
  bool sent = true;
  for (; sent && readExactly(decode, in.data(), in.size()); n++) {
    QVideoFrame frame(format);
    frame.map(QVideoFrame::WriteOnly);
    const char* from = in.constData();
    for (int p = 0; p < 3; p++)
      for (int y = 0; y < heights[p]; y++, from += widths[p])
        memcpy(frame.bits(p) + qint64(y) * frame.bytesPerLine(p), from, widths[p]);
    frame.unmap();
    const double ms = sourceMs(keep, fps, n);
    frame.setStartTime(qint64(ms * 1000));
    sink->setVideoFrame(frame);
    root->setProperty("ms", ms);

    control.polishItems();
    control.beginFrame();
    control.sync();
    control.render();
    QRhiReadbackResult back;
    QRhiResourceUpdateBatch* batch = rhi->nextResourceUpdateBatch();
    batch->readBackTexture(QRhiReadbackDescription(texture.get()), &back);
    control.commandBuffer()->resourceUpdate(batch);
    control.endFrame(); // off screen it waits for the GPU: `back` is ready
    // all of it into the pipe now: ffmpeg's reader takes it whole, and
    // encodes it while the next frame is drawn
    encode.write(back.data);
    while (sent && encode.bytesToWrite() > 0)
      sent = encode.waitForBytesWritten(-1);
    if (n % 5 == 0) {
      printf("frame %lld\n", static_cast<long long>(n));
      fflush(stdout);
    }
  }
  // the encoder stopped (no NVENC, say): the rest isn't read
  if (!sent)
    decode.kill();
  encode.closeWriteChannel();
  encode.waitForFinished(-1);
  decode.waitForFinished(-1);
  if (encode.exitStatus() != QProcess::NormalExit || encode.exitCode() != 0)
    return fail(QString::fromUtf8(encode.readAllStandardError()));
  if (decode.exitStatus() != QProcess::NormalExit || decode.exitCode() != 0)
    return fail(QString::fromUtf8(decode.readAllStandardError()));
  if (n == 0)
    return fail(QStringLiteral("no frames to draw"));
  printf("frame %lld\n", static_cast<long long>(n));
  fflush(stdout);
  return 0;
}
