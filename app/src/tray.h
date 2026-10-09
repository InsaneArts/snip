#pragma once

#include <QObject>
#include <QString>

// Snip in the bar (OMA-08, D35): a StatusNotifierItem, which omarchy-shell's
// tray draws. Its icon is named "-symbolic", so the bar tints it to its own
// colour. Not a menu item: a click activates it, and Snip opens its own menu
// (the bar's menus can't show keys).
class Tray : public QObject
{
  Q_OBJECT
  Q_CLASSINFO("D-Bus Interface", "org.kde.StatusNotifierItem")
  Q_PROPERTY(QString Category READ category CONSTANT)
  Q_PROPERTY(QString Id READ id CONSTANT)
  Q_PROPERTY(QString Title READ title CONSTANT)
  Q_PROPERTY(QString Status READ status CONSTANT)
  Q_PROPERTY(QString IconName READ iconName NOTIFY NewIcon)
  Q_PROPERTY(QString IconThemePath READ iconThemePath CONSTANT)
  Q_PROPERTY(bool ItemIsMenu READ itemIsMenu CONSTANT)

public:
  // `iconDir` holds snip-tray-symbolic.svg
  Tray(const QString& iconDir, QObject* parent);

  QString category() const { return QStringLiteral("ApplicationStatus"); }
  QString id() const { return QStringLiteral("snip"); }
  QString title() const { return QStringLiteral("Snip"); }
  QString status() const { return QStringLiteral("Active"); }
  // REC-06: red while Snip records (a whole-screen take shows no controls)
  QString iconName() const { return m_recording ? QStringLiteral("snip-tray-recording") : QStringLiteral("snip-tray-symbolic"); }
  QString iconThemePath() const { return m_iconDir; }
  bool itemIsMenu() const { return false; }
  void setRecording(bool on);

public Q_SLOTS:
  // a click; a right or middle click asks for the menu
  void Activate(int x, int y) { Q_EMIT activated(x, y); }
  void SecondaryActivate(int x, int y) { Q_EMIT menuRequested(x, y); }
  void ContextMenu(int x, int y) { Q_EMIT menuRequested(x, y); }
  void Scroll(int, const QString&) {}

Q_SIGNALS:
  void activated(int x, int y);
  void menuRequested(int x, int y);
  // the StatusNotifierItem signal: the bar reads IconName again
  Q_SCRIPTABLE void NewIcon();

private:
  // to the bar's watcher, now and whenever it (re)starts
  void announce();
  QString m_iconDir;
  QString m_service;
  bool m_recording = false;
};
