#pragma once

#include <QMainWindow>

class QEvent;
class QStackedWidget;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class ConnectionBar;
class LanguageManager;
class MixerWidget;
class SessionStartView;

class MainWindow final : public QMainWindow {
    Q_OBJECT

public:
    MainWindow(Flow8Device& device, LanguageManager& languageManager,
               QWidget* parent = nullptr);

protected:
    void changeEvent(QEvent* event) override;

private:
    void retranslateUi();

    Flow8Device& device_;
    LanguageManager& languageManager_;
    ConnectionBar* connectionBar_ {};
    MixerWidget* mixer_ {};
    SessionStartView* sessionStart_ {};
    QStackedWidget* workspace_ {};
    bool pendingAssistedSetup_ {};
    bool pendingSnapshots_ {};
};

} // namespace flow8::ui
