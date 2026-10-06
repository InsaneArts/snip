cmake_minimum_required(VERSION 3.24)
project(snip_spike_s01 LANGUAGES CXX)

set(CMAKE_AUTOMOC ON)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(Qt6 REQUIRED COMPONENTS Core Gui Qml Quick QmlImportScanner)
find_package(KF6CoreAddons REQUIRED)

find_package(CxxQt QUIET)
if(NOT CxxQt_FOUND)
  include(FetchContent)
  FetchContent_Declare(
    CxxQt
    GIT_REPOSITORY https://github.com/kdab/cxx-qt-cmake.git
    GIT_TAG 0.10.0
  )
  FetchContent_MakeAvailable(CxxQt)
endif()

cxx_qt_import_crate(
  MANIFEST_PATH rust/Cargo.toml
  CRATES snip_spike
  QT_MODULES Qt::Core Qt::Gui Qt::Qml Qt::Quick
)

cxx_qt_import_qml_module(snip_spike_qml
  URI "com.snip.spike"
  SOURCE_CRATE snip_spike)

add_executable(snip-spike
  cpp/main.cpp
  cpp/frozenframe.cpp
  cpp/frozenframe.h
  cpp/screens.h
  cpp/draghelper.cpp
  cpp/draghelper.h)

target_link_libraries(snip-spike PRIVATE snip_spike_qml Qt::Quick KF6::CoreAddons)
qt_import_qml_plugins(snip-spike)
