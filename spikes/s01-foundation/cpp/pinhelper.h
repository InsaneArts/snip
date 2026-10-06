#pragma once

#include <QGuiApplication>
#include <QObject>
#include <QQuickWindow>
#include <QWindow>
#include <qpa/qplatformnativeinterface.h>
#include <wayland-client-protocol.h>
#include <LayerShellQt/window.h>

// Lock mode for pins (S6). QWindow::setMask can't express "no input at all":
// an empty QRegion means "no mask" (full input), and regions outside the
// window are clipped to empty. So set an empty wl_region as the input region
// directly on Qt's wl_surface; it is applied with Qt's next commit.
class PinHelper : public QObject
{
  Q_OBJECT
public:
  using QObject::QObject;
  // Position a layer-shell window at a logical (x, y) on its output (margins).
  Q_INVOKABLE void placeLayer(QWindow* w, int x, int y)
  {
    if (auto* lw = LayerShellQt::Window::get(w))
      lw->setMargins(QMargins(x, y, 0, 0));
  }

  Q_INVOKABLE void setClickThrough(QWindow* w, bool on)
  {
    if (!w || !w->handle())
      return;
    auto* ni = QGuiApplication::platformNativeInterface();
    auto* surface = static_cast<wl_surface*>(ni->nativeResourceForWindow("surface", w));
    auto* compositor = static_cast<wl_compositor*>(ni->nativeResourceForIntegration("compositor"));
    if (!surface || !compositor)
      return;
    if (on) {
      wl_region* r = wl_compositor_create_region(compositor); // empty region
      wl_surface_set_input_region(surface, r);
      wl_region_destroy(r);
    } else {
      wl_surface_set_input_region(surface, nullptr); // infinite = whole surface
    }
    // Input region is double-buffered surface state. Qt only commits when it
    // renders a new frame, and an unchanged scene renders nothing, so commit
    // it ourselves (no buffer attached: only the region is applied).
    wl_surface_commit(surface);
    wl_display_flush(static_cast<wl_display*>(ni->nativeResourceForIntegration("display")));
  }
};
