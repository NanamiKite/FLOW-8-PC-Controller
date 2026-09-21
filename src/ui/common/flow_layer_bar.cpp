#include "ui/common/flow_layer_bar.h"

#include "ui/ui_text.h"

#include <QButtonGroup>
#include <QHBoxLayout>
#include <QSignalBlocker>
#include <QToolButton>
#include <QVariant>

#include <array>

namespace flow8::ui {
namespace {

constexpr std::array<const char*, 8> objectNames {
    "layerMixer", "layerStage", "layerFx1", "layerFx2",
    "layerMonitor1", "layerMonitor2", "layerMain", "layerMainOut",
};

constexpr std::array<const char*, 8> roles {
    "mix", "mix", "fx", "fx", "monitor", "monitor", "main", "main",
};

} // namespace

FlowLayerBar::FlowLayerBar(QWidget* parent)
    : QWidget(parent)
    , group_(new QButtonGroup(this))
{
    setObjectName(QStringLiteral("flowLayerBar"));
    auto* layout = new QHBoxLayout(this);
    layout->setContentsMargins(18, 7, 18, 7);
    layout->setSpacing(4);
    layout->addStretch();

    group_->setExclusive(true);
    for (int index = 0; index < static_cast<int>(objectNames.size()); ++index) {
        auto* button = new QToolButton(this);
        button->setObjectName(QString::fromLatin1(objectNames[static_cast<std::size_t>(index)]));
        button->setProperty("class", QStringLiteral("layerButton"));
        button->setProperty("layerRole", QString::fromLatin1(roles[static_cast<std::size_t>(index)]));
        button->setCheckable(true);
        button->setFocusPolicy(Qt::StrongFocus);
        group_->addButton(button, index);
        buttons_.append(button);
        layout->addWidget(button);
    }
    layout->addStretch();

    connect(group_, &QButtonGroup::idClicked, this, [this](const int id) {
        if (id < 0 || id >= buttons_.size()) {
            return;
        }
        currentLayer_ = static_cast<FlowLayer>(id);
        emit layerSelected(currentLayer_);
    });

    setCurrentLayer(FlowLayer::Mixer);
    retranslateUi();
}

FlowLayer FlowLayerBar::currentLayer() const noexcept
{
    return currentLayer_;
}

void FlowLayerBar::setCurrentLayer(const FlowLayer layer)
{
    const int index = static_cast<int>(layer);
    if (index < 0 || index >= buttons_.size()) {
        return;
    }
    currentLayer_ = layer;
    const QSignalBlocker blocker(group_);
    buttons_.at(index)->setChecked(true);
}

void FlowLayerBar::retranslateUi()
{
    const std::array<const char*, 8> labels {
        "Mixer", "Stage", "FX 1", "FX 2", "Monitor 1", "Monitor 2",
        "Main", "Main Out",
    };
    for (int index = 0; index < buttons_.size(); ++index) {
        buttons_[index]->setText(uiText(labels[static_cast<std::size_t>(index)]));
    }
}

} // namespace flow8::ui
