#include "ui/settings/settings_dialog.h"

#include "core/flow8_device.h"
#include "model/flow8_capabilities.h"
#include "ui/language_manager.h"
#include "ui/ui_text.h"

#include <QCheckBox>
#include <QComboBox>
#include <QDialogButtonBox>
#include <QEvent>
#include <QFormLayout>
#include <QLabel>
#include <QPushButton>
#include <QSignalBlocker>
#include <QTabWidget>
#include <QVBoxLayout>

namespace flow8::ui {
namespace {

QWidget* settingsPage(QTabWidget* tabs)
{
    auto* page = new QWidget(tabs);
    auto* layout = new QVBoxLayout(page);
    layout->setContentsMargins(16, 14, 16, 14);
    return page;
}

} // namespace

SettingsDialog::SettingsDialog(Flow8Device& device, LanguageManager& languageManager,
                               QWidget* parent)
    : QDialog(parent)
    , device_(device)
    , languageManager_(languageManager)
    , title_(new QLabel(this))
    , tabs_(new QTabWidget(this))
    , connectionName_(new QLabel(this))
    , connectionValue_(new QLabel(this))
    , transportName_(new QLabel(this))
    , transportValue_(new QLabel(this))
    , simulatorName_(new QLabel(this))
    , simulatorValue_(new QLabel(this))
    , midiName_(new QLabel(this))
    , midiValue_(new QLabel(this))
    , languageName_(new QLabel(this))
    , languageSelector_(new QComboBox(this))
    , note_(new QLabel(this))
    , showMuteButtons_(new QCheckBox(this))
    , showChannelIcons_(new QCheckBox(this))
    , controlGesture_(new QComboBox(this))
    , eqEditingMode_(new QComboBox(this))
    , outputDelayIndicator_(new QCheckBox(this))
    , footswitchMode_(new QComboBox(this))
    , sysexDump_(new QPushButton(this))
    , diagnosticsNote_(new QLabel(this))
    , buttons_(new QDialogButtonBox(QDialogButtonBox::Close, this))
{
    setObjectName(QStringLiteral("settingsDialog"));
    setMinimumSize(680, 560);
    title_->setProperty("class", QStringLiteral("inspectorTitle"));
    languageSelector_->setObjectName(QStringLiteral("languageSelector"));
    languageSelector_->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    note_->setWordWrap(true);
    note_->setProperty("class", QStringLiteral("secondaryText"));
    diagnosticsNote_->setWordWrap(true);
    diagnosticsNote_->setProperty("class", QStringLiteral("secondaryText"));
    sysexDump_->setObjectName(QStringLiteral("requestSysExDump"));
    sysexDump_->setEnabled(false);

    auto* connectionPage = settingsPage(tabs_);
    auto* connectionForm = new QFormLayout;
    connectionForm->setFieldGrowthPolicy(QFormLayout::AllNonFixedFieldsGrow);
    connectionForm->addRow(connectionName_, connectionValue_);
    connectionForm->addRow(transportName_, transportValue_);
    connectionForm->addRow(simulatorName_, simulatorValue_);
    connectionForm->addRow(midiName_, midiValue_);
    connectionForm->addRow(languageName_, languageSelector_);
    static_cast<QVBoxLayout*>(connectionPage->layout())->addLayout(connectionForm);
    static_cast<QVBoxLayout*>(connectionPage->layout())->addWidget(note_);
    static_cast<QVBoxLayout*>(connectionPage->layout())->addStretch();

    auto* mixerPage = settingsPage(tabs_);
    showMuteButtons_->setObjectName(QStringLiteral("showMuteButtons"));
    showChannelIcons_->setObjectName(QStringLiteral("showChannelIcons"));
    static_cast<QVBoxLayout*>(mixerPage->layout())->addWidget(showMuteButtons_);
    static_cast<QVBoxLayout*>(mixerPage->layout())->addWidget(showChannelIcons_);
    for (int index = 0; index < model::conventionalMixerInputCount; ++index) {
        auto* visible = new QCheckBox(mixerPage);
        visible->setObjectName(QStringLiteral("channelVisible%1").arg(index));
        channelVisibility_.append(visible);
        static_cast<QVBoxLayout*>(mixerPage->layout())->addWidget(visible);
        connect(visible, &QCheckBox::toggled, this,
                [this, index](const bool checked) {
                    (void)device_.setChannelVisible(index, checked);
                });
    }
    static_cast<QVBoxLayout*>(mixerPage->layout())->addStretch();

    auto* controlPage = settingsPage(tabs_);
    auto* controlForm = new QFormLayout;
    auto* gestureLabel = new QLabel(controlPage);
    gestureLabel->setObjectName(QStringLiteral("gestureLabel"));
    auto* footswitchLabel = new QLabel(controlPage);
    footswitchLabel->setObjectName(QStringLiteral("footswitchLabel"));
    controlForm->addRow(gestureLabel, controlGesture_);
    controlForm->addRow(footswitchLabel, footswitchMode_);
    static_cast<QVBoxLayout*>(controlPage->layout())->addLayout(controlForm);
    static_cast<QVBoxLayout*>(controlPage->layout())->addStretch();

    auto* eqPage = settingsPage(tabs_);
    auto* eqForm = new QFormLayout;
    auto* eqModeLabel = new QLabel(eqPage);
    eqModeLabel->setObjectName(QStringLiteral("eqModeLabel"));
    eqForm->addRow(eqModeLabel, eqEditingMode_);
    static_cast<QVBoxLayout*>(eqPage->layout())->addLayout(eqForm);
    auto* eqNote = new QLabel(eqPage);
    eqNote->setObjectName(QStringLiteral("eqPreferenceNote"));
    eqNote->setWordWrap(true);
    eqNote->setProperty("class", QStringLiteral("secondaryText"));
    static_cast<QVBoxLayout*>(eqPage->layout())->addWidget(eqNote);
    static_cast<QVBoxLayout*>(eqPage->layout())->addStretch();

    auto* outputPage = settingsPage(tabs_);
    outputDelayIndicator_->setObjectName(QStringLiteral("outputDelayIndicator"));
    static_cast<QVBoxLayout*>(outputPage->layout())->addWidget(outputDelayIndicator_);
    auto* outputNote = new QLabel(outputPage);
    outputNote->setObjectName(QStringLiteral("outputPreferenceNote"));
    outputNote->setWordWrap(true);
    outputNote->setProperty("class", QStringLiteral("secondaryText"));
    static_cast<QVBoxLayout*>(outputPage->layout())->addWidget(outputNote);
    static_cast<QVBoxLayout*>(outputPage->layout())->addStretch();

    auto* diagnosticsPage = settingsPage(tabs_);
    static_cast<QVBoxLayout*>(diagnosticsPage->layout())->addWidget(sysexDump_);
    static_cast<QVBoxLayout*>(diagnosticsPage->layout())->addWidget(diagnosticsNote_);
    static_cast<QVBoxLayout*>(diagnosticsPage->layout())->addStretch();

    tabs_->addTab(connectionPage, {});
    tabs_->addTab(mixerPage, {});
    tabs_->addTab(controlPage, {});
    tabs_->addTab(eqPage, {});
    tabs_->addTab(outputPage, {});
    tabs_->addTab(diagnosticsPage, {});

    auto* layout = new QVBoxLayout(this);
    layout->addWidget(title_);
    layout->addWidget(tabs_, 1);
    layout->addWidget(buttons_);

    connect(languageSelector_, &QComboBox::currentIndexChanged, this, [this](const int index) {
        if (index < 0) return;
        const auto language = static_cast<UiLanguage>(languageSelector_->itemData(index).toInt());
        (void)languageManager_.setLanguage(language);
    });
    for (auto* check : {showMuteButtons_, showChannelIcons_, outputDelayIndicator_}) {
        connect(check, &QCheckBox::toggled, this, &SettingsDialog::commitPreferences);
    }
    connect(controlGesture_, &QComboBox::currentIndexChanged,
            this, &SettingsDialog::commitPreferences);
    connect(eqEditingMode_, &QComboBox::currentIndexChanged,
            this, &SettingsDialog::commitPreferences);
    connect(footswitchMode_, &QComboBox::currentIndexChanged,
            this, &SettingsDialog::commitPreferences);
    connect(buttons_, &QDialogButtonBox::rejected, this, &QDialog::reject);

    const auto& preferences = device_.state().preferences();
    showMuteButtons_->setChecked(preferences.showMuteButtons);
    showChannelIcons_->setChecked(preferences.showChannelIcons);
    outputDelayIndicator_->setChecked(preferences.showOutputDelayIndicator);
    retranslateUi();
}

void SettingsDialog::changeEvent(QEvent* event)
{
    QDialog::changeEvent(event);
    if (event->type() == QEvent::LanguageChange) {
        retranslateUi();
    }
}

void SettingsDialog::commitPreferences()
{
    if (controlGesture_->currentIndex() < 0 || eqEditingMode_->currentIndex() < 0
        || footswitchMode_->currentIndex() < 0) {
        return;
    }
    auto preferences = device_.state().preferences();
    preferences.showMuteButtons = showMuteButtons_->isChecked();
    preferences.showChannelIcons = showChannelIcons_->isChecked();
    preferences.showOutputDelayIndicator = outputDelayIndicator_->isChecked();
    preferences.controlGesture = static_cast<model::ControlGesture>(
        controlGesture_->currentData().toInt());
    preferences.eqEditingMode = static_cast<model::EqEditingMode>(
        eqEditingMode_->currentData().toInt());
    preferences.footswitchMode = static_cast<model::FootswitchMode>(
        footswitchMode_->currentData().toInt());
    device_.setPreferences(preferences);
}

void SettingsDialog::retranslateUi()
{
    setWindowTitle(uiText("FLOW 8 Preferences"));
    title_->setText(uiText("Preferences"));
    tabs_->setTabText(0, uiText("Connection"));
    tabs_->setTabText(1, uiText("Mixer"));
    tabs_->setTabText(2, uiText("Control"));
    tabs_->setTabText(3, uiText("Equalizer"));
    tabs_->setTabText(4, uiText("Output"));
    tabs_->setTabText(5, uiText("Diagnostics"));
    connectionName_->setText(uiText("Connection"));
    connectionValue_->setText(uiText("Simulator connected locally"));
    transportName_->setText(uiText("Transport"));
    transportValue_->setText(uiText("Simulator / BLE when available"));
    simulatorName_->setText(uiText("Simulator"));
    simulatorValue_->setText(uiText("SYNTHETIC data, deterministic state"));
    midiName_->setText(QStringLiteral("MIDI"));
    midiValue_->setText(uiText("Official mapping model ready; transport not implemented"));
    languageName_->setText(uiText("Language"));

    const QSignalBlocker languageBlocker(languageSelector_);
    languageSelector_->clear();
    languageSelector_->addItem(QStringLiteral("中文"),
                                static_cast<int>(UiLanguage::SimplifiedChinese));
    languageSelector_->addItem(QStringLiteral("English"),
                                static_cast<int>(UiLanguage::English));
    languageSelector_->setCurrentIndex(languageSelector_->findData(
        static_cast<int>(languageManager_.language())));

    showMuteButtons_->setText(uiText("Show Mute Buttons"));
    showChannelIcons_->setText(uiText("Show Channel Icons"));
    for (int index = 0; index < channelVisibility_.size(); ++index) {
        channelVisibility_[index]->setText(uiText("Show %1").arg(
            inputDisplayName(static_cast<model::InputId>(index))));
        const auto* channel = device_.state().channel(index);
        const QSignalBlocker blocker(channelVisibility_[index]);
        channelVisibility_[index]->setChecked(
            channel == nullptr || channel->visible.value.value_or(true));
        channelVisibility_[index]->setEnabled(channel != nullptr);
    }

    findChild<QLabel*>(QStringLiteral("gestureLabel"))->setText(uiText("Control Gesture"));
    const QSignalBlocker gestureBlocker(controlGesture_);
    const int gesture = static_cast<int>(device_.state().preferences().controlGesture);
    controlGesture_->clear();
    controlGesture_->addItem(uiText("Linear"), static_cast<int>(model::ControlGesture::Linear));
    controlGesture_->addItem(uiText("Rotary"), static_cast<int>(model::ControlGesture::Rotary));
    controlGesture_->setCurrentIndex(controlGesture_->findData(gesture));

    findChild<QLabel*>(QStringLiteral("footswitchLabel"))->setText(uiText("Footswitch Mode"));
    const QSignalBlocker footswitchBlocker(footswitchMode_);
    const int footswitch = static_cast<int>(device_.state().preferences().footswitchMode);
    footswitchMode_->clear();
    footswitchMode_->addItem(uiText("FX"), static_cast<int>(model::FootswitchMode::Fx));
    footswitchMode_->addItem(uiText("Snapshot"),
                             static_cast<int>(model::FootswitchMode::Snapshot));
    footswitchMode_->setCurrentIndex(footswitchMode_->findData(footswitch));

    findChild<QLabel*>(QStringLiteral("eqModeLabel"))->setText(uiText("EQ Editing Mode"));
    const QSignalBlocker eqBlocker(eqEditingMode_);
    const int eqMode = static_cast<int>(device_.state().preferences().eqEditingMode);
    eqEditingMode_->clear();
    eqEditingMode_->addItem(uiText("Standard"),
                            static_cast<int>(model::EqEditingMode::Standard));
    eqEditingMode_->addItem(uiText("Parametric"),
                            static_cast<int>(model::EqEditingMode::Parametric));
    eqEditingMode_->setCurrentIndex(eqEditingMode_->findData(eqMode));
    findChild<QLabel*>(QStringLiteral("eqPreferenceNote"))->setText(uiText(
        "Standard / Parametric is a PC interaction preference, not a protocol claim."));

    outputDelayIndicator_->setText(uiText("Show Output Delay Indicator"));
    findChild<QLabel*>(QStringLiteral("outputPreferenceNote"))->setText(uiText(
        "Output hardware values are unknown until a FLOW 8 is connected and verified."));
    sysexDump_->setText(uiText("Request / Save MIDI SysEx Dump"));
    diagnosticsNote_->setText(uiText(
        "MIDI SysEx dump is an official capability. Requesting a real dump needs hardware."));
    note_->setText(uiText("Hardware BLE and MIDI controls are not available in this build."));
    buttons_->button(QDialogButtonBox::Close)->setText(uiText("Close"));
}

} // namespace flow8::ui
