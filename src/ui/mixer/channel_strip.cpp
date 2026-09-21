#include "ui/mixer/channel_strip.h"

#include "core/flow8_device.h"

#include <QCheckBox>
#include <QDial>
#include <QLabel>
#include <QSignalBlocker>
#include <QSlider>
#include <QVBoxLayout>

#include <cmath>

namespace flow8::ui {

ChannelStrip::ChannelStrip(Flow8Device& device, const int channelIndex, QWidget* parent)
    : QWidget(parent)
    , device_(device)
    , channelIndex_(channelIndex)
    , nameLabel_(new QLabel(this))
    , gainSlider_(new QSlider(Qt::Horizontal, this))
    , faderSlider_(new QSlider(Qt::Vertical, this))
    , muteButton_(new QCheckBox(QStringLiteral("Mute"), this))
    , soloButton_(new QCheckBox(QStringLiteral("Solo"), this))
    , panDial_(new QDial(this))
{
    setObjectName(QStringLiteral("channelStrip%1").arg(channelIndex));
    setMinimumWidth(118);
    nameLabel_->setAlignment(Qt::AlignCenter);
    gainSlider_->setRange(0, 1000);
    gainSlider_->setObjectName(QStringLiteral("gain%1").arg(channelIndex));
    faderSlider_->setRange(0, 1000);
    faderSlider_->setObjectName(QStringLiteral("fader%1").arg(channelIndex));
    faderSlider_->setMinimumHeight(210);
    muteButton_->setObjectName(QStringLiteral("mute%1").arg(channelIndex));
    soloButton_->setObjectName(QStringLiteral("solo%1").arg(channelIndex));
    panDial_->setRange(-100, 100);
    panDial_->setNotchesVisible(true);
    panDial_->setObjectName(QStringLiteral("pan%1").arg(channelIndex));

    auto* layout = new QVBoxLayout(this);
    layout->addWidget(nameLabel_);
    layout->addWidget(new QLabel(QStringLiteral("Gain"), this));
    layout->addWidget(gainSlider_);
    layout->addWidget(faderSlider_, 1, Qt::AlignHCenter);
    layout->addWidget(new QLabel(QStringLiteral("Pan"), this), 0, Qt::AlignHCenter);
    layout->addWidget(panDial_, 0, Qt::AlignHCenter);
    layout->addWidget(muteButton_);
    layout->addWidget(soloButton_);

    connect(gainSlider_, &QSlider::valueChanged, this,
            [this](const int value) { (void)device_.setChannelGain(channelIndex_, value / 1000.0); });
    connect(faderSlider_, &QSlider::valueChanged, this,
            [this](const int value) { (void)device_.setChannelFader(channelIndex_, value / 1000.0); });
    connect(muteButton_, &QCheckBox::toggled, this,
            [this](const bool checked) { (void)device_.setChannelMuted(channelIndex_, checked); });
    connect(soloButton_, &QCheckBox::toggled, this,
            [this](const bool checked) { (void)device_.setChannelSoloed(channelIndex_, checked); });
    connect(panDial_, &QDial::valueChanged, this,
            [this](const int value) { (void)device_.setChannelPan(channelIndex_, value / 100.0); });
    refresh();
}

int ChannelStrip::channelIndex() const noexcept
{
    return channelIndex_;
}

void ChannelStrip::refresh()
{
    const auto* channel = device_.state().channel(channelIndex_);
    const bool enabled = channel != nullptr
        && device_.state().connectionState() == ConnectionState::Ready;
    setEnabled(enabled);
    if (channel == nullptr) {
        nameLabel_->setText(QStringLiteral("Unknown"));
        return;
    }

    nameLabel_->setText(channel->name.value.value_or(QStringLiteral("Unknown")));
    const QSignalBlocker gainBlocker(gainSlider_);
    const QSignalBlocker faderBlocker(faderSlider_);
    const QSignalBlocker muteBlocker(muteButton_);
    const QSignalBlocker soloBlocker(soloButton_);
    const QSignalBlocker panBlocker(panDial_);
    gainSlider_->setValue(static_cast<int>(std::lround(channel->gain.value.value_or(0.0) * 1000.0)));
    faderSlider_->setValue(static_cast<int>(std::lround(channel->fader.value.value_or(0.0) * 1000.0)));
    muteButton_->setChecked(channel->muted.value.value_or(false));
    soloButton_->setChecked(channel->soloed.value.value_or(false));
    panDial_->setValue(static_cast<int>(std::lround(channel->pan.value.value_or(0.0) * 100.0)));
}

} // namespace flow8::ui
