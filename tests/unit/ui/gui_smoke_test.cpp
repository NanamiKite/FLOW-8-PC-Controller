#include "core/flow8_device.h"
#include "simulator/fake_transport.h"
#include "ui/language_manager.h"
#include "ui/channel/channel_edit_view.h"
#include "ui/layers/layer_views.h"
#include "ui/main_window.h"
#include "ui/mixer/channel_strip.h"
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
    QTest::mouseDClick(firstStrip, Qt::LeftButton);
    auto* channelEdit = window.findChild<flow8::ui::ChannelEditView*>(
        QStringLiteral("channelEditView"));
    QVERIFY(channelEdit != nullptr);
    QTRY_VERIFY(channelEdit->isVisible());
    QCOMPARE(channelEdit->channel(), 0);
    QVERIFY(channelEdit->findChild<flow8::ui::EqGraphWidget*>() != nullptr);
    auto* phantom = channelEdit->findChild<QCheckBox*>(QStringLiteral("channelEditPhantom"));
    QVERIFY(phantom != nullptr);
    phantom->setChecked(true);
    QCOMPARE(device.state().channel(0)->phantom48V->value, std::optional(true));
    auto* channelBack = channelEdit->findChild<QPushButton*>(QStringLiteral("channelEditBack"));
    QVERIFY(channelBack != nullptr);
    QTest::mouseClick(channelBack, Qt::LeftButton);

    auto* stageNavigation = window.findChild<QToolButton*>(QStringLiteral("layerStage"));
    QVERIFY(stageNavigation != nullptr);
    QTest::mouseClick(stageNavigation, Qt::LeftButton);
    auto* stage = window.findChild<flow8::ui::StageView*>();
    QVERIFY(stage != nullptr);
    QVERIFY(stage->isVisible());
    QVERIFY(window.findChild<flow8::ui::FaderWidget*>(QStringLiteral("stageFader0")) != nullptr);

    auto* monitorNavigation = window.findChild<QToolButton*>(QStringLiteral("layerMonitor1"));
    QVERIFY(monitorNavigation != nullptr);
    QTest::mouseClick(monitorNavigation, Qt::LeftButton);
    auto* monitor = window.findChild<flow8::ui::MonitorView*>(QStringLiteral("monitor1View"));
    QVERIFY(monitor != nullptr);
    QTRY_VERIFY(monitor->isVisible());
    QVERIFY(monitor->findChild<flow8::ui::FaderWidget*>(
        QStringLiteral("monitor1Send0")) != nullptr);
    auto* sendMode = monitor->findChild<QComboBox*>(QStringLiteral("monitor1Mode0"));
    QVERIFY(sendMode != nullptr);
    sendMode->setCurrentIndex(sendMode->findData(
        static_cast<int>(flow8::model::MonitorSendMode::PreFader)));
    QCOMPARE(device.state().channel(0)->monitorSends[0].mode.value,
             std::optional(flow8::model::MonitorSendMode::PreFader));
    auto* monitor2Navigation = window.findChild<QToolButton*>(QStringLiteral("layerMonitor2"));
    QVERIFY(monitor2Navigation != nullptr);
    QTest::mouseClick(monitor2Navigation, Qt::LeftButton);
    auto* monitor2 = window.findChild<flow8::ui::MonitorView*>(QStringLiteral("monitor2View"));
    QVERIFY(monitor2 != nullptr);
    QTRY_VERIFY(monitor2->isVisible());
    QVERIFY(monitor2->findChild<flow8::ui::FaderWidget*>(
        QStringLiteral("monitor2Send0")) != nullptr);

    auto* mainNavigation = window.findChild<QToolButton*>(QStringLiteral("layerMain"));
    QVERIFY(mainNavigation != nullptr);
    QTest::mouseClick(mainNavigation, Qt::LeftButton);
    auto* mainView = window.findChild<flow8::ui::MainView*>(QStringLiteral("mainView"));
    QVERIFY(mainView != nullptr);
    QTRY_VERIFY(mainView->isVisible());
    QVERIFY(mainView->findChild<flow8::ui::FaderWidget*>(
        QStringLiteral("mainSend0")) != nullptr);

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
    auto* fxView = window.findChild<flow8::ui::FxView*>(QStringLiteral("fx1View"));
    QVERIFY(fxView != nullptr);
    QTRY_VERIFY(fxView->isVisible());
    auto* tapTempo = fxView->findChild<QPushButton*>(QStringLiteral("fxTapTempo"));
    QVERIFY(tapTempo != nullptr);
    QTest::mouseClick(tapTempo, Qt::LeftButton);
    QVERIFY(device.state().effects().at(0).tapTempoBpm.value.has_value());
    QVERIFY(fxView->findChild<flow8::ui::FaderWidget*>(
        QStringLiteral("fx1Send0")) != nullptr);
    auto* fx1MainRoute = fxView->findChild<QCheckBox*>(QStringLiteral("fx1Output0"));
    QVERIFY(fx1MainRoute != nullptr);
    fx1MainRoute->setChecked(false);
    QCOMPARE(device.state().routing().fxOutputRoute(
                 0, flow8::model::FxOutputDestination::Main)->enabled.value,
             std::optional(false));
    auto* fx2Navigation = window.findChild<QToolButton*>(QStringLiteral("layerFx2"));
    QVERIFY(fx2Navigation != nullptr);
    QTest::mouseClick(fx2Navigation, Qt::LeftButton);
    auto* fx2View = window.findChild<flow8::ui::FxView*>(QStringLiteral("fx2View"));
    QVERIFY(fx2View != nullptr);
    QTRY_VERIFY(fx2View->isVisible());
    QVERIFY(fx2View->findChild<flow8::ui::FaderWidget*>(
        QStringLiteral("fx2Send0")) != nullptr);

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
