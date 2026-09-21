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
    void realTransportDoesNotClaimReadyBeforeHandshake();
};

void Flow8DeviceTest::simulatorConnectsAndCreatesProfile()
{
    flow8::Flow8Device device;
    device.setTransport(std::make_unique<flow8::simulator::FakeTransport>());
    device.connectDevice();

    QTRY_COMPARE(device.state().connectionState(), flow8::ConnectionState::Ready);
    QCOMPARE(device.state().channels().size(), 7);
    QVERIFY(device.isControlAvailable(flow8::Flow8Device::Control::ChannelFader));
    QVERIFY(device.state().channel(0)->name.value.has_value());
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
    QCOMPARE(transportPointer->sentPackets().size(), 1);

    QVERIFY(device.setChannelMuted(0, true));
    QCOMPARE(*device.state().channel(0)->muted.value, true);
    QVERIFY(device.setChannelSoloed(0, true));
    QCOMPARE(*device.state().channel(0)->soloed.value, true);
    QVERIFY(device.setChannelPan(0, -0.5));
    QCOMPARE(*device.state().channel(0)->pan.value, -0.5);
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
