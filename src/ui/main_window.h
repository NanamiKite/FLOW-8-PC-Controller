#pragma once

#include <QMainWindow>

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class ConnectionBar;

class MainWindow final : public QMainWindow {
    Q_OBJECT

public:
    explicit MainWindow(Flow8Device& device, QWidget* parent = nullptr);

private:
    Flow8Device& device_;
    ConnectionBar* connectionBar_ {};
};

} // namespace flow8::ui
