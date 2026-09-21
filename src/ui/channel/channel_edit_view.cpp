#include "ui/channel/channel_edit_view.h"

#include "core/flow8_device.h"
#include "ui/inspector/inspector_widget.h"
#include "ui/ui_text.h"
#include "ui/widgets/fader_widget.h"

#include <QCheckBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QMessageBox>
#include <QPushButton>
#include <QSignalBlocker>
#include <QSlider>
#include <QToolButton>
#include <QVBoxLayout>
#include <QVariant>

#include <cmath>

namespace flow8::ui {

ChannelEditView::ChannelEditView(Flow8Device& device, QWidget* parent)
    : QWidget(parent)
    , device_(device)
    , back_(new QPushButton(this))
    , title_(new QLabel(this))
    , identity_(new QLabel(this))
    , preampTitle_(new QLabel(this))
    , mixTitle_(new QLabel(this))
    , gainLabel_(new QLabel(this))
    , panLabel_(new QLabel(this))
    , gain_(new QSlider(Qt::Horizontal, this))
    , phantom_(new QCheckBox(this))
    , fader_(new FaderWidget(this))
    , pan_(new QSlider(Qt::Horizontal, this))
    , mute_(new QToolButton(this))
    , solo_(new QToolButton(this))
    , inspector_(new InspectorWidget(device_, this))
{
    setObjectName(QStringLiteral("channelEditView"));
    back_->setObjectName(QStringLiteral("channelEditBack"));
    title_->setObjectName(QStringLiteral("channelEditTitle"));
    title_->setProperty("class", QStringLiteral("sessionTitle"));
    identity_->setProperty("class", QStringLiteral("secondaryText"));
    preampTitle_->setProperty("class", QStringLiteral("sectionLabel"));
    mixTitle_->setProperty("class", QStringLiteral("sectionLabel"));

    gain_->setObjectName(QStringLiteral("channelEditGain"));
    gain_->setRange(0, 1000);
    phantom_->setObjectName(QStringLiteral("channelEditPhantom"));
    fader_->setObjectName(QStringLiteral("channelEditFader"));
    pan_->setObjectName(QStringLiteral("channelEditPan"));
    pan_->setRange(-100, 100);
    mute_->setObjectName(QStringLiteral("channelEditMute"));
    mute_->setProperty("class", QStringLiteral("muteButton"));
    mute_->setCheckable(true);
    solo_->setObjectName(QStringLiteral("channelEditSolo"));
    solo_->setProperty("class", QStringLiteral("soloButton"));
    solo_->setCheckable(true);

    auto* header = new QHBoxLayout;
    header->addWidget(back_);
    auto* heading = new QVBoxLayout;
    heading->addWidget(title_);
    heading->addWidget(identity_);
    header->addLayout(heading);
    header->addStretch();

    auto* controls = new QWidget(this);
    controls->setObjectName(QStringLiteral("channelEditControls"));
    controls->setProperty("class", QStringLiteral("detailCard"));
    controls->setMinimumWidth(300);
    controls->setMaximumWidth(380);
    auto* controlsLayout = new QVBoxLayout(controls);
    controlsLayout->setContentsMargins(18, 18, 18, 18);
    controlsLayout->setSpacing(10);
    controlsLayout->addWidget(preampTitle_);
    controlsLayout->addWidget(gainLabel_);
    controlsLayout->addWidget(gain_);
    controlsLayout->addWidget(phantom_);
    controlsLayout->addSpacing(12);
    controlsLayout->addWidget(mixTitle_);
    auto* faderRow = new QHBoxLayout;
    faderRow->addStretch();
    faderRow->addWidget(fader_);
    faderRow->addStretch();
    controlsLayout->addLayout(faderRow, 1);
    controlsLayout->addWidget(panLabel_);
    controlsLayout->addWidget(pan_);
    auto* buttons = new QHBoxLayout;
    buttons->addWidget(mute_);
    buttons->addWidget(solo_);
    controlsLayout->addLayout(buttons);

    auto* content = new QHBoxLayout;
    content->setSpacing(12);
    content->addWidget(controls);
    content->addWidget(inspector_, 1);

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(18, 14, 18, 16);
    layout->setSpacing(12);
    layout->addLayout(header);
    layout->addLayout(content, 1);

    connect(back_, &QPushButton::clicked, this, &ChannelEditView::backRequested);
    connect(gain_, &QSlider::valueChanged, this, [this](const int value) {
        (void)device_.setChannelGain(channelIndex_, value / 1000.0);
    });
    connect(phantom_, &QCheckBox::clicked, this, [this](const bool enabled) {
        if (enabled && QMessageBox::warning(
                this, uiText("Enable Phantom Power?"),
                uiText("Confirm that the connected source supports 48 V phantom power."),
                QMessageBox::Yes | QMessageBox::Cancel, QMessageBox::Cancel)
                != QMessageBox::Yes) {
            const QSignalBlocker blocker(phantom_);
            phantom_->setChecked(false);
            return;
        }
        (void)device_.setChannelPhantom(channelIndex_, enabled);
    });
    connect(fader_, &FaderWidget::valueChanged, this, [this](const double value) {
        (void)device_.setChannelFader(channelIndex_, value);
    });
    connect(pan_, &QSlider::valueChanged, this, [this](const int value) {
        (void)device_.setChannelPan(channelIndex_, value / 100.0);
    });
    connect(mute_, &QToolButton::toggled, this, [this](const bool muted) {
        (void)device_.setChannelMuted(channelIndex_, muted);
    });
    connect(solo_, &QToolButton::toggled, this, [this](const bool soloed) {
        (void)device_.setChannelSoloed(channelIndex_, soloed);
    });
    connect(&device_.state(), &Flow8State::channelChanged, this, [this](const int index) {
        if (index == channelIndex_) {
            refresh();
        }
    });
    connect(&device_.state(), &Flow8State::stateReset, this, &ChannelEditView::refresh);
    retranslateUi();
}

void ChannelEditView::setChannel(const int index)
{
    if (index < 0 || index >= device_.state().channels().size()) {
        return;
    }
    channelIndex_ = index;
    inspector_->setSelectedChannel(index);
    refresh();
}

int ChannelEditView::channel() const noexcept
{
    return channelIndex_;
}

void ChannelEditView::refresh()
{
    const auto* channelState = device_.state().channel(channelIndex_);
    if (channelState == nullptr) {
        setEnabled(false);
        return;
    }
    setEnabled(device_.state().connectionState() == ConnectionState::Ready);
    title_->setText(channelState->name.value.has_value() && !channelState->name.value->isEmpty()
        ? *channelState->name.value : inputDisplayName(channelState->inputId));
    identity_->setText(QStringLiteral("%1 · %2")
        .arg(inputDisplayName(channelState->inputId), inputTypeDisplayName(channelState->inputType)));
    gain_->setVisible(channelState->capabilities.gain);
    gainLabel_->setVisible(channelState->capabilities.gain);
    phantom_->setVisible(channelState->capabilities.phantom48V);
    const QSignalBlocker gainBlocker(gain_);
    const QSignalBlocker phantomBlocker(phantom_);
    const QSignalBlocker faderBlocker(fader_);
    const QSignalBlocker panBlocker(pan_);
    const QSignalBlocker muteBlocker(mute_);
    const QSignalBlocker soloBlocker(solo_);
    gain_->setValue(static_cast<int>(std::lround(
        channelState->gain.value.value_or(0.0) * 1000.0)));
    phantom_->setChecked(channelState->phantom48V.has_value()
        && channelState->phantom48V->value.value_or(false));
    fader_->setValue(channelState->fader.value.value_or(0.0));
    pan_->setValue(static_cast<int>(std::lround(
        channelState->pan.value.value_or(0.0) * 100.0)));
    mute_->setChecked(channelState->muted.value.value_or(false));
    solo_->setChecked(channelState->soloed.value.value_or(false));
    inspector_->refresh();
}

void ChannelEditView::retranslateUi()
{
    back_->setText(uiText("Back"));
    preampTitle_->setText(uiText("Preamp"));
    mixTitle_->setText(uiText("Main Mix"));
    gainLabel_->setText(uiText("Gain"));
    phantom_->setText(uiText("Phantom Power") + QStringLiteral(" · 48 V"));
    panLabel_->setText(uiText("Pan / Balance"));
    mute_->setText(uiText("Mute"));
    solo_->setText(uiText("Solo"));
    fader_->retranslateUi();
    inspector_->retranslateUi();
    refresh();
}

} // namespace flow8::ui
