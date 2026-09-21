#pragma once

#include <QWidget>

class QLabel;
class QPushButton;

namespace flow8::ui {

class SessionStartView final : public QWidget {
    Q_OBJECT

public:
    explicit SessionStartView(QWidget* parent = nullptr);
    void retranslateUi();

signals:
    void assistedSetupRequested();
    void loadSnapshotRequested();
    void startNewRequested();
    void continueSessionRequested();

private:
    QLabel* title_ {};
    QLabel* subtitle_ {};
    QLabel* note_ {};
    QPushButton* assistedSetup_ {};
    QPushButton* loadSnapshot_ {};
    QPushButton* startNew_ {};
    QPushButton* continueSession_ {};
};

} // namespace flow8::ui
