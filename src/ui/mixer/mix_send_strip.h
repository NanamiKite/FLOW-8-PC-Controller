#pragma once

#include "model/routing.h"

#include <QWidget>

class QComboBox;
class QLabel;

namespace flow8 {
class Flow8Device;
}

namespace flow8::ui {

class FaderWidget;
class MeterWidget;

// A focused mixer-layer strip. MAIN uses the input channel fader, while MON
// and FX layers use the matching input-send value. FX output routing is not
// represented here; it belongs to the FX engine return panel.
class MixSendStrip final : public QWidget {
    Q_OBJECT

public:
    MixSendStrip(Flow8Device& device, int inputIndex,
                 model::RoutingDestination destination, QWidget* parent = nullptr);

    void refresh();
    void retranslateUi();

private:
    [[nodiscard]] int sendIndex() const noexcept;
    [[nodiscard]] double normalizedLevel() const noexcept;

    Flow8Device& device_;
    int inputIndex_ {};
    model::RoutingDestination destination_ {model::RoutingDestination::Main};
    QLabel* name_ {};
    QLabel* type_ {};
    QLabel* destinationLabel_ {};
    FaderWidget* fader_ {};
    MeterWidget* meter_ {};
    QComboBox* monitorMode_ {};
};

} // namespace flow8::ui
