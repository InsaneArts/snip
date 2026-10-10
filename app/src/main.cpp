// snipd: the resident Snip daemon (architecture §3). Rust owns capture,
// storage and the API; this process also draws every Snip surface.

#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <cstdio>
#include <malloc.h>

#include "redact.h"
#include "snip_bridge/src/bridge.cxxqt.h"

int
main(int argc, char* argv[])
{
  // The session sets QT_QPA_PLATFORMTHEME=gtk3: ~90 ms and ~23 MB at start
  // for nothing (Omasnap #112). We draw our own theme.
  qunsetenv("QT_QPA_PLATFORMTHEME");
  // Qt picks the basic loop on NVIDIA, which caps at 125 fps (spike S05).
  if (!qEnvironmentVariableIsSet("QSG_RENDER_LOOP"))
    qputenv("QSG_RENDER_LOOP", "threaded");
  // QtMultimedia tries every FFmpeg hardware device on the GUI thread at its
  // first use (at start: 0.87 s to ready instead of 0.3 s) unless both lists
  // are given. Snip never encodes through Qt, and decodes in software: no
  // CUDA context per video, and no CUDA teardown at exit.
  for (const char* var : { "QT_FFMPEG_ENCODING_HW_DEVICE_TYPES", "QT_FFMPEG_DECODING_HW_DEVICE_TYPES" })
    if (!qEnvironmentVariableIsSet(var))
      qputenv(var, ",");
  QQuickWindow::setDefaultAlphaBuffer(true);
  // A buffer of 1 MB or more (a frame, an image, a decoded PNG) is its own
  // mapping, given back when freed. glibc's default raises that threshold
  // after the first big free, so they land in its per-thread arenas, which
  // it never gives back: snipd grew by every capture, editor and recording
  // (176 MB at start, 738 MB after a morning). About 1–2 ms more page faults
  // per capture.
  mallopt(M_MMAP_THRESHOLD, 1 << 20);

  QGuiApplication app(argc, argv);
  QGuiApplication::setApplicationName(QStringLiteral("snip"));
  // The window class (Wayland app_id). Test instances (SNIP_APP_ID) get
  // their own, so a test can steer its editor window away from the user.
  const QString appId = qEnvironmentVariable("SNIP_APP_ID");
  QGuiApplication::setDesktopFileName(appId.isEmpty() ? QStringLiteral("snip") : appId);
  QGuiApplication::setQuitOnLastWindowClosed(false);

  QString mode = QStringLiteral("interactive");
  const QStringList args = app.arguments();
  if (args.contains(QStringLiteral("--test")))
    mode = QStringLiteral("test");
  else if (args.contains(QStringLiteral("--gallery")))
    mode = QStringLiteral("gallery");

  if (mode == QLatin1String("interactive") && snip::already_running()) {
    fprintf(stderr, "snipd: already running\n");
    return 0;
  }

  QQmlApplicationEngine engine;
  engine.addImageProvider(QStringLiteral("snip-redact"), new RedactProvider);
  engine.setInitialProperties({ { QStringLiteral("mode"), mode } });
  engine.loadFromModule("Snip.Ui", "Main");
  if (engine.rootObjects().isEmpty())
    return 1;
  return app.exec();
}
