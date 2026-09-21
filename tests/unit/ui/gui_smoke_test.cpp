#include "core/flow8_device.h"
#include "simulator/fake_transport.h"
#include "ui/language_manager.h"
#include "ui/main_window.h"
#include "ui/mixer/channel_strip.h"
#include "ui/setup/assisted_setup_wizard.h"
#include "ui/settings/settings_dialog.h"
#include "ui/stage/stage_view.h"
#include "ui/widgets/eq_graph_widget.h"
#include "ui/widgets/fader_widget.h"

#include <QPushButton>
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

    auto* connectButton = window.findChild<QPushButton*>(QStringLiteral("connectButton"));
    QVERIFY(connectButton != nullptr);
    QTest::mouseClick(connectButton, Qt::LeftButton);
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

    QVERIFY(window.findChild<flow8::ui::EqGraphWidget*>() != nullptr);
    auto* stageNavigation = window.findChild<QToolButton*>(QStringLiteral("navigation1"));
    QVERIFY(stageNavigation != nullptr);
    QTest::mouseClick(stageNavigation, Qt::LeftButton);
    auto* stage = window.findChild<flow8::ui::StageView*>();
    QVERIFY(stage != nullptr);
    QVERIFY(stage->isVisible());
    QVERIFY(window.findChild<flow8::ui::FaderWidget*>(QStringLiteral("stageFader0")) != nullptr);

    auto* monitorNavigation = window.findChild<QToolButton*>(QStringLiteral("navigation4"));
    QVERIFY(monitorNavigation != nullptr);
    QTest::mouseClick(monitorNavigation, Qt::LeftButton);
    QVERIFY(window.findChild<QSlider*>(QStringLiteral("busLevel"))->isVisible());

    auto* mainNavigation = window.findChild<QToolButton*>(QStringLiteral("navigation6"));
    QVERIFY(mainNavigation != nullptr);
    QTest::mouseClick(mainNavigation, Qt::LeftButton);

    auto* snapshotNavigation = window.findChild<QToolButton*>(QStringLiteral("navigation7"));
    QVERIFY(snapshotNavigation != nullptr);
    QTest::mouseClick(snapshotNavigation, Qt::LeftButton);
    auto* snapshots = window.findChild<QListWidget*>(QStringLiteral("hardwareSnapshots"));
    QVERIFY(snapshots != nullptr);
    QCOMPARE(snapshots->count(), 15);

    auto* fxNavigation = window.findChild<QToolButton*>(QStringLiteral("navigation2"));
    QVERIFY(fxNavigation != nullptr);
    QTest::mouseClick(fxNavigation, Qt::LeftButton);
    auto* tapTempo = window.findChild<QPushButton*>(QStringLiteral("fxTapTempo"));
    QVERIFY(tapTempo != nullptr);
    QTest::mouseClick(tapTempo, Qt::LeftButton);
    QVERIFY(device.state().effects().at(0).tapTempoBpm.value.has_value());

    auto* routingNavigation = window.findChild<QToolButton*>(QStringLiteral("navigation8"));
    QVERIFY(routingNavigation != nullptr);
    QTest::mouseClick(routingNavigation, Qt::LeftButton);
    auto* usbMode = window.findChild<QComboBox*>();
    QVERIFY(usbMode != nullptr);
    QVERIFY(window.findChild<QCheckBox*>(QStringLiteral("usbRoute7")) != nullptr);

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
