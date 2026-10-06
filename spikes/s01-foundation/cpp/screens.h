#pragma once

#include <QGuiApplication>
#include <QObject>
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
      if (s->name() == name)
        return s;
    return nullptr;
  }
};
