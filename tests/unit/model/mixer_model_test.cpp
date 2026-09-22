#include "core/flow8_state.h"
#include "model/flow8_capabilities.h"
#include "model/preferences.h"
#include "model/session.h"

#include <QTest>

class MixerModelTest final : public QObject {
    Q_OBJECT

private slots:
    void officialInputTopologyIsExplicit();
    void inputGainUsesFlowMixEngineeringRange();
    void mixerSignalSourcesAndUsbAudioEndpointsAreDistinct();
    void mixBusesAndPhysicalOutputsAreDistinct();
    void snapshotAndRoutingShapesAreDistinct();
    void flowMixFunctionalExtensionsRemainExplicit();
};

void MixerModelTest::officialInputTopologyIsExplicit()
{
    const auto inputs = flow8::model::createOfficialInputProfile();
    QCOMPARE(inputs.size(), 7);
    QCOMPARE(inputs[0].inputId, flow8::model::InputId::Input1);
    QCOMPARE(inputs[4].inputId, flow8::model::InputId::Input56);
    QCOMPARE(inputs[5].inputId, flow8::model::InputId::Input78);
    QCOMPARE(inputs[6].inputId, flow8::model::InputId::UsbBluetooth);
    QVERIFY(!inputs[0].stereoPair);
    QVERIFY(inputs[4].stereoPair);
    QVERIFY(inputs[5].stereoPair);
    QVERIFY(inputs[6].stereoPair);
    QCOMPARE(inputs[4].spatialControl, flow8::model::SpatialControl::Balance);
    QVERIFY(inputs[0].phantom48V.has_value());
    QVERIFY(inputs[0].phaseInverted.has_value());
    QVERIFY(inputs[0].lowCut.has_value());
    QCOMPARE(inputs[0].visible.value, std::optional(true));
    QCOMPARE(inputs[6].icon.value, std::optional(flow8::model::ChannelIcon::Playback));
    QVERIFY(inputs[1].phantom48V.has_value());
    for (int index = 2; index < inputs.size(); ++index) {
        QVERIFY(!inputs[index].phantom48V.has_value());
    }
    QVERIFY(inputs[5].capabilities.compressor);
    QVERIFY(!inputs[6].capabilities.gain);
    QVERIFY(!inputs[6].capabilities.lowCut);
    QVERIFY(!inputs[6].capabilities.compressor);
    QVERIFY(!inputs[6].capabilities.phase);
    QVERIFY(!inputs[6].phaseInverted.has_value());
    QCOMPARE(inputs[0].capabilities.evidence.source,
             flow8::model::CapabilitySource::OfficialManual);
}

void MixerModelTest::inputGainUsesFlowMixEngineeringRange()
{
    using namespace flow8::model;

    QCOMPARE(inputGainDbFromNormalized(0.0), -20.0);
    QCOMPARE(inputGainDbFromNormalized(0.5), 20.0);
    QCOMPARE(inputGainDbFromNormalized(1.0), 60.0);
    QCOMPARE(inputGainDbFromNormalized(-1.0), -20.0);
    QCOMPARE(inputGainDbFromNormalized(2.0), 60.0);

    QCOMPARE(normalizedInputGainFromDb(-20.0), 0.0);
    QCOMPARE(normalizedInputGainFromDb(20.0), 0.5);
    QCOMPARE(normalizedInputGainFromDb(60.0), 1.0);
    QCOMPARE(normalizedInputGainFromDb(-40.0), 0.0);
    QCOMPARE(normalizedInputGainFromDb(80.0), 1.0);
}

void MixerModelTest::mixerSignalSourcesAndUsbAudioEndpointsAreDistinct()
{
    const auto sources = flow8::model::createSignalSourceProfile();
    QCOMPARE(sources.size(), 7);
    for (int index = 0; index < 7; ++index) {
        QCOMPARE(sources[index].mixerInputIndex, std::optional(index));
        QVERIFY(sources[index].mixerEndpoint.has_value());
    }
    QCOMPARE(sources[6].id, flow8::model::SignalSourceId::BluetoothUsbMixer);

    const auto usbEndpoints = flow8::model::createUsbAudioEndpointProfile();
    QCOMPARE(usbEndpoints.size(), 2);
    QCOMPARE(usbEndpoints[0].id, flow8::model::UsbAudioEndpointId::Usb12);
    QCOMPARE(usbEndpoints[1].id, flow8::model::UsbAudioEndpointId::Usb34);
    QCOMPARE(usbEndpoints[0].defaultLabel, QStringLiteral("USB 1/2"));
    QCOMPARE(usbEndpoints[0].evidence.source,
             flow8::model::CapabilitySource::OfficialApk);
}

void MixerModelTest::mixBusesAndPhysicalOutputsAreDistinct()
{
    const auto buses = flow8::model::createOfficialBusProfile();
    QCOMPARE(buses.size(), 3);
    QCOMPARE(buses[0].busId, flow8::model::BusId::Main);
    QCOMPARE(buses[1].busId, flow8::model::BusId::Monitor1);
    QCOMPARE(buses[2].busId, flow8::model::BusId::Monitor2);
    QVERIFY(buses[0].balance.has_value());
    QVERIFY(buses[0].eq.has_value());
    QVERIFY(buses[0].limiterDb.has_value());
    QVERIFY(buses[0].outputDelay.has_value());
    QVERIFY(!buses[1].balance.has_value());
    QVERIFY(buses[1].eq.has_value());
    QVERIFY(buses[2].limiterDb.has_value());

    const auto outputs = flow8::model::createPhysicalOutputProfile();
    QCOMPARE(outputs.size(), 4);
    QCOMPARE(outputs[0].id, flow8::model::PhysicalOutputId::MainOut);
    QCOMPARE(outputs[1].id, flow8::model::PhysicalOutputId::MonitorOut1);
    QCOMPARE(outputs[2].id, flow8::model::PhysicalOutputId::MonitorOut2);
    QCOMPARE(outputs[3].id, flow8::model::PhysicalOutputId::Headphones);
    QCOMPARE(outputs[0].nominalSource,
             std::optional(flow8::model::PhysicalOutputSource::Main));
    QCOMPARE(outputs[1].nominalSource,
             std::optional(flow8::model::PhysicalOutputSource::Monitor1));
    QCOMPARE(outputs[2].nominalSource,
             std::optional(flow8::model::PhysicalOutputSource::Monitor2));
    QVERIFY(!outputs[3].nominalSource.has_value());
    QVERIFY(!outputs[3].padMinus10Dbv.has_value());
}

void MixerModelTest::snapshotAndRoutingShapesAreDistinct()
{
    const auto snapshots = flow8::model::createHardwareSnapshotProfile();
    QCOMPARE(snapshots.size(), 15);
    QCOMPARE(snapshots.first().storage, flow8::model::SnapshotStorage::HardwareSlot);
    QCOMPARE(snapshots.first().id, QStringLiteral("hardware:1"));
    QVERIFY(!snapshots.first().scope.value.has_value());

    const auto routing = flow8::model::createRoutingProfile();
    QCOMPARE(routing.routeLevels.cells.size(), 35);
    QCOMPARE(routing.fxOutputRoutes.size(), 6);
    QVERIFY(routing.routeLevels.level(4, flow8::model::RoutingDestination::Fx2) != nullptr);
    QVERIFY(routing.fxOutputRoute(0, flow8::model::FxOutputDestination::Main) != nullptr);
    QVERIFY(routing.fxOutputRoute(1, flow8::model::FxOutputDestination::Monitor2) != nullptr);
    QVERIFY(routing.routeLevels.level(7, flow8::model::RoutingDestination::Main) == nullptr);
    QCOMPARE(routing.routeLevels.cells.first().sourceEndpoint,
             flow8::model::EndpointId::Input1);
    QCOMPARE(routing.routeLevels.cells.first().destinationEndpoint,
             flow8::model::EndpointId::MainLr);
}

void MixerModelTest::flowMixFunctionalExtensionsRemainExplicit()
{
    const auto routing = flow8::model::createRoutingProfile();
    QVERIFY(routing.fxOutputRoute(1, flow8::model::FxOutputDestination::Monitor2) != nullptr);
    QVERIFY(!routing.usbAudio.mode.value.has_value());
    QVERIFY(!routing.usbAudio.input56Assignment.value.has_value());
    QVERIFY(!routing.usbAudio.input78Assignment.value.has_value());
    QVERIFY(!routing.usbAudio.monitorOutputFeeds[0].value.has_value());
    QVERIFY(!routing.headphones.source.value.has_value());
    QVERIFY(!routing.headphones.tapPoint.value.has_value());
    const auto monitorLink = flow8::model::createMonitorLinkProfile();
    QVERIFY(!monitorLink.stereoLinked.value.has_value());
    QCOMPARE(monitorLink.propagationEvidence,
             flow8::model::EvidenceStatus::Unknown);

    const flow8::model::AppPreferences preferences;
    QVERIFY(preferences.showMuteButtons);
    QVERIFY(preferences.showChannelIcons);
    QCOMPARE(preferences.eqEditingMode, flow8::model::EqEditingMode::Parametric);

    flow8::model::AssistedSetupState setup;
    QCOMPARE(setup.step, flow8::model::AssistedSetupStep::SelectInput);
    QVERIFY(!setup.input.has_value());
    flow8::model::EzGainSession ezGain;
    QVERIFY(!ezGain.running);
    QVERIFY(ezGain.results.isEmpty());
}

QTEST_GUILESS_MAIN(MixerModelTest)

#include "mixer_model_test.moc"
