#include "core/flow8_device.h"
#include "simulator/fake_transport.h"
#include "ui/language_manager.h"
#include "ui/channel/channel_edit_view.h"
#include "ui/inspector/detail_panel.h"
#include "ui/inspector/inspector_widget.h"
#include "ui/layers/layer_views.h"
#include "ui/main_window.h"
#include "ui/mixer/channel_strip.h"
#include "ui/mixer/mixer_widget.h"
#include "ui/setup/assisted_setup_wizard.h"
#include "ui/setup/setup_window.h"
#include "ui/settings/settings_dialog.h"
#include "ui/stage/stage_view.h"
#include "ui/widgets/eq_graph_widget.h"
#include "ui/widgets/fader_widget.h"

#include <QPushButton>
#include <QRegularExpression>
#include <QCheckBox>
#include <QComboBox>
#include <QListWidget>
#include <QSlider>
#include <QToolButton>
#include <QTest>

#include <cmath>
#include <memory>

class GuiSmokeTest final : public QObject {
    Q_OBJECT

private slots:
    void simulatorConnectsAndBuildsMixer_data();
    void simulatorConnectsAndBuildsMixer();
};

void GuiSmokeTest::simulatorConnectsAndBuildsMixer_data()
{
    QTest::addColumn<int>("language");
    QTest::newRow("en-US") << static_cast<int>(flow8::ui::UiLanguage::English);
    QTest::newRow("zh-CN") << static_cast<int>(flow8::ui::UiLanguage::SimplifiedChinese);
}

void GuiSmokeTest::simulatorConnectsAndBuildsMixer()
{
    QFETCH(int, language);
    flow8::ui::LanguageManager languageManager;
    QVERIFY(languageManager.setLanguage(static_cast<flow8::ui::UiLanguage>(language), false));
    flow8::Flow8Device device;
    auto transport = std::make_unique<flow8::simulator::FakeTransport>();
    transport->setRemoteChangesEnabled(false);
    device.setTransport(std::move(transport));
    flow8::ui::MainWindow window(device, languageManager);
    window.show();

    QVERIFY(window.findChild<QWidget*>(QStringLiteral("sessionStartView")) != nullptr);

    auto* startNew = window.findChild<QPushButton*>(QStringLiteral("startNewSession"));
    QVERIFY(startNew != nullptr);
    QTest::mouseClick(startNew, Qt::LeftButton);
    QTRY_COMPARE(device.state().connectionState(), flow8::ConnectionState::Ready);
    QCOMPARE(device.state().channels().size(), 7);
    QCOMPARE(window.findChildren<flow8::ui::ChannelStrip*>().size(), 7);

    auto* fader = window.findChild<flow8::ui::FaderWidget*>(QStringLiteral("fader0"));
    QVERIFY(fader != nullptr);
    fader->setFocus();
    QTest::keyClick(fader, Qt::Key_Home);
    for (int step = 0; step < 17; ++step) {
        QTest::keyClick(fader, Qt::Key_Down);
    }
    QTRY_VERIFY(device.state().channel(0)->fader.value.has_value());
    QTRY_VERIFY(std::abs(*device.state().channel(0)->fader.value - 0.83) <= (1.0 / 255.0));

    auto* firstStrip = window.findChild<flow8::ui::ChannelStrip*>(
        QStringLiteral("channelStrip0"));
    QVERIFY(firstStrip != nullptr);
    QTest::mouseClick(firstStrip, Qt::LeftButton);
    auto* inspector = window.findChild<flow8::ui::InspectorWidget*>(
        QStringLiteral("mixerInputInspector"));
    QVERIFY(inspector != nullptr);
    QTRY_VERIFY(inspector->isVisible());
    QCOMPARE(inspector->selectedChannel(), 0);
    QCOMPARE(inspector->selectedDestination(), flow8::model::RoutingDestination::Main);
    auto* currentSend = inspector->findChild<QSlider*>(
        QStringLiteral("currentDestinationSend"));
    QVERIFY(currentSend != nullptr);
    currentSend->setValue(310);
    QCOMPARE(device.state().routeLevel(
                 0, flow8::model::RoutingDestination::Main)->effectiveValue(), 0.31);
    const auto originalPan = device.state().channel(0)->pan.value;
    const auto originalMute = device.state().channel(0)->muted.value;

    auto* stageNavigation = window.findChild<QToolButton*>(QStringLiteral("layerStage"));
    QVERIFY(stageNavigation != nullptr);
    QTest::mouseClick(stageNavigation, Qt::LeftButton);
    auto* stage = window.findChild<flow8::ui::StageView*>();
    QVERIFY(stage != nullptr);
    QVERIFY(stage->isVisible());
    QCOMPARE(stage->destination(), flow8::model::RoutingDestination::Main);
    QVERIFY(window.findChild<flow8::ui::FaderWidget*>(QStringLiteral("stageFader0")) != nullptr);

    auto* monitorNavigation = window.findChild<QToolButton*>(QStringLiteral("layerMonitor1"));
    QVERIFY(monitorNavigation != nullptr);
    QTest::mouseClick(monitorNavigation, Qt::LeftButton);
    auto* mixer = window.findChild<flow8::ui::MixerWidget*>();
    QVERIFY(mixer != nullptr);
    auto* showMaster = mixer->findChild<QPushButton*>(
        QStringLiteral("showDestinationMaster"));
    QVERIFY(showMaster != nullptr);
    QTRY_VERIFY(mixer->isVisible());
    QCOMPARE(mixer->destination(), flow8::model::RoutingDestination::Monitor1);
    QCOMPARE(firstStrip->destination(), flow8::model::RoutingDestination::Monitor1);
    QCOMPARE(inspector->selectedDestination(), flow8::model::RoutingDestination::Monitor1);
    currentSend->setValue(410);
    QCOMPARE(device.state().routeLevel(
                 0, flow8::model::RoutingDestination::Monitor1)->effectiveValue(), 0.41);
    QCOMPARE(device.state().routeLevel(
                 0, flow8::model::RoutingDestination::Main)->effectiveValue(), 0.31);
    QCOMPARE(device.state().channel(0)->pan.value, originalPan);
    QCOMPARE(device.state().channel(0)->muted.value, originalMute);
    auto* monitor2Navigation = window.findChild<QToolButton*>(QStringLiteral("layerMonitor2"));
    QVERIFY(monitor2Navigation != nullptr);
    QTest::mouseClick(monitor2Navigation, Qt::LeftButton);
    QCOMPARE(mixer->destination(), flow8::model::RoutingDestination::Monitor2);
    QTest::mouseClick(showMaster, Qt::LeftButton);
    auto* master = mixer->findChild<flow8::ui::DetailPanel*>(
        QStringLiteral("destinationMasterPanel"));
    QVERIFY(master != nullptr);
    QTRY_VERIFY(master->isVisible());
    QVERIFY(master->findChild<QSlider*>(QStringLiteral("busLevel")) != nullptr);

    auto* mainNavigation = window.findChild<QToolButton*>(QStringLiteral("layerMain"));
    QVERIFY(mainNavigation != nullptr);
    QTest::mouseClick(mainNavigation, Qt::LeftButton);
    QCOMPARE(mixer->destination(), flow8::model::RoutingDestination::Main);
    QTRY_VERIFY(mixer->isVisible());

    auto* mainOutNavigation = window.findChild<QToolButton*>(QStringLiteral("layerMainOut"));
    QVERIFY(mainOutNavigation != nullptr);
    QTest::mouseClick(mainOutNavigation, Qt::LeftButton);
    auto* mainOut = window.findChild<flow8::ui::MainOutView*>(QStringLiteral("mainOutView"));
    QVERIFY(mainOut != nullptr);
    QTRY_VERIFY(mainOut->isVisible());
    QVERIFY(mainOut->findChild<flow8::ui::FaderWidget*>(QStringLiteral("mainOutFader")) != nullptr);

    auto* fxNavigation = window.findChild<QToolButton*>(QStringLiteral("layerFx1"));
    QVERIFY(fxNavigation != nullptr);
    QTest::mouseClick(fxNavigation, Qt::LeftButton);
    QCOMPARE(mixer->destination(), flow8::model::RoutingDestination::Fx1);
    QTest::mouseClick(showMaster, Qt::LeftButton);
    QTRY_VERIFY(master->isVisible());
    auto* tapTempo = master->findChild<QPushButton*>(QStringLiteral("fxTapTempo"));
    QVERIFY(tapTempo != nullptr);
    QTest::mouseClick(tapTempo, Qt::LeftButton);
    QVERIFY(device.state().effects().at(0).tapTempoBpm.value.has_value());
    auto* fx1MainRoute = master->findChild<QCheckBox*>(
        QStringLiteral("fxInspectorOutput0"));
    QVERIFY(fx1MainRoute != nullptr);
    fx1MainRoute->setChecked(false);
    QCOMPARE(device.state().routing().fxOutputRoute(
                 0, flow8::model::FxOutputDestination::Main)->enabled.value,
             std::optional(false));
    auto* fx2Navigation = window.findChild<QToolButton*>(QStringLiteral("layerFx2"));
    QVERIFY(fx2Navigation != nullptr);
    QTest::mouseClick(fx2Navigation, Qt::LeftButton);
    QCOMPARE(mixer->destination(), flow8::model::RoutingDestination::Fx2);
    QTRY_VERIFY(master->isVisible());

    QTest::mouseClick(stageNavigation, Qt::LeftButton);
    QTRY_VERIFY(stage->isVisible());
    QCOMPARE(stage->destination(), flow8::model::RoutingDestination::Fx2);

    auto* setupButton = window.findChild<QPushButton*>(QStringLiteral("setupButton"));
    QVERIFY(setupButton != nullptr);
    QTest::mouseClick(setupButton, Qt::LeftButton);
    auto* setup = window.findChild<flow8::ui::SetupWindow*>(QStringLiteral("setupWindow"));
    QVERIFY(setup != nullptr);
    QTRY_VERIFY(setup->isVisible());
    auto* setupNavigation = setup->findChild<QListWidget*>(QStringLiteral("setupNavigation"));
    QVERIFY(setupNavigation != nullptr);
    setupNavigation->setCurrentRow(2);
    QCoreApplication::processEvents();
    QCOMPARE(setup->findChildren<QPushButton*>(QRegularExpression(
        QStringLiteral("hardwareSnapshot\\d+"))).size(), 15);
    setupNavigation->setCurrentRow(4);
    QCoreApplication::processEvents();
    QVERIFY(setup->findChild<QCheckBox*>(QStringLiteral("usbRoute7")) != nullptr);
    QVERIFY(setup->findChild<QCheckBox*>(QStringLiteral("fxOutputRoute0")) != nullptr);
    auto* stereoLink = setup->findChild<QCheckBox*>(
        QStringLiteral("monitorStereoLink"));
    QVERIFY(stereoLink != nullptr);
    stereoLink->setChecked(true);
    QCOMPARE(device.state().routing().monitorLink.stereoLinked.value,
             std::optional(true));
    setupNavigation->setCurrentRow(3);
    QCoreApplication::processEvents();
    QVERIFY(setup->findChild<QComboBox*>(QStringLiteral("setupLanguage")) != nullptr);
    QTest::keyClick(&window, Qt::Key_Escape);
    QTRY_VERIFY(!setup->isVisible());

    flow8::ui::SettingsDialog preferences(device, languageManager, &window);
    preferences.show();
    QCoreApplication::processEvents();
    QVERIFY(preferences.findChild<QCheckBox*>(QStringLiteral("showChannelIcons")) != nullptr);
    QVERIFY(preferences.findChild<QComboBox*>(QStringLiteral("languageSelector")) != nullptr);
    preferences.close();

    flow8::ui::AssistedSetupWizard wizard(device, &window);
    wizard.show();
    auto* next = wizard.findChild<QPushButton*>(QStringLiteral("assistedNext"));
    QVERIFY(next != nullptr);
    QTest::mouseClick(next, Qt::LeftButton);
    QTest::mouseClick(next, Qt::LeftButton);
    QTest::mouseClick(next, Qt::LeftButton);
    auto* apply = wizard.findChild<QPushButton*>(QStringLiteral("applyAssistedSetup"));
    QVERIFY(apply != nullptr);
    QTest::mouseClick(apply, Qt::LeftButton);
    QVERIFY(device.state().assistedSetup().applied);
}

QTEST_MAIN(GuiSmokeTest)
#include "gui_smoke_test.moc"
