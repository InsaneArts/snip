// snipd: the resident Snip daemon (architecture §3). Rust owns capture,
// storage and the API; this process also draws every Snip surface.

#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <cstdio>

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
  QQuickWindow::setDefaultAlphaBuffer(true);

  QGuiApplication app(argc, argv);
  QGuiApplication::setApplicationName(QStringLiteral("snip"));
  QGuiApplication::setDesktopFileName(QStringLiteral("snip"));
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
  engine.setInitialProperties({ { QStringLiteral("mode"), mode } });
  engine.loadFromModule("Snip.Ui", "Main");
  if (engine.rootObjects().isEmpty())
    return 1;
  return app.exec();
}
