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
#include "ui/ui_text.h"
#include "ui/widgets/eq_graph_widget.h"
#include "ui/widgets/fader_widget.h"
#include "ui/widgets/knob_widget.h"
#include "ui/widgets/meter_widget.h"

#include <QPushButton>
#include <QRegularExpression>
#include <QCheckBox>
#include <QComboBox>
#include <QLabel>
#include <QListWidget>
#include <QSlider>
#include <QTabWidget>
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
    auto* initialStrip = window.findChild<flow8::ui::ChannelStrip*>(
        QStringLiteral("channelStrip0"));
    QVERIFY(initialStrip != nullptr);
    auto* firstMeter = initialStrip->findChild<flow8::ui::MeterWidget*>();
    QVERIFY(firstMeter != nullptr);
    QCOMPARE(flow8::ui::meterDbFromNormalized(0.0), -60.0);
    QCOMPARE(flow8::ui::meterDbFromNormalized(1.0), 10.0);
    QCOMPARE(flow8::ui::postFaderMeterNormalized(0.8, 0.0), 0.0);
    QVERIFY(flow8::ui::postFaderMeterNormalized(0.8, 1.0)
        > flow8::ui::postFaderMeterNormalized(0.8, 0.5));
    QCOMPARE(firstMeter->height(), fader->height());
    auto* routeStatus = window.findChild<QLabel*>(QStringLiteral("routeStatus0"));
    QVERIFY(routeStatus != nullptr);
    QVERIFY(routeStatus->isHidden());
    fader->setFocus();
    QTest::keyClick(fader, Qt::Key_End);
    QTRY_COMPARE(firstMeter->targetLevelDb(), -60.0);
    QTest::keyClick(fader, Qt::Key_Home);
    for (int step = 0; step < 17; ++step) {
        QTest::keyClick(fader, Qt::Key_Down);
    }
    QTRY_VERIFY(device.state().channel(0)->fader.value.has_value());
    QTRY_VERIFY(std::abs(*device.state().channel(0)->fader.value - 0.83) <= (1.0 / 255.0));
    QTRY_VERIFY(firstMeter->targetLevelDb() > -60.0);

    auto* firstStrip = window.findChild<flow8::ui::ChannelStrip*>(
        QStringLiteral("channelStrip0"));
    QVERIFY(firstStrip != nullptr);
    auto* secondStrip = window.findChild<flow8::ui::ChannelStrip*>(
        QStringLiteral("channelStrip1"));
    auto* thirdStrip = window.findChild<flow8::ui::ChannelStrip*>(
        QStringLiteral("channelStrip2"));
    QVERIFY(secondStrip != nullptr);
    QVERIFY(thirdStrip != nullptr);
    auto* input1PhantomIndicator = firstStrip->findChild<QLabel*>(
        QStringLiteral("phantomIndicator0"));
    auto* input2PhantomIndicator = secondStrip->findChild<QLabel*>(
        QStringLiteral("phantomIndicator1"));
    auto* input3PhantomIndicator = thirdStrip->findChild<QLabel*>(
        QStringLiteral("phantomIndicator2"));
    QVERIFY(input1PhantomIndicator != nullptr);
    QVERIFY(input2PhantomIndicator != nullptr);
    QVERIFY(input3PhantomIndicator != nullptr);
    QTRY_VERIFY(input1PhantomIndicator->isVisible());
    QTRY_VERIFY(input2PhantomIndicator->isVisible());
    QVERIFY(input3PhantomIndicator->isHidden());
    QCOMPARE(input1PhantomIndicator->property("active").toBool(), false);
    QVERIFY(device.setChannelPhantom(0, true));
    QTRY_COMPARE(input1PhantomIndicator->property("active").toBool(), true);
    QVERIFY(device.setChannelPhantom(0, false));
    QTRY_COMPARE(input1PhantomIndicator->property("active").toBool(), false);
    QTest::mouseClick(firstStrip, Qt::LeftButton);
    auto* inspector = window.findChild<flow8::ui::InspectorWidget*>(
        QStringLiteral("mixerInputInspector"));
    QVERIFY(inspector != nullptr);
    QTRY_VERIFY(inspector->isVisible());
    QCOMPARE(inspector->selectedChannel(), 0);
    QCOMPARE(inspector->selectedDestination(), flow8::model::RoutingDestination::Main);
    for (int capability = 0; capability < 7; ++capability) {
        auto* capabilityName = inspector->findChild<QLabel*>(
            QStringLiteral("capabilityName%1").arg(capability));
        auto* capabilityValue = inspector->findChild<QLabel*>(
            QStringLiteral("capabilityValue%1").arg(capability));
        QVERIFY(capabilityName != nullptr);
        QVERIFY(capabilityValue != nullptr);
        QVERIFY(!capabilityName->text().isEmpty());
        QVERIFY(!capabilityValue->text().isEmpty());
    }
    QCOMPARE(inspector->findChild<QLabel*>(QStringLiteral("capabilityValue4"))->text(),
             flow8::ui::uiText("4-band Parametric EQ"));
    QCOMPARE(inspector->findChild<QLabel*>(QStringLiteral("capabilityValue6"))->text(),
             QStringLiteral("MON1 / MON2 / FX1 / FX2"));
    QCOMPARE(inspector->findChild<QLabel*>(QStringLiteral("capabilityValue1"))
                 ->property("available").toBool(),
             true);
    inspector->setSelectedChannel(2);
    QCOMPARE(inspector->findChild<QLabel*>(QStringLiteral("capabilityValue1"))
                 ->property("available").toBool(),
             false);
    QCOMPARE(inspector->findChild<QLabel*>(QStringLiteral("capabilityValue1"))->text(),
             flow8::ui::uiText("Not supported"));
    inspector->setSelectedChannel(0);
    auto* inspectorPhantom = inspector->findChild<QCheckBox*>(
        QStringLiteral("inspectorPhantom"));
    QVERIFY(inspectorPhantom != nullptr);
    QTRY_VERIFY(inspectorPhantom->isVisible());
    QCOMPARE(inspectorPhantom->property("class").toString(),
             QStringLiteral("phantomControl"));
    auto* inspectorPan = inspector->findChild<QSlider*>(
        QStringLiteral("inspectorPan"));
    auto* inspectorPanValue = inspector->findChild<QLabel*>(
        QStringLiteral("inspectorPanValue"));
    QVERIFY(inspectorPan != nullptr);
    QVERIFY(inspectorPanValue != nullptr);
    inspectorPan->setValue(-37);
    QCOMPARE(inspectorPanValue->text(), flow8::ui::panBalanceValueText(-37));
    QCOMPARE(device.state().channel(0)->pan.value, std::optional(-0.37));
    inspectorPan->setValue(0);
    QCOMPARE(inspectorPanValue->text(), flow8::ui::panBalanceValueText(0));
    auto* inspectorGain = inspector->findChild<QSlider*>(
        QStringLiteral("inspectorGain"));
    auto* inspectorGainValue = inspector->findChild<QLabel*>(
        QStringLiteral("inspectorGainValue"));
    auto* lowCutFrequency = inspector->findChild<QSlider*>(
        QStringLiteral("lowCutFrequency"));
    auto* lowCutFrequencyValue = inspector->findChild<QLabel*>(
        QStringLiteral("lowCutFrequencyValue"));
    QVERIFY(inspectorGain != nullptr);
    QVERIFY(inspectorGainValue != nullptr);
    QVERIFY(lowCutFrequency != nullptr);
    QVERIFY(lowCutFrequencyValue != nullptr);
    QCOMPARE(inspectorGain->minimum(), -200);
    QCOMPARE(inspectorGain->maximum(), 600);
    inspectorGain->setValue(-75);
    QCOMPARE(inspectorGainValue->text(), flow8::ui::decibelValueText(-7.5));
    QVERIFY(qAbs(device.state().channel(0)->gain.value.value_or(-1.0)
                 - flow8::model::normalizedInputGainFromDb(-7.5)) < 0.000001);
    lowCutFrequency->setValue(600);
    QCOMPARE(lowCutFrequencyValue->text(), flow8::ui::frequencyValueText(600.0));
    auto* inspectorTabs = inspector->findChild<QTabWidget*>();
    QVERIFY(inspectorTabs != nullptr);
    inspectorTabs->setCurrentIndex(1);
    QCoreApplication::processEvents();
    auto* channelEqGraph = inspector->findChild<flow8::ui::EqGraphWidget*>(
        QStringLiteral("channelEqGraph"));
    QVERIFY(channelEqGraph != nullptr);
    QTRY_VERIFY(channelEqGraph->isVisible());
    for (int band = 0; band < 4; ++band) {
        auto* eqGain = inspector->findChild<QSlider*>(
            QStringLiteral("eqGain%1").arg(band));
        QVERIFY(eqGain != nullptr);
        QCOMPARE(eqGain->orientation(), Qt::Vertical);
        QVERIFY2(channelEqGraph->geometry().bottom() <= eqGain->geometry().top(),
                 "EQ graph must be above the band faders");
        const auto* eqValue = inspector->findChild<QLabel*>(
            QStringLiteral("eqBandValue%1").arg(band));
        QVERIFY(eqValue != nullptr);
        QVERIFY2(!eqValue->text().contains(QRegularExpression(
                     QStringLiteral("[eE][+-]?\\d"))),
                 "EQ value must not use scientific notation");
    }

    const double frequencyBefore = device.state().channel(0)->eq.frequencyHz[0]
        .value.value_or(80.0);
    const double normalizedFrequency =
        (std::log10(frequencyBefore) - std::log10(20.0))
        / (std::log10(20000.0) - std::log10(20.0));
    const int handleX = qRound(8.0 + normalizedFrequency * (channelEqGraph->width() - 16.0));
    const int zeroGainY = qRound((channelEqGraph->height() - 20.0)
        - 0.5 * (channelEqGraph->height() - 28.0));
    const QPoint dragStart(handleX, zeroGainY);
    const QPoint dragEnd(
        qMin(channelEqGraph->width() - 9, handleX + 70),
        qMax(9, zeroGainY - 45));
    QTest::mousePress(channelEqGraph, Qt::LeftButton, Qt::NoModifier, dragStart);
    QTest::mouseMove(channelEqGraph, dragEnd);
    QTest::mouseRelease(channelEqGraph, Qt::LeftButton, Qt::NoModifier, dragEnd);
    QTRY_VERIFY(device.state().channel(0)->eq.gainDb[0].value.value_or(0.0) > 0.0);
    QCOMPARE(device.state().channel(0)->eq.frequencyHz[0].value,
             std::optional(frequencyBefore));

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

    QTest::mouseDClick(firstStrip, Qt::LeftButton);
    auto* channelEdit = window.findChild<flow8::ui::ChannelEditView*>(
        QStringLiteral("channelEditView"));
    QVERIFY(channelEdit != nullptr);
    QTRY_VERIFY(channelEdit->isVisible());
    QCOMPARE(channelEdit->channel(), 0);
    auto* channelEditPhantom = channelEdit->findChild<QCheckBox*>(
        QStringLiteral("channelEditPhantom"));
    QVERIFY(channelEditPhantom != nullptr);
    QTRY_VERIFY(channelEditPhantom->isVisible());
    QCOMPARE(channelEditPhantom->property("class").toString(),
             QStringLiteral("phantomControl"));
    QVERIFY(device.setChannelPhantom(0, true));
    QTRY_VERIFY(channelEditPhantom->isChecked());
    auto* channelEditPan = channelEdit->findChild<QSlider*>(
        QStringLiteral("channelEditPan"));
    auto* channelEditPanValue = channelEdit->findChild<QLabel*>(
        QStringLiteral("channelEditPanValue"));
    QVERIFY(channelEditPan != nullptr);
    QVERIFY(channelEditPanValue != nullptr);
    channelEditPan->setValue(42);
    QCOMPARE(channelEditPanValue->text(), flow8::ui::panBalanceValueText(42));
    QCOMPARE(device.state().channel(0)->pan.value, std::optional(0.42));
    auto* channelEditGain = channelEdit->findChild<QSlider*>(
        QStringLiteral("channelEditGain"));
    auto* channelEditGainValue = channelEdit->findChild<QLabel*>(
        QStringLiteral("channelEditGainValue"));
    QVERIFY(channelEditGain != nullptr);
    QVERIFY(channelEditGainValue != nullptr);
    QCOMPARE(channelEditGain->minimum(), -200);
    QCOMPARE(channelEditGain->maximum(), 600);
    channelEditGain->setValue(420);
    QCOMPARE(channelEditGainValue->text(), flow8::ui::decibelValueText(42.0));
    QVERIFY(qAbs(device.state().channel(0)->gain.value.value_or(-1.0)
                 - flow8::model::normalizedInputGainFromDb(42.0)) < 0.000001);

    channelEdit->setChannel(1);
    QCoreApplication::processEvents();
    QTRY_VERIFY(channelEditPhantom->isVisible());
    channelEdit->setChannel(2);
    QCoreApplication::processEvents();
    QTRY_VERIFY(channelEditPhantom->isHidden());
    QTest::keyClick(&window, Qt::Key_Escape);
    QTRY_VERIFY(mixer->isVisible());

    auto* monitor2Navigation = window.findChild<QToolButton*>(QStringLiteral("layerMonitor2"));
    QVERIFY(monitor2Navigation != nullptr);
    QTest::mouseClick(monitor2Navigation, Qt::LeftButton);
    QCOMPARE(mixer->destination(), flow8::model::RoutingDestination::Monitor2);
    QTest::mouseClick(showMaster, Qt::LeftButton);
    auto* master = mixer->findChild<flow8::ui::DetailPanel*>(
        QStringLiteral("destinationMasterPanel"));
    QVERIFY(master != nullptr);
    QTRY_VERIFY(master->isVisible());
    auto* busLevel = master->findChild<flow8::ui::FaderWidget*>(
        QStringLiteral("busLevel"));
    auto* busLimiter = master->findChild<flow8::ui::KnobWidget*>(
        QStringLiteral("busLimiter"));
    QVERIFY(busLevel != nullptr);
    QVERIFY(busLimiter != nullptr);
    QCOMPARE(busLevel->minimumDb(), -60.0);
    QCOMPARE(busLevel->maximumDb(), 10.0);
    QCOMPARE(busLimiter->minimum(), -30.0);
    QCOMPARE(busLimiter->maximum(), 0.0);
    QCOMPARE(busLimiter->displayMode(),
             flow8::ui::KnobWidget::DisplayMode::Decibels);
    QTRY_VERIFY(busLimiter->isVisible());
    busLevel->setFocus();
    QTest::keyClick(busLevel, Qt::Key_Home);
    QCOMPARE(device.state().bus(2)->fader.value, std::optional(1.0));
    const double limiterBefore = device.state().bus(2)->limiterDb->value.value_or(-30.0);
    busLimiter->setFocus();
    QTest::keyClick(busLimiter, Qt::Key_Right);
    QVERIFY(qAbs(device.state().bus(2)->limiterDb->value.value_or(-30.0)
                 - (limiterBefore + 0.1)) < 0.000001);
    QVERIFY(busLimiter->valueText().endsWith(QStringLiteral(" dB")));
    for (int band = 0; band < 9; ++band) {
        const auto* busEqValue = master->findChild<QLabel*>(
            QStringLiteral("busEqValue%1").arg(band));
        QVERIFY(busEqValue != nullptr);
        QVERIFY2(!busEqValue->text().contains(QRegularExpression(
                     QStringLiteral("[eE][+-]?\\d"))),
                 "Bus EQ value must not use scientific notation");
    }

    auto* mainNavigation = window.findChild<QToolButton*>(QStringLiteral("layerMain"));
    QVERIFY(mainNavigation != nullptr);
    QTest::mouseClick(mainNavigation, Qt::LeftButton);
    QCOMPARE(mixer->destination(), flow8::model::RoutingDestination::Main);
    QTRY_VERIFY(mixer->isVisible());
    auto* busBalance = master->findChild<flow8::ui::KnobWidget*>(
        QStringLiteral("busBalance"));
    QVERIFY(busBalance != nullptr);
    QTRY_VERIFY(busBalance->isVisible());
    QCOMPARE(busBalance->minimum(), -1.0);
    QCOMPARE(busBalance->maximum(), 1.0);
    QCOMPARE(busBalance->displayMode(),
             flow8::ui::KnobWidget::DisplayMode::PanBalance);
    busBalance->setFocus();
    QTest::keyClick(busBalance, Qt::Key_Right);
    QVERIFY(qAbs(device.state().bus(0)->balance->value.value_or(0.0) - 0.01)
            < 0.000001);
    QCOMPARE(busBalance->valueText(), flow8::ui::panBalanceValueText(1));

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
    QVERIFY(setup->findChild<QComboBox*>(QStringLiteral("usbInput56Assignment")) != nullptr);
    QVERIFY(setup->findChild<QComboBox*>(QStringLiteral("usbInput78Assignment")) != nullptr);
    QVERIFY(setup->findChild<QComboBox*>(QStringLiteral("usbMonitorOutputFeed0")) != nullptr);
    QVERIFY(setup->findChild<QComboBox*>(QStringLiteral("usbMonitorOutputFeed1")) != nullptr);
    QVERIFY(setup->findChild<QComboBox*>(QStringLiteral("headphoneSource")) != nullptr);
    QVERIFY(setup->findChild<QComboBox*>(QStringLiteral("headphoneTapPoint")) != nullptr);
    QVERIFY(setup->findChild<QCheckBox*>(
        QStringLiteral("bluetoothUsbPhonesOnly")) != nullptr);
    QVERIFY(setup->findChild<QCheckBox*>(
        QStringLiteral("outputPadMinus10Dbv0")) != nullptr);
    QVERIFY(setup->findChild<QCheckBox*>(QStringLiteral("fxOutputRoute0")) != nullptr);
    auto* stereoLink = setup->findChild<QCheckBox*>(
        QStringLiteral("monitorStereoLink"));
    QVERIFY(stereoLink != nullptr);
    stereoLink->setChecked(true);
    QCOMPARE(device.state().monitorLink().stereoLinked.value,
             std::optional(true));
    mixer->setDestination(flow8::model::RoutingDestination::Monitor1);
    fader->setFocus();
    QTest::keyClick(fader, Qt::Key_Home);
    for (int step = 0; step < 43; ++step) {
        QTest::keyClick(fader, Qt::Key_Down);
    }
    QCOMPARE(device.state().routeLevel(
                 0, flow8::model::RoutingDestination::Monitor1)->effectiveValue(), 0.57);
    QCOMPARE(device.state().routeLevel(
                 0, flow8::model::RoutingDestination::Monitor2)->effectiveValue(), 0.57);
    mixer->setDestination(flow8::model::RoutingDestination::Monitor2);
    QCOMPARE(fader->value(), 0.57);
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
