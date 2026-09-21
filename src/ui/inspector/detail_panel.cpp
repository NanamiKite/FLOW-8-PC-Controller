#include "ui/inspector/detail_panel.h"

#include "core/flow8_device.h"
#include "ui/widgets/eq_graph_widget.h"
#include "ui/widgets/meter_widget.h"
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
    , busMeter_(new MeterWidget(this))
    , busLevel_(new QSlider(Qt::Horizontal, this))
    , busMute_(new QCheckBox(this))
    , busBalance_(new QSlider(Qt::Horizontal, this))
    , busLimiter_(new QSlider(Qt::Horizontal, this))
    , busEqGraph_(new EqGraphWidget(this))
    , busOutputDelay_(new QLabel(this))
    , fxTitle_(new QLabel(this))
    , fxMeter_(new MeterWidget(this))
    , fxMaster_(new QSlider(Qt::Horizontal, this))
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
    , deleteAppSnapshot_(new QPushButton(this))
    , shareSnapshot_(new QPushButton(this))
    , routingTitle_(new QLabel(this))
    , advancedRoutingTitle_(new QLabel(this))
    , usbMode_(new QComboBox(this))
    , usbInput56_(new QComboBox(this))
    , usbInput78_(new QComboBox(this))
    , headphoneSource_(new QComboBox(this))
    , headphoneTapPoint_(new QComboBox(this))
    , bluetoothUsbPhonesOnly_(new QCheckBox(this))
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
    levelRow->addWidget(busMeter_);
    levelRow->addWidget(busLevelLabel_);
    levelRow->addWidget(busLevel_, 1);
    levelRow->addWidget(busMute_);
    levelRow->addWidget(busBalanceLabel_);
    levelRow->addWidget(busBalance_, 1);
    levelRow->addWidget(busLimiterLabel_);
    levelRow->addWidget(busLimiter_, 1);
    busLayout->addLayout(busHeader);
    busLayout->addLayout(levelRow);
    auto* busEqRow = new QVBoxLayout;
    busEqRow->setSpacing(8);
    busEqGraph_->setObjectName(QStringLiteral("busEqGraph"));
    busEqRow->addWidget(busEqGraph_, 3);
    auto* busSliders = new QGridLayout;
    busSliders->setHorizontalSpacing(12);
    for (int band = 0; band < 9; ++band) {
        auto* slider = new QSlider(Qt::Vertical, busPage);
        slider->setRange(-150, 150);
        slider->setMinimumHeight(110);
        slider->setObjectName(QStringLiteral("busEqGain%1").arg(band));
        busEqSliders_.append(slider);
        auto* valueLabel = new QLabel(busPage);
        valueLabel->setObjectName(QStringLiteral("busEqValue%1").arg(band));
        valueLabel->setAlignment(Qt::AlignCenter);
        busEqLabels_.append(valueLabel);
        busSliders->addWidget(slider, 0, band);
        busSliders->addWidget(valueLabel, 1, band, Qt::AlignCenter);
        connect(slider, &QSlider::valueChanged, this, [this, band](const int value) {
            (void)device_.setBusEqGain(selectedBus_, band, value / 10.0);
        });
    }
    busEqRow->addLayout(busSliders, 2);
    busLayout->addLayout(busEqRow, 1);
    connect(busEqGraph_, &EqGraphWidget::bandGainEdited, this,
            [this](const int band, const double gainDb) {
                (void)device_.setBusEqGain(selectedBus_, band, gainDb);
            });
    connect(busLevel_, &QSlider::valueChanged, this, [this](const int value) {
        (void)device_.setBusFader(selectedBus_, value / 1000.0);
    });
    connect(busMute_, &QCheckBox::toggled, this, [this](const bool muted) {
        (void)device_.setBusMuted(selectedBus_, muted);
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
    auto* fxHeader = new QHBoxLayout;
    fxHeader->addWidget(fxTitle_);
    fxHeader->addStretch();
    fxHeader->addWidget(fxMeter_);
    fxLayout->addLayout(fxHeader);
    auto* fxForm = new QFormLayout;
    auto* masterLabel = new QLabel(fxPage);
    auto* presetLabel = new QLabel(fxPage);
    auto* effectTypeLabel = new QLabel(fxPage);
    fxMaster_->setRange(0, 1000);
    fxMaster_->setObjectName(QStringLiteral("fxMaster"));
    fxFormLabels_.append(masterLabel);
    fxFormLabels_.append(presetLabel);
    fxFormLabels_.append(effectTypeLabel);
    fxForm->addRow(masterLabel, fxMaster_);
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
    auto* returnRow = new QWidget(fxPage);
    auto* returnLayout = new QHBoxLayout(returnRow);
    returnLayout->setContentsMargins(0, 0, 0, 0);
    for (int destination = 0; destination < 3; ++destination) {
        auto* route = new QCheckBox(returnRow);
        route->setObjectName(QStringLiteral("fxInspectorOutput%1").arg(destination));
        fxReturnChecks_.append(route);
        returnLayout->addWidget(route);
        connect(route, &QCheckBox::toggled, this,
                [this, destination](const bool enabled) {
                    (void)device_.setFxOutputRouteEnabled(
                        selectedFx_,
                        static_cast<model::FxOutputDestination>(destination),
                        enabled);
                });
    }
    auto* returnLabel = new QLabel(fxPage);
    fxFormLabels_.append(returnLabel);
    fxForm->addRow(returnLabel, returnRow);
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
    connect(fxMaster_, &QSlider::valueChanged, this, [this](const int value) {
        (void)device_.setDestinationMaster(
            selectedFx_ == 0 ? model::RoutingDestination::Fx1
                             : model::RoutingDestination::Fx2,
            value / 1000.0);
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
    hardwareStore_ = new QPushButton(hardwareCard);
    hardwareRename_ = new QPushButton(hardwareCard);
    hardwareDelete_ = new QPushButton(hardwareCard);
    hardwareReset_ = new QPushButton(hardwareCard);
    auto* hardwareActions = new QGridLayout;
    hardwareActions->addWidget(recallButton_, 0, 0);
    hardwareActions->addWidget(hardwareStore_, 0, 1);
    hardwareActions->addWidget(hardwareRename_, 1, 0);
    hardwareActions->addWidget(hardwareDelete_, 1, 1);
    hardwareActions->addWidget(hardwareReset_, 2, 0, 1, 2);
    hardwareLayout->addLayout(hardwareActions);
    for (auto* unavailable : {hardwareStore_, hardwareRename_, hardwareDelete_, hardwareReset_}) {
        unavailable->setEnabled(false);
    }
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
    deleteAppSnapshot_->setObjectName(QStringLiteral("deleteAppSnapshot"));
    shareSnapshot_->setObjectName(QStringLiteral("shareAppSnapshot"));
    shareSnapshot_->setEnabled(false);
    libraryActions->addWidget(storeSnapshot_);
    libraryActions->addWidget(loadAppSnapshot_);
    libraryActions->addWidget(renameAppSnapshot_);
    libraryActions->addWidget(deleteAppSnapshot_);
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
    connect(deleteAppSnapshot_, &QPushButton::clicked, this, [this] {
        if (appSnapshots_->currentRow() >= 0) {
            (void)device_.deleteAppSnapshot(appSnapshots_->currentRow());
            refreshSnapshots();
        }
    });

    auto* routingPage = new QWidget(pages_);
    auto* routingLayout = new QVBoxLayout(routingPage);
    routingTitle_->setParent(routingPage);
    routingTitle_->setProperty("class", QStringLiteral("inspectorTitle"));
    routingLayout->addWidget(routingTitle_);
    advancedRoutingTitle_->setParent(routingPage);
    advancedRoutingTitle_->setProperty("class", QStringLiteral("navigationTitle"));
    routingLayout->addWidget(advancedRoutingTitle_);
    auto* advancedRouting = new QWidget(routingPage);
    auto* advancedGrid = new QGridLayout(advancedRouting);
    auto* usbCard = new QWidget(advancedRouting);
    usbCard->setProperty("class", QStringLiteral("detailCard"));
    auto* usbLayout = new QVBoxLayout(usbCard);
    auto* usbSection = new QLabel(usbCard);
    usbSection->setObjectName(QStringLiteral("usbRoutingSection"));
    usbSection->setProperty("class", QStringLiteral("sectionLabel"));
    usbLayout->addWidget(usbSection);
    auto* usbModeRow = new QHBoxLayout;
    auto* usbModeLabel = new QLabel(usbCard);
    usbModeLabel->setObjectName(QStringLiteral("usbModeLabel"));
    usbModeRow->addWidget(usbModeLabel);
    usbModeRow->addWidget(usbMode_, 1);
    usbLayout->addLayout(usbModeRow);
    auto* usb56Label = new QLabel(usbCard);
    usb56Label->setObjectName(QStringLiteral("usbInput56Label"));
    usbInput56_->setObjectName(QStringLiteral("usbInput56Assignment"));
    usbLayout->addWidget(usb56Label);
    usbLayout->addWidget(usbInput56_);
    auto* usb78Label = new QLabel(usbCard);
    usb78Label->setObjectName(QStringLiteral("usbInput78Label"));
    usbInput78_->setObjectName(QStringLiteral("usbInput78Assignment"));
    usbLayout->addWidget(usb78Label);
    usbLayout->addWidget(usbInput78_);
    for (int output = 0; output < 2; ++output) {
        auto* label = new QLabel(usbCard);
        label->setObjectName(QStringLiteral("usbMonitorOutputLabel%1").arg(output));
        auto* feed = new QComboBox(usbCard);
        feed->setObjectName(QStringLiteral("usbMonitorOutputFeed%1").arg(output));
        monitorOutputFeeds_.append(feed);
        usbLayout->addWidget(label);
        usbLayout->addWidget(feed);
        connect(feed, &QComboBox::currentIndexChanged, this,
                [this, output, feed](const int index) {
                    if (index >= 0) {
                        (void)device_.setPhysicalMonitorOutputFeed(
                            output, static_cast<model::PhysicalMonitorOutputFeed>(
                                feed->itemData(index).toInt()));
                    }
                });
    }

    auto* monitorCard = new QWidget(advancedRouting);
    monitorCard->setProperty("class", QStringLiteral("detailCard"));
    auto* monitorLayout = new QVBoxLayout(monitorCard);
    auto* monitorSection = new QLabel(monitorCard);
    monitorSection->setObjectName(QStringLiteral("monitorRoutingSection"));
    monitorSection->setProperty("class", QStringLiteral("sectionLabel"));
    monitorLayout->addWidget(monitorSection);
    monitorStereoLink_->setObjectName(QStringLiteral("monitorStereoLink"));
    monitorLayout->addWidget(monitorStereoLink_);
    monitorLayout->addStretch();

    auto* headphoneCard = new QWidget(advancedRouting);
    headphoneCard->setProperty("class", QStringLiteral("detailCard"));
    auto* headphoneLayout = new QVBoxLayout(headphoneCard);
    auto* headphoneSection = new QLabel(headphoneCard);
    headphoneSection->setObjectName(QStringLiteral("headphoneRoutingSection"));
    headphoneSection->setProperty("class", QStringLiteral("sectionLabel"));
    headphoneLayout->addWidget(headphoneSection);
    auto* headphoneLabel = new QLabel(headphoneCard);
    headphoneLabel->setObjectName(QStringLiteral("headphoneSourceLabel"));
    headphoneLayout->addWidget(headphoneLabel);
    headphoneSource_->setObjectName(QStringLiteral("headphoneSource"));
    headphoneLayout->addWidget(headphoneSource_);
    auto* tapLabel = new QLabel(headphoneCard);
    tapLabel->setObjectName(QStringLiteral("headphoneTapLabel"));
    headphoneLayout->addWidget(tapLabel);
    headphoneTapPoint_->setObjectName(QStringLiteral("headphoneTapPoint"));
    headphoneLayout->addWidget(headphoneTapPoint_);
    bluetoothUsbPhonesOnly_->setObjectName(QStringLiteral("bluetoothUsbPhonesOnly"));
    headphoneLayout->addWidget(bluetoothUsbPhonesOnly_);
    headphoneLayout->addStretch();

    auto* fxCard = new QWidget(advancedRouting);
    fxCard->setProperty("class", QStringLiteral("detailCard"));
    auto* fxRoutingLayout = new QGridLayout(fxCard);
    auto* fxSection = new QLabel(fxCard);
    fxSection->setObjectName(QStringLiteral("fxRoutingSection"));
    fxSection->setProperty("class", QStringLiteral("sectionLabel"));
    fxRoutingLayout->addWidget(fxSection, 0, 0, 1, 3);
    for (int effect = 0; effect < 2; ++effect) {
        for (int destination = 0; destination < 3; ++destination) {
            const int route = effect * 3 + destination;
            auto* check = new QCheckBox(fxCard);
            check->setObjectName(QStringLiteral("fxOutputRoute%1").arg(route));
            fxOutputChecks_.append(check);
            fxRoutingLayout->addWidget(check, 1 + effect, destination);
            connect(check, &QCheckBox::toggled, this,
                    [this, effect, destination](const bool enabled) {
                        (void)device_.setFxOutputRouteEnabled(
                            effect, static_cast<model::FxOutputDestination>(destination),
                            enabled);
                    });
        }
    }

    auto* outputCard = new QWidget(advancedRouting);
    outputCard->setProperty("class", QStringLiteral("detailCard"));
    auto* outputLayout = new QVBoxLayout(outputCard);
    auto* outputSection = new QLabel(outputCard);
    outputSection->setObjectName(QStringLiteral("outputRoutingSection"));
    outputSection->setProperty("class", QStringLiteral("sectionLabel"));
    outputLayout->addWidget(outputSection);
    constexpr std::array outputIds {
        model::PhysicalOutputId::MainOut,
        model::PhysicalOutputId::MonitorOut1,
        model::PhysicalOutputId::MonitorOut2,
    };
    for (int index = 0; index < static_cast<int>(outputIds.size()); ++index) {
        auto* check = new QCheckBox(outputCard);
        check->setObjectName(QStringLiteral("outputPadMinus10Dbv%1").arg(index));
        outputPadChecks_.append(check);
        outputLayout->addWidget(check);
        connect(check, &QCheckBox::toggled, this,
                [this, output = outputIds[static_cast<std::size_t>(index)]](
                    const bool enabled) {
                    (void)device_.setOutputPadMinus10Dbv(output, enabled);
                });
    }
    outputLayout->addStretch();

    advancedGrid->addWidget(usbCard, 0, 0);
    advancedGrid->addWidget(monitorCard, 0, 1);
    advancedGrid->addWidget(headphoneCard, 1, 0);
    advancedGrid->addWidget(fxCard, 1, 1);
    advancedGrid->addWidget(outputCard, 2, 0, 1, 2);
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
    connect(usbInput56_, &QComboBox::currentIndexChanged, this, [this](const int index) {
        if (index >= 0) {
            (void)device_.setUsbInputAssignment(
                0, static_cast<model::UsbPlaybackAssignment>(
                    usbInput56_->itemData(index).toInt()));
        }
    });
    connect(usbInput78_, &QComboBox::currentIndexChanged, this, [this](const int index) {
        if (index >= 0) {
            (void)device_.setUsbInputAssignment(
                1, static_cast<model::UsbPlaybackAssignment>(
                    usbInput78_->itemData(index).toInt()));
        }
    });
    connect(headphoneSource_, &QComboBox::currentIndexChanged, this, [this](const int index) {
        if (index >= 0) {
            (void)device_.setHeadphoneSource(
                static_cast<model::HeadphoneSource>(headphoneSource_->itemData(index).toInt()));
        }
    });
    connect(headphoneTapPoint_, &QComboBox::currentIndexChanged, this,
            [this](const int index) {
                if (index >= 0) {
                    (void)device_.setHeadphoneTapPoint(
                        static_cast<model::RoutingTapPoint>(
                            headphoneTapPoint_->itemData(index).toInt()));
                }
            });
    connect(bluetoothUsbPhonesOnly_, &QCheckBox::toggled, this,
            [this](const bool enabled) {
                (void)device_.setBluetoothUsbPhonesOnly(enabled);
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
    connect(&device_.state(), &Flow8State::routingChanged, this, [this] {
        refreshRouting();
        refreshFx();
    });
    connect(&device_.state(), &Flow8State::outputMeterChanged, this,
            [this](const model::RoutingDestination destination) {
                if (model::busIndexForDestination(destination) == selectedBus_) {
                    refreshBusMeter();
                }
                if (static_cast<int>(destination)
                    == static_cast<int>(model::RoutingDestination::Fx1) + selectedFx_) {
                    refreshFxMeter();
                }
            });
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
        if (device_.state().monitorLink().stereoLinked.value.value_or(false)) {
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
    const QSignalBlocker muteBlocker(busMute_);
    const QSignalBlocker balanceBlocker(busBalance_);
    const QSignalBlocker limiterBlocker(busLimiter_);
    busLevel_->setValue(static_cast<int>(std::lround(bus->fader.value.value_or(0.0) * 1000.0)));
    refreshBusMeter();
    busMute_->setVisible(bus->muted.has_value());
    busMute_->setChecked(bus->muted.has_value()
        && bus->muted->value.value_or(false));
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
        busEqLabels_[band]->setVisible(hasEq);
        busEqLabels_[band]->setText(QStringLiteral("%1\n%2")
            .arg(frequencyValueText(model::busEqFrequenciesHz[index]),
                 decibelValueText(gain)));
    }
    busEqGraph_->setBands(std::move(graphBands));
}

void DetailPanel::refreshBusMeter()
{
    const auto* meter = device_.state().outputMeter(
        static_cast<model::RoutingDestination>(selectedBus_));
    busMeter_->setLevel(meter == nullptr ? 0.0 : meter->level.value.value_or(0.0));
    busMeter_->setPeak(meter == nullptr ? 0.0 : meter->peak.value.value_or(0.0));
    busMeter_->setClipping(meter != nullptr && meter->clipping.value.value_or(false));
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
    refreshFxMeter();
    {
        const QSignalBlocker blocker(fxMaster_);
        fxMaster_->setValue(static_cast<int>(std::lround(
            effect.master.value.value_or(0.0) * 1000.0)));
    }
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
    for (int destination = 0; destination < fxReturnChecks_.size(); ++destination) {
        const auto* route = device_.state().routing().fxOutputRoute(
            selectedFx_, static_cast<model::FxOutputDestination>(destination));
        const QSignalBlocker blocker(fxReturnChecks_[destination]);
        fxReturnChecks_[destination]->setChecked(
            route != nullptr && route->enabled.value.value_or(false));
    }
    fxTempo_->setText(QStringLiteral("%1 BPM").arg(
        QLocale().toString(effect.tapTempoBpm.value.value_or(120.0), 'f', 1)));
}

void DetailPanel::refreshFxMeter()
{
    const auto* meter = device_.state().outputMeter(
        selectedFx_ == 0 ? model::RoutingDestination::Fx1
                         : model::RoutingDestination::Fx2);
    fxMeter_->setLevel(meter == nullptr ? 0.0 : meter->level.value.value_or(0.0));
    fxMeter_->setPeak(meter == nullptr ? 0.0 : meter->peak.value.value_or(0.0));
    fxMeter_->setClipping(meter != nullptr && meter->clipping.value.value_or(false));
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
    const auto& routing = device_.state().routing();
    {
        const QSignalBlocker blocker(usbMode_);
        usbMode_->setCurrentIndex(usbMode_->findData(static_cast<int>(
            routing.usbAudio.mode.value.value_or(model::UsbMode::Streaming))));
    }
    {
        const QSignalBlocker blocker(usbInput56_);
        usbInput56_->setCurrentIndex(usbInput56_->findData(static_cast<int>(
            routing.usbAudio.input56Assignment.value.value_or(
                model::UsbPlaybackAssignment::AnalogInput))));
    }
    {
        const QSignalBlocker blocker(usbInput78_);
        usbInput78_->setCurrentIndex(usbInput78_->findData(static_cast<int>(
            routing.usbAudio.input78Assignment.value.value_or(
                model::UsbPlaybackAssignment::AnalogInput))));
    }
    for (int output = 0; output < monitorOutputFeeds_.size(); ++output) {
        const QSignalBlocker blocker(monitorOutputFeeds_[output]);
        monitorOutputFeeds_[output]->setCurrentIndex(
            monitorOutputFeeds_[output]->findData(static_cast<int>(
                routing.usbAudio.monitorOutputFeeds[static_cast<std::size_t>(output)]
                    .value.value_or(
                        model::PhysicalMonitorOutputFeed::NominalMonitorMix))));
    }
    for (int effect = 0; effect < 2; ++effect) {
        for (int destination = 0; destination < 3; ++destination) {
            const int index = effect * 3 + destination;
            const auto* route = routing.fxOutputRoute(
                effect, static_cast<model::FxOutputDestination>(destination));
            const QSignalBlocker blocker(fxOutputChecks_[index]);
            fxOutputChecks_[index]->setChecked(
                route != nullptr && route->enabled.value.value_or(false));
        }
    }
    {
        const QSignalBlocker blocker(headphoneSource_);
        headphoneSource_->setCurrentIndex(headphoneSource_->findData(static_cast<int>(
            routing.headphones.source.value.value_or(model::HeadphoneSource::Main))));
    }
    {
        const QSignalBlocker blocker(headphoneTapPoint_);
        headphoneTapPoint_->setCurrentIndex(
            headphoneTapPoint_->findData(static_cast<int>(
                routing.headphones.tapPoint.value.value_or(
                    model::RoutingTapPoint::PostFader))));
    }
    {
        const QSignalBlocker blocker(bluetoothUsbPhonesOnly_);
        bluetoothUsbPhonesOnly_->setChecked(
            routing.headphones.bluetoothUsbPhonesOnly.value.value_or(false));
    }
    {
        const QSignalBlocker blocker(monitorStereoLink_);
        monitorStereoLink_->setChecked(
            device_.state().monitorLink().stereoLinked.value.value_or(false));
    }
    constexpr std::array outputIds {
        model::PhysicalOutputId::MainOut,
        model::PhysicalOutputId::MonitorOut1,
        model::PhysicalOutputId::MonitorOut2,
    };
    for (int index = 0; index < outputPadChecks_.size(); ++index) {
        const auto* output = device_.state().physicalOutput(
            outputIds[static_cast<std::size_t>(index)]);
        const QSignalBlocker blocker(outputPadChecks_[index]);
        outputPadChecks_[index]->setChecked(
            output != nullptr && output->padMinus10Dbv.has_value()
            && output->padMinus10Dbv->value.value_or(false));
    }
}

void DetailPanel::retranslateUi()
{
    busLevelLabel_->setText(uiText("Level"));
    busMute_->setText(uiText("Mute"));
    busBalanceLabel_->setText(uiText("Balance"));
    busLimiterLabel_->setText(uiText("Limiter"));

    const QSignalBlocker presetBlocker(fxPreset_);
    fxPreset_->clear();
    for (int preset = 1; preset <= 16; ++preset) {
        fxPreset_->addItem(uiText("Preset %1").arg(preset), preset);
    }
    if (fxFormLabels_.size() >= 7) {
        fxFormLabels_[0]->setText(uiText("Master"));
        fxFormLabels_[1]->setText(uiText("Preset"));
        fxFormLabels_[2]->setText(uiText("Effect Type"));
        fxFormLabels_[3]->setText(uiText("Parameter %1 (UNKNOWN)").arg(1));
        fxFormLabels_[4]->setText(uiText("Parameter %1 (UNKNOWN)").arg(2));
        fxFormLabels_[5]->setText(uiText("Engine"));
        fxFormLabels_[6]->setText(uiText("Output Routing"));
    }
    fxMute_->setText(uiText("Mute"));
    fxTap_->setText(uiText("Tap Tempo"));
    const std::array<const char*, 3> returnLabels {
        "Main", "Monitor 1", "Monitor 2",
    };
    for (int index = 0; index < fxReturnChecks_.size(); ++index) {
        fxReturnChecks_[index]->setText(
            uiText(returnLabels[static_cast<std::size_t>(index)]));
    }
    fxInfo_->setText(uiText(
        "Tap tempo is global in the official MIDI chart and applies only to compatible effects.\n"
        "Effect-specific parameter names remain unavailable."));

    hardwareTitle_->setText(uiText("Hardware Slots · 15"));
    recallButton_->setText(uiText("Recall in Simulator"));
    hardwareStore_->setText(uiText("Store"));
    hardwareRename_->setText(uiText("Rename"));
    hardwareDelete_->setText(uiText("Delete"));
    hardwareReset_->setText(uiText("Reset"));
    for (auto* unavailable : {hardwareStore_, hardwareRename_, hardwareDelete_, hardwareReset_}) {
        unavailable->setToolTip(uiText("Needs Hardware Verification"));
    }
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
    deleteAppSnapshot_->setText(uiText("Delete"));
    shareSnapshot_->setText(uiText("Share / Export"));
    shareSnapshot_->setToolTip(uiText("Snapshot export is not implemented yet."));
    routingTitle_->setText(uiText("Routing · Signal Paths and Physical Outputs"));
    advancedRoutingTitle_->setText(
        uiText("USB, Monitor, Headphones, FX Returns and Outputs"));
    findChild<QLabel*>(QStringLiteral("usbModeLabel"))->setText(uiText("USB Mode"));
    findChild<QLabel*>(QStringLiteral("usbInput56Label"))->setText(
        uiText("Input 5/6 Playback Source"));
    findChild<QLabel*>(QStringLiteral("usbInput78Label"))->setText(
        uiText("Input 7/8 Playback Source"));
    findChild<QLabel*>(QStringLiteral("headphoneSourceLabel"))->setText(
        uiText("Headphone Source"));
    findChild<QLabel*>(QStringLiteral("headphoneTapLabel"))->setText(
        uiText("Headphone Tap Point"));
    findChild<QLabel*>(QStringLiteral("usbRoutingSection"))->setText(
        uiText("USB Audio / Loopback"));
    findChild<QLabel*>(QStringLiteral("monitorRoutingSection"))->setText(
        uiText("MON1 / MON2 Mix Link"));
    findChild<QLabel*>(QStringLiteral("headphoneRoutingSection"))->setText(
        uiText("Headphones"));
    findChild<QLabel*>(QStringLiteral("fxRoutingSection"))->setText(uiText("FX Returns"));
    findChild<QLabel*>(QStringLiteral("outputRoutingSection"))->setText(
        uiText("Physical Output Settings"));
    for (int output = 0; output < 2; ++output) {
        findChild<QLabel*>(QStringLiteral("usbMonitorOutputLabel%1").arg(output))->setText(
            uiText("Monitor OUT %1 Hardware Feed").arg(output + 1));
    }
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
    const auto populateUsbAssignment = [this](QComboBox* combo,
                                               const QString& analogLabel,
                                               const QString& usbLabel) {
        const int selected = combo->currentData().toInt();
        const QSignalBlocker blocker(combo);
        combo->clear();
        combo->addItem(analogLabel,
                       static_cast<int>(model::UsbPlaybackAssignment::AnalogInput));
        combo->addItem(usbLabel,
                       static_cast<int>(model::UsbPlaybackAssignment::UsbAudioLoopback));
        combo->setCurrentIndex(qMax(0, combo->findData(selected)));
    };
    populateUsbAssignment(usbInput56_, uiText("Analog Input 5/6"),
                          uiText("USB 1/2"));
    populateUsbAssignment(usbInput78_, uiText("Analog Input 7/8"),
                          uiText("USB 3/4"));
    for (int output = 0; output < monitorOutputFeeds_.size(); ++output) {
        auto* combo = monitorOutputFeeds_[output];
        const int selected = combo->currentData().toInt();
        const QSignalBlocker blocker(combo);
        combo->clear();
        combo->addItem(
            output == 0 ? uiText("MON1 Mix (Default)") : uiText("MON2 Mix (Default)"),
            static_cast<int>(model::PhysicalMonitorOutputFeed::NominalMonitorMix));
        combo->addItem(uiText("USB 1/2"),
                       static_cast<int>(model::PhysicalMonitorOutputFeed::Usb12));
        combo->addItem(uiText("USB 3/4"),
                       static_cast<int>(model::PhysicalMonitorOutputFeed::Usb34));
        combo->setCurrentIndex(qMax(0, combo->findData(selected)));
    }
    const std::array<const char*, 6> fxRoutes {
        "FX 1 → Main", "FX 1 → Monitor 1", "FX 1 → Monitor 2",
        "FX 2 → Main", "FX 2 → Monitor 1", "FX 2 → Monitor 2",
    };
    for (int index = 0; index < fxOutputChecks_.size(); ++index) {
        fxOutputChecks_[index]->setText(uiText(fxRoutes[static_cast<std::size_t>(index)]));
    }
    const int selectedHeadphone = headphoneSource_->currentData().toInt();
    {
        const QSignalBlocker blocker(headphoneSource_);
        headphoneSource_->clear();
        headphoneSource_->addItem(uiText("Main"),
            static_cast<int>(model::HeadphoneSource::Main));
        headphoneSource_->addItem(uiText("Monitor"),
            static_cast<int>(model::HeadphoneSource::Monitor));
        headphoneSource_->setCurrentIndex(
            qMax(0, headphoneSource_->findData(selectedHeadphone)));
    }
    const int selectedTapPoint = headphoneTapPoint_->currentData().toInt();
    {
        const QSignalBlocker blocker(headphoneTapPoint_);
        headphoneTapPoint_->clear();
        headphoneTapPoint_->addItem(uiText("Pre-Fader"),
            static_cast<int>(model::RoutingTapPoint::PreFader));
        headphoneTapPoint_->addItem(uiText("Post-Fader"),
            static_cast<int>(model::RoutingTapPoint::PostFader));
        headphoneTapPoint_->setCurrentIndex(
            qMax(0, headphoneTapPoint_->findData(selectedTapPoint)));
    }
    bluetoothUsbPhonesOnly_->setText(uiText("Bluetooth / USB to Headphones Only"));
    monitorStereoLink_->setText(uiText("Stereo Link: MON1 ↔ MON2"));
    const std::array<const char*, 3> outputPadLabels {
        "MAIN OUT -10 dBV", "MON OUT 1 -10 dBV", "MON OUT 2 -10 dBV",
    };
    for (int index = 0; index < outputPadChecks_.size(); ++index) {
        outputPadChecks_[index]->setText(
            uiText(outputPadLabels[static_cast<std::size_t>(index)]));
    }
    routingEvidence_->setText(uiText(
        "Official capability · Simulator only · BLE routing protocol UNKNOWN"));

    refresh();
}

} // namespace flow8::ui
