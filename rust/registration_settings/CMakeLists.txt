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
    MANIFEST_PATH
        "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"

    CRATES
        registration_settings

    LOCKED

    QT_MODULES
        Qt6::Core
        Qt6::Gui
        Qt6::Qml
        Qt6::Quick
        Qt6::QuickControls2
)

cxx_qt_import_qml_module(
    jtml_settings_rust

    URI
        "JTML.Settings"

  SOURCE_CRATE
    registration_settings

)
