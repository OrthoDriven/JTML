#ifndef SETTINGS_VIEW_MODEL_H_
#define SETTINGS_VIEW_MODEL_H_
#include <QObject>

class SettingsViewModel : public QObject {
    Q_OBJECT

public:
    explicit SettingsViewModel(QObject* parent = nullptr) : QObject(parent) {}
};
#endif  // SETTINGS_VIEW_MODEL_H_
