#include "ui/mixer/mixer_widget.h"

#include "core/flow8_device.h"
#include "ui/mixer/channel_strip.h"
#include "ui/mixer/main_strip.h"

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
    setObjectName(QStringLiteral("mixerWidget"));
    stripsLayout_->setAlignment(Qt::AlignLeft);
    auto* scroll = new QScrollArea(this);
    scroll->setWidgetResizable(true);
    scroll->setWidget(stripsContainer_);
    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->addWidget(scroll);

    connect(&device_.state(), &Flow8State::stateReset, this, &MixerWidget::rebuild);
    connect(&device_.state(), &Flow8State::channelChanged, this, &MixerWidget::refreshChannel);
    connect(&device_.state(), &Flow8State::mainChanged, this, [this] {
        if (mainStrip_ != nullptr) {
            mainStrip_->refresh();
        }
    });
    connect(&device_.state(), &Flow8State::connectionStateChanged, this,
            [this](ConnectionState) { refreshAll(); });
    rebuild();
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
        stripsLayout_->addWidget(strip);
    }
    mainStrip_ = new MainStrip(device_, stripsContainer_);
    stripsLayout_->addWidget(mainStrip_);
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
