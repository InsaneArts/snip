#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QtQml>
#include <QTimer>
#include <QWindow>
#include <cstdio>
#include <time.h>

#include "frozenframe.h"
#include "screens.h"
#include "draghelper.h"
#include <QQmlContext>
#include "snip_spike/src/bridge.cxxqt.h"

static double
msSince(const timespec& a)
{
  timespec b;
  clock_gettime(CLOCK_MONOTONIC, &b);
  return double(b.tv_sec - a.tv_sec) * 1e3 + double(b.tv_nsec - a.tv_nsec) / 1e6;
}

int
main(int argc, char* argv[])
{
  timespec t0;
  clock_gettime(CLOCK_MONOTONIC, &t0);

  // The session sets QT_QPA_PLATFORMTHEME=gtk3, which costs ~90 ms and ~23 MB
  // at startup (Omasnap #112). We theme ourselves.
  qunsetenv("QT_QPA_PLATFORMTHEME");
  QQuickWindow::setDefaultAlphaBuffer(true);

  QGuiApplication app(argc, argv);
  const double tApp = msSince(t0);

  QString mode = QStringLiteral("interactive");
  for (const QString& a : app.arguments()) {
    if (a == QLatin1String("--bench"))
      mode = QStringLiteral("bench");
    else if (a == QLatin1String("--selftest"))
      mode = QStringLiteral("selftest");
    else if (a == QLatin1String("--soak"))
      mode = QStringLiteral("soak");
    else if (a == QLatin1String("--test"))
      mode = QStringLiteral("test");
    else if (a == QLatin1String("--gallery"))
      mode = QStringLiteral("gallery");
    else if (a == QLatin1String("--cover"))
      mode = QStringLiteral("cover");
  }

  qmlRegisterType<FrozenFrame>("SnipCpp", 1, 0, "FrozenFrame");

  ScreenLookup screenLookup; // must outlive the engine (bindings re-evaluate on teardown)
  DragHelper dragHelper;
  QQmlApplicationEngine engine;
  engine.rootContext()->setContextProperty(QStringLiteral("ScreenLookup"), &screenLookup);
  engine.rootContext()->setContextProperty(QStringLiteral("DragHelper"), &dragHelper);
  engine.setInitialProperties({ { QStringLiteral("mode"), mode } });
  engine.load(QUrl(QStringLiteral("qrc:/qt/qml/com/snip/spike/qml/Main.qml")));
  if (engine.rootObjects().isEmpty())
    return 1;

  snip::mark_time(QStringLiteral("startup"),
                  QStringLiteral("\"mode\":\"%1\",\"qguiapp_ms\":%2,\"qml_loaded_ms\":%3")
                    .arg(mode)
                    .arg(tApp, 0, 'f', 2)
                    .arg(msSince(t0), 0, 'f', 2));
  if (qEnvironmentVariableIsSet("SNIP_DUMP_SCREENS")) {
    auto dump = [](const char* when) {
      for (QWindow* w : QGuiApplication::allWindows())
        fprintf(stderr, "[%s] window %p '%s' visible=%d screen=%p handle=%p\n", when, (void*)w,
                qPrintable(w->objectName()), w->isVisible(), (void*)w->screen(),
                w->screen() ? (void*)w->screen()->handle() : nullptr);
      fprintf(stderr, "[%s] screens=%lld primary=%p\n", when, (long long)QGuiApplication::screens().size(),
              (void*)QGuiApplication::primaryScreen());
    };
    dump("after-load");
    for (int ms : { 20, 60, 100, 300, 1000 })
      QTimer::singleShot(ms, [dump, ms] { dump(qPrintable(QStringLiteral("t+%1ms").arg(ms))); });
  }
  return app.exec();
}
