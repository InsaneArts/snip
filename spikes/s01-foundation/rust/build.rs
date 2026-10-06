use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("com.snip.spike").qml_file("../qml/Main.qml"))
        .qt_module("Qml")
        .qt_module("Quick")
        .files(["src/bridge.rs"])
        .build();
}
