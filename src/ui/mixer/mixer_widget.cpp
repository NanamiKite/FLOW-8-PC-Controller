#include "ui/mixer/mixer_widget.h"

#include "core/flow8_device.h"
#include "ui/inspector/detail_panel.h"
#include "ui/inspector/inspector_widget.h"
#include "ui/mixer/channel_strip.h"
#include "ui/mixer/main_strip.h"
#include "ui/stage/stage_view.h"
#include "ui/ui_text.h"

#include <QButtonGroup>
#include <QFrame>
#include <QHBoxLayout>
#include <QLabel>
#include <QScrollArea>
#include <QSplitter>
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
    , inspector_(new InspectorWidget(device_, this))
    , detailPanel_(new DetailPanel(device_, this))
    , detailStack_(new QStackedWidget(this))
    , workspaceStack_(new QStackedWidget(this))
    , mixerScroll_(new QScrollArea(this))
    , mixerSplitter_(new QSplitter(Qt::Vertical, this))
    , stageView_(new StageView(device_, this))
    , sectionTitle_(new QLabel(this))
    , syntheticBadge_(new QLabel(this))
{
    setObjectName(QStringLiteral("mixerWidget"));
    stripsContainer_->setObjectName(QStringLiteral("stripsContainer"));
    stripsLayout_->setAlignment(Qt::AlignLeft);
    stripsLayout_->setContentsMargins(12, 12, 12, 12);
    stripsLayout_->setSpacing(8);

    auto* navigation = new QWidget(this);
    navigation->setObjectName(QStringLiteral("mixerNavigation"));
    auto* navigationLayout = new QHBoxLayout(navigation);
    navigationLayout->setContentsMargins(14, 8, 14, 8);
    navigationLayout->setSpacing(5);
    sectionTitle_->setParent(navigation);
    sectionTitle_->setProperty("class", QStringLiteral("navigationTitle"));
    navigationLayout->addWidget(sectionTitle_);
    navigationLayout->addSpacing(14);
    auto* group = new QButtonGroup(navigation);
    for (int index = 0; index < 9; ++index) {
        auto* button = new QToolButton(navigation);
        button->setCheckable(true);
        button->setProperty("class", QStringLiteral("navigationButton"));
        button->setObjectName(QStringLiteral("navigation%1").arg(index));
        group->addButton(button, index);
        navigationButtons_.append(button);
        navigationLayout->addWidget(button);
    }
    navigationLayout->addStretch();
    syntheticBadge_->setParent(navigation);
    syntheticBadge_->setProperty("class", QStringLiteral("syntheticBadge"));
    navigationLayout->addWidget(syntheticBadge_);

    mixerScroll_->setObjectName(QStringLiteral("mixerScroll"));
    mixerScroll_->setFrameShape(QFrame::NoFrame);
    mixerScroll_->setWidgetResizable(true);
    mixerScroll_->setWidget(stripsContainer_);

    detailStack_->addWidget(inspector_);
    detailStack_->addWidget(detailPanel_);
    mixerSplitter_->setObjectName(QStringLiteral("mixerSplitter"));
    mixerSplitter_->addWidget(mixerScroll_);
    mixerSplitter_->addWidget(detailStack_);
    mixerSplitter_->setStretchFactor(0, 3);
    mixerSplitter_->setStretchFactor(1, 2);
    mixerSplitter_->setSizes({520, 310});
    workspaceStack_->addWidget(mixerSplitter_);
    workspaceStack_->addWidget(stageView_);

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);
    layout->addWidget(navigation);
    layout->addWidget(workspaceStack_, 1);

    connect(group, &QButtonGroup::idClicked, this, [this](const int id) {
        if (id == 0) {
            showMixer();
            return;
        }
        if (id == 1) {
            mixerScroll_->setVisible(true);
            workspaceStack_->setCurrentWidget(stageView_);
            stageView_->refresh();
            return;
        }
        workspaceStack_->setCurrentWidget(mixerSplitter_);
        mixerScroll_->setVisible(false);
        detailStack_->setCurrentWidget(detailPanel_);
        if (id == 2 || id == 3) {
            detailPanel_->showFx(id - 2);
        } else if (id == 4 || id == 5) {
            detailPanel_->showBus(id - 3);
        } else if (id == 6) {
            detailPanel_->showBus(0);
        } else if (id == 7) {
            detailPanel_->showSnapshots();
        } else if (id == 8) {
            detailPanel_->showRouting();
        }
    });
    connect(&device_.state(), &Flow8State::stateReset, this, &MixerWidget::rebuild);
    connect(&device_.state(), &Flow8State::channelChanged, this, &MixerWidget::refreshChannel);
    connect(&device_.state(), &Flow8State::mainChanged, this, [this] {
        if (mainStrip_ != nullptr) mainStrip_->refresh();
    });
    connect(&device_.state(), &Flow8State::connectionStateChanged, this,
            [this](ConnectionState) { refreshAll(); });
    connect(&device_.state(), &Flow8State::preferencesChanged,
            this, &MixerWidget::refreshAll);
    rebuild();
    navigationButtons_.first()->setChecked(true);
    retranslateUi();
}

void MixerWidget::retranslateUi()
{
    sectionTitle_->setText(uiText("Mixer"));
    const std::array<const char*, 9> names {
        "Mixer", "Stage", "FX 1", "FX 2", "Monitor 1", "Monitor 2", "Main",
        "Snapshot", "Routing",
    };
    for (int index = 0; index < navigationButtons_.size(); ++index) {
        navigationButtons_[index]->setText(uiText(names[static_cast<std::size_t>(index)]));
    }
    syntheticBadge_->setText(uiText("Simulator · SYNTHETIC"));
    for (auto* strip : channelStrips_) strip->retranslateUi();
    if (mainStrip_ != nullptr) mainStrip_->retranslateUi();
    inspector_->retranslateUi();
    detailPanel_->retranslateUi();
    stageView_->retranslateUi();
}

void MixerWidget::showMixer()
{
    workspaceStack_->setCurrentWidget(mixerSplitter_);
    mixerScroll_->setVisible(true);
    detailStack_->setCurrentWidget(inspector_);
    if (!navigationButtons_.isEmpty()) {
        navigationButtons_.at(0)->setChecked(true);
    }
}

void MixerWidget::showSnapshots()
{
    workspaceStack_->setCurrentWidget(mixerSplitter_);
    mixerScroll_->setVisible(false);
    detailStack_->setCurrentWidget(detailPanel_);
    detailPanel_->showSnapshots();
    if (navigationButtons_.size() > 7) {
        navigationButtons_.at(7)->setChecked(true);
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
        stripsLayout_->addWidget(strip);
        connect(strip, &ChannelStrip::selected, this, &MixerWidget::selectChannel);
    }
    mainStrip_ = new MainStrip(device_, stripsContainer_);
    stripsLayout_->addWidget(mainStrip_);
    connect(mainStrip_, &MainStrip::selected, this, [this] {
        detailStack_->setCurrentWidget(detailPanel_);
        detailPanel_->showBus(0);
    });
    selectChannel(inspector_->selectedChannel());
}

void MixerWidget::selectChannel(const int index)
{
    if (index < 0 || index >= channelStrips_.size()) {
        return;
    }
    for (int channel = 0; channel < channelStrips_.size(); ++channel) {
        channelStrips_[channel]->setSelected(channel == index);
    }
    inspector_->setSelectedChannel(index);
    detailStack_->setCurrentWidget(inspector_);
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
    if (mainStrip_ != nullptr) mainStrip_->refresh();
    inspector_->refresh();
    detailPanel_->refresh();
    stageView_->refresh();
}

} // namespace flow8::ui
