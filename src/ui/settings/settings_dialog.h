#pragma once

#include <QDialog>
#include <QVector>

class QComboBox;
class QCheckBox;
class QDialogButtonBox;
class QLabel;
class QPushButton;
class QTabWidget;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class LanguageManager;

class SettingsDialog final : public QDialog {
    Q_OBJECT

public:
    SettingsDialog(Flow8Device& device, LanguageManager& languageManager,
                   QWidget* parent = nullptr);

protected:
    void changeEvent(QEvent* event) override;

private:
    void retranslateUi();
    void commitPreferences();

    Flow8Device& device_;
    LanguageManager& languageManager_;
    QLabel* title_ {};
    QTabWidget* tabs_ {};
    QLabel* connectionName_ {};
    QLabel* connectionValue_ {};
    QLabel* transportName_ {};
    QLabel* transportValue_ {};
    QLabel* simulatorName_ {};
    QLabel* simulatorValue_ {};
    QLabel* midiName_ {};
    QLabel* midiValue_ {};
    QLabel* languageName_ {};
    QComboBox* languageSelector_ {};
    QLabel* note_ {};
    QCheckBox* showMuteButtons_ {};
    QCheckBox* showChannelIcons_ {};
    QVector<QCheckBox*> channelVisibility_;
    QComboBox* controlGesture_ {};
    QComboBox* eqEditingMode_ {};
    QCheckBox* outputDelayIndicator_ {};
    QCheckBox* outputLevel10dBV_ {};
    QComboBox* footswitchMode_ {};
    QPushButton* sysexDump_ {};
    QLabel* diagnosticsNote_ {};
    QDialogButtonBox* buttons_ {};
};

} // namespace flow8::ui
