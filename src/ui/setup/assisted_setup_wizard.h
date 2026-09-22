#pragma once

#include <QDialog>

class QComboBox;
class QEvent;
class QLabel;
class QPushButton;
class QStackedWidget;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class AssistedSetupWizard final : public QDialog {
    Q_OBJECT

public:
    explicit AssistedSetupWizard(Flow8Device& device, QWidget* parent = nullptr);

protected:
    void changeEvent(QEvent* event) override;

private:
    void moveTo(int page);
    void refreshRecommendation();
    void retranslateUi();

    Flow8Device& device_;
    QStackedWidget* pages_ {};
    QComboBox* input_ {};
    QComboBox* sourceType_ {};
    QLabel* recommendation_ {};
    QLabel* instruction_ {};
    QLabel* evidence_ {};
    QPushButton* back_ {};
    QPushButton* next_ {};
    QPushButton* apply_ {};
    QPushButton* cancel_ {};
};

} // namespace flow8::ui
