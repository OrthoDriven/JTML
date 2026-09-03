use cxx_qt_build::{CxxQtBuilder, QmlModule};

const QML_FILES: &[&str] = &[
    "qml/SettingsControl.qml",
    // add every other .qml here as you split the UI up
];

const RUST_FILES: &[&str] = &["src/qml_settings.rs"];

fn main() {
    let mut module = QmlModule::new("JTML.Settings");

    for file in QML_FILES {
        module = module.qml_file(file);
        println!("cargo:rerun-if-changed={file}");
    }

    for file in RUST_FILES {
        println!("cargo:rerun-if-changed={file}");
    }

    println!("cargo:rerun-if-changed=build.rs");

    CxxQtBuilder::new_qml_module(module)
        .files(RUST_FILES)
        .build();
}
