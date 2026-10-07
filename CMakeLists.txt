cmake_minimum_required(VERSION 3.24)
project(snip VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(Qt6 6.8 REQUIRED COMPONENTS Core Gui Qml Quick QmlImportScanner)
find_package(Qt6 REQUIRED COMPONENTS GuiPrivate)
find_package(KF6CoreAddons REQUIRED)
find_package(LayerShellQt REQUIRED)
find_package(PkgConfig REQUIRED)
pkg_check_modules(WAYLAND_CLIENT REQUIRED IMPORTED_TARGET wayland-client)
# rusqlite links the system SQLite; a Rust staticlib does not carry that along.
pkg_check_modules(SQLITE3 REQUIRED IMPORTED_TARGET sqlite3)

qt_standard_project_setup(REQUIRES 6.8)

# cxx-qt's CMake glue (Corrosion inside). Packagers point
# FETCHCONTENT_SOURCE_DIR_CXXQT at a local checkout to build offline.
find_package(CxxQt QUIET)
if(NOT CxxQt_FOUND)
  include(FetchContent)
  FetchContent_Declare(CxxQt GIT_REPOSITORY https://github.com/kdab/cxx-qt-cmake.git GIT_TAG 0.10.0)
  FetchContent_MakeAvailable(CxxQt)
endif()

# Rust: the Engine bridge (QML module Snip.Core) ...
cxx_qt_import_crate(
  MANIFEST_PATH crates/snip-bridge/Cargo.toml
  CRATES snip_bridge
  QT_MODULES Qt::Core Qt::Gui Qt::Qml Qt::Quick)
cxx_qt_import_qml_module(snip_core_qml URI "Snip.Core" SOURCE_CRATE snip_bridge)

# ... and the `snip` CLI (no Qt).
corrosion_import_crate(MANIFEST_PATH crates/snip-cli/Cargo.toml CRATES snip-cli)

qt_add_executable(snipd app/src/main.cpp)

set_source_files_properties(app/qml/Theme.qml PROPERTIES QT_QML_SINGLETON_TYPE TRUE)
qt_add_qml_module(snipd
  URI Snip.Ui
  VERSION 1.0
  QML_FILES
    app/qml/Main.qml
    app/qml/Theme.qml
    app/qml/Overlay.qml
    app/qml/Magnifier.qml
    app/qml/QuickAccess.qml
    app/qml/Card.qml
    app/qml/CardMenu.qml
    app/qml/Pill.qml
    app/qml/IconButton.qml
    app/qml/FrostButton.qml
    app/qml/Icon.qml
    app/qml/Toast.qml
    app/qml/Countdown.qml
    app/qml/Preview.qml
    app/qml/Gallery.qml
    app/qml/Editor.qml
    app/qml/EditorItem.qml
    app/qml/ToolButton.qml
    app/qml/editor.js
  SOURCES
    app/src/frozenframe.cpp app/src/frozenframe.h
    app/src/loupe.cpp app/src/loupe.h
    app/src/platform.cpp app/src/platform.h)

target_link_libraries(snipd PRIVATE
  snip_core_qml
  Qt::Quick Qt::GuiPrivate
  KF6::CoreAddons LayerShellQt::Interface PkgConfig::WAYLAND_CLIENT PkgConfig::SQLITE3)
target_include_directories(snipd PRIVATE app/src)
qt_import_qml_plugins(snipd)

include(GNUInstallDirs)
install(TARGETS snipd RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
install(PROGRAMS $<TARGET_FILE:snip> DESTINATION ${CMAKE_INSTALL_BINDIR})
install(FILES systemd/snipd.service DESTINATION lib/systemd/user)
install(FILES hypr/snip.lua DESTINATION ${CMAKE_INSTALL_DATADIR}/snip/hypr)
install(FILES app/snip.desktop DESTINATION ${CMAKE_INSTALL_DATADIR}/applications)
install(FILES app/icons/snip.svg DESTINATION ${CMAKE_INSTALL_DATADIR}/icons/hicolor/scalable/apps)
install(FILES LICENSE DESTINATION ${CMAKE_INSTALL_DATADIR}/licenses/snip OPTIONAL)
