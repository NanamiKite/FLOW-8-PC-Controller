#include "ui/mixer/channel_strip.h"

#include "core/flow8_device.h"
#include "ui/widgets/fader_widget.h"
#include "ui/widgets/meter_widget.h"
#include "ui/ui_text.h"

#include <QHBoxLayout>
#include <QLabel>
#include <QMouseEvent>
#include <QSignalBlocker>
#include <QSlider>
#include <QStyle>
#include <QToolButton>
#include <QVBoxLayout>

#include <cmath>

namespace flow8::ui {
namespace {

QLabel* indicator(const QString& text, QWidget* parent)
{
    auto* label = new QLabel(text, parent);
    label->setProperty("class", QStringLiteral("stripIndicator"));
    label->setAlignment(Qt::AlignCenter);
    return label;
}

} // namespace

ChannelStrip::ChannelStrip(Flow8Device& device, const int channelIndex, QWidget* parent)
    : QWidget(parent)
    , device_(device)
    , channelIndex_(channelIndex)
    , nameLabel_(new QLabel(this))
    , iconLabel_(new QLabel(this))
    , typeLabel_(new QLabel(this))
    , eqIndicator_(indicator({}, this))
    , compressorIndicator_(indicator({}, this))
    , sendIndicator_(indicator({}, this))
    , panLabel_(new QLabel(this))
    , fader_(new FaderWidget(this))
    , meter_(new MeterWidget(this))
    , muteButton_(new QToolButton(this))
    , soloButton_(new QToolButton(this))
    , panSlider_(new QSlider(Qt::Horizontal, this))
{
    setObjectName(QStringLiteral("channelStrip%1").arg(channelIndex));
    setProperty("class", QStringLiteral("channelStrip"));
    setProperty("selected", false);
    setMinimumWidth(142);
    setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Expanding);
    setCursor(Qt::PointingHandCursor);

    nameLabel_->setProperty("class", QStringLiteral("channelName"));
    nameLabel_->setObjectName(QStringLiteral("channelName%1").arg(channelIndex));
    nameLabel_->setAlignment(Qt::AlignCenter);
    iconLabel_->setAlignment(Qt::AlignCenter);
    iconLabel_->setMinimumHeight(24);
    typeLabel_->setProperty("class", QStringLiteral("inputBadge"));
    typeLabel_->setAlignment(Qt::AlignCenter);

    fader_->setObjectName(QStringLiteral("fader%1").arg(channelIndex));
    muteButton_->setCheckable(true);
    muteButton_->setObjectName(QStringLiteral("mute%1").arg(channelIndex));
    muteButton_->setProperty("class", QStringLiteral("muteButton"));
    soloButton_->setCheckable(true);
    soloButton_->setObjectName(QStringLiteral("solo%1").arg(channelIndex));
    soloButton_->setProperty("class", QStringLiteral("soloButton"));
    panSlider_->setRange(-100, 100);
    panSlider_->setObjectName(QStringLiteral("pan%1").arg(channelIndex));

    auto* indicators = new QHBoxLayout;
    indicators->setSpacing(4);
    indicators->addWidget(eqIndicator_);
    indicators->addWidget(compressorIndicator_);
    indicators->addWidget(sendIndicator_);

    auto* faderRow = new QHBoxLayout;
    faderRow->setContentsMargins(8, 0, 8, 0);
    faderRow->setSpacing(8);
    faderRow->addWidget(meter_, 0, Qt::AlignVCenter);
    faderRow->addWidget(fader_, 1, Qt::AlignHCenter);

    auto* buttons = new QHBoxLayout;
    buttons->setSpacing(6);
    buttons->addWidget(muteButton_);
    buttons->addWidget(soloButton_);

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(10, 12, 10, 10);
    layout->setSpacing(7);
    layout->addWidget(iconLabel_);
    layout->addWidget(nameLabel_);
    layout->addWidget(typeLabel_);
    layout->addWidget(panLabel_);
    layout->addWidget(panSlider_);
    layout->addLayout(indicators);
    layout->addLayout(faderRow, 1);
    layout->addLayout(buttons);

    connect(fader_, &FaderWidget::valueChanged, this, [this](const double value) {
        requestSelection();
        (void)device_.setChannelFader(channelIndex_, value);
    });
    connect(muteButton_, &QToolButton::toggled, this, [this](const bool checked) {
        requestSelection();
        (void)device_.setChannelMuted(channelIndex_, checked);
    });
    connect(soloButton_, &QToolButton::toggled, this, [this](const bool checked) {
        requestSelection();
        (void)device_.setChannelSoloed(channelIndex_, checked);
    });
    connect(panSlider_, &QSlider::valueChanged, this, [this](const int value) {
        requestSelection();
        (void)device_.setChannelPan(channelIndex_, value / 100.0);
    });
    retranslateUi();
}

int ChannelStrip::channelIndex() const noexcept
{
    return channelIndex_;
}

void ChannelStrip::setSelected(const bool selected)
{
    if (property("selected").toBool() == selected) {
        return;
    }
    setProperty("selected", selected);
    style()->unpolish(this);
    style()->polish(this);
    update();
}

void ChannelStrip::refresh()
{
    const auto* channel = device_.state().channel(channelIndex_);
    const bool ready = channel != nullptr
        && device_.state().connectionState() == ConnectionState::Ready;
    setEnabled(ready);
    if (channel == nullptr) {
        nameLabel_->setText(uiText("Unknown"));
        return;
    }

    setVisible(channel->visible.value.value_or(true));

    nameLabel_->setText(channel->name.value.has_value() && !channel->name.value->isEmpty()
        ? *channel->name.value : inputDisplayName(channel->inputId));
    typeLabel_->setText(inputTypeDisplayName(channel->inputType));
    compressorIndicator_->setVisible(channel->capabilities.compressor);
    const auto icon = channel->icon.value.value_or(model::ChannelIcon::None);
    QStyle::StandardPixmap pixmap = QStyle::SP_FileIcon;
    switch (icon) {
    case model::ChannelIcon::Microphone: pixmap = QStyle::SP_MediaVolume; break;
    case model::ChannelIcon::Instrument: pixmap = QStyle::SP_DriveHDIcon; break;
    case model::ChannelIcon::GuitarBass: pixmap = QStyle::SP_MediaPlay; break;
    case model::ChannelIcon::Playback: pixmap = QStyle::SP_ComputerIcon; break;
    case model::ChannelIcon::None: break;
    }
    iconLabel_->setPixmap(style()->standardIcon(pixmap).pixmap(20, 20));
    iconLabel_->setVisible(device_.state().preferences().showChannelIcons);
    muteButton_->setVisible(device_.state().preferences().showMuteButtons);
    const QSignalBlocker faderBlocker(fader_);
    const QSignalBlocker muteBlocker(muteButton_);
    const QSignalBlocker soloBlocker(soloButton_);
    const QSignalBlocker panBlocker(panSlider_);
    fader_->setValue(channel->fader.value.value_or(0.0));
    muteButton_->setChecked(channel->muted.value.value_or(false));
    soloButton_->setChecked(channel->soloed.value.value_or(false));
    panSlider_->setValue(static_cast<int>(std::lround(channel->pan.value.value_or(0.0) * 100.0)));
    meter_->setLevel(channel->meterLevel.value.value_or(0.0));
    meter_->setPeak(channel->meterPeak.value.value_or(0.0));
    meter_->setClipping(channel->clipping.value.value_or(false));
}

void ChannelStrip::retranslateUi()
{
    eqIndicator_->setText(QStringLiteral("EQ"));
    compressorIndicator_->setText(uiText("Compressor"));
    sendIndicator_->setText(uiText("4 Sends"));
    panLabel_->setText(uiText("Pan / Balance"));
    muteButton_->setText(uiText("Mute"));
    muteButton_->setToolTip(uiText("Mute"));
    soloButton_->setText(uiText("Solo"));
    soloButton_->setToolTip(uiText("Solo"));
    fader_->retranslateUi();
    refresh();
}

void ChannelStrip::mousePressEvent(QMouseEvent* event)
{
    if (event->button() == Qt::LeftButton) {
        requestSelection();
    }
    QWidget::mousePressEvent(event);
}

void ChannelStrip::mouseDoubleClickEvent(QMouseEvent* event)
{
    if (event->button() == Qt::LeftButton) {
        requestSelection();
        emit editRequested(channelIndex_);
        event->accept();
        return;
    }
    QWidget::mouseDoubleClickEvent(event);
}

void ChannelStrip::requestSelection()
{
    emit selected(channelIndex_);
}

} // namespace flow8::ui
