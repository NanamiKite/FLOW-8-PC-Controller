#pragma once

#include "core/flow8_state.h"

#include <QWidget>

class QComboBox;
class QLabel;
class QPushButton;

namespace flow8::ui {

class ConnectionBar final : public QWidget {
    Q_OBJECT

public:
    explicit ConnectionBar(QWidget* parent = nullptr);

    void setConnectionState(ConnectionState state);
    void retranslateUi();

signals:
    void connectRequested();
    void disconnectRequested();
    void settingsRequested();

private:
    QComboBox* transportSelector_ {};
    QPushButton* connectButton_ {};
    QPushButton* settingsButton_ {};
    QLabel* statusLabel_ {};
    QLabel* subtitle_ {};
    ConnectionState state_ {ConnectionState::Disconnected};
};

} // namespace flow8::ui
