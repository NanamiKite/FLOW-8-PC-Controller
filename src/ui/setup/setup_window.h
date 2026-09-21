#pragma once

#include "model/preferences.h"

#include <QWidget>

#include <QVector>

class QCheckBox;
class QComboBox;
class QLabel;
class QLineEdit;
class QListWidget;
class QPushButton;
class QStackedWidget;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class DetailPanel;
class LanguageManager;

enum class SetupSection {
    ConfigureInputs,
    SnapshotLibrary,
    MixerSnapshots,
    Preferences,
    Routing,
    Info,
};

class SetupWindow final : public QWidget {
    Q_OBJECT

public:
    SetupWindow(Flow8Device& device, LanguageManager& languageManager,
                QWidget* parent = nullptr);

    void selectSection(SetupSection section);
    void refresh();
    void retranslateUi();

signals:
    void backRequested();
    void inputEditRequested(int channelIndex);

private:
    QWidget* buildConfigureInputsPage();
    QWidget* buildSnapshotLibraryPage();
    QWidget* buildMixerSnapshotsPage();
    QWidget* buildPreferencesPage();
    QWidget* buildInfoPage();
    void refreshSnapshots();
    void commitPreferences();

    Flow8Device& device_;
    LanguageManager& languageManager_;
    QPushButton* back_ {};
    QLabel* title_ {};
    QListWidget* navigation_ {};
    QStackedWidget* pages_ {};
    QVector<QLabel*> inputCardTitles_;
    QVector<QLabel*> inputCardTypes_;
    QVector<QCheckBox*> inputVisibility_;
    QVector<QPushButton*> inputEditButtons_;
    QListWidget* appSnapshots_ {};
    QLineEdit* snapshotName_ {};
    QComboBox* snapshotScope_ {};
    QPushButton* snapshotStore_ {};
    QPushButton* snapshotLoad_ {};
    QPushButton* snapshotRename_ {};
    QPushButton* snapshotDelete_ {};
    QVector<QPushButton*> hardwareSlots_;
    QPushButton* hardwareReset_ {};
    QCheckBox* showMuteButtons_ {};
    QCheckBox* showChannelIcons_ {};
    QComboBox* controlGesture_ {};
    QComboBox* eqEditingMode_ {};
    QCheckBox* outputDelayIndicator_ {};
    QCheckBox* outputLevel10dBV_ {};
    QComboBox* footswitchMode_ {};
    QComboBox* language_ {};
    QPushButton* sysExDump_ {};
    DetailPanel* routing_ {};
};

} // namespace flow8::ui
