qt_add_qml_module(jtml_settings_qml
    URI JTML.Settings

    QML_FILES
        SettingsControl.qml

    SOURCES
        qml_settings_dialog.cpp
        qml_settings_dialog.h
        settings_view_model.cpp
        settings_view_model.h
)

target_link_libraries(jtml_settings_qml
    PRIVATE
        Qt6::Quick
        Qt6::QuickWidgets
        Qt6::QuickControls2
)

message(STATUS "JTML.Settings resource: ${settings_resource_path}")
qt_query_qml_module(
    jtml_settings_qml
    URI settings_uri
    TARGET_PATH settings_target_path
    MODULE_RESOURCE_PATH settings_resource_path
    PLUGIN_TARGET settings_plugin
    QMLDIR settings_qmldir
    QML_FILES settings_qml_files
)

message(STATUS "JTML.Settings URI:          '${settings_uri}'")
message(STATUS "JTML.Settings target path:  '${settings_target_path}'")
message(STATUS "JTML.Settings resource:     '${settings_resource_path}'")
message(STATUS "JTML.Settings plugin:       '${settings_plugin}'")
message(STATUS "JTML.Settings qmldir:       '${settings_qmldir}'")
message(STATUS "JTML.Settings QML files:    '${settings_qml_files}'")
