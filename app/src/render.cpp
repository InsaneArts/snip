// A Studio export, drawn: the kept parts of a recording come from one
// ffmpeg (`decode`, raw I420 frames, read ahead in a thread), each is drawn
// with StudioFrame.qml (the background still, the card where the zoom
// evaluator puts it), made I420 again on the GPU (yuv.frag), and goes to a
// second ffmpeg (`encode`) that adds the sound. It prints "frame N" as it
// goes, and "error …" lines when it fails (its stderr is the daemon's). A
// child of snipd: its GPU work, and a crash, stay out of the daemon.

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
#include <QThread>
#include <QVideoFrame>
#include <QVideoFrameFormat>
#include <QVideoSink>
#include <condition_variable>
#include <cstdio>
#include <deque>
#include <malloc.h>
#include <memory>
#include <mutex>
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

QShader
shader(const QString& name)
{
  QFile f(name);
  return f.open(QIODevice::ReadOnly) ? QShader::fromSerialized(f.readAll()) : QShader();
}

// The decoder, read in a thread of its own: each frame straight into a video
// frame's memory, a few of them ahead of the drawing.
class Reader
{
public:
  Reader(const QStringList& args, const QVideoFrameFormat& format)
  {
    thread.reset(QThread::create([this, args, format] { run(args, format); }));
    thread->start();
  }
  ~Reader()
  {
    stop();
    thread->wait();
  }
  // the next frame; an invalid one at the end
  QVideoFrame next()
  {
    std::unique_lock l(m);
    cv.wait(l, [this] { return !ready.empty() || done; });
    if (ready.empty())
      return {};
    QVideoFrame f = ready.front();
    ready.pop_front();
    cv.notify_all();
    return f;
  }
  // no more is wanted: ffmpeg goes
  void stop()
  {
    std::lock_guard l(m);
    stopping = true;
    cv.notify_all();
  }
  // why it ended before its end (ffmpeg's errors), once it has
  QString error()
  {
    thread->wait();
    return why;
  }

private:
  void run(const QStringList& args, const QVideoFrameFormat& format)
  {
    QProcess decode;
    decode.start(QStringLiteral("ffmpeg"), args);
    bool ok = decode.waitForStarted();
    const int w = format.frameWidth(), h = format.frameHeight();
    const int widths[3] = { w, (w + 1) / 2, (w + 1) / 2 }, heights[3] = { h, (h + 1) / 2, (h + 1) / 2 };
    while (ok) {
      QVideoFrame frame(format);
      frame.map(QVideoFrame::WriteOnly);
      for (int p = 0; ok && p < 3; p++)
        for (int y = 0; ok && y < heights[p]; y++)
          ok = readExactly(decode, reinterpret_cast<char*>(frame.bits(p)) + qint64(y) * frame.bytesPerLine(p), widths[p]);
      frame.unmap();
      if (!ok)
        break;
      std::unique_lock l(m);
      cv.wait(l, [this] { return ready.size() < 4 || stopping; });
      if (stopping)
        break;
      ready.push_back(frame);
      cv.notify_all();
    }
    std::unique_lock l(m);
    const bool stopped = stopping;
    l.unlock();
    if (stopped)
      decode.kill();
    decode.waitForFinished(-1);
    l.lock();
    if (!stopped && decode.error() == QProcess::FailedToStart)
      why = QStringLiteral("ffmpeg didn't start");
    else if (!stopped && (decode.exitStatus() != QProcess::NormalExit || decode.exitCode() != 0))
      why = QString::fromUtf8(decode.readAllStandardError());
    done = true;
    cv.notify_all();
  }

  std::unique_ptr<QThread> thread;
  std::mutex m;
  std::condition_variable cv;
  std::deque<QVideoFrame> ready;
  bool done = false, stopping = false;
  QString why;
};

} // namespace

int
renderVideo(const QString& specPath)
{
  // frames come and go, the same size: from the heap, reused (main() maps
  // every block of 1 MB and up, for the daemon; here that's a new mapping
  // and its page faults for each frame, 15 % of the time)
  mallopt(M_MMAP_THRESHOLD, 32 << 20);
  QFile file(specPath);
  if (!file.open(QIODevice::ReadOnly))
    return fail(QStringLiteral("can't read ") + specPath);
  const QJsonObject spec = QJsonDocument::fromJson(file.readAll()).object();
  const QJsonArray source = spec["source"].toArray(), size = spec["size"].toArray(), keep = spec["keep"].toArray();
  const int iw = source[0].toInt(), ih = source[1].toInt(), W = size[0].toInt(), H = size[1].toInt();
  const double fps = spec["fps"].toDouble(30), scale = spec["scale"].toDouble(1);
  const QJsonObject scene = spec["scene"].toObject();
  if (iw < 2 || ih < 2 || W < 2 || H < 2 || W % 2 || H % 2)
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
                                                          { QStringLiteral("log"), scene["log"].toString() },
                                                          { QStringLiteral("pointer"), scene["pointer"].toVariant() },
                                                          { QStringLiteral("pointerLook"), scene["pointerLook"].toVariant() },
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
  std::unique_ptr<QRhiTexture> texture(rhi->newTexture(QRhiTexture::RGBA8, QSize(W, H), 1, QRhiTexture::RenderTarget));
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
  // its first row in memory is the frame's top, as yuv.frag reads it
  rendered.setMirrorVertically(rhi->isYUpInFramebuffer());
  window.setRenderTarget(rendered);

  // each drawn frame made I420 on the GPU (yuv.frag), its bytes four to a
  // texel of an RGBA target (GL reads an R8 one back as its alpha): what's
  // read back is what the encoder takes
  const QShader vs = shader(QStringLiteral(":/snip/shaders/yuv.vert.qsb")), fs = shader(QStringLiteral(":/snip/shaders/yuv.frag.qsb"));
  if (!vs.isValid() || !fs.isValid())
    return fail(QStringLiteral("no shaders for the colours"));
  const qint64 bytes = qint64(W) * H * 3 / 2;
  const QSize packed(W / 2, int((bytes + 2 * W - 1) / (2 * W)));
  std::unique_ptr<QRhiTexture> yuv(rhi->newTexture(QRhiTexture::RGBA8, packed, 1, QRhiTexture::RenderTarget | QRhiTexture::UsedAsTransferSource));
  yuv->create();
  std::unique_ptr<QRhiTextureRenderTarget> yuvTarget(rhi->newTextureRenderTarget({ QRhiColorAttachment(yuv.get()) }));
  std::unique_ptr<QRhiRenderPassDescriptor> yuvPass(yuvTarget->newCompatibleRenderPassDescriptor());
  yuvTarget->setRenderPassDescriptor(yuvPass.get());
  yuvTarget->create();
  std::unique_ptr<QRhiBuffer> sizes(rhi->newBuffer(QRhiBuffer::Dynamic, QRhiBuffer::UniformBuffer, 16));
  sizes->create();
  std::unique_ptr<QRhiSampler> sampler(rhi->newSampler(QRhiSampler::Nearest, QRhiSampler::Nearest, QRhiSampler::None, QRhiSampler::ClampToEdge, QRhiSampler::ClampToEdge));
  sampler->create();
  std::unique_ptr<QRhiShaderResourceBindings> bindings(rhi->newShaderResourceBindings());
  bindings->setBindings({ QRhiShaderResourceBinding::uniformBuffer(0, QRhiShaderResourceBinding::FragmentStage, sizes.get()),
                          QRhiShaderResourceBinding::sampledTexture(1, QRhiShaderResourceBinding::FragmentStage, texture.get(), sampler.get()) });
  bindings->create();
  std::unique_ptr<QRhiGraphicsPipeline> pipeline(rhi->newGraphicsPipeline());
  pipeline->setShaderStages({ { QRhiShaderStage::Vertex, vs }, { QRhiShaderStage::Fragment, fs } });
  pipeline->setShaderResourceBindings(bindings.get());
  pipeline->setRenderPassDescriptor(yuvPass.get());
  if (!pipeline->create())
    return fail(QStringLiteral("no shaders for the colours"));
  const qint32 wh[4] = { W, H, packed.width(), 0 };

  QVideoSink* sink = nullptr;
  if (auto* video = root->property("video").value<QObject*>())
    sink = video->property("videoSink").value<QVideoSink*>();
  if (!sink)
    return fail(QStringLiteral("the scene has no video"));

  QProcess encode;
  encode.start(QStringLiteral("ffmpeg"), strings(spec["encode"]));
  if (!encode.waitForStarted())
    return fail(QStringLiteral("ffmpeg didn't start"));
  QVideoFrameFormat format(QSize(iw, ih), QVideoFrameFormat::Format_YUV420P);
  format.setColorSpace(QVideoFrameFormat::ColorSpace_BT709);
  format.setColorRange(QVideoFrameFormat::ColorRange_Video);
  format.setStreamFrameRate(fps);
  Reader reader(strings(spec["decode"]), format);

  qint64 n = 0;
  bool sent = true;
  for (QVideoFrame frame; sent && (frame = reader.next()).isValid(); n++) {
    const double ms = sourceMs(keep, fps, n);
    frame.setStartTime(qint64(ms * 1000));
    sink->setVideoFrame(frame);
    root->setProperty("ms", ms);

    control.polishItems();
    control.beginFrame();
    control.sync();
    control.render();
    QRhiCommandBuffer* cb = control.commandBuffer();
    QRhiResourceUpdateBatch* first = rhi->nextResourceUpdateBatch();
    first->updateDynamicBuffer(sizes.get(), 0, sizeof(wh), wh);
    cb->beginPass(yuvTarget.get(), Qt::black, { 1.0f, 0 }, first);
    cb->setGraphicsPipeline(pipeline.get());
    cb->setViewport(QRhiViewport(0, 0, packed.width(), packed.height()));
    cb->setShaderResources();
    cb->draw(3);
    QRhiReadbackResult back;
    QRhiResourceUpdateBatch* readback = rhi->nextResourceUpdateBatch();
    readback->readBackTexture(QRhiReadbackDescription(yuv.get()), &back);
    cb->endPass(readback);
    control.endFrame(); // off screen it waits for the GPU: `back` is ready
    if (back.data.size() < bytes) {
      reader.stop();
      encode.kill();
      encode.waitForFinished(-1);
      return fail(QStringLiteral("the colours came back in the wrong size"));
    }
    // all of it into the pipe now: ffmpeg's reader takes it whole, and
    // encodes it while the next frame is drawn
    encode.write(back.data.constData(), bytes);
    while (sent && encode.bytesToWrite() > 0)
      sent = encode.waitForBytesWritten(-1);
    if (n % 5 == 0) {
      printf("frame %lld\n", static_cast<long long>(n));
      fflush(stdout);
    }
  }
  // the encoder stopped (no NVENC, say): the rest isn't read
  if (!sent)
    reader.stop();
  encode.closeWriteChannel();
  encode.waitForFinished(-1);
  // the decoder's first: when it fails, the encoder fails for want of frames
  if (const QString why = reader.error(); !why.isEmpty())
    return fail(why);
  if (encode.exitStatus() != QProcess::NormalExit || encode.exitCode() != 0)
    return fail(QString::fromUtf8(encode.readAllStandardError()));
  if (n == 0)
    return fail(QStringLiteral("no frames to draw"));
  printf("frame %lld\n", static_cast<long long>(n));
  fflush(stdout);
  return 0;
}
