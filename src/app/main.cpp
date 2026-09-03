// Copyright 2023 Gary J. Miller Orthopaedic Biomechanics Lab
// SPDX-License-Identifier: AGPL-3.0-only OR MIT

#include <QVTKOpenGLNativeWidget.h>

#include <QCommandLineOption>
#include <QCommandLineParser>
#include <QSurfaceFormat>
#include <QtWidgets/QApplication>

#include "view/mainscreen.h"
#include "view/settings_impl.h"

int main(int argc, char* argv[]) {
    QSurfaceFormat::setDefaultFormat(QVTKOpenGLNativeWidget::defaultFormat());

    QApplication app(argc, argv);

    QCommandLineParser parser;
    parser.setApplicationDescription("Joint Track Machine Learning");
    parser.addHelpOption();

    QCommandLineOption settings_impl_option(
        "settings-impl",
        "Settings UI implementation: widgets or qml.",
        "implementation",
        "qml");

    parser.addOption(settings_impl_option);
    parser.process(app);

    const QString settings_impl = parser.value(settings_impl_option);

    if (settings_impl.compare("widgets", Qt::CaseInsensitive) == 0) {
        set_settings_impl(SettingsImpl::CppWidgets);
    } else if (settings_impl.compare("qml", Qt::CaseInsensitive) == 0) {
        set_settings_impl(SettingsImpl::RustQml);
    } else {
        qFatal(
            "Invalid --settings-impl value '%s'. Expected 'widgets' or 'qml'.",
            qPrintable(settings_impl));
    }

    MainScreen w;
    w.show();

    return app.exec();
}
