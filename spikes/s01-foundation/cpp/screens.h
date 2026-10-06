#pragma once

#include <QGuiApplication>
#include <QObject>
#include <QQmlEngine>
#include <QScreen>

// QML's Qt.application.screens yields QQuickScreenInfo wrappers, which
// LayerShellQt's `screen` (QScreen*) cannot take. Hand QML the real QScreen.
class ScreenLookup : public QObject
{
  Q_OBJECT
public:
  using QObject::QObject;
  Q_INVOKABLE QScreen* byName(const QString& name) const
  {
    for (QScreen* s : QGuiApplication::screens())
      if (s->name() == name) {
        // QObjects returned to QML without a parent become JS-owned: the GC
        // would deleteLater() Qt's real QScreen, every window loses its screen
        // and the next map segfaults in QWaylandWindow::devicePixelRatio (S17).
        QQmlEngine::setObjectOwnership(s, QQmlEngine::CppOwnership);
        return s;
      }
    return nullptr;
  }
};
