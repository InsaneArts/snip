use cxx_qt_build::{CxxQtBuilder, QmlModule};

include!("../version.rs");

fn main() {
    snip_version();
    // Only Rust-defined types live in this module; the QML UI is a separate
    // CMake qt_add_qml_module (Snip.Ui), compiled ahead of time by qmlcachegen.
    CxxQtBuilder::new_qml_module(QmlModule::new("Snip.Core"))
        .qt_module("Qml")
        .files(["src/bridge.rs"])
        .build();
}
