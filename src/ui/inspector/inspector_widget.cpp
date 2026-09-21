#include "ui/inspector/inspector_widget.h"

#include "core/flow8_device.h"
#include "ui/widgets/eq_graph_widget.h"
#include "ui/ui_text.h"

#include <QCheckBox>
#include <QComboBox>
#include <QFormLayout>
#include <QGridLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QMessageBox>
#include <QPushButton>
#include <QLocale>
#include <QSignalBlocker>
#include <QSlider>
#include <QTabWidget>
#include <QVBoxLayout>

#include <array>
#include <cmath>

namespace flow8::ui {
namespace {

QWidget* createPage(QWidget* parent)
{
    auto* page = new QWidget(parent);
    page->setProperty("class", QStringLiteral("inspectorPage"));
    return page;
}

QString destinationName(const model::RoutingDestination destination)
{
    switch (destination) {
    case model::RoutingDestination::Main: return uiText("Main");
    case model::RoutingDestination::Monitor1: return uiText("Monitor 1");
    case model::RoutingDestination::Monitor2: return uiText("Monitor 2");
    case model::RoutingDestination::Fx1: return uiText("FX 1");
    case model::RoutingDestination::Fx2: return uiText("FX 2");
    }
    return uiText("Unknown");
}

} // namespace

InspectorWidget::InspectorWidget(Flow8Device& device, QWidget* parent)
    : QWidget(parent)
    , device_(device)
    , title_(new QLabel(this))
    , metadata_(new QLabel(this))
    , inputCapabilities_(new QLabel(this))
    , evidenceNote_(new QLabel(this))
    , syntheticBadge_(new QLabel(this))
    , currentRouteLabel_(new QLabel(this))
    , currentRoute_(new QSlider(Qt::Horizontal, this))
    , channelName_(new QLineEdit(this))
    , channelIcon_(new QComboBox(this))
    , channelVisible_(new QCheckBox(this))
    , gain_(new QSlider(Qt::Horizontal, this))
    , phantom_(new QCheckBox(this))
    , phase_(new QCheckBox(this))
    , lowCutEnabled_(new QCheckBox(this))
    , lowCutFrequency_(new QSlider(Qt::Horizontal, this))
    , pan_(new QSlider(Qt::Horizontal, this))
    , mute_(new QCheckBox(this))
    , solo_(new QCheckBox(this))
    , ezGainSelected_(new QPushButton(this))
    , ezGainAll_(new QPushButton(this))
    , ezGainCancel_(new QPushButton(this))
    , ezGainStatus_(new QLabel(this))
    , tabs_(new QTabWidget(this))
    , eqGraph_(new EqGraphWidget(this))
    , compressorAmount_(new QSlider(Qt::Horizontal, this))
    , gainReduction_(new QLabel(this))
{
    setObjectName(QStringLiteral("inspector"));
    setProperty("class", QStringLiteral("inspector"));
    setMinimumHeight(290);
    title_->setProperty("class", QStringLiteral("inspectorTitle"));
    metadata_->setProperty("class", QStringLiteral("secondaryText"));
    inputCapabilities_->setWordWrap(true);

    auto* channelPage = createPage(tabs_);
    auto* channelLayout = new QVBoxLayout(channelPage);
    channelLayout->setContentsMargins(14, 12, 14, 12);
    auto* channelForm = new QFormLayout;
    for (int row = 0; row < 11; ++row) {
        channelFormLabels_.append(new QLabel(channelPage));
    }
    channelName_->setObjectName(QStringLiteral("channelCustomName"));
    channelIcon_->setObjectName(QStringLiteral("channelIcon"));
    channelVisible_->setObjectName(QStringLiteral("channelVisible"));
    gain_->setObjectName(QStringLiteral("inspectorGain"));
    gain_->setRange(0, 1000);
    phantom_->setObjectName(QStringLiteral("inspectorPhantom"));
    phase_->setObjectName(QStringLiteral("inspectorPhase"));
    lowCutEnabled_->setObjectName(QStringLiteral("lowCutEnabled"));
    lowCutFrequency_->setObjectName(QStringLiteral("lowCutFrequency"));
    lowCutFrequency_->setRange(20, 600);
    pan_->setObjectName(QStringLiteral("inspectorPan"));
    pan_->setRange(-100, 100);
    mute_->setObjectName(QStringLiteral("inspectorMute"));
    solo_->setObjectName(QStringLiteral("inspectorSolo"));
    channelForm->addRow(channelFormLabels_[0], channelName_);
    channelForm->addRow(channelFormLabels_[1], channelIcon_);
    channelForm->addRow(channelFormLabels_[2], channelVisible_);
    channelForm->addRow(channelFormLabels_[3], gain_);
    channelForm->addRow(channelFormLabels_[4], phantom_);
    channelForm->addRow(channelFormLabels_[5], phase_);
    channelForm->addRow(channelFormLabels_[6], lowCutEnabled_);
    channelForm->addRow(channelFormLabels_[7], lowCutFrequency_);
    channelForm->addRow(channelFormLabels_[8], pan_);
    channelForm->addRow(channelFormLabels_[9], mute_);
    channelForm->addRow(channelFormLabels_[10], solo_);
    channelLayout->addLayout(channelForm);
    channelLayout->addWidget(inputCapabilities_);
    auto* ezGainRow = new QHBoxLayout;
    ezGainSelected_->setObjectName(QStringLiteral("ezGainSelected"));
    ezGainAll_->setObjectName(QStringLiteral("ezGainAll"));
    ezGainCancel_->setObjectName(QStringLiteral("ezGainCancel"));
    ezGainStatus_->setProperty("class", QStringLiteral("secondaryText"));
    ezGainRow->addWidget(ezGainSelected_);
    ezGainRow->addWidget(ezGainAll_);
    ezGainRow->addWidget(ezGainCancel_);
    ezGainRow->addWidget(ezGainStatus_, 1);
    channelLayout->addLayout(ezGainRow);
    channelLayout->addStretch();
    evidenceNote_->setParent(channelPage);
    evidenceNote_->setProperty("class", QStringLiteral("secondaryText"));
    evidenceNote_->setWordWrap(true);
    channelLayout->addWidget(evidenceNote_);
    connect(channelName_, &QLineEdit::editingFinished, this, [this] {
        if (!channelName_->text().trimmed().isEmpty()) {
            (void)device_.setChannelName(selectedChannel_, channelName_->text());
        }
    });
    connect(channelIcon_, &QComboBox::currentIndexChanged, this, [this](const int index) {
        if (index >= 0) {
            (void)device_.setChannelIcon(
                selectedChannel_, static_cast<model::ChannelIcon>(
                    channelIcon_->itemData(index).toInt()));
        }
    });
    connect(channelVisible_, &QCheckBox::toggled, this, [this](const bool visible) {
        (void)device_.setChannelVisible(selectedChannel_, visible);
    });
    connect(gain_, &QSlider::valueChanged, this, [this](const int value) {
        (void)device_.setChannelGain(selectedChannel_, value / 1000.0);
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
        (void)device_.setChannelPhantom(selectedChannel_, enabled);
    });
    connect(phase_, &QCheckBox::toggled, this, [this](const bool inverted) {
        (void)device_.setChannelPhaseInverted(selectedChannel_, inverted);
    });
    connect(pan_, &QSlider::valueChanged, this, [this](const int value) {
        (void)device_.setChannelPan(selectedChannel_, value / 100.0);
    });
    connect(mute_, &QCheckBox::toggled, this, [this](const bool enabled) {
        (void)device_.setChannelMuted(selectedChannel_, enabled);
    });
    connect(solo_, &QCheckBox::toggled, this, [this](const bool enabled) {
        (void)device_.setChannelSoloed(selectedChannel_, enabled);
    });
    const auto updateLowCut = [this] {
        (void)device_.setChannelLowCut(selectedChannel_, lowCutEnabled_->isChecked(),
                                        lowCutFrequency_->value());
    };
    connect(lowCutEnabled_, &QCheckBox::toggled, this,
            [updateLowCut](bool) { updateLowCut(); });
    connect(lowCutFrequency_, &QSlider::valueChanged, this,
            [updateLowCut](int) { updateLowCut(); });
    connect(ezGainSelected_, &QPushButton::clicked, this, [this] {
        const auto* channel = device_.state().channel(selectedChannel_);
        if (channel != nullptr) {
            (void)device_.startEzGain({channel->inputId});
        }
    });
    connect(ezGainAll_, &QPushButton::clicked, this, [this] {
        QVector<model::InputId> targets;
        for (const auto& channel : device_.state().channels()) {
            if (channel.capabilities.gain) {
                targets.append(channel.inputId);
            }
        }
        (void)device_.startEzGain(targets);
    });
    connect(ezGainCancel_, &QPushButton::clicked,
            &device_, &Flow8Device::cancelEzGain);

    auto* eqPage = createPage(tabs_);
    auto* eqLayout = new QHBoxLayout(eqPage);
    eqLayout->setContentsMargins(10, 8, 10, 8);
    eqGraph_->setObjectName(QStringLiteral("channelEqGraph"));
    eqLayout->addWidget(eqGraph_, 2);
    auto* eqControls = new QGridLayout;
    for (int band = 0; band < 4; ++band) {
        auto* label = new QLabel(eqPage);
        auto* slider = new QSlider(Qt::Vertical, eqPage);
        slider->setRange(-150, 150);
        slider->setMinimumHeight(140);
        slider->setObjectName(QStringLiteral("eqGain%1").arg(band));
        eqGainSliders_.append(slider);
        eqBandLabels_.append(label);
        eqControls->addWidget(slider, 0, band, Qt::AlignHCenter);
        eqControls->addWidget(label, 1, band, Qt::AlignHCenter);
        connect(slider, &QSlider::valueChanged, this, [this, band](const int value) {
            (void)device_.setChannelEqGain(selectedChannel_, band, value / 10.0);
        });
    }
    eqLayout->addLayout(eqControls, 1);
    connect(eqGraph_, &EqGraphWidget::bandGainEdited, this,
            [this](const int band, const double gainDb) {
                (void)device_.setChannelEqGain(selectedChannel_, band, gainDb);
            });

    auto* compressorPage = createPage(tabs_);
    auto* compressorLayout = new QFormLayout(compressorPage);
    compressorLayout->setContentsMargins(18, 12, 18, 12);
    compressorAmount_->setRange(0, 1000);
    compressorAmount_->setObjectName(QStringLiteral("compressorAmount"));
    auto* amountLabel = new QLabel(compressorPage);
    compressorLabels_.append(amountLabel);
    compressorLayout->addRow(amountLabel, compressorAmount_);
    connect(compressorAmount_, &QSlider::valueChanged, this, [this](const int value) {
        (void)device_.setChannelCompressorAmount(selectedChannel_, value / 1000.0);
    });
    for (int row = 0; row < 5; ++row) {
        auto* label = new QLabel(compressorPage);
        auto* unavailable = new QLabel(compressorPage);
        unavailable->setEnabled(false);
        compressorLabels_.append(label);
        compressorUnavailable_.append(unavailable);
        compressorLayout->addRow(label, unavailable);
    }
    auto* reductionLabel = new QLabel(compressorPage);
    compressorLabels_.append(reductionLabel);
    compressorLayout->addRow(reductionLabel, gainReduction_);

    auto* sendsPage = createPage(tabs_);
    auto* sendsLayout = new QFormLayout(sendsPage);
    sendsLayout->setContentsMargins(18, 12, 18, 12);
    for (int send = 0; send < 4; ++send) {
        auto* slider = new QSlider(Qt::Horizontal, sendsPage);
        slider->setRange(0, 1000);
        slider->setObjectName(QStringLiteral("send%1").arg(send));
        sendSliders_.append(slider);
        auto* label = new QLabel(sendsPage);
        sendLabels_.append(label);
        if (send < 2) {
            auto* row = new QWidget(sendsPage);
            auto* rowLayout = new QHBoxLayout(row);
            rowLayout->setContentsMargins(0, 0, 0, 0);
            rowLayout->addWidget(slider, 1);
            auto* mode = new QComboBox(row);
            mode->setObjectName(QStringLiteral("monitorSendMode%1").arg(send));
            monitorSendModes_.append(mode);
            rowLayout->addWidget(mode);
            sendsLayout->addRow(label, row);
            connect(mode, &QComboBox::currentIndexChanged, this,
                    [this, send, mode](const int index) {
                        if (index >= 0) {
                            (void)device_.setMonitorSendMode(
                                selectedChannel_, send,
                                static_cast<model::MonitorSendMode>(
                                    mode->itemData(index).toInt()));
                        }
                    });
        } else {
            sendsLayout->addRow(label, slider);
        }
        connect(slider, &QSlider::valueChanged, this, [this, send](const int value) {
            (void)device_.setChannelSendLevel(selectedChannel_, send, value / 1000.0);
        });
    }

    tabs_->addTab(channelPage, {});
    tabs_->addTab(eqPage, {});
    tabs_->addTab(compressorPage, {});
    tabs_->addTab(sendsPage, {});

    auto* header = new QHBoxLayout;
    header->addWidget(title_);
    header->addSpacing(10);
    header->addWidget(metadata_);
    header->addStretch();
    syntheticBadge_->setProperty("class", QStringLiteral("syntheticBadge"));
    header->addWidget(syntheticBadge_);

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(14, 10, 14, 12);
    layout->setSpacing(7);
    layout->addLayout(header);
    auto* routeRow = new QHBoxLayout;
    currentRoute_->setRange(0, 1000);
    currentRoute_->setObjectName(QStringLiteral("currentDestinationSend"));
    currentRouteLabel_->setProperty("class", QStringLiteral("sectionLabel"));
    routeRow->addWidget(currentRouteLabel_);
    routeRow->addWidget(currentRoute_, 1);
    layout->addLayout(routeRow);
    layout->addWidget(tabs_, 1);

    connect(currentRoute_, &QSlider::valueChanged, this, [this](const int value) {
        (void)device_.setRouteLevel(
            selectedChannel_, selectedDestination_, value / 1000.0);
    });

    connect(&device_.state(), &Flow8State::stateReset, this, &InspectorWidget::refresh);
    connect(&device_.state(), &Flow8State::channelChanged, this, [this](const int index) {
        if (index == selectedChannel_) {
            refresh();
        }
    });
    connect(&device_.state(), &Flow8State::routingChanged, this, &InspectorWidget::refresh);
    connect(&device_.state(), &Flow8State::ezGainSessionChanged,
            this, &InspectorWidget::refresh);
    retranslateUi();
}

void InspectorWidget::setSelectedChannel(const int index)
{
    if (selectedChannel_ == index) {
        return;
    }
    selectedChannel_ = index;
    refresh();
}

void InspectorWidget::setSelectedDestination(
    const model::RoutingDestination destination)
{
    if (selectedDestination_ == destination) {
        return;
    }
    selectedDestination_ = destination;
    refresh();
}

int InspectorWidget::selectedChannel() const noexcept
{
    return selectedChannel_;
}

model::RoutingDestination InspectorWidget::selectedDestination() const noexcept
{
    return selectedDestination_;
}

void InspectorWidget::refresh()
{
    const auto* channel = device_.state().channel(selectedChannel_);
    if (channel == nullptr) {
        title_->setText(uiText("No Input Selected"));
        tabs_->setEnabled(false);
        return;
    }
    tabs_->setEnabled(device_.state().connectionState() == ConnectionState::Ready);
    tabs_->setTabVisible(1, channel->capabilities.equalizer);
    tabs_->setTabVisible(2, channel->capabilities.compressor);
    tabs_->setTabVisible(3, channel->capabilities.monitorSends
        || channel->capabilities.fxSends);
    title_->setText(channel->name.value.has_value() && !channel->name.value->isEmpty()
        ? *channel->name.value : inputDisplayName(channel->inputId));
    metadata_->setText(QStringLiteral("%1 · %2")
        .arg(channel->stereoPair ? uiText("Stereo Pair") : uiText("Mono"),
             channel->spatialControl == model::SpatialControl::Balance
                 ? uiText("Balance") : uiText("Pan")));
    inputCapabilities_->setText(uiText(
        "Gain: %1   48 V: %2   Phase: %3   Low Cut: %4   EQ: 4-band parametric   "
        "Compressor: %5   Sends: MON1 / MON2 / FX1 / FX2")
        .arg(channel->capabilities.gain ? uiText("Available") : uiText("Unavailable"),
             channel->capabilities.phantom48V ? uiText("Available") : uiText("Not supported"),
             channel->capabilities.phase ? uiText("Available") : uiText("Unavailable"),
             channel->capabilities.lowCut ? uiText("Available") : uiText("Unavailable"),
             channel->capabilities.compressor ? uiText("Available") : uiText("Unavailable")));
    currentRouteLabel_->setText(uiText("%1 → %2 Send")
        .arg(inputDisplayName(channel->inputId), destinationName(selectedDestination_)));
    {
        const auto* route = device_.state().routeLevel(
            selectedChannel_, selectedDestination_);
        const QSignalBlocker blocker(currentRoute_);
        currentRoute_->setValue(static_cast<int>(std::lround(
            (route == nullptr ? 0.0 : route->effectiveValue()) * 1000.0)));
    }
    const auto& ezGain = device_.state().ezGainSession();
    ezGainCancel_->setVisible(ezGain.running);
    if (ezGain.running) {
        ezGainStatus_->setText(uiText("EZ-GAIN running · %1 s · SYNTHETIC")
            .arg(ezGain.durationSeconds));
    } else if (ezGain.cancelled) {
        ezGainStatus_->setText(uiText("EZ-GAIN cancelled"));
    } else if (!ezGain.results.isEmpty()) {
        ezGainStatus_->setText(uiText("EZ-GAIN complete · SYNTHETIC"));
    } else {
        ezGainStatus_->setText(uiText("EZ-GAIN ready"));
    }

    {
        const QSignalBlocker nameBlocker(channelName_);
        const QSignalBlocker iconBlocker(channelIcon_);
        const QSignalBlocker visibleBlocker(channelVisible_);
        const QSignalBlocker gainBlocker(gain_);
        const QSignalBlocker phantomBlocker(phantom_);
        const QSignalBlocker phaseBlocker(phase_);
        const QSignalBlocker lowCutEnabledBlocker(lowCutEnabled_);
        const QSignalBlocker lowCutFrequencyBlocker(lowCutFrequency_);
        const QSignalBlocker panBlocker(pan_);
        const QSignalBlocker muteBlocker(mute_);
        const QSignalBlocker soloBlocker(solo_);
        channelName_->setText(channel->name.value.value_or(inputDisplayName(channel->inputId)));
        channelIcon_->setCurrentIndex(channelIcon_->findData(
            static_cast<int>(channel->icon.value.value_or(model::ChannelIcon::None))));
        channelVisible_->setChecked(channel->visible.value.value_or(true));
        gain_->setVisible(channel->capabilities.gain);
        channelFormLabels_[3]->setVisible(channel->capabilities.gain);
        gain_->setValue(static_cast<int>(std::lround(
            channel->gain.value.value_or(0.0) * 1000.0)));
        phantom_->setVisible(channel->capabilities.phantom48V);
        channelFormLabels_[4]->setVisible(channel->capabilities.phantom48V);
        phantom_->setChecked(channel->phantom48V.has_value()
            && channel->phantom48V->value.value_or(false));
        phase_->setVisible(channel->capabilities.phase);
        channelFormLabels_[5]->setVisible(channel->capabilities.phase);
        phase_->setChecked(channel->phaseInverted.has_value()
            && channel->phaseInverted->value.value_or(false));
        lowCutEnabled_->setEnabled(channel->lowCut.has_value());
        lowCutFrequency_->setEnabled(channel->lowCut.has_value());
        lowCutEnabled_->setVisible(channel->capabilities.lowCut);
        lowCutFrequency_->setVisible(channel->capabilities.lowCut);
        channelFormLabels_[6]->setVisible(channel->capabilities.lowCut);
        channelFormLabels_[7]->setVisible(channel->capabilities.lowCut);
        lowCutEnabled_->setChecked(channel->lowCut.has_value()
            && channel->lowCut->enabled.value.value_or(false));
        lowCutFrequency_->setValue(static_cast<int>(std::lround(
            channel->lowCut.has_value()
                ? channel->lowCut->frequencyHz.value.value_or(20.0) : 20.0)));
        pan_->setValue(static_cast<int>(std::lround(
            channel->pan.value.value_or(0.0) * 100.0)));
        mute_->setChecked(channel->muted.value.value_or(false));
        solo_->setChecked(channel->soloed.value.value_or(false));
    }

    QVector<EqGraphBand> graphBands;
    graphBands.reserve(4);
    for (int band = 0; band < 4; ++band) {
        const auto arrayIndex = static_cast<std::size_t>(band);
        graphBands.append({
            .frequencyHz = channel->eq.frequencyHz[arrayIndex].value.value_or(
                model::defaultChannelEqFrequenciesHz[arrayIndex]),
            .gainDb = channel->eq.gainDb[arrayIndex].value.value_or(0.0),
            .q = channel->eq.q[arrayIndex].value.value_or(1.0),
        });
        const QSignalBlocker blocker(eqGainSliders_[band]);
        eqGainSliders_[band]->setValue(static_cast<int>(std::lround(graphBands.back().gainDb * 10.0)));
    }
    eqGraph_->setBands(std::move(graphBands));

    {
        const QSignalBlocker blocker(compressorAmount_);
        compressorAmount_->setEnabled(channel->capabilities.compressor);
        compressorAmount_->setValue(static_cast<int>(std::lround(
            channel->compressor.amount.value.value_or(0.0) * 1000.0)));
    }
    gainReduction_->setText(QStringLiteral("%1 dB (%2)")
        .arg(QLocale().toString(channel->compressor.gainReductionDb.value.value_or(0.0), 'f', 1),
             uiText("SYNTHETIC")));
    if (compressorUnavailable_.size() == 5) {
        compressorUnavailable_[0]->setText(QStringLiteral("%1 dB").arg(
            QLocale().toString(channel->compressor.thresholdDb.value.value_or(-18.0), 'f', 1)));
        compressorUnavailable_[1]->setText(QStringLiteral("%1:1").arg(
            QLocale().toString(channel->compressor.ratio.value.value_or(3.0), 'f', 2)));
        compressorUnavailable_[2]->setText(QStringLiteral("%1 ms").arg(
            QLocale().toString(channel->compressor.attackMs.value.value_or(10.0), 'f', 1)));
        compressorUnavailable_[3]->setText(QStringLiteral("%1 ms").arg(
            QLocale().toString(channel->compressor.releaseMs.value.value_or(120.0), 'f', 1)));
        compressorUnavailable_[4]->setText(QStringLiteral("%1 dB").arg(
            QLocale().toString(channel->compressor.makeupGainDb.value.value_or(0.0), 'f', 1)));
    }

    for (int send = 0; send < sendSliders_.size(); ++send) {
        const double db = channel->sendLevelDb[static_cast<std::size_t>(send)].value.value_or(-144.0);
        const double normalized = db <= -70.0 ? 0.0 : (db + 70.0) / 80.0;
        const QSignalBlocker blocker(sendSliders_[send]);
        sendSliders_[send]->setValue(static_cast<int>(std::lround(normalized * 1000.0)));
        if (send < monitorSendModes_.size()) {
            const QSignalBlocker modeBlocker(monitorSendModes_[send]);
            monitorSendModes_[send]->setCurrentIndex(monitorSendModes_[send]->findData(
                static_cast<int>(channel->monitorSends[static_cast<std::size_t>(send)]
                    .mode.value.value_or(model::MonitorSendMode::PostFader))));
        }
    }

}

void InspectorWidget::retranslateUi()
{
    tabs_->setTabText(0, uiText("Channel"));
    tabs_->setTabText(1, uiText("Equalizer"));
    tabs_->setTabText(2, uiText("Compressor"));
    tabs_->setTabText(3, uiText("Sends"));
    evidenceNote_->setText(uiText(
        "Hardware control for advanced parameters is unavailable in this build."));
    syntheticBadge_->setText(uiText("SYNTHETIC"));
    currentRouteLabel_->setText(uiText("Current Destination Send"));

    const int selectedIcon = channelIcon_->currentData().toInt();
    {
        const QSignalBlocker blocker(channelIcon_);
        channelIcon_->clear();
        channelIcon_->addItem(uiText("No Icon"), static_cast<int>(model::ChannelIcon::None));
        channelIcon_->addItem(uiText("Microphone"),
                              static_cast<int>(model::ChannelIcon::Microphone));
        channelIcon_->addItem(uiText("Instrument"),
                              static_cast<int>(model::ChannelIcon::Instrument));
        channelIcon_->addItem(uiText("Guitar / Bass"),
                              static_cast<int>(model::ChannelIcon::GuitarBass));
        channelIcon_->addItem(uiText("Playback"),
                              static_cast<int>(model::ChannelIcon::Playback));
        channelIcon_->setCurrentIndex(qMax(0, channelIcon_->findData(selectedIcon)));
    }
    const std::array<const char*, 11> channelLabels {
        "Channel Name", "Channel Icon", "Channel Visibility", "Gain",
        "Phantom Power", "Polarity", "Low Cut", "Frequency", "Pan / Balance",
        "Mute", "Solo",
    };
    for (int index = 0; index < channelFormLabels_.size(); ++index) {
        channelFormLabels_[index]->setText(
            uiText(channelLabels[static_cast<std::size_t>(index)]));
    }
    channelVisible_->setText(uiText("Visible in Mixer and Stage"));
    ezGainSelected_->setText(uiText("EZ-GAIN Selected"));
    ezGainAll_->setText(uiText("EZ-GAIN All Inputs"));
    ezGainCancel_->setText(uiText("Cancel"));

    const std::array<const char*, 4> eqNames {"Low", "Low Mid", "High Mid", "High"};
    for (int index = 0; index < eqBandLabels_.size(); ++index) {
        eqBandLabels_[index]->setText(uiText(eqNames[static_cast<std::size_t>(index)]));
    }
    const std::array<const char*, 7> compressorNames {
        "Compressor Amount", "Threshold", "Ratio", "Attack", "Release",
        "Makeup Gain", "Gain Reduction",
    };
    for (int index = 0; index < compressorLabels_.size(); ++index) {
        compressorLabels_[index]->setText(
            uiText(compressorNames[static_cast<std::size_t>(index)]));
    }
    const std::array<const char*, 4> sendNames {"Monitor 1", "Monitor 2", "FX 1", "FX 2"};
    for (int index = 0; index < sendLabels_.size(); ++index) {
        sendLabels_[index]->setText(uiText(sendNames[static_cast<std::size_t>(index)]));
    }
    for (auto* mode : monitorSendModes_) {
        const int selected = mode->currentData().toInt();
        const QSignalBlocker blocker(mode);
        mode->clear();
        mode->addItem(uiText("Pre-Fader"),
                      static_cast<int>(model::MonitorSendMode::PreFader));
        mode->addItem(uiText("Post-Fader"),
                      static_cast<int>(model::MonitorSendMode::PostFader));
        mode->setCurrentIndex(qMax(0, mode->findData(selected)));
    }
    refresh();
}

} // namespace flow8::ui
