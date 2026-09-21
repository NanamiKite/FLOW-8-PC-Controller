#include "ui/stage/stage_view.h"

#include "core/flow8_device.h"
#include "ui/ui_text.h"
#include "ui/widgets/fader_widget.h"
#include "ui/widgets/meter_widget.h"

#include <QFrame>
#include <QHBoxLayout>
#include <QLabel>
#include <QLocale>
#include <QPushButton>
#include <QScrollArea>
#include <QSignalBlocker>
#include <QStyle>
#include <QToolButton>
#include <QVBoxLayout>

namespace flow8::ui {
namespace {

QStyle::StandardPixmap iconFor(const model::ChannelIcon icon)
{
    switch (icon) {
    case model::ChannelIcon::Microphone: return QStyle::SP_MediaVolume;
    case model::ChannelIcon::Instrument: return QStyle::SP_DriveHDIcon;
    case model::ChannelIcon::GuitarBass: return QStyle::SP_MediaPlay;
    case model::ChannelIcon::Playback: return QStyle::SP_ComputerIcon;
    case model::ChannelIcon::None: return QStyle::SP_FileIcon;
    }
    return QStyle::SP_FileIcon;
}

} // namespace

StageView::StageView(Flow8Device& device, QWidget* parent)
    : QWidget(parent)
    , device_(device)
    , cardsContainer_(new QWidget(this))
    , cardsLayout_(new QHBoxLayout(cardsContainer_))
    , title_(new QLabel(this))
    , description_(new QLabel(this))
    , tempo_(new QLabel(this))
    , tapTempo_(new QPushButton(this))
{
    setObjectName(QStringLiteral("stageView"));
    cardsLayout_->setContentsMargins(16, 16, 16, 16);
    cardsLayout_->setSpacing(12);
    cardsLayout_->setAlignment(Qt::AlignLeft);
    title_->setProperty("class", QStringLiteral("inspectorTitle"));
    description_->setProperty("class", QStringLiteral("secondaryText"));
    tapTempo_->setObjectName(QStringLiteral("stageTapTempo"));

    auto* header = new QHBoxLayout;
    auto* heading = new QVBoxLayout;
    heading->addWidget(title_);
    heading->addWidget(description_);
    header->addLayout(heading);
    header->addStretch();
    header->addWidget(tempo_);
    header->addWidget(tapTempo_);

    auto* scroll = new QScrollArea(this);
    scroll->setFrameShape(QFrame::NoFrame);
    scroll->setWidgetResizable(true);
    scroll->setWidget(cardsContainer_);

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(16, 12, 16, 12);
    layout->addLayout(header);
    layout->addWidget(scroll, 1);

    connect(tapTempo_, &QPushButton::clicked, this, [this] { (void)device_.tapTempo(); });
    connect(&device_.state(), &Flow8State::stateReset, this, &StageView::rebuild);
    connect(&device_.state(), &Flow8State::channelChanged, this,
            [this](int) { refresh(); });
    connect(&device_.state(), &Flow8State::globalTempoChanged, this, &StageView::refresh);
    connect(&device_.state(), &Flow8State::preferencesChanged, this, &StageView::refresh);
    rebuild();
    retranslateUi();
}

void StageView::rebuild()
{
    while (auto* item = cardsLayout_->takeAt(0)) {
        delete item->widget();
        delete item;
    }
    cards_.clear();
    for (int index = 0; index < device_.state().channels().size(); ++index) {
        Card card;
        card.root = new QWidget(cardsContainer_);
        card.root->setProperty("class", QStringLiteral("stageCard"));
        card.root->setObjectName(QStringLiteral("stageCard%1").arg(index));
        card.root->setMinimumWidth(190);
        card.icon = new QLabel(card.root);
        card.icon->setAlignment(Qt::AlignCenter);
        card.icon->setMinimumHeight(30);
        card.name = new QLabel(card.root);
        card.name->setObjectName(QStringLiteral("stageName%1").arg(index));
        card.name->setProperty("class", QStringLiteral("stageName"));
        card.name->setAlignment(Qt::AlignCenter);
        card.type = new QLabel(card.root);
        card.type->setProperty("class", QStringLiteral("inputBadge"));
        card.type->setAlignment(Qt::AlignCenter);
        card.meter = new MeterWidget(card.root);
        card.fader = new FaderWidget(card.root);
        card.fader->setObjectName(QStringLiteral("stageFader%1").arg(index));
        card.mute = new QToolButton(card.root);
        card.mute->setObjectName(QStringLiteral("stageMute%1").arg(index));
        card.mute->setCheckable(true);
        card.mute->setProperty("class", QStringLiteral("muteButton"));
        card.solo = new QToolButton(card.root);
        card.solo->setObjectName(QStringLiteral("stageSolo%1").arg(index));
        card.solo->setCheckable(true);
        card.solo->setProperty("class", QStringLiteral("soloButton"));

        auto* meterFader = new QHBoxLayout;
        meterFader->setSpacing(12);
        meterFader->addWidget(card.meter);
        meterFader->addWidget(card.fader, 1, Qt::AlignHCenter);
        auto* actions = new QHBoxLayout;
        actions->addWidget(card.mute);
        actions->addWidget(card.solo);
        auto* cardLayout = new QVBoxLayout(card.root);
        cardLayout->setContentsMargins(14, 16, 14, 14);
        cardLayout->setSpacing(9);
        cardLayout->addWidget(card.icon);
        cardLayout->addWidget(card.name);
        cardLayout->addWidget(card.type);
        cardLayout->addLayout(meterFader, 1);
        cardLayout->addLayout(actions);

        connect(card.fader, &FaderWidget::valueChanged, this,
                [this, index](const double value) {
                    (void)device_.setChannelFader(index, value);
                });
        connect(card.mute, &QToolButton::toggled, this,
                [this, index](const bool muted) {
                    (void)device_.setChannelMuted(index, muted);
                });
        connect(card.solo, &QToolButton::toggled, this,
                [this, index](const bool soloed) {
                    (void)device_.setChannelSoloed(index, soloed);
                });
        cardsLayout_->addWidget(card.root);
        cards_.append(card);
    }
    refresh();
    retranslateUi();
}

void StageView::refresh()
{
    const auto& preferences = device_.state().preferences();
    for (int index = 0; index < cards_.size(); ++index) {
        const auto* channel = device_.state().channel(index);
        auto& card = cards_[index];
        if (channel == nullptr) {
            card.root->hide();
            continue;
        }
        card.root->setVisible(channel->visible.value.value_or(true));
        card.name->setText(channel->name.value.has_value() && !channel->name.value->isEmpty()
            ? *channel->name.value : inputDisplayName(channel->inputId));
        card.type->setText(inputTypeDisplayName(channel->inputType));
        card.icon->setVisible(preferences.showChannelIcons);
        card.icon->setPixmap(style()->standardIcon(
            iconFor(channel->icon.value.value_or(model::ChannelIcon::None)))
            .pixmap(24, 24));
        card.mute->setVisible(preferences.showMuteButtons);
        const QSignalBlocker faderBlocker(card.fader);
        const QSignalBlocker muteBlocker(card.mute);
        const QSignalBlocker soloBlocker(card.solo);
        card.fader->setValue(channel->fader.value.value_or(0.0));
        card.meter->setLevel(channel->meterLevel.value.value_or(0.0));
        card.meter->setPeak(channel->meterPeak.value.value_or(0.0));
        card.meter->setClipping(channel->clipping.value.value_or(false));
        card.mute->setChecked(channel->muted.value.value_or(false));
        card.solo->setChecked(channel->soloed.value.value_or(false));
    }
    tempo_->setText(QStringLiteral("%1 BPM · %2")
        .arg(QLocale().toString(device_.state().globalTempo().bpm.value.value_or(120.0), 'f', 1),
             uiText("SYNTHETIC")));
}

void StageView::retranslateUi()
{
    title_->setText(uiText("Stage View"));
    description_->setText(uiText("Large controls for fast live operation"));
    tapTempo_->setText(uiText("Tap Tempo"));
    for (auto& card : cards_) {
        card.mute->setText(uiText("Mute"));
        card.solo->setText(uiText("Solo"));
        card.fader->retranslateUi();
    }
    refresh();
}

} // namespace flow8::ui
