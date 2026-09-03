#ifndef QML_SETTINGS_DIALOG_H_
#define QML_SETTINGS_DIALOG_H_

#include <QDialog>
#include <QQuickWidget>
#include <QUrl>
#include <QVBoxLayout>

class QQuickWidget;

class QmlSettingsDialog : public QDialog {
    Q_OBJECT

public:
    explicit QmlSettingsDialog(QWidget* parent = nullptr);
    ~QmlSettingsDialog() override = default;

private:
    QQuickWidget* quick_widget_ = nullptr;
};
#endif  // QML_SETTINGS_DIALOG_H_
