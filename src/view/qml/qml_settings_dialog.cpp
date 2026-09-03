#include "qml_settings_dialog.h"

#include <QDebug>
#include <QFile>
#include <QQmlEngine>
#include <QQuickWidget>
#include <QVBoxLayout>
QmlSettingsDialog::QmlSettingsDialog(QWidget* parent) : QDialog(parent) {
    setWindowTitle("Optimizer Settings");
    resize(1000, 700);

    quick_widget_ = new QQuickWidget(this);
    quick_widget_->setResizeMode(QQuickWidget::SizeRootObjectToView);
    qDebug() << "qmldir exists:"
             << QFile::exists(":/qt/qml/JTML/Settings/qmldir");

    qDebug() << "SettingsControl exists:"
             << QFile::exists(":/qt/qml/JTML/Settings/SettingsControl.qml");

    qDebug() << "QML import paths:"
             << quick_widget_->engine()->importPathList();

    quick_widget_->loadFromModule("JTML.Settings", "SettingsControl");
    quick_widget_->loadFromModule("JTML.Settings", "SettingsControl");

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->addWidget(quick_widget_);
}
