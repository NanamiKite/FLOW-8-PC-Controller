#include "core/flow8_device.h"
#include "simulator/fake_transport.h"

#include <QTest>

#include <memory>

namespace {

class NonSimulatorTransport final : public flow8::Flow8Transport {
public:
    using Flow8Transport::Flow8Transport;

    [[nodiscard]] QString displayName() const override { return QStringLiteral("test transport"); }
    [[nodiscard]] State state() const noexcept override { return state_; }
    void connectTransport() override
    {
        state_ = State::Connected;
        emit stateChanged(state_);
    }
    void disconnectTransport() override
    {
        state_ = State::Disconnected;
        emit stateChanged(state_);
    }
    bool send(const QByteArray&) override { return state_ == State::Connected; }

private:
    State state_ {State::Disconnected};
};

} // namespace

class Flow8DeviceTest final : public QObject {
    Q_OBJECT

private slots:
    void simulatorConnectsAndCreatesProfile();
    void controlsFlowThroughDeviceApi();
    void mixBusesAndFxRoutingRemainIndependent();
    void usbAndPhysicalOutputRoutingRemainIndependent();
    void realTransportDoesNotClaimReadyBeforeHandshake();
};

void Flow8DeviceTest::simulatorConnectsAndCreatesProfile()
{
    flow8::Flow8Device device;
    device.setTransport(std::make_unique<flow8::simulator::FakeTransport>());
    device.connectDevice();

    QTRY_COMPARE(device.state().connectionState(), flow8::ConnectionState::Ready);
    QCOMPARE(device.state().channels().size(), 7);
    QCOMPARE(device.state().channels().at(4).inputId, flow8::model::InputId::Input56);
    QVERIFY(device.state().channels().at(4).stereoPair);
    QVERIFY(device.state().channels().at(5).stereoPair);
    QCOMPARE(device.state().channels().at(6).inputType,
             flow8::model::InputType::UsbBluetooth);
    QCOMPARE(device.state().signalSources().size(), 9);
    QCOMPARE(device.state().buses().size(), 3);
    QCOMPARE(device.state().effects().size(), 2);
    QCOMPARE(device.state().physicalOutputs().size(), 4);
    QCOMPARE(device.state().snapshots().size(), 15);
    QCOMPARE(device.state().routing().routeLevels.cells.size(), 35);
    QCOMPARE(device.state().routing().fxOutputRoutes.size(), 6);
    QCOMPARE(device.state().buses().at(0).busId, flow8::model::BusId::Main);
    QCOMPARE(device.state().buses().at(1).busId, flow8::model::BusId::Monitor1);
    QCOMPARE(device.state().buses().at(2).busId, flow8::model::BusId::Monitor2);
    QCOMPARE(device.state().signalSources().at(7).id,
             flow8::model::SignalSourceId::UsbReturn12);
    QCOMPARE(device.state().signalSources().at(8).id,
             flow8::model::SignalSourceId::UsbReturn34);
    QVERIFY(!device.state().signalSources().at(7).mixerInputIndex.has_value());
    QCOMPARE(device.state().globalTempo().bpm.value, std::optional(120.0));
    QCOMPARE(device.state().channel(0)->monitorSends[0].levelDb.value,
             std::optional(-10.0));
    QCOMPARE(device.state().channel(0)->monitorSends[1].levelDb.value,
             std::optional(-15.0));
    QCOMPARE(device.state().channel(0)->fxSendLevelDb[0].value,
             std::optional(-8.0));
    QCOMPARE(device.state().channel(0)->fxSendLevelDb[1].value,
             std::optional(-12.0));
    QCOMPARE(device.state().routeLevel(
                 0, flow8::model::RoutingDestination::Main)->confirmed.value,
             std::optional(0.72));
    QCOMPARE(device.state().routing().fxOutputRoute(
                 0, flow8::model::FxOutputDestination::Main)->enabled.value,
             std::optional(true));
    QCOMPARE(device.state().routing().fxOutputRoute(
                 0, flow8::model::FxOutputDestination::Monitor1)->enabled.value,
             std::optional(true));
    QCOMPARE(device.state().routing().fxOutputRoute(
                 0, flow8::model::FxOutputDestination::Monitor2)->enabled.value,
             std::optional(false));
    QCOMPARE(device.state().routing().fxOutputRoute(
                 1, flow8::model::FxOutputDestination::Monitor2)->enabled.value,
             std::optional(true));
    QCOMPARE(device.state().routing().monitor.stereoLinked.value,
             std::optional(false));
    QCOMPARE(device.state().routing().headphones.source.value,
             std::optional(flow8::model::HeadphoneSource::Main));
    QCOMPARE(device.state().physicalOutput(flow8::model::PhysicalOutputId::MainOut)
                 ->nominalSource,
             std::optional(flow8::model::PhysicalOutputSource::Main));
    QVERIFY(device.isControlAvailable(flow8::Flow8Device::Control::ChannelFader));
    QCOMPARE(device.state().channel(0)->inputId, flow8::model::InputId::Input1);
    QCOMPARE(device.state().channel(0)->defaultLabel, QStringLiteral("Input 1"));
    QVERIFY(!device.state().channel(0)->name.value.has_value());
}

void Flow8DeviceTest::controlsFlowThroughDeviceApi()
{
    flow8::Flow8Device device;
    auto transport = std::make_unique<flow8::simulator::FakeTransport>();
    auto* transportPointer = transport.get();
    device.setTransport(std::move(transport));
    device.connectDevice();
    QTRY_COMPARE(device.state().connectionState(), flow8::ConnectionState::Ready);

    QVERIFY(device.setChannelFader(0, 0.25));
    QTRY_VERIFY(qAbs(*device.state().channel(0)->fader.value - 0.25) < 0.01);
    QCOMPARE(transportPointer->sentPackets().size(), 0);
    QCOMPARE(device.state().channel(0)->fader.evidence,
             flow8::model::EvidenceStatus::Synthetic);
    QVERIFY(device.state().channel(0)->fader.source.contains(QStringLiteral("SYNTHETIC")));

    QVERIFY(device.setChannelMuted(0, true));
    QCOMPARE(*device.state().channel(0)->muted.value, true);
    QVERIFY(device.setChannelSoloed(0, true));
    QCOMPARE(*device.state().channel(0)->soloed.value, true);
    QVERIFY(device.setChannelPan(0, -0.5));
    QCOMPARE(*device.state().channel(0)->pan.value, -0.5);
    QVERIFY(device.setChannelName(0, QStringLiteral("Lead Vocal")));
    QCOMPARE(device.state().channel(0)->name.value, std::optional(QStringLiteral("Lead Vocal")));
    QVERIFY(device.setChannelIcon(0, flow8::model::ChannelIcon::Microphone));
    QVERIFY(device.setChannelVisible(0, false));
    QCOMPARE(device.state().channel(0)->visible.value, std::optional(false));
    QVERIFY(device.setChannelPhantom(0, true));
    QCOMPARE(device.state().channel(0)->phantom48V->value, std::optional(true));
    QVERIFY(!device.setChannelPhantom(2, true));
    QVERIFY(device.setChannelPhaseInverted(0, true));
    QCOMPARE(device.state().channel(0)->phaseInverted->value, std::optional(true));
    QVERIFY(!device.setChannelPhaseInverted(6, true));
    QVERIFY(device.setChannelLowCut(0, true, 95.0));
    QCOMPARE(device.state().channel(0)->lowCut->enabled.value, std::optional(true));
    QCOMPARE(device.state().channel(0)->lowCut->frequencyHz.value, std::optional(95.0));
    QVERIFY(!device.setChannelLowCut(6, true, 95.0));
    QVERIFY(device.setMonitorSendMode(0, 0, flow8::model::MonitorSendMode::PreFader));
    QCOMPARE(device.state().channel(0)->monitorSends[0].mode.value,
             std::optional(flow8::model::MonitorSendMode::PreFader));
    QVERIFY(device.setChannelEqGain(0, 1, -3.5));
    QCOMPARE(device.state().channel(0)->eq.gainDb[1].value, std::optional(-3.5));
    QVERIFY(device.setChannelCompressorAmount(0, 0.4));
    QCOMPARE(device.state().channel(0)->compressor.amount.value, std::optional(0.4));
    QVERIFY(!device.setChannelCompressorAmount(6, 0.4));
    QVERIFY(device.setBusEqGain(1, 4, 2.0));
    QVERIFY(!device.setBusEqGain(3, 4, 2.0));
    QVERIFY(device.setBusBalance(0, -0.25));
    QVERIFY(!device.setBusBalance(1, -0.25));
    QVERIFY(device.setBusLimiterDb(2, -8.0));
    QVERIFY(!device.setBusLimiterDb(3, -8.0));
    QVERIFY(device.setFxPreset(1, 12));
    QCOMPARE(device.state().effects().at(1).preset.value, std::optional(12));
    QVERIFY(device.setFxMuted(1, true));
    QCOMPARE(device.state().effects().at(1).muted.value, std::optional(true));
    QVERIFY(device.tapTempo());
    QVERIFY(device.state().globalTempo().bpm.value.has_value());
    QVERIFY(device.state().effects().at(0).tapTempoBpm.value.has_value());
    QVERIFY(device.state().effects().at(1).tapTempoBpm.value.has_value());
    QVERIFY(device.recallSnapshot(14));
    QCOMPARE(device.state().activeSnapshotIndex(), 14);
    QVERIFY(device.setRouteLevel(4, flow8::model::RoutingDestination::Fx2, 0.44));
    QCOMPARE(device.state().routeLevel(
                 4, flow8::model::RoutingDestination::Fx2)->confirmed.value,
             std::optional(0.44));
    QVERIFY(device.setUsbMode(flow8::model::UsbMode::Recording));
    QCOMPARE(device.state().routing().usb.mode.value,
             std::optional(flow8::model::UsbMode::Recording));
    QVERIFY(device.setUsbPlaybackAssignment(
        0, flow8::model::UsbPlaybackAssignment::UsbReturn));
    QVERIFY(device.setMonitorRouteSource(
        0, flow8::model::MonitorRouteSource::UsbReturn12));
    QVERIFY(device.setFxOutputRouteEnabled(
        1, flow8::model::FxOutputDestination::Monitor1, true));
    QVERIFY(device.setHeadphoneSource(flow8::model::HeadphoneSource::Monitor));
    QVERIFY(device.setHeadphoneTapPoint(flow8::model::RoutingTapPoint::PreFader));
    QVERIFY(device.setOutputPadMinus10Dbv(
        flow8::model::PhysicalOutputId::MainOut, true));
    QVERIFY(device.setBluetoothUsbPhonesOnly(true));
    QVERIFY(device.setOutputPadMinus10Dbv(
        flow8::model::PhysicalOutputId::MainOut, true));
    QVERIFY(device.setMonitorStereoLink(true));
    QCOMPARE(device.state().routing().monitor.stereoLinked.value, std::optional(true));

    QVERIFY(device.storeAppSnapshot(QStringLiteral("Soundcheck"),
                                    flow8::model::SnapshotScope::Routing));
    QCOMPARE(device.state().snapshots().size(), 16);
    QCOMPARE(device.state().snapshots().last().storage,
             flow8::model::SnapshotStorage::AppLibrary);
    QVERIFY(device.state().snapshots().last().name.source.contains(QStringLiteral("SYNTHETIC")));
    QVERIFY(device.renameAppSnapshot(0, QStringLiteral("Show")));
    QCOMPARE(device.state().snapshots().last().name.value,
             std::optional(QStringLiteral("Show")));
    QVERIFY(device.setUsbMode(flow8::model::UsbMode::Streaming));
    QVERIFY(device.setFxOutputRouteEnabled(
        1, flow8::model::FxOutputDestination::Monitor1, false));
    QVERIFY(device.loadAppSnapshot(0));
    QCOMPARE(device.state().activeSnapshotIndex(), 15);
    QCOMPARE(device.state().routing().usb.mode.value,
             std::optional(flow8::model::UsbMode::Recording));
    QCOMPARE(device.state().routing().usb.input56Source.value,
             std::optional(flow8::model::UsbPlaybackAssignment::UsbReturn));
    QCOMPARE(device.state().routing().headphones.source.value,
             std::optional(flow8::model::HeadphoneSource::Monitor));
    QCOMPARE(device.state().physicalOutput(flow8::model::PhysicalOutputId::MainOut)
                 ->padMinus10Dbv->value, std::optional(true));
    QCOMPARE(device.state().routing().fxOutputRoute(
                 1, flow8::model::FxOutputDestination::Monitor1)->enabled.value,
             std::optional(true));
    QVERIFY(device.deleteAppSnapshot(0));
    QCOMPARE(device.state().snapshots().size(), 15);

    QVERIFY(device.configureAssistedSetup(
        flow8::model::InputId::Input1,
        flow8::model::AssistedSourceType::CondenserMicrophone));
    QVERIFY(device.applyAssistedSetup());
    QVERIFY(device.state().assistedSetup().applied);
    QVERIFY(device.startEzGain({flow8::model::InputId::Input1,
                               flow8::model::InputId::Input2}));
    QVERIFY(device.state().ezGainSession().running);
    QTRY_VERIFY(!device.state().ezGainSession().running);
    QCOMPARE(device.state().ezGainSession().results.size(), 2);
    // Advanced simulator controls are functional-model changes, not guessed BLE writes.
    QCOMPARE(transportPointer->sentPackets().size(), 0);
    QTRY_VERIFY(device.state().inputMeter(0)->level.value.value_or(0.0) > 0.0);
    QCOMPARE(device.state().inputMeter(0)->level.evidence,
             flow8::model::EvidenceStatus::Synthetic);
}

void Flow8DeviceTest::mixBusesAndFxRoutingRemainIndependent()
{
    flow8::Flow8Device device;
    auto transport = std::make_unique<flow8::simulator::FakeTransport>();
    device.setTransport(std::move(transport));
    device.connectDevice();
    QTRY_COMPARE(device.state().connectionState(), flow8::ConnectionState::Ready);

    const double originalMon2Send =
        *device.state().channel(0)->monitorSends[1].levelDb.value;
    const double originalFx2Send =
        *device.state().channel(0)->fxSendLevelDb[1].value;
    QVERIFY(device.setChannelSendLevel(0, 0, 0.75));
    QVERIFY(device.state().channel(0)->monitorSends[0].levelDb.value.has_value());
    QCOMPARE(*device.state().channel(0)->monitorSends[1].levelDb.value, originalMon2Send);
    QVERIFY(device.setChannelSendLevel(0, 2, 0.60));
    QVERIFY(device.state().channel(0)->fxSendLevelDb[0].value.has_value());
    QCOMPARE(*device.state().channel(0)->fxSendLevelDb[1].value, originalFx2Send);
    const double fx1 = device.state().routeLevel(
        0, flow8::model::RoutingDestination::Fx1)->effectiveValue();
    const double fx2 = device.state().routeLevel(
        0, flow8::model::RoutingDestination::Fx2)->effectiveValue();
    QVERIFY(fx1 != fx2);

    const double originalMainMaster = *device.state().bus(0)->fader.value;
    const double originalMon2Master = *device.state().bus(2)->fader.value;
    QVERIFY(device.setDestinationMaster(
        flow8::model::RoutingDestination::Monitor1, 0.22));
    QCOMPARE(device.state().bus(1)->fader.value, std::optional(0.22));
    QCOMPARE(*device.state().bus(0)->fader.value, originalMainMaster);
    QCOMPARE(*device.state().bus(2)->fader.value, originalMon2Master);
    QVERIFY(device.setDestinationMaster(
        flow8::model::RoutingDestination::Fx1, 0.31));
    QCOMPARE(device.state().effects().at(0).master.value, std::optional(0.31));
    QVERIFY(device.state().effects().at(1).master.value != std::optional(0.31));

    QVERIFY(device.setFxOutputRouteEnabled(
        1, flow8::model::FxOutputDestination::Monitor2, false));
    QCOMPARE(device.state().routing().fxOutputRoute(
                 0, flow8::model::FxOutputDestination::Monitor2)->enabled.value,
             std::optional(false));
    QCOMPARE(device.state().routing().fxOutputRoute(
                 1, flow8::model::FxOutputDestination::Monitor2)->enabled.value,
             std::optional(false));
    QCOMPARE(device.state().routing().fxOutputRoute(
                 1, flow8::model::FxOutputDestination::Monitor1)->enabled.value,
             std::optional(false));

    QVERIFY(device.setMonitorStereoLink(true));
    QCOMPARE(device.state().routing().monitor.stereoLinked.value, std::optional(true));
    QCOMPARE(device.state().bus(1)->fader.value, std::optional(0.22));
    QCOMPARE(*device.state().bus(2)->fader.value, originalMon2Master);
}

void Flow8DeviceTest::usbAndPhysicalOutputRoutingRemainIndependent()
{
    flow8::Flow8Device device;
    device.setTransport(std::make_unique<flow8::simulator::FakeTransport>());
    device.connectDevice();
    QTRY_COMPARE(device.state().connectionState(), flow8::ConnectionState::Ready);

    const double mainMaster = device.state().bus(0)->fader.value.value_or(0.0);
    const double monMaster = device.state().bus(1)->fader.value.value_or(0.0);
    const double mainRoute = device.state().routeLevel(
        0, flow8::model::RoutingDestination::Main)->effectiveValue();

    QVERIFY(device.setUsbPlaybackAssignment(
        0, flow8::model::UsbPlaybackAssignment::UsbReturn));
    QVERIFY(device.setMonitorRouteSource(
        1, flow8::model::MonitorRouteSource::UsbReturn34));
    QVERIFY(device.setHeadphoneSource(flow8::model::HeadphoneSource::Monitor));
    QVERIFY(device.setHeadphoneTapPoint(flow8::model::RoutingTapPoint::PreFader));

    QCOMPARE(device.state().routing().usb.input56Source.value,
             std::optional(flow8::model::UsbPlaybackAssignment::UsbReturn));
    QCOMPARE(device.state().routing().monitor.outputSources[1].value,
             std::optional(flow8::model::MonitorRouteSource::UsbReturn34));
    QCOMPARE(device.state().routing().headphones.source.value,
             std::optional(flow8::model::HeadphoneSource::Monitor));
    QCOMPARE(device.state().bus(0)->fader.value, std::optional(mainMaster));
    QCOMPARE(device.state().bus(1)->fader.value, std::optional(monMaster));
    QCOMPARE(device.state().routeLevel(
                 0, flow8::model::RoutingDestination::Main)->effectiveValue(), mainRoute);
}

void Flow8DeviceTest::realTransportDoesNotClaimReadyBeforeHandshake()
{
    flow8::Flow8Device device;
    device.setTransport(std::make_unique<NonSimulatorTransport>());
    device.connectDevice();

    QCOMPARE(device.state().connectionState(), flow8::ConnectionState::Connected);
    QVERIFY(!device.isControlAvailable(flow8::Flow8Device::Control::ChannelFader));
    QVERIFY(!device.setChannelFader(0, 0.5));
}

QTEST_GUILESS_MAIN(Flow8DeviceTest)

#include "flow8_device_test.moc"
