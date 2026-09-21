#include "ui/mixer/mix_send_strip.h"

#include "core/flow8_device.h"
#include "ui/ui_text.h"
#include "ui/widgets/fader_widget.h"
#include "ui/widgets/meter_widget.h"

#include <QComboBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QSignalBlocker>
#include <QVBoxLayout>

#include <cmath>

namespace flow8::ui {
namespace {

double dbToNormalized(const double db)
{
    return db <= -70.0 ? 0.0 : qBound(0.0, (db + 70.0) / 80.0, 1.0);
}

QString destinationName(const model::RoutingDestination destination)
{
    switch (destination) {
    case model::RoutingDestination::Main: return uiText("Main Mix");
    case model::RoutingDestination::Monitor1: return uiText("Monitor 1 Send");
    case model::RoutingDestination::Monitor2: return uiText("Monitor 2 Send");
    case model::RoutingDestination::Fx1: return uiText("FX 1 Send");
    case model::RoutingDestination::Fx2: return uiText("FX 2 Send");
    }
    return uiText("Unknown");
}

QString objectPrefix(const model::RoutingDestination destination)
{
    switch (destination) {
    case model::RoutingDestination::Main: return QStringLiteral("main");
    case model::RoutingDestination::Monitor1: return QStringLiteral("monitor1");
    case model::RoutingDestination::Monitor2: return QStringLiteral("monitor2");
    case model::RoutingDestination::Fx1: return QStringLiteral("fx1");
    case model::RoutingDestination::Fx2: return QStringLiteral("fx2");
    }
    return QStringLiteral("unknown");
}

} // namespace

MixSendStrip::MixSendStrip(Flow8Device& device, const int inputIndex,
                           const model::RoutingDestination destination, QWidget* parent)
    : QWidget(parent)
    , device_(device)
    , inputIndex_(inputIndex)
    , destination_(destination)
    , name_(new QLabel(this))
    , type_(new QLabel(this))
    , destinationLabel_(new QLabel(this))
    , fader_(new FaderWidget(this))
    , meter_(new MeterWidget(this))
{
    const QString prefix = objectPrefix(destination_);
    setObjectName(QStringLiteral("%1SendStrip%2").arg(prefix).arg(inputIndex_));
    setProperty("class", QStringLiteral("mixSendStrip"));
    setMinimumWidth(132);
    setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Expanding);

    name_->setProperty("class", QStringLiteral("channelName"));
    name_->setAlignment(Qt::AlignCenter);
    type_->setProperty("class", QStringLiteral("inputBadge"));
    type_->setAlignment(Qt::AlignCenter);
    destinationLabel_->setProperty("class", QStringLiteral("stripIndicator"));
    destinationLabel_->setAlignment(Qt::AlignCenter);
    fader_->setObjectName(QStringLiteral("%1Send%2").arg(prefix).arg(inputIndex_));

    if (destination_ == model::RoutingDestination::Monitor1
        || destination_ == model::RoutingDestination::Monitor2) {
        monitorMode_ = new QComboBox(this);
        monitorMode_->setObjectName(QStringLiteral("%1Mode%2").arg(prefix).arg(inputIndex_));
        connect(monitorMode_, &QComboBox::currentIndexChanged, this,
                [this](const int index) {
                    if (index >= 0) {
                        (void)device_.setMonitorSendMode(
                            inputIndex_, sendIndex(),
                            static_cast<model::MonitorSendMode>(
                                monitorMode_->itemData(index).toInt()));
                    }
                });
    }

    auto* faderRow = new QHBoxLayout;
    faderRow->setContentsMargins(6, 0, 6, 0);
    faderRow->setSpacing(8);
    faderRow->addWidget(meter_);
    faderRow->addWidget(fader_, 1, Qt::AlignHCenter);

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(9, 11, 9, 9);
    layout->setSpacing(7);
    layout->addWidget(name_);
    layout->addWidget(type_);
    layout->addWidget(destinationLabel_);
    layout->addLayout(faderRow, 1);
    if (monitorMode_ != nullptr) {
        layout->addWidget(monitorMode_);
    }

    connect(fader_, &FaderWidget::valueChanged, this, [this](const double value) {
        if (destination_ == model::RoutingDestination::Main) {
            (void)device_.setChannelFader(inputIndex_, value);
        } else {
            (void)device_.setChannelSendLevel(inputIndex_, sendIndex(), value);
        }
    });
    connect(&device_.state(), &Flow8State::channelChanged, this,
            [this](const int index) {
                if (index == inputIndex_) refresh();
            });
    connect(&device_.state(), &Flow8State::preferencesChanged,
            this, &MixSendStrip::refresh);
    retranslateUi();
}

void MixSendStrip::refresh()
{
    const auto* channel = device_.state().channel(inputIndex_);
    if (channel == nullptr) {
        setEnabled(false);
        return;
    }
    setEnabled(device_.state().connectionState() == ConnectionState::Ready);
    setVisible(channel->visible.value.value_or(true));
    name_->setText(channel->name.value.has_value() && !channel->name.value->isEmpty()
        ? *channel->name.value : inputDisplayName(channel->inputId));
    type_->setText(inputTypeDisplayName(channel->inputType));
    destinationLabel_->setText(destinationName(destination_));
    const QSignalBlocker faderBlocker(fader_);
    fader_->setValue(normalizedLevel());
    meter_->setLevel(channel->meterLevel.value.value_or(0.0));
    meter_->setPeak(channel->meterPeak.value.value_or(0.0));
    meter_->setClipping(channel->clipping.value.value_or(false));
    if (monitorMode_ != nullptr) {
        const QSignalBlocker blocker(monitorMode_);
        monitorMode_->setCurrentIndex(monitorMode_->findData(static_cast<int>(
            channel->monitorSends[static_cast<std::size_t>(sendIndex())]
                .mode.value.value_or(model::MonitorSendMode::PostFader))));
    }
}

void MixSendStrip::retranslateUi()
{
    if (monitorMode_ != nullptr) {
        const int selected = monitorMode_->currentData().toInt();
        const QSignalBlocker blocker(monitorMode_);
        monitorMode_->clear();
        monitorMode_->addItem(uiText("Pre-Fader"),
                              static_cast<int>(model::MonitorSendMode::PreFader));
        monitorMode_->addItem(uiText("Post-Fader"),
                              static_cast<int>(model::MonitorSendMode::PostFader));
        monitorMode_->setCurrentIndex(qMax(0, monitorMode_->findData(selected)));
    }
    fader_->retranslateUi();
    refresh();
}

int MixSendStrip::sendIndex() const noexcept
{
    return static_cast<int>(destination_) - 1;
}

double MixSendStrip::normalizedLevel() const noexcept
{
    const auto* channel = device_.state().channel(inputIndex_);
    if (channel == nullptr) {
        return 0.0;
    }
    switch (destination_) {
    case model::RoutingDestination::Main:
        return channel->fader.value.value_or(0.0);
    case model::RoutingDestination::Monitor1:
    case model::RoutingDestination::Monitor2:
        return dbToNormalized(channel->monitorSends[static_cast<std::size_t>(sendIndex())]
            .levelDb.value.value_or(-144.0));
    case model::RoutingDestination::Fx1:
    case model::RoutingDestination::Fx2:
        return dbToNormalized(channel->fxSendLevelDb[static_cast<std::size_t>(sendIndex() - 2)]
            .value.value_or(-144.0));
    }
    return 0.0;
}

} // namespace flow8::ui
