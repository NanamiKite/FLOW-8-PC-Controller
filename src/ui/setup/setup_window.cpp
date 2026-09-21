#include "ui/setup/setup_window.h"

#include "core/flow8_device.h"
#include "model/flow8_capabilities.h"
#include "ui/inspector/detail_panel.h"
#include "ui/language_manager.h"
#include "ui/ui_text.h"

#include <QCheckBox>
#include <QComboBox>
#include <QFormLayout>
#include <QFrame>
#include <QGridLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QLocale>
#include <QPushButton>
#include <QScrollArea>
#include <QSignalBlocker>
#include <QStackedWidget>
#include <QVBoxLayout>
#include <QVariant>

#include <array>

namespace flow8::ui {
namespace {

QWidget* pageWithTitle(const char* title, QLabel*& titleLabel, QWidget* parent)
{
    auto* page = new QWidget(parent);
    auto* layout = new QVBoxLayout(page);
    layout->setContentsMargins(22, 18, 22, 18);
    layout->setSpacing(12);
    titleLabel = new QLabel(page);
    titleLabel->setObjectName(QStringLiteral("setupPageTitle_%1").arg(
        QString::fromLatin1(title).remove(QLatin1Char(' '))));
    titleLabel->setProperty("class", QStringLiteral("sessionTitle"));
    layout->addWidget(titleLabel);
    return page;
}

} // namespace

SetupWindow::SetupWindow(Flow8Device& device, LanguageManager& languageManager,
                         QWidget* parent)
    : QWidget(parent)
    , device_(device)
    , languageManager_(languageManager)
    , back_(new QPushButton(this))
    , title_(new QLabel(this))
    , navigation_(new QListWidget(this))
    , pages_(new QStackedWidget(this))
    , appSnapshots_(new QListWidget(this))
    , snapshotName_(new QLineEdit(this))
    , snapshotScope_(new QComboBox(this))
    , snapshotStore_(new QPushButton(this))
    , snapshotLoad_(new QPushButton(this))
    , snapshotRename_(new QPushButton(this))
    , snapshotDelete_(new QPushButton(this))
    , showMuteButtons_(new QCheckBox(this))
    , showChannelIcons_(new QCheckBox(this))
    , controlGesture_(new QComboBox(this))
    , eqEditingMode_(new QComboBox(this))
    , outputDelayIndicator_(new QCheckBox(this))
    , outputLevel10dBV_(new QCheckBox(this))
    , footswitchMode_(new QComboBox(this))
    , language_(new QComboBox(this))
    , sysExDump_(new QPushButton(this))
    , routing_(new DetailPanel(device_, this))
{
    setObjectName(QStringLiteral("setupWindow"));
    back_->setObjectName(QStringLiteral("setupBack"));
    title_->setProperty("class", QStringLiteral("sessionTitle"));
    navigation_->setObjectName(QStringLiteral("setupNavigation"));
    navigation_->setMinimumWidth(210);
    navigation_->setMaximumWidth(270);
    pages_->setObjectName(QStringLiteral("setupPages"));
    appSnapshots_->setObjectName(QStringLiteral("setupAppSnapshots"));
    snapshotName_->setObjectName(QStringLiteral("setupSnapshotName"));
    language_->setObjectName(QStringLiteral("setupLanguage"));
    sysExDump_->setObjectName(QStringLiteral("setupSysExDump"));
    sysExDump_->setEnabled(false);

    pages_->addWidget(buildConfigureInputsPage());
    pages_->addWidget(buildSnapshotLibraryPage());
    pages_->addWidget(buildMixerSnapshotsPage());
    pages_->addWidget(buildPreferencesPage());
    auto* routingPage = new QWidget(pages_);
    auto* routingLayout = new QVBoxLayout(routingPage);
    routingLayout->setContentsMargins(0, 0, 0, 0);
    routingLayout->addWidget(routing_);
    pages_->addWidget(routingPage);
    pages_->addWidget(buildInfoPage());
    routing_->showRouting();

    auto* header = new QHBoxLayout;
    header->setContentsMargins(18, 12, 18, 10);
    header->addWidget(back_);
    header->addWidget(title_);
    header->addStretch();
    auto* content = new QHBoxLayout;
    content->setContentsMargins(14, 0, 14, 14);
    content->setSpacing(12);
    content->addWidget(navigation_);
    content->addWidget(pages_, 1);
    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->addLayout(header);
    layout->addLayout(content, 1);

    connect(back_, &QPushButton::clicked, this, &SetupWindow::backRequested);
    connect(navigation_, &QListWidget::currentRowChanged,
            pages_, &QStackedWidget::setCurrentIndex);
    connect(snapshotStore_, &QPushButton::clicked, this, [this] {
        if (snapshotScope_->currentIndex() >= 0
            && device_.storeAppSnapshot(snapshotName_->text(),
                static_cast<model::SnapshotScope>(snapshotScope_->currentData().toInt()))) {
            snapshotName_->clear();
            refreshSnapshots();
        }
    });
    connect(snapshotLoad_, &QPushButton::clicked, this, [this] {
        if (appSnapshots_->currentRow() >= 0) {
            (void)device_.loadAppSnapshot(appSnapshots_->currentRow());
        }
    });
    connect(snapshotRename_, &QPushButton::clicked, this, [this] {
        if (appSnapshots_->currentRow() >= 0 && !snapshotName_->text().trimmed().isEmpty()) {
            (void)device_.renameAppSnapshot(appSnapshots_->currentRow(), snapshotName_->text());
            snapshotName_->clear();
            refreshSnapshots();
        }
    });
    connect(snapshotDelete_, &QPushButton::clicked, this, [this] {
        if (appSnapshots_->currentRow() >= 0
            && device_.deleteAppSnapshot(appSnapshots_->currentRow())) {
            refreshSnapshots();
        }
    });
    connect(language_, &QComboBox::currentIndexChanged, this, [this](const int index) {
        if (index >= 0) {
            (void)languageManager_.setLanguage(
                static_cast<UiLanguage>(language_->itemData(index).toInt()));
        }
    });
    for (auto* check : {showMuteButtons_, showChannelIcons_, outputDelayIndicator_,
                        outputLevel10dBV_}) {
        connect(check, &QCheckBox::toggled, this, &SetupWindow::commitPreferences);
    }
    connect(controlGesture_, &QComboBox::currentIndexChanged,
            this, &SetupWindow::commitPreferences);
    connect(eqEditingMode_, &QComboBox::currentIndexChanged,
            this, &SetupWindow::commitPreferences);
    connect(footswitchMode_, &QComboBox::currentIndexChanged,
            this, &SetupWindow::commitPreferences);
    connect(&device_.state(), &Flow8State::stateReset, this, &SetupWindow::refresh);
    connect(&device_.state(), &Flow8State::snapshotChanged,
            this, [this](int) { refreshSnapshots(); });
    connect(&device_.state(), &Flow8State::channelChanged,
            this, [this](int) { refresh(); });
    retranslateUi();
    selectSection(SetupSection::ConfigureInputs);
}

QWidget* SetupWindow::buildConfigureInputsPage()
{
    QLabel* heading {};
    auto* page = pageWithTitle("ConfigureInputs", heading, pages_);
    auto* pageLayout = qobject_cast<QVBoxLayout*>(page->layout());
    auto* container = new QWidget(page);
    auto* grid = new QGridLayout(container);
    grid->setSpacing(10);
    for (int input = 0; input < model::inputStripCount; ++input) {
        auto* card = new QWidget(container);
        card->setProperty("class", QStringLiteral("inputCard"));
        card->setObjectName(QStringLiteral("inputCard%1").arg(input));
        auto* cardLayout = new QVBoxLayout(card);
        auto* name = new QLabel(card);
        name->setProperty("class", QStringLiteral("channelName"));
        auto* type = new QLabel(card);
        type->setProperty("class", QStringLiteral("inputBadge"));
        auto* visible = new QCheckBox(card);
        auto* edit = new QPushButton(card);
        edit->setObjectName(QStringLiteral("configureInput%1").arg(input));
        inputCardTitles_.append(name);
        inputCardTypes_.append(type);
        inputVisibility_.append(visible);
        inputEditButtons_.append(edit);
        cardLayout->addWidget(name);
        cardLayout->addWidget(type);
        cardLayout->addWidget(visible);
        cardLayout->addStretch();
        cardLayout->addWidget(edit);
        grid->addWidget(card, input / 3, input % 3);
        connect(visible, &QCheckBox::toggled, this,
                [this, input](const bool enabled) {
                    (void)device_.setChannelVisible(input, enabled);
                });
        connect(edit, &QPushButton::clicked, this,
                [this, input] { emit inputEditRequested(input); });
    }
    auto* scroll = new QScrollArea(page);
    scroll->setFrameShape(QFrame::NoFrame);
    scroll->setWidgetResizable(true);
    scroll->setWidget(container);
    pageLayout->addWidget(scroll, 1);
    return page;
}

QWidget* SetupWindow::buildSnapshotLibraryPage()
{
    QLabel* heading {};
    auto* page = pageWithTitle("SnapshotLibrary", heading, pages_);
    auto* layout = qobject_cast<QVBoxLayout*>(page->layout());
    auto* note = new QLabel(page);
    note->setObjectName(QStringLiteral("snapshotLibraryNote"));
    note->setProperty("class", QStringLiteral("secondaryText"));
    note->setWordWrap(true);
    layout->addWidget(note);
    layout->addWidget(appSnapshots_, 1);
    layout->addWidget(snapshotName_);
    layout->addWidget(snapshotScope_);
    auto* actions = new QHBoxLayout;
    actions->addWidget(snapshotStore_);
    actions->addWidget(snapshotLoad_);
    actions->addWidget(snapshotRename_);
    actions->addWidget(snapshotDelete_);
    layout->addLayout(actions);
    return page;
}

QWidget* SetupWindow::buildMixerSnapshotsPage()
{
    QLabel* heading {};
    auto* page = pageWithTitle("MixerSnapshots", heading, pages_);
    auto* layout = qobject_cast<QVBoxLayout*>(page->layout());
    auto* note = new QLabel(page);
    note->setObjectName(QStringLiteral("mixerSnapshotsNote"));
    note->setProperty("class", QStringLiteral("secondaryText"));
    note->setWordWrap(true);
    layout->addWidget(note);
    auto* grid = new QGridLayout;
    for (int slot = 0; slot < 15; ++slot) {
        auto* button = new QPushButton(page);
        button->setObjectName(QStringLiteral("hardwareSnapshot%1").arg(slot + 1));
        button->setMinimumHeight(60);
        hardwareSlots_.append(button);
        grid->addWidget(button, slot / 5, slot % 5);
        connect(button, &QPushButton::clicked,
                this, [this, slot] { (void)device_.recallSnapshot(slot); });
    }
    hardwareReset_ = new QPushButton(page);
    hardwareReset_->setObjectName(QStringLiteral("hardwareSnapshotReset"));
    hardwareReset_->setEnabled(false);
    grid->addWidget(hardwareReset_, 3, 0, 1, 5);
    layout->addLayout(grid);
    layout->addStretch();
    return page;
}

QWidget* SetupWindow::buildPreferencesPage()
{
    QLabel* heading {};
    auto* page = pageWithTitle("Preferences", heading, pages_);
    auto* layout = qobject_cast<QVBoxLayout*>(page->layout());
    auto* form = new QFormLayout;
    auto* gestureLabel = new QLabel(page);
    gestureLabel->setObjectName(QStringLiteral("setupGestureLabel"));
    auto* eqLabel = new QLabel(page);
    eqLabel->setObjectName(QStringLiteral("setupEqLabel"));
    auto* footswitchLabel = new QLabel(page);
    footswitchLabel->setObjectName(QStringLiteral("setupFootswitchLabel"));
    auto* languageLabel = new QLabel(page);
    languageLabel->setObjectName(QStringLiteral("setupLanguageLabel"));
    form->addRow(showMuteButtons_);
    form->addRow(showChannelIcons_);
    form->addRow(gestureLabel, controlGesture_);
    form->addRow(eqLabel, eqEditingMode_);
    form->addRow(outputDelayIndicator_);
    form->addRow(outputLevel10dBV_);
    form->addRow(footswitchLabel, footswitchMode_);
    form->addRow(languageLabel, language_);
    layout->addLayout(form);
    layout->addWidget(sysExDump_);
    auto* note = new QLabel(page);
    note->setObjectName(QStringLiteral("setupPreferencesNote"));
    note->setProperty("class", QStringLiteral("secondaryText"));
    note->setWordWrap(true);
    layout->addWidget(note);
    layout->addStretch();
    return page;
}

QWidget* SetupWindow::buildInfoPage()
{
    QLabel* heading {};
    auto* page = pageWithTitle("Info", heading, pages_);
    auto* layout = qobject_cast<QVBoxLayout*>(page->layout());
    auto* info = new QLabel(page);
    info->setObjectName(QStringLiteral("setupInfoText"));
    info->setProperty("class", QStringLiteral("secondaryText"));
    info->setWordWrap(true);
    layout->addWidget(info);
    layout->addStretch();
    return page;
}

void SetupWindow::selectSection(const SetupSection section)
{
    const int index = static_cast<int>(section);
    navigation_->setCurrentRow(index);
    pages_->setCurrentIndex(index);
    if (section == SetupSection::Routing) {
        routing_->showRouting();
        routing_->refresh();
    }
}

void SetupWindow::refresh()
{
    for (int input = 0; input < inputCardTitles_.size(); ++input) {
        const auto* channel = device_.state().channel(input);
        if (channel == nullptr) {
            continue;
        }
        inputCardTitles_[input]->setText(channel->name.value.value_or(
            inputDisplayName(channel->inputId)));
        inputCardTypes_[input]->setText(inputTypeDisplayName(channel->inputType));
        const QSignalBlocker blocker(inputVisibility_[input]);
        inputVisibility_[input]->setChecked(channel->visible.value.value_or(true));
    }
    refreshSnapshots();
    routing_->refresh();
}

void SetupWindow::refreshSnapshots()
{
    appSnapshots_->clear();
    for (const auto& snapshot : device_.state().snapshots()) {
        if (snapshot.storage != model::SnapshotStorage::AppLibrary) {
            continue;
        }
        appSnapshots_->addItem(QStringLiteral("%1 · %2")
            .arg(snapshot.name.value.value_or(uiText("Untitled Snapshot")),
                 snapshot.timestamp.value.has_value()
                    ? QLocale().toString(*snapshot.timestamp.value, QLocale::ShortFormat)
                    : uiText("Unknown")));
    }
    int hardware = 0;
    for (const auto& snapshot : device_.state().snapshots()) {
        if (snapshot.storage != model::SnapshotStorage::HardwareSlot
            || hardware >= hardwareSlots_.size()) {
            continue;
        }
        const QString name = snapshot.name.value.value_or(uiText("Empty Slot"));
        hardwareSlots_[hardware]->setText(QStringLiteral("%1\n%2")
            .arg(hardware + 1, 2, 10, QLatin1Char('0')).arg(name));
        ++hardware;
    }
}

void SetupWindow::commitPreferences()
{
    if (controlGesture_->currentIndex() < 0 || eqEditingMode_->currentIndex() < 0
        || footswitchMode_->currentIndex() < 0) {
        return;
    }
    auto preferences = device_.state().preferences();
    preferences.showMuteButtons = showMuteButtons_->isChecked();
    preferences.showChannelIcons = showChannelIcons_->isChecked();
    preferences.showOutputDelayIndicator = outputDelayIndicator_->isChecked();
    preferences.outputLevel10dBV = outputLevel10dBV_->isChecked();
    preferences.controlGesture = static_cast<model::ControlGesture>(
        controlGesture_->currentData().toInt());
    preferences.eqEditingMode = static_cast<model::EqEditingMode>(
        eqEditingMode_->currentData().toInt());
    preferences.footswitchMode = static_cast<model::FootswitchMode>(
        footswitchMode_->currentData().toInt());
    device_.setPreferences(preferences);
}

void SetupWindow::retranslateUi()
{
    title_->setText(uiText("Setup"));
    back_->setText(uiText("Back"));
    const std::array<const char*, 6> sections {
        "Configure Inputs", "Snapshot Library", "Mixer Snapshots",
        "Preferences", "Routing", "Info",
    };
    const int current = navigation_->currentRow();
    navigation_->clear();
    for (const char* section : sections) {
        navigation_->addItem(uiText(section));
    }
    navigation_->setCurrentRow(qMax(0, current));
    findChild<QLabel*>(QStringLiteral("setupPageTitle_ConfigureInputs"))->setText(
        uiText("Configure Inputs"));
    findChild<QLabel*>(QStringLiteral("setupPageTitle_SnapshotLibrary"))->setText(
        uiText("Snapshot Library"));
    findChild<QLabel*>(QStringLiteral("setupPageTitle_MixerSnapshots"))->setText(
        uiText("Mixer Snapshots"));
    findChild<QLabel*>(QStringLiteral("setupPageTitle_Preferences"))->setText(
        uiText("Preferences"));
    findChild<QLabel*>(QStringLiteral("setupPageTitle_Info"))->setText(uiText("Info"));

    for (int input = 0; input < inputEditButtons_.size(); ++input) {
        inputVisibility_[input]->setText(uiText("Visible in Mixer and Stage"));
        inputEditButtons_[input]->setText(uiText("Edit Channel"));
    }
    findChild<QLabel*>(QStringLiteral("snapshotLibraryNote"))->setText(uiText(
        "App snapshots are stored separately from the 15 hardware slots."));
    snapshotName_->setPlaceholderText(uiText("Snapshot Name"));
    const int selectedScope = snapshotScope_->currentData().toInt();
    snapshotScope_->clear();
    const std::array scopes {
        model::SnapshotScope::Full, model::SnapshotScope::Fx,
        model::SnapshotScope::Channel, model::SnapshotScope::Main,
        model::SnapshotScope::Monitor, model::SnapshotScope::Routing,
    };
    const std::array<const char*, 6> scopeNames {
        "Full", "FX", "Channel", "Main", "Monitor", "Routing",
    };
    for (int index = 0; index < static_cast<int>(scopes.size()); ++index) {
        snapshotScope_->addItem(uiText(scopeNames[static_cast<std::size_t>(index)]),
            static_cast<int>(scopes[static_cast<std::size_t>(index)]));
    }
    snapshotScope_->setCurrentIndex(qMax(0, snapshotScope_->findData(selectedScope)));
    snapshotStore_->setText(uiText("Store"));
    snapshotLoad_->setText(uiText("Load"));
    snapshotRename_->setText(uiText("Rename"));
    snapshotDelete_->setText(uiText("Delete"));
    findChild<QLabel*>(QStringLiteral("mixerSnapshotsNote"))->setText(uiText(
        "15 hardware slots · Recall is simulated; hardware command needs verification."));
    hardwareReset_->setText(uiText("Reset"));
    hardwareReset_->setToolTip(uiText("Hardware Required"));

    showMuteButtons_->setText(uiText("Show Mute Buttons"));
    showChannelIcons_->setText(uiText("Show Channel Icons"));
    outputDelayIndicator_->setText(uiText("Show Output Delay Indicator"));
    outputLevel10dBV_->setText(uiText("10 dBV Output Level"));
    findChild<QLabel*>(QStringLiteral("setupGestureLabel"))->setText(uiText("Control Gesture"));
    findChild<QLabel*>(QStringLiteral("setupEqLabel"))->setText(uiText("EQ Editing Mode"));
    findChild<QLabel*>(QStringLiteral("setupFootswitchLabel"))->setText(uiText("Footswitch Mode"));
    findChild<QLabel*>(QStringLiteral("setupLanguageLabel"))->setText(uiText("Language"));
    {
        const QSignalBlocker blocker(controlGesture_);
        const auto selected = device_.state().preferences().controlGesture;
        controlGesture_->clear();
        controlGesture_->addItem(uiText("Linear"), static_cast<int>(model::ControlGesture::Linear));
        controlGesture_->addItem(uiText("Rotary"), static_cast<int>(model::ControlGesture::Rotary));
        controlGesture_->setCurrentIndex(controlGesture_->findData(static_cast<int>(selected)));
    }
    {
        const QSignalBlocker blocker(eqEditingMode_);
        const auto selected = device_.state().preferences().eqEditingMode;
        eqEditingMode_->clear();
        eqEditingMode_->addItem(uiText("Standard"), static_cast<int>(model::EqEditingMode::Standard));
        eqEditingMode_->addItem(uiText("Parametric"), static_cast<int>(model::EqEditingMode::Parametric));
        eqEditingMode_->setCurrentIndex(eqEditingMode_->findData(static_cast<int>(selected)));
    }
    {
        const QSignalBlocker blocker(footswitchMode_);
        const auto selected = device_.state().preferences().footswitchMode;
        footswitchMode_->clear();
        footswitchMode_->addItem(uiText("FX"), static_cast<int>(model::FootswitchMode::Fx));
        footswitchMode_->addItem(uiText("Snapshot"), static_cast<int>(model::FootswitchMode::Snapshot));
        footswitchMode_->setCurrentIndex(footswitchMode_->findData(static_cast<int>(selected)));
    }
    {
        const QSignalBlocker blocker(language_);
        language_->clear();
        language_->addItem(QStringLiteral("中文"), static_cast<int>(UiLanguage::SimplifiedChinese));
        language_->addItem(QStringLiteral("English"), static_cast<int>(UiLanguage::English));
        language_->setCurrentIndex(language_->findData(static_cast<int>(languageManager_.language())));
    }
    showMuteButtons_->setChecked(device_.state().preferences().showMuteButtons);
    showChannelIcons_->setChecked(device_.state().preferences().showChannelIcons);
    outputDelayIndicator_->setChecked(device_.state().preferences().showOutputDelayIndicator);
    outputLevel10dBV_->setChecked(device_.state().preferences().outputLevel10dBV);
    sysExDump_->setText(uiText("Request / Save MIDI SysEx Dump"));
    sysExDump_->setToolTip(uiText("Hardware Required"));
    findChild<QLabel*>(QStringLiteral("setupPreferencesNote"))->setText(uiText(
        "Hardware-facing preferences and SysEx requests remain unavailable without a verified device."));
    findChild<QLabel*>(QStringLiteral("setupInfoText"))->setText(uiText(
        "FLOW 8 PC Controller\nSimulator mode uses SYNTHETIC data. BLE, GATT and device commands still need hardware verification."));
    routing_->retranslateUi();
    refresh();
}

} // namespace flow8::ui
