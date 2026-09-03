import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    implicitWidth: 1000
    implicitHeight: 700
    color: palette.window

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 24

        Label {
            text: "JTML Registration Settings"
            font.pixelSize: 24
            font.bold: true
        }

        Label {
            text: "QML settings dialog is alive."
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
