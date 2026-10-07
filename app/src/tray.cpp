#include "tray.h"

#include <QCoreApplication>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusServiceWatcher>

static const QString WATCHER = QStringLiteral("org.kde.StatusNotifierWatcher");

Tray::Tray(const QString& iconDir, QObject* parent)
  : QObject(parent)
  , m_iconDir(iconDir)
  , m_service(QStringLiteral("org.kde.StatusNotifierItem-%1-1").arg(QCoreApplication::applicationPid()))
{
  QDBusConnection bus = QDBusConnection::sessionBus();
  bus.registerService(m_service);
  bus.registerObject(QStringLiteral("/StatusNotifierItem"), QStringLiteral("org.kde.StatusNotifierItem"), this,
                     QDBusConnection::ExportAllProperties | QDBusConnection::ExportAllSlots);
  auto* watcher = new QDBusServiceWatcher(WATCHER, bus, QDBusServiceWatcher::WatchForRegistration, this);
  connect(watcher, &QDBusServiceWatcher::serviceRegistered, this, &Tray::announce);
  announce();
}

void
Tray::announce()
{
  QDBusMessage call = QDBusMessage::createMethodCall(WATCHER, QStringLiteral("/StatusNotifierWatcher"), WATCHER,
                                                     QStringLiteral("RegisterStatusNotifierItem"));
  call << m_service;
  QDBusConnection::sessionBus().call(call, QDBus::NoBlock);
}
