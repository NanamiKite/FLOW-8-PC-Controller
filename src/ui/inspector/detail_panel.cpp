#include "ui/inspector/detail_panel.h"

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
#include <QListWidget>
#include <QLocale>
#include <QPushButton>
#include <QSignalBlocker>
#include <QSlider>
#include <QStackedWidget>
#include <QStringList>
#include <QVBoxLayout>

#include <algorithm>
#include <array>
#include <cmath>

namespace flow8::ui {

DetailPanel::DetailPanel(Flow8Device& device, QWidget* parent)
    : QWidget(parent)
    , device_(device)
    , pages_(new QStackedWidget(this))
    , busTitle_(new QLabel(this))
    , busCapability_(new QLabel(this))
    , busLevel_(new QSlider(Qt::Horizontal, this))
    , busBalance_(new QSlider(Qt::Horizontal, this))
    , busLimiter_(new QSlider(Qt::Horizontal, this))
    , busEqGraph_(new EqGraphWidget(this))
    , busOutputDelay_(new QLabel(this))
    , fxTitle_(new QLabel(this))
    , fxPreset_(new QComboBox(this))
    , fxType_(new QLabel(this))
    , fxMute_(new QCheckBox(this))
    , fxTempo_(new QLabel(this))
    , fxTap_(new QPushButton(this))
    , hardwareSnapshots_(new QListWidget(this))
    , appLibrary_(new QLabel(this))
    , appSnapshots_(new QListWidget(this))
    , appSnapshotName_(new QLineEdit(this))
    , appSnapshotScope_(new QComboBox(this))
    , storeSnapshot_(new QPushButton(this))
    , loadAppSnapshot_(new QPushButton(this))
    , renameAppSnapshot_(new QPushButton(this))
    , shareSnapshot_(new QPushButton(this))
    , routingGrid_(new QWidget(this))
    , routingTitle_(new QLabel(this))
    , advancedRoutingTitle_(new QLabel(this))
    , usbMode_(new QComboBox(this))
    , headphoneSource_(new QComboBox(this))
    , monitorStereoLink_(new QCheckBox(this))
    , routingEvidence_(new QLabel(this))
{
    setProperty("class", QStringLiteral("inspector"));
    setMinimumHeight(290);
    fxTap_->setObjectName(QStringLiteral("fxTapTempo"));
    fxMute_->setObjectName(QStringLiteral("fxMute"));

    auto* busPage = new QWidget(pages_);
    auto* busLayout = new QVBoxLayout(busPage);
    auto* busHeader = new QHBoxLayout;
    busTitle_->setProperty("class", QStringLiteral("inspectorTitle"));
    busCapability_->setProperty("class", QStringLiteral("secondaryText"));
    busHeader->addWidget(busTitle_);
    busHeader->addSpacing(12);
    busHeader->addWidget(busCapability_);
    busHeader->addStretch();
    busOutputDelay_->setProperty("class", QStringLiteral("stripIndicator"));
    busHeader->addWidget(busOutputDelay_);
    busLevelLabel_ = new QLabel(busPage);
    busBalanceLabel_ = new QLabel(busPage);
    busLimiterLabel_ = new QLabel(busPage);
    busLevel_->setRange(0, 1000);
    busLevel_->setObjectName(QStringLiteral("busLevel"));
    busBalance_->setRange(-100, 100);
    busBalance_->setObjectName(QStringLiteral("busBalance"));
    busLimiter_->setRange(-300, 0);
    busLimiter_->setObjectName(QStringLiteral("busLimiter"));
    auto* levelRow = new QHBoxLayout;
    levelRow->addWidget(busLevelLabel_);
    levelRow->addWidget(busLevel_, 1);
    levelRow->addWidget(busBalanceLabel_);
    levelRow->addWidget(busBalance_, 1);
    levelRow->addWidget(busLimiterLabel_);
    levelRow->addWidget(busLimiter_, 1);
    busLayout->addLayout(busHeader);
    busLayout->addLayout(levelRow);
    auto* busEqRow = new QHBoxLayout;
    busEqRow->addWidget(busEqGraph_, 2);
    auto* busSliders = new QGridLayout;
    for (int band = 0; band < 9; ++band) {
        auto* slider = new QSlider(Qt::Vertical, busPage);
        slider->setRange(-150, 150);
        slider->setMinimumHeight(110);
        slider->setObjectName(QStringLiteral("busEqGain%1").arg(band));
        busEqSliders_.append(slider);
        busSliders->addWidget(slider, 0, band);
        busSliders->addWidget(new QLabel(
            QString::number(model::busEqFrequenciesHz[static_cast<std::size_t>(band)], 'g', 3),
            busPage), 1, band, Qt::AlignCenter);
        connect(slider, &QSlider::valueChanged, this, [this, band](const int value) {
            (void)device_.setBusEqGain(selectedBus_, band, value / 10.0);
        });
    }
    busEqRow->addLayout(busSliders, 3);
    busLayout->addLayout(busEqRow, 1);
    connect(busLevel_, &QSlider::valueChanged, this, [this](const int value) {
        (void)device_.setBusFader(selectedBus_, value / 1000.0);
    });
    connect(busBalance_, &QSlider::valueChanged, this, [this](const int value) {
        (void)device_.setBusBalance(selectedBus_, value / 100.0);
    });
    connect(busLimiter_, &QSlider::valueChanged, this, [this](const int value) {
        (void)device_.setBusLimiterDb(selectedBus_, value / 10.0);
    });

    auto* fxPage = new QWidget(pages_);
    auto* fxLayout = new QVBoxLayout(fxPage);
    fxTitle_->setProperty("class", QStringLiteral("inspectorTitle"));
    fxLayout->addWidget(fxTitle_);
    auto* fxForm = new QFormLayout;
    auto* presetLabel = new QLabel(fxPage);
    auto* effectTypeLabel = new QLabel(fxPage);
    fxFormLabels_.append(presetLabel);
    fxFormLabels_.append(effectTypeLabel);
    fxForm->addRow(presetLabel, fxPreset_);
    fxForm->addRow(effectTypeLabel, fxType_);
    for (int parameter = 0; parameter < 2; ++parameter) {
        auto* slider = new QSlider(Qt::Horizontal, fxPage);
        slider->setRange(0, 1000);
        slider->setObjectName(QStringLiteral("fxParameter%1").arg(parameter));
        fxParameters_.append(slider);
        auto* label = new QLabel(fxPage);
        fxFormLabels_.append(label);
        fxForm->addRow(label, slider);
        connect(slider, &QSlider::valueChanged, this, [this, parameter](const int value) {
            (void)device_.setFxParameter(selectedFx_, parameter, value / 1000.0);
        });
    }
    auto* fxActions = new QHBoxLayout;
    fxActions->addWidget(fxMute_);
    fxActions->addWidget(fxTap_);
    fxActions->addWidget(fxTempo_);
    fxActions->addStretch();
    auto* engineLabel = new QLabel(fxPage);
    fxFormLabels_.append(engineLabel);
    fxForm->addRow(engineLabel, fxActions);
    fxInfo_ = new QLabel(fxPage);
    fxInfo_->setWordWrap(true);
    fxInfo_->setProperty("class", QStringLiteral("secondaryText"));
    fxForm->addRow(fxInfo_);
    fxLayout->addLayout(fxForm);
    fxLayout->addStretch();
    connect(fxPreset_, &QComboBox::currentIndexChanged, this, [this](const int index) {
        if (index >= 0) {
            (void)device_.setFxPreset(selectedFx_, fxPreset_->itemData(index).toInt());
        }
    });
    connect(fxMute_, &QCheckBox::toggled, this, [this](const bool muted) {
        (void)device_.setFxMuted(selectedFx_, muted);
    });
    connect(fxTap_, &QPushButton::clicked, this, [this] { (void)device_.tapTempo(); });

    auto* snapshotPage = new QWidget(pages_);
    auto* snapshotLayout = new QHBoxLayout(snapshotPage);
    auto* hardwareCard = new QWidget(snapshotPage);
    hardwareCard->setProperty("class", QStringLiteral("detailCard"));
    auto* hardwareLayout = new QVBoxLayout(hardwareCard);
    hardwareTitle_ = new QLabel(hardwareCard);
    hardwareLayout->addWidget(hardwareTitle_);
    hardwareSnapshots_->setObjectName(QStringLiteral("hardwareSnapshots"));
    hardwareLayout->addWidget(hardwareSnapshots_);
    recallButton_ = new QPushButton(hardwareCard);
    hardwareLayout->addWidget(recallButton_);
    connect(recallButton_, &QPushButton::clicked, this, [this] {
        if (hardwareSnapshots_->currentRow() >= 0) {
            (void)device_.recallSnapshot(hardwareSnapshots_->currentRow());
        }
    });
    auto* libraryCard = new QWidget(snapshotPage);
    libraryCard->setProperty("class", QStringLiteral("detailCard"));
    auto* libraryLayout = new QVBoxLayout(libraryCard);
    libraryTitle_ = new QLabel(libraryCard);
    libraryLayout->addWidget(libraryTitle_);
    appLibrary_->setProperty("class", QStringLiteral("secondaryText"));
    appLibrary_->setAlignment(Qt::AlignCenter);
    appLibrary_->setWordWrap(true);
    libraryLayout->addWidget(appLibrary_);
    appSnapshots_->setObjectName(QStringLiteral("appSnapshots"));
    libraryLayout->addWidget(appSnapshots_, 1);
    appSnapshotName_->setObjectName(QStringLiteral("appSnapshotName"));
    libraryLayout->addWidget(appSnapshotName_);
    libraryLayout->addWidget(appSnapshotScope_);
    auto* libraryActions = new QHBoxLayout;
    storeSnapshot_->setObjectName(QStringLiteral("storeAppSnapshot"));
    loadAppSnapshot_->setObjectName(QStringLiteral("loadAppSnapshot"));
    renameAppSnapshot_->setObjectName(QStringLiteral("renameAppSnapshot"));
    shareSnapshot_->setObjectName(QStringLiteral("shareAppSnapshot"));
    shareSnapshot_->setEnabled(false);
    libraryActions->addWidget(storeSnapshot_);
    libraryActions->addWidget(loadAppSnapshot_);
    libraryActions->addWidget(renameAppSnapshot_);
    libraryActions->addWidget(shareSnapshot_);
    libraryLayout->addLayout(libraryActions);
    snapshotLayout->addWidget(hardwareCard, 1);
    snapshotLayout->addWidget(libraryCard, 1);
    connect(storeSnapshot_, &QPushButton::clicked, this, [this] {
        if (appSnapshotScope_->currentIndex() >= 0
            && device_.storeAppSnapshot(
                appSnapshotName_->text(),
                static_cast<model::SnapshotScope>(
                    appSnapshotScope_->currentData().toInt()))) {
            appSnapshotName_->clear();
            refreshSnapshots();
        }
    });
    connect(loadAppSnapshot_, &QPushButton::clicked, this, [this] {
        if (appSnapshots_->currentRow() >= 0) {
            (void)device_.loadAppSnapshot(appSnapshots_->currentRow());
        }
    });
    connect(renameAppSnapshot_, &QPushButton::clicked, this, [this] {
        if (appSnapshots_->currentRow() >= 0 && !appSnapshotName_->text().trimmed().isEmpty()) {
            (void)device_.renameAppSnapshot(
                appSnapshots_->currentRow(), appSnapshotName_->text());
            appSnapshotName_->clear();
            refreshSnapshots();
        }
    });

    auto* routingPage = new QWidget(pages_);
    auto* routingLayout = new QVBoxLayout(routingPage);
    routingTitle_->setParent(routingPage);
    routingTitle_->setProperty("class", QStringLiteral("inspectorTitle"));
    routingLayout->addWidget(routingTitle_);
    routingLayout->addWidget(routingGrid_);
    advancedRoutingTitle_->setParent(routingPage);
    advancedRoutingTitle_->setProperty("class", QStringLiteral("navigationTitle"));
    routingLayout->addWidget(advancedRoutingTitle_);
    auto* advancedRouting = new QWidget(routingPage);
    auto* advancedGrid = new QGridLayout(advancedRouting);
    auto* usbModeLabel = new QLabel(advancedRouting);
    usbModeLabel->setObjectName(QStringLiteral("usbModeLabel"));
    auto* headphoneLabel = new QLabel(advancedRouting);
    headphoneLabel->setObjectName(QStringLiteral("headphoneSourceLabel"));
    advancedGrid->addWidget(usbModeLabel, 0, 0);
    advancedGrid->addWidget(usbMode_, 0, 1);
    advancedGrid->addWidget(headphoneLabel, 0, 2);
    advancedGrid->addWidget(headphoneSource_, 0, 3);
    advancedGrid->addWidget(monitorStereoLink_, 0, 4);
    for (int destination = 0; destination < 9; ++destination) {
        auto* check = new QCheckBox(advancedRouting);
        check->setObjectName(QStringLiteral("usbRoute%1").arg(destination));
        usbRouteChecks_.append(check);
        advancedGrid->addWidget(check, 1 + destination / 5, destination % 5);
        connect(check, &QCheckBox::toggled, this,
                [this, destination](const bool enabled) {
                    (void)device_.setUsbRouteEnabled(
                        static_cast<model::UsbRouteDestination>(destination), enabled);
                });
    }
    for (int effect = 0; effect < 2; ++effect) {
        for (int monitor = 0; monitor < 2; ++monitor) {
            const int route = effect * 2 + monitor;
            auto* check = new QCheckBox(advancedRouting);
            check->setObjectName(QStringLiteral("fxMonitorRoute%1").arg(route));
            fxMonitorChecks_.append(check);
            advancedGrid->addWidget(check, 3, route);
            connect(check, &QCheckBox::toggled, this,
                    [this, effect, monitor](const bool enabled) {
                        (void)device_.setFxMonitorRouteEnabled(effect, monitor, enabled);
                    });
        }
    }
    routingEvidence_->setParent(routingPage);
    routingEvidence_->setWordWrap(true);
    routingEvidence_->setProperty("class", QStringLiteral("secondaryText"));
    routingLayout->addWidget(advancedRouting);
    routingLayout->addWidget(routingEvidence_);
    routingLayout->addStretch();
    connect(usbMode_, &QComboBox::currentIndexChanged, this, [this](const int index) {
        if (index >= 0) {
            (void)device_.setUsbMode(
                static_cast<model::UsbMode>(usbMode_->itemData(index).toInt()));
        }
    });
    connect(headphoneSource_, &QComboBox::currentIndexChanged, this, [this](const int index) {
        if (index >= 0) {
            (void)device_.setHeadphoneSource(
                static_cast<model::HeadphoneSource>(headphoneSource_->itemData(index).toInt()));
        }
    });
    connect(monitorStereoLink_, &QCheckBox::toggled,
            this, [this](const bool linked) {
                (void)device_.setMonitorStereoLink(linked);
            });

    pages_->addWidget(busPage);
    pages_->addWidget(fxPage);
    pages_->addWidget(snapshotPage);
    pages_->addWidget(routingPage);
    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(8, 8, 8, 8);
    layout->addWidget(pages_);

    connect(&device_.state(), &Flow8State::stateReset, this, &DetailPanel::refresh);
    connect(&device_.state(), &Flow8State::busChanged, this, [this](const int index) {
        if (index == selectedBus_) refreshBus();
    });
    connect(&device_.state(), &Flow8State::effectChanged, this, [this](const int index) {
        if (index == selectedFx_) refreshFx();
    });
    connect(&device_.state(), &Flow8State::snapshotChanged, this, &DetailPanel::refreshSnapshots);
    connect(&device_.state(), &Flow8State::routingChanged, this, &DetailPanel::refreshRouting);
    retranslateUi();
}

void DetailPanel::showBus(const int busIndex)
{
    selectedBus_ = busIndex;
    pages_->setCurrentIndex(0);
    refreshBus();
}

void DetailPanel::showFx(const int effectIndex)
{
    selectedFx_ = effectIndex;
    pages_->setCurrentIndex(1);
    refreshFx();
}

void DetailPanel::showSnapshots()
{
    pages_->setCurrentIndex(2);
    refreshSnapshots();
}

void DetailPanel::showRouting()
{
    pages_->setCurrentIndex(3);
    refreshRouting();
}

void DetailPanel::refresh()
{
    refreshBus();
    refreshFx();
    refreshSnapshots();
    refreshRouting();
}

void DetailPanel::refreshBus()
{
    const auto* bus = device_.state().bus(selectedBus_);
    if (bus == nullptr) {
        return;
    }
    busTitle_->setText(busDisplayName(bus->busId));
    QStringList features {uiText("Level")};
    if (bus->capabilities.balance) features.append(uiText("Balance"));
    if (bus->capabilities.equalizer) features.append(uiText("9-band EQ"));
    if (bus->capabilities.limiter) features.append(uiText("Limiter"));
    if (bus->busId == model::BusId::Monitor1 || bus->busId == model::BusId::Monitor2) {
        features.append(uiText("Channel Sends · Pre/Post-Fader"));
        if (device_.state().routing().monitorStereoLink.value.value_or(false)) {
            features.append(uiText("MON1/2 LINKED"));
        }
    }
    busCapability_->setText(features.join(QStringLiteral(" · ")));
    busOutputDelay_->setVisible(
        bus->outputDelay.has_value() && device_.state().preferences().showOutputDelayIndicator);
    if (bus->outputDelay.has_value()) {
        if (bus->outputDelay->milliseconds.value.has_value()) {
            busOutputDelay_->setText(uiText("Delay: %1 ms").arg(
                QLocale().toString(*bus->outputDelay->milliseconds.value, 'f', 1)));
        } else {
            busOutputDelay_->setText(uiText("Delay: Unknown"));
        }
    }
    const QSignalBlocker levelBlocker(busLevel_);
    const QSignalBlocker balanceBlocker(busBalance_);
    const QSignalBlocker limiterBlocker(busLimiter_);
    busLevel_->setValue(static_cast<int>(std::lround(bus->fader.value.value_or(0.0) * 1000.0)));
    busBalance_->setVisible(bus->balance.has_value());
    busBalance_->setValue(static_cast<int>(std::lround(
        (bus->balance.has_value() ? bus->balance->value.value_or(0.0) : 0.0) * 100.0)));
    busLimiter_->setVisible(bus->limiterDb.has_value());
    busLimiter_->setValue(static_cast<int>(std::lround(
        (bus->limiterDb.has_value() ? bus->limiterDb->value.value_or(-30.0) : -30.0)
            * 10.0)));
    busEqGraph_->setVisible(bus->eq.has_value());
    QVector<EqGraphBand> graphBands;
    graphBands.reserve(9);
    for (int band = 0; band < 9; ++band) {
        const auto index = static_cast<std::size_t>(band);
        const bool hasEq = bus->eq.has_value();
        const double gain = hasEq ? bus->eq->gainDb[index].value.value_or(0.0) : 0.0;
        graphBands.append({model::busEqFrequenciesHz[index], gain, 2.5, hasEq});
        const QSignalBlocker blocker(busEqSliders_[band]);
        busEqSliders_[band]->setVisible(hasEq);
        busEqSliders_[band]->setValue(static_cast<int>(std::lround(gain * 10.0)));
    }
    busEqGraph_->setBands(std::move(graphBands));
}

void DetailPanel::refreshFx()
{
    if (selectedFx_ < 0 || selectedFx_ >= device_.state().effects().size()) {
        return;
    }
    const auto& effect = device_.state().effects().at(selectedFx_);
    fxTitle_->setText(uiText("FX %1 · Independent Engine").arg(selectedFx_ + 1));
    fxType_->setText(effect.effectType.value.value_or(
        uiText("Preset-specific type (unavailable)")));
    const QSignalBlocker presetBlocker(fxPreset_);
    fxPreset_->setCurrentIndex(std::max(0, effect.preset.value.value_or(1) - 1));
    for (int parameter = 0; parameter < fxParameters_.size(); ++parameter) {
        const QSignalBlocker blocker(fxParameters_[parameter]);
        fxParameters_[parameter]->setValue(static_cast<int>(std::lround(
            effect.parameters[static_cast<std::size_t>(parameter)].value.value.value_or(0.0)
                * 1000.0)));
    }
    const QSignalBlocker muteBlocker(fxMute_);
    fxMute_->setChecked(effect.muted.value.value_or(false));
    fxTempo_->setText(QStringLiteral("%1 BPM").arg(
        QLocale().toString(effect.tapTempoBpm.value.value_or(120.0), 'f', 1)));
}

void DetailPanel::refreshSnapshots()
{
    const QSignalBlocker blocker(hardwareSnapshots_);
    hardwareSnapshots_->clear();
    for (const auto& snapshot : device_.state().snapshots()) {
        if (snapshot.storage != model::SnapshotStorage::HardwareSlot) continue;
        const QString name = snapshot.name.value.has_value() && !snapshot.name.value->isEmpty()
            ? *snapshot.name.value : uiText("Empty Slot");
        hardwareSnapshots_->addItem(QStringLiteral("%1   %2")
            .arg(snapshot.index + 1, 2, 10, QLatin1Char('0')).arg(name));
    }
    hardwareSnapshots_->setCurrentRow(device_.state().activeSnapshotIndex());
    appSnapshots_->clear();
    int appRow = 0;
    int activeAppRow = -1;
    for (int stateIndex = 0; stateIndex < device_.state().snapshots().size(); ++stateIndex) {
        const auto& snapshot = device_.state().snapshots().at(stateIndex);
        if (snapshot.storage != model::SnapshotStorage::AppLibrary) continue;
        const QString name = snapshot.name.value.value_or(uiText("Untitled Snapshot"));
        const QString timestamp = snapshot.timestamp.value.has_value()
            ? QLocale().toString(*snapshot.timestamp.value, QLocale::ShortFormat)
            : uiText("Unknown");
        appSnapshots_->addItem(QStringLiteral("%1 · %2").arg(name, timestamp));
        if (stateIndex == device_.state().activeSnapshotIndex()) {
            activeAppRow = appRow;
        }
        ++appRow;
    }
    appSnapshots_->setCurrentRow(activeAppRow);
    appLibrary_->setVisible(appSnapshots_->count() == 0);
}

void DetailPanel::refreshRouting()
{
    if (routingGrid_->layout() == nullptr) {
        auto* grid = new QGridLayout(routingGrid_);
        for (int destination = 0; destination < 5; ++destination) {
            auto* label = new QLabel(routingGrid_);
            routingDestinationLabels_.append(label);
            grid->addWidget(label, 0, destination + 1, Qt::AlignCenter);
        }
        for (int input = 0; input < 7; ++input) {
            const auto* channel = device_.state().channel(input);
            auto* label = new QLabel(routingGrid_);
            if (channel != nullptr) label->setProperty("flow8InputId", static_cast<int>(channel->inputId));
            routingInputLabels_.append(label);
            grid->addWidget(label, input + 1, 0);
            for (int destination = 0; destination < 5; ++destination) {
                auto* check = new QCheckBox(routingGrid_);
                check->setObjectName(QStringLiteral("routing_%1_%2").arg(input).arg(destination));
                routingChecks_.append(check);
                grid->addWidget(check, input + 1, destination + 1, Qt::AlignCenter);
                connect(check, &QCheckBox::toggled, this,
                        [this, input, destination](const bool enabled) {
                            (void)device_.setRouteEnabled(
                                input, static_cast<model::RoutingDestination>(destination), enabled);
                        });
            }
        }
    }
    const std::array<model::BusId, 5> destinations {
        model::BusId::Main, model::BusId::Monitor1, model::BusId::Monitor2,
        model::BusId::Fx1, model::BusId::Fx2,
    };
    for (int index = 0; index < routingDestinationLabels_.size(); ++index) {
        routingDestinationLabels_[index]->setText(
            busDisplayName(destinations[static_cast<std::size_t>(index)]));
    }
    for (int index = 0; index < routingInputLabels_.size(); ++index) {
        const auto* channel = device_.state().channel(index);
        routingInputLabels_[index]->setText(channel == nullptr
            ? uiText("Input %1").arg(index + 1) : inputDisplayName(channel->inputId));
    }
    for (int input = 0; input < 7; ++input) {
        for (int destination = 0; destination < 5; ++destination) {
            const int widgetIndex = input * 5 + destination;
            if (widgetIndex >= routingChecks_.size()) continue;
            const auto* route = device_.state().routing().route(
                input, static_cast<model::RoutingDestination>(destination));
            const QSignalBlocker blocker(routingChecks_[widgetIndex]);
            routingChecks_[widgetIndex]->setChecked(
                route != nullptr && route->enabled.value.value_or(false));
        }
    }
    const auto& routing = device_.state().routing();
    {
        const QSignalBlocker blocker(usbMode_);
        usbMode_->setCurrentIndex(usbMode_->findData(static_cast<int>(
            routing.usbMode.value.value_or(model::UsbMode::Streaming))));
    }
    for (int destination = 0; destination < usbRouteChecks_.size(); ++destination) {
        const auto* route = routing.usbRoute(
            static_cast<model::UsbRouteDestination>(destination));
        const QSignalBlocker blocker(usbRouteChecks_[destination]);
        usbRouteChecks_[destination]->setChecked(
            route != nullptr && route->enabled.value.value_or(false));
    }
    for (int effect = 0; effect < 2; ++effect) {
        for (int monitor = 0; monitor < 2; ++monitor) {
            const int index = effect * 2 + monitor;
            const auto* route = routing.fxMonitorRoute(effect, monitor);
            const QSignalBlocker blocker(fxMonitorChecks_[index]);
            fxMonitorChecks_[index]->setChecked(
                route != nullptr && route->enabled.value.value_or(false));
        }
    }
    {
        const QSignalBlocker blocker(headphoneSource_);
        headphoneSource_->setCurrentIndex(headphoneSource_->findData(static_cast<int>(
            routing.headphoneSource.value.value_or(model::HeadphoneSource::Main))));
    }
    {
        const QSignalBlocker blocker(monitorStereoLink_);
        monitorStereoLink_->setChecked(routing.monitorStereoLink.value.value_or(false));
    }
}

void DetailPanel::retranslateUi()
{
    busLevelLabel_->setText(uiText("Level"));
    busBalanceLabel_->setText(uiText("Balance"));
    busLimiterLabel_->setText(uiText("Limiter"));

    const QSignalBlocker presetBlocker(fxPreset_);
    fxPreset_->clear();
    for (int preset = 1; preset <= 16; ++preset) {
        fxPreset_->addItem(uiText("Preset %1").arg(preset), preset);
    }
    if (fxFormLabels_.size() >= 5) {
        fxFormLabels_[0]->setText(uiText("Preset"));
        fxFormLabels_[1]->setText(uiText("Effect Type"));
        fxFormLabels_[2]->setText(uiText("Parameter %1 (UNKNOWN)").arg(1));
        fxFormLabels_[3]->setText(uiText("Parameter %1 (UNKNOWN)").arg(2));
        fxFormLabels_[4]->setText(uiText("Engine"));
    }
    fxMute_->setText(uiText("Mute"));
    fxTap_->setText(uiText("Tap Tempo"));
    fxInfo_->setText(uiText(
        "Tap tempo is global in the official MIDI chart and applies only to compatible effects.\n"
        "Effect-specific parameter names remain unavailable."));

    hardwareTitle_->setText(uiText("Hardware Slots · 15"));
    recallButton_->setText(uiText("Recall in Simulator"));
    libraryTitle_->setText(uiText("App Library"));
    appLibrary_->setText(uiText(
        "No app-library snapshots yet.\nThis is separate from the 15 hardware slots."));
    appSnapshotName_->setPlaceholderText(uiText("Snapshot Name"));
    const int selectedScope = appSnapshotScope_->currentData().toInt();
    {
        const QSignalBlocker blocker(appSnapshotScope_);
        appSnapshotScope_->clear();
        const std::array scopes {
            model::SnapshotScope::Full, model::SnapshotScope::Fx,
            model::SnapshotScope::Channel, model::SnapshotScope::Main,
            model::SnapshotScope::Monitor, model::SnapshotScope::Routing,
        };
        const std::array<const char*, 6> names {
            "Full", "FX", "Channel", "Main", "Monitor", "Routing",
        };
        for (int index = 0; index < static_cast<int>(scopes.size()); ++index) {
            appSnapshotScope_->addItem(
                uiText(names[static_cast<std::size_t>(index)]),
                static_cast<int>(scopes[static_cast<std::size_t>(index)]));
        }
        appSnapshotScope_->setCurrentIndex(qMax(0, appSnapshotScope_->findData(selectedScope)));
    }
    storeSnapshot_->setText(uiText("Store in App Library"));
    loadAppSnapshot_->setText(uiText("Load in Simulator"));
    renameAppSnapshot_->setText(uiText("Rename"));
    shareSnapshot_->setText(uiText("Share / Export"));
    shareSnapshot_->setToolTip(uiText("Snapshot export is not implemented yet."));
    routingTitle_->setText(uiText("Routing · Source → Destination"));
    advancedRoutingTitle_->setText(uiText("USB, FX and Headphone Routing"));
    findChild<QLabel*>(QStringLiteral("usbModeLabel"))->setText(uiText("USB Mode"));
    findChild<QLabel*>(QStringLiteral("headphoneSourceLabel"))->setText(
        uiText("Headphone Source"));
    const int selectedUsbMode = usbMode_->currentData().toInt();
    {
        const QSignalBlocker blocker(usbMode_);
        usbMode_->clear();
        usbMode_->addItem(uiText("USB Streaming"),
                          static_cast<int>(model::UsbMode::Streaming));
        usbMode_->addItem(uiText("USB Recording"),
                          static_cast<int>(model::UsbMode::Recording));
        usbMode_->setCurrentIndex(qMax(0, usbMode_->findData(selectedUsbMode)));
    }
    const std::array<const char*, 9> usbRoutes {
        "USB → Input 1", "USB → Input 2", "USB → Input 3", "USB → Input 4",
        "USB → Input 5/6", "USB → Input 7/8", "USB → USB / Bluetooth",
        "USB → Monitor 1", "USB → Monitor 2",
    };
    for (int index = 0; index < usbRouteChecks_.size(); ++index) {
        usbRouteChecks_[index]->setText(uiText(usbRoutes[static_cast<std::size_t>(index)]));
    }
    const std::array<const char*, 4> fxRoutes {
        "FX 1 → Monitor 1", "FX 1 → Monitor 2",
        "FX 2 → Monitor 1", "FX 2 → Monitor 2",
    };
    for (int index = 0; index < fxMonitorChecks_.size(); ++index) {
        fxMonitorChecks_[index]->setText(uiText(fxRoutes[static_cast<std::size_t>(index)]));
    }
    const int selectedHeadphone = headphoneSource_->currentData().toInt();
    {
        const QSignalBlocker blocker(headphoneSource_);
        headphoneSource_->clear();
        headphoneSource_->addItem(uiText("Main"),
            static_cast<int>(model::HeadphoneSource::Main));
        headphoneSource_->addItem(uiText("Monitor 1"),
            static_cast<int>(model::HeadphoneSource::Monitor1));
        headphoneSource_->addItem(uiText("Monitor 2"),
            static_cast<int>(model::HeadphoneSource::Monitor2));
        headphoneSource_->setCurrentIndex(
            qMax(0, headphoneSource_->findData(selectedHeadphone)));
    }
    monitorStereoLink_->setText(uiText("MON1/2 Linked"));
    routingEvidence_->setText(uiText(
        "Official capability · Simulator only · BLE routing protocol UNKNOWN"));

    refresh();
}

} // namespace flow8::ui
