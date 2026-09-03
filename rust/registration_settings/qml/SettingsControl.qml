pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import JTML.Settings

Rectangle {
    implicitWidth: 1000
    implicitHeight: 700
    color: palette.window

    RegistrationEditor {
        id: registrationEditor
    }

    RowLayout {
        anchors.fill: parent
        anchors.margins: 24
        spacing: 24

        // -------------------------------------------------------------
        // Stage list
        // -------------------------------------------------------------

        ColumnLayout {
            Layout.preferredWidth: 320
            Layout.fillHeight: true
            spacing: 12

            Label {
                text: "Registration Pipeline"
                font.pixelSize: 24
                font.bold: true
            }

            Frame {
                Layout.fillWidth: true
                Layout.fillHeight: true

                ListView {
                    id: stageList

                    anchors.fill: parent
                    clip: true
                    spacing: 8

                    // The editor IS the model. No revision token, no manual
                    // invalidation — insert/remove/move notifications from
                    // QAbstractListModel drive everything.
                    model: registrationEditor

                    // Still no currentIndex binding: ListView writes to its
                    // own currentIndex and would destroy it. selected_stage
                    // stays the single source of truth.

                    delegate: ItemDelegate {
                        required property int index
                        required property string name
                        required property string costFunction

                        width: stageList.width

                        highlighted: registrationEditor.selected_stage === index
                        onClicked: registrationEditor.selected_stage = index

                        contentItem: Column {
                            spacing: 2

                            Label {
                                text: name
                                font.bold: registrationEditor.selected_stage === index
                            }

                            Label {
                                text: costFunction
                                opacity: 0.7
                            }
                        }
                    }

                    // Keep the selected stage visible after add/move/remove.
                    Connections {
                        target: registrationEditor
                        function onSelected_stageChanged() {
                            if (registrationEditor.selected_stage >= 0) {
                                stageList.positionViewAtIndex(
                                    registrationEditor.selected_stage,
                                    ListView.Contain);
                            }
                        }
                    }

                    ScrollBar.vertical: ScrollBar {
                        policy: ScrollBar.AlwaysOn
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true

                Button {
                    text: "+"
                    onClicked: registrationEditor.add_stage()
                }

                Button {
                    text: "-"
                    enabled: registrationEditor.selected_stage >= 0
                    onClicked: registrationEditor.remove_stage(registrationEditor.selected_stage)
                }

                Button {
                    text: "\u2191"
                    enabled: registrationEditor.selected_stage > 0
                    onClicked: registrationEditor.move_stage_up(registrationEditor.selected_stage)
                }

                Button {
                    text: "\u2193"

                    // stageList.count is a real property now, so this is
                    // natively reactive.
                    enabled: registrationEditor.selected_stage >= 0
                             && registrationEditor.selected_stage < stageList.count - 1

                    onClicked: registrationEditor.move_stage_down(registrationEditor.selected_stage)
                }

                Item {
                    Layout.fillWidth: true
                }
            }
        }

        // -------------------------------------------------------------
        // Selected stage editor
        // -------------------------------------------------------------

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true

            radius: 8
            color: palette.base
            border.color: palette.mid

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 24
                spacing: 16

                Label {
                    // Read the name back out of the model so it tracks any
                    // future rename without extra plumbing.
                    text: {
                        if (registrationEditor.selected_stage < 0) {
                            return "No Stage Selected";
                        }
                        return stageList.model.data(
                            stageList.model.index(registrationEditor.selected_stage, 0),
                            256);   // ROLE_NAME
                    }
                    font.pixelSize: 20
                    font.bold: true
                }

                Label {
                    text: "Cost Function"
                    visible: registrationEditor.selected_stage >= 0
                }

                ComboBox {
                    id: costFunctionCombo

                    Layout.preferredWidth: 420
                    visible: registrationEditor.selected_stage >= 0

                    // The option list never changes at runtime, so a plain
                    // Component.onCompleted fill is enough.
                    Component.onCompleted: {
                        let values = [];
                        for (let i = 0; i < registrationEditor.cost_function_count(); ++i) {
                            values.push(registrationEditor.cost_function_name(i));
                        }
                        model = values;
                    }

                    // ComboBox writes its own currentIndex in onActivated,
                    // which would kill a plain binding. A Binding element
                    // re-asserts itself afterwards.
                    Binding {
                        target: costFunctionCombo
                        property: "currentIndex"
                        restoreMode: Binding.RestoreBindingOrValue

                        value: registrationEditor.selected_stage < 0
                               ? -1
                               : registrationEditor.stage_cost_function_index(
                                     registrationEditor.selected_stage)
                    }

                    onActivated: function (index) {
                        registrationEditor.set_stage_cost_function(
                            registrationEditor.selected_stage, index);
                    }
                }

                Item {
                    Layout.fillHeight: true
                }

                RowLayout {
                    Layout.alignment: Qt.AlignRight

                    Button {
                        text: "Cancel"
                    }

                    Button {
                        text: "Save"
                    }
                }
            }
        }
    }
}
