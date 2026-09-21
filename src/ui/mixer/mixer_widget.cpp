#include "ui/mixer/mixer_widget.h"

#include "core/flow8_device.h"
#include "ui/mixer/channel_strip.h"
#include "ui/mixer/main_strip.h"

#include <QFrame>
#include <QHBoxLayout>
#include <QScrollArea>
#include <QVBoxLayout>

namespace flow8::ui {

MixerWidget::MixerWidget(Flow8Device& device, QWidget* parent)
    : QWidget(parent)
    , device_(device)
    , stripsContainer_(new QWidget(this))
    , stripsLayout_(new QHBoxLayout(stripsContainer_))
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

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->addWidget(scroll);

    connect(&device_.state(), &Flow8State::stateReset, this, &MixerWidget::rebuild);
    connect(&device_.state(), &Flow8State::channelChanged, this, &MixerWidget::refreshChannel);
    connect(&device_.state(), &Flow8State::busChanged, this, [this](const int bus) {
        if (bus == 0 && mainStrip_ != nullptr) mainStrip_->refresh();
    });
    connect(&device_.state(), &Flow8State::connectionStateChanged,
            this, [this](ConnectionState) { refreshAll(); });
    connect(&device_.state(), &Flow8State::preferencesChanged,
            this, &MixerWidget::refreshAll);
    rebuild();
}

void MixerWidget::retranslateUi()
{
    for (auto* strip : channelStrips_) {
        strip->retranslateUi();
    }
    if (mainStrip_ != nullptr) {
        mainStrip_->retranslateUi();
    }
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
        channelStrips_.append(strip);
        stripsLayout_->addWidget(strip, 1);
        connect(strip, &ChannelStrip::selected, this, &MixerWidget::selectChannel);
        connect(strip, &ChannelStrip::editRequested,
                this, &MixerWidget::channelEditRequested);
    }
    mainStrip_ = new MainStrip(device_, stripsContainer_);
    stripsLayout_->addWidget(mainStrip_, 1);
    connect(mainStrip_, &MainStrip::editRequested, this, &MixerWidget::mainOutRequested);
    if (!channelStrips_.isEmpty()) {
        selectChannel(0);
    }
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
    if (mainStrip_ != nullptr) {
        mainStrip_->refresh();
    }
}

} // namespace flow8::ui
