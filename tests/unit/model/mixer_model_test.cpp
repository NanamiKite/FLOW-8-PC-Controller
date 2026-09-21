#include "core/flow8_state.h"
#include "model/flow8_capabilities.h"
#include "model/preferences.h"
#include "model/session.h"

#include <QTest>

class MixerModelTest final : public QObject {
    Q_OBJECT

private slots:
    void officialInputTopologyIsExplicit();
    void busCapabilitiesDoNotLeakIntoFxBuses();
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
    QCOMPARE(inputs[0].capabilities.evidence.source,
             flow8::model::CapabilitySource::OfficialManual);
}

void MixerModelTest::busCapabilitiesDoNotLeakIntoFxBuses()
{
    const auto buses = flow8::model::createOfficialBusProfile();
    QCOMPARE(buses.size(), 5);
    QVERIFY(buses[0].balance.has_value());
    QVERIFY(buses[0].eq.has_value());
    QVERIFY(buses[0].limiterDb.has_value());
    QVERIFY(buses[0].outputDelay.has_value());
    QVERIFY(!buses[1].balance.has_value());
    QVERIFY(buses[1].eq.has_value());
    QVERIFY(buses[2].limiterDb.has_value());
    for (int index = 3; index < 5; ++index) {
        QVERIFY(!buses[index].balance.has_value());
        QVERIFY(!buses[index].eq.has_value());
        QVERIFY(!buses[index].limiterDb.has_value());
        QVERIFY(!buses[index].outputDelay.has_value());
        QVERIFY(buses[index].capabilities.fxEngine);
    }
}

void MixerModelTest::snapshotAndRoutingShapesAreDistinct()
{
    const auto snapshots = flow8::model::createHardwareSnapshotProfile();
    QCOMPARE(snapshots.size(), 15);
    QCOMPARE(snapshots.first().storage, flow8::model::SnapshotStorage::HardwareSlot);
    QCOMPARE(snapshots.first().id, QStringLiteral("hardware:1"));
    QVERIFY(!snapshots.first().scope.value.has_value());

    const auto routing = flow8::model::createRoutingProfile();
    QCOMPARE(routing.routes.size(), 35);
    QCOMPARE(routing.usbRoutes.size(), 9);
    QCOMPARE(routing.fxOutputRoutes.size(), 6);
    QVERIFY(routing.route(4, flow8::model::RoutingDestination::Fx2) != nullptr);
    QVERIFY(routing.fxOutputRoute(0, flow8::model::FxOutputDestination::Main) != nullptr);
    QVERIFY(routing.fxOutputRoute(1, flow8::model::FxOutputDestination::Monitor2) != nullptr);
    QVERIFY(routing.route(7, flow8::model::RoutingDestination::Main) == nullptr);
}

void MixerModelTest::flowMixFunctionalExtensionsRemainExplicit()
{
    const auto routing = flow8::model::createRoutingProfile();
    QVERIFY(routing.usbRoute(flow8::model::UsbRouteDestination::Monitor2) != nullptr);
    QVERIFY(routing.fxOutputRoute(1, flow8::model::FxOutputDestination::Monitor2) != nullptr);
    QVERIFY(!routing.usbMode.value.has_value());
    QVERIFY(!routing.headphoneSource.value.has_value());
    QVERIFY(!routing.monitorLink.stereoLinked.value.has_value());

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
