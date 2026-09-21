#include "ui/mixer/mixer_widget.h"

#include "core/flow8_device.h"
#include "ui/inspector/detail_panel.h"
#include "ui/inspector/inspector_widget.h"
#include "ui/mixer/channel_strip.h"
#include "ui/ui_text.h"

#include <QButtonGroup>
#include <QFrame>
#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QScrollArea>
#include <QSignalBlocker>
#include <QStackedWidget>
#include <QToolButton>
#include <QVBoxLayout>

#include <array>

namespace flow8::ui {

MixerWidget::MixerWidget(Flow8Device& device, QWidget* parent)
    : QWidget(parent)
    , device_(device)
    , stripsContainer_(new QWidget(this))
    , stripsLayout_(new QHBoxLayout(stripsContainer_))
    , destinationGroup_(new QButtonGroup(this))
    , routeTitle_(new QLabel(this))
    , masterButton_(new QPushButton(this))
    , inspectorStack_(new QStackedWidget(this))
    , masterPanel_(new DetailPanel(device_, inspectorStack_))
    , inputInspector_(new InspectorWidget(device_, inspectorStack_))
{
    setObjectName(QStringLiteral("mixerView"));
    stripsContainer_->setObjectName(QStringLiteral("stripsContainer"));
    stripsContainer_->setSizePolicy(QSizePolicy::MinimumExpanding, QSizePolicy::Expanding);
    stripsLayout_->setAlignment(Qt::AlignLeft);
    stripsLayout_->setContentsMargins(14, 14, 14, 14);
    stripsLayout_->setSpacing(7);

    auto* scroll = new QScrollArea(this);
    scroll->setObjectName(QStringLiteral("mixerScroll"));
    scroll->setFrameShape(QFrame::NoFrame);
    scroll->setWidgetResizable(true);
    scroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAsNeeded);
    scroll->setVerticalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    scroll->setWidget(stripsContainer_);

    destinationGroup_->setExclusive(true);
    masterPanel_->setObjectName(QStringLiteral("destinationMasterPanel"));
    inputInspector_->setObjectName(QStringLiteral("mixerInputInspector"));
    auto* destinationBar = new QHBoxLayout;
    destinationBar->setContentsMargins(14, 9, 14, 7);
    destinationBar->setSpacing(5);
    routeTitle_->setProperty("class", QStringLiteral("sectionLabel"));
    destinationBar->addWidget(routeTitle_);
    destinationBar->addSpacing(10);
    constexpr std::array<const char*, 5> names {
        "destinationMain", "destinationMonitor1", "destinationMonitor2",
        "destinationFx1", "destinationFx2",
    };
    for (int index = 0; index < static_cast<int>(names.size()); ++index) {
        auto* button = new QToolButton(this);
        button->setObjectName(QString::fromLatin1(names[static_cast<std::size_t>(index)]));
        button->setProperty("class", QStringLiteral("destinationButton"));
        button->setCheckable(true);
        button->setFocusPolicy(Qt::StrongFocus);
        destinationGroup_->addButton(button, index);
        destinationButtons_.append(button);
        destinationBar->addWidget(button);
    }
    destinationBar->addStretch();
    masterButton_->setObjectName(QStringLiteral("showDestinationMaster"));
    destinationBar->addWidget(masterButton_);

    inspectorStack_->addWidget(masterPanel_);
    inspectorStack_->addWidget(inputInspector_);
    inspectorStack_->setMinimumWidth(390);
    inspectorStack_->setMaximumWidth(520);

    auto* workspace = new QHBoxLayout;
    workspace->setContentsMargins(0, 0, 0, 0);
    workspace->setSpacing(8);
    workspace->addWidget(scroll, 1);
    workspace->addWidget(inspectorStack_);

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);
    layout->addLayout(destinationBar);
    layout->addLayout(workspace, 1);

    connect(destinationGroup_, &QButtonGroup::idClicked, this, [this](const int id) {
        if (id >= 0 && id < 5) {
            setDestination(static_cast<model::RoutingDestination>(id));
        }
    });
    connect(masterButton_, &QPushButton::clicked, this, &MixerWidget::showMaster);

    connect(&device_.state(), &Flow8State::stateReset, this, &MixerWidget::rebuild);
    connect(&device_.state(), &Flow8State::channelChanged, this, &MixerWidget::refreshChannel);
    connect(&device_.state(), &Flow8State::inputMeterChanged, this,
            [this](const int index) {
                if (index >= 0 && index < channelStrips_.size()) {
                    channelStrips_[index]->refreshMeter();
                }
            });
    connect(&device_.state(), &Flow8State::routingChanged,
            this, &MixerWidget::refreshAll);
    connect(&device_.state(), &Flow8State::connectionStateChanged,
            this, [this](ConnectionState) { refreshAll(); });
    connect(&device_.state(), &Flow8State::preferencesChanged,
            this, &MixerWidget::refreshAll);
    rebuild();
}

void MixerWidget::retranslateUi()
{
    routeTitle_->setText(uiText("Source → Destination"));
    const std::array<const char*, 5> labels {
        "Main", "Monitor 1", "Monitor 2", "FX 1", "FX 2",
    };
    for (int index = 0; index < destinationButtons_.size(); ++index) {
        destinationButtons_[index]->setText(
            uiText(labels[static_cast<std::size_t>(index)]));
    }
    masterButton_->setText(uiText("Destination Master"));
    for (auto* strip : channelStrips_) {
        strip->retranslateUi();
    }
    masterPanel_->retranslateUi();
    inputInspector_->retranslateUi();
}

void MixerWidget::rebuild()
{
    while (auto* item = stripsLayout_->takeAt(0)) {
        delete item->widget();
        delete item;
    }
    channelStrips_.clear();
    for (int index = 0; index < device_.state().channels().size(); ++index) {
        auto* strip = new ChannelStrip(device_, index, stripsContainer_);
        strip->setDestination(destination_);
        channelStrips_.append(strip);
        stripsLayout_->addWidget(strip, 1);
        connect(strip, &ChannelStrip::selected, this, &MixerWidget::selectChannel);
        connect(strip, &ChannelStrip::editRequested, this,
                [this](const int channel) { selectChannel(channel); });
    }
    if (!channelStrips_.isEmpty()) {
        selectedChannel_ = 0;
        for (int channel = 0; channel < channelStrips_.size(); ++channel) {
            channelStrips_[channel]->setSelected(channel == selectedChannel_);
        }
    }
    setDestination(destination_);
    showMaster();
    refreshAll();
}

void MixerWidget::selectChannel(const int index)
{
    if (index < 0 || index >= channelStrips_.size()) {
        return;
    }
    for (int channel = 0; channel < channelStrips_.size(); ++channel) {
        channelStrips_[channel]->setSelected(channel == index);
    }
    selectedChannel_ = index;
    inputInspector_->setSelectedChannel(index);
    inputInspector_->setSelectedDestination(destination_);
    inspectorStack_->setCurrentWidget(inputInspector_);
}

void MixerWidget::showMaster()
{
    const int destinationIndex = static_cast<int>(destination_);
    if (destination_ == model::RoutingDestination::Fx1
        || destination_ == model::RoutingDestination::Fx2) {
        masterPanel_->showFx(destinationIndex - static_cast<int>(model::RoutingDestination::Fx1));
    } else {
        masterPanel_->showBus(model::busIndexForDestination(destination_));
    }
    inspectorStack_->setCurrentWidget(masterPanel_);
}

void MixerWidget::refreshChannel(const int index)
{
    if (index >= 0 && index < channelStrips_.size()) {
        channelStrips_.at(index)->refresh();
    }
}

void MixerWidget::refreshAll()
{
    for (auto* strip : channelStrips_) {
        strip->refresh();
    }
    masterPanel_->refresh();
    inputInspector_->refresh();
}

void MixerWidget::setDestination(const model::RoutingDestination destination)
{
    const bool showingInput = inspectorStack_->currentWidget() == inputInspector_;
    destination_ = destination;
    const int index = static_cast<int>(destination_);
    if (index >= 0 && index < destinationButtons_.size()) {
        const QSignalBlocker blocker(destinationGroup_);
        destinationButtons_[index]->setChecked(true);
    }
    for (auto* strip : channelStrips_) {
        strip->setDestination(destination_);
    }
    inputInspector_->setSelectedDestination(destination_);
    if (showingInput) {
        inspectorStack_->setCurrentWidget(inputInspector_);
    } else {
        showMaster();
    }
    emit destinationChanged(destination_);
}

model::RoutingDestination MixerWidget::destination() const noexcept
{
    return destination_;
}

int MixerWidget::selectedChannel() const noexcept
{
    return selectedChannel_;
}

} // namespace flow8::ui
