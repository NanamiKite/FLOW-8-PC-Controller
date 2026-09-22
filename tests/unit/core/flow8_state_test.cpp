#include "core/flow8_state.h"
#include "model/flow8_capabilities.h"

#include <QTest>
#include <QSignalSpy>

#include <array>

class Flow8StateTest final : public QObject {
    Q_OBJECT

private slots:
    void startsDisconnected();
    void storesConnectionState();
    void rejectsInvalidChannelValues();
    void preservesStrongerEvidence();
    void keepsMixBusesAndFxRoutesIndependent();
    void routeMatrixKeepsDestinationsIndependent();
    void usbAudioRoutingDoesNotMutateMonitorMixes();
    void headphoneRoutingDoesNotMutateMixerState();
    void meterUpdatesStayOutsideControlSignals();
};

void Flow8StateTest::startsDisconnected()
{
    const flow8::Flow8State state;

    QCOMPARE(state.connectionState(), flow8::ConnectionState::Disconnected);
}

void Flow8StateTest::storesConnectionState()
{
    flow8::Flow8State state;

    state.setConnectionState(flow8::ConnectionState::Authenticating);
    QCOMPARE(state.connectionState(), flow8::ConnectionState::Authenticating);

    state.setConnectionState(flow8::ConnectionState::Ready);
    QCOMPARE(state.connectionState(), flow8::ConnectionState::Ready);
}

void Flow8StateTest::rejectsInvalidChannelValues()
{
    flow8::Flow8State state;
    flow8::model::ChannelState channel;
    channel.index = 0;
    state.replaceChannels({channel});

    QVERIFY(!state.setChannelFader(0, -0.1, flow8::model::EvidenceStatus::Unknown, {}));
    QVERIFY(!state.setChannelFader(1, 0.5, flow8::model::EvidenceStatus::Unknown, {}));
    QVERIFY(state.setChannelPan(0, -1.0, flow8::model::EvidenceStatus::Inferred,
                                QStringLiteral("SYNTHETIC test")));
    QVERIFY(!state.setChannelPan(0, 1.1, flow8::model::EvidenceStatus::Unknown, {}));
}

void Flow8StateTest::preservesStrongerEvidence()
{
    flow8::Flow8State state;
    flow8::model::ChannelState channel;
    channel.index = 0;
    state.replaceChannels({channel});

    QVERIFY(state.setChannelPan(0, 0.4, flow8::model::EvidenceStatus::Verified,
                                QStringLiteral("verified test observation")));
    QVERIFY(!state.setChannelPan(0, -0.4, flow8::model::EvidenceStatus::Inferred,
                                 QStringLiteral("reference candidate")));
    QCOMPARE(state.channel(0)->pan.value, std::optional(0.4));
    QCOMPARE(state.channel(0)->pan.evidence, flow8::model::EvidenceStatus::Verified);

    flow8::model::StateValue<int> empty;
    QVERIFY(!flow8::model::mergeObservedValue(empty, 1,
                                               flow8::model::EvidenceStatus::Blocked,
                                               QStringLiteral("NEED_HARDWARE")));
    QVERIFY(!empty.value.has_value());
}

void Flow8StateTest::keepsMixBusesAndFxRoutesIndependent()
{
    flow8::Flow8State state;
    state.replaceBuses(flow8::model::createOfficialBusProfile());
    state.replaceRouting(flow8::model::createRoutingProfile());
    const QString source = QStringLiteral("SYNTHETIC unit test");

    QVERIFY(state.setBusFader(0, 0.70, flow8::model::EvidenceStatus::Unknown, source));
    QVERIFY(state.setBusFader(1, 0.40, flow8::model::EvidenceStatus::Unknown, source));
    QVERIFY(state.setBusFader(2, 0.20, flow8::model::EvidenceStatus::Unknown, source));
    QCOMPARE(state.bus(0)->fader.value, std::optional(0.70));
    QCOMPARE(state.bus(1)->fader.value, std::optional(0.40));
    QCOMPARE(state.bus(2)->fader.value, std::optional(0.20));

    QVERIFY(state.setFxOutputRouteEnabled(
        0, flow8::model::FxOutputDestination::Main, true,
        flow8::model::EvidenceStatus::Unknown, source));
    QVERIFY(state.setFxOutputRouteEnabled(
        1, flow8::model::FxOutputDestination::Monitor2, true,
        flow8::model::EvidenceStatus::Unknown, source));
    QVERIFY(!state.routing().fxOutputRoute(
        0, flow8::model::FxOutputDestination::Monitor2)->enabled.value.has_value());
    QCOMPARE(state.routing().fxOutputRoute(
                 0, flow8::model::FxOutputDestination::Main)->enabled.value,
             std::optional(true));
    QCOMPARE(state.routing().fxOutputRoute(
                 1, flow8::model::FxOutputDestination::Monitor2)->enabled.value,
             std::optional(true));

    QVERIFY(state.setMonitorStereoLink(
        true, flow8::model::EvidenceStatus::Unknown, source));
    QCOMPARE(state.monitorLink().stereoLinked.value, std::optional(true));
    QCOMPARE(state.monitorLink().propagationEvidence,
             flow8::model::EvidenceStatus::Unknown);
    QCOMPARE(state.bus(1)->fader.value, std::optional(0.40));
    QCOMPARE(state.bus(2)->fader.value, std::optional(0.20));
}

void Flow8StateTest::usbAudioRoutingDoesNotMutateMonitorMixes()
{
    flow8::Flow8State state;
    state.replaceBuses(flow8::model::createOfficialBusProfile());
    state.replaceRouting(flow8::model::createRoutingProfile());
    const QString source = QStringLiteral("SYNTHETIC USB audio test");
    QVERIFY(state.setBusFader(1, 0.61, flow8::model::EvidenceStatus::Synthetic, source));
    QVERIFY(state.setBusFader(2, 0.27, flow8::model::EvidenceStatus::Synthetic, source));

    QVERIFY(state.setUsbInputAssignment(
        0, flow8::model::UsbPlaybackAssignment::UsbAudioLoopback,
        flow8::model::EvidenceStatus::Synthetic, source));
    QVERIFY(state.setPhysicalMonitorOutputFeed(
        1, flow8::model::PhysicalMonitorOutputFeed::Usb34,
        flow8::model::EvidenceStatus::Synthetic, source));

    QCOMPARE(state.routing().usbAudio.input56Assignment.value,
             std::optional(flow8::model::UsbPlaybackAssignment::UsbAudioLoopback));
    QCOMPARE(state.routing().usbAudio.monitorOutputFeeds[1].value,
             std::optional(flow8::model::PhysicalMonitorOutputFeed::Usb34));
    QCOMPARE(state.bus(1)->fader.value, std::optional(0.61));
    QCOMPARE(state.bus(2)->fader.value, std::optional(0.27));
}

void Flow8StateTest::headphoneRoutingDoesNotMutateMixerState()
{
    flow8::Flow8State state;
    state.replaceChannels(flow8::model::createOfficialInputProfile());
    state.replaceBuses(flow8::model::createOfficialBusProfile());
    state.replacePhysicalOutputs(flow8::model::createPhysicalOutputProfile());
    const QString source = QStringLiteral("SYNTHETIC physical-output test");
    QVERIFY(state.setBusFader(0, 0.73, flow8::model::EvidenceStatus::Synthetic, source));
    QVERIFY(state.setBusFader(1, 0.41, flow8::model::EvidenceStatus::Synthetic, source));
    QVERIFY(state.setRouteLevel(0, flow8::model::RoutingDestination::Main, 0.62,
                                flow8::model::EvidenceStatus::Synthetic, source));

    QVERIFY(state.setHeadphoneSource(
        flow8::model::HeadphoneSource::Monitor,
        flow8::model::EvidenceStatus::Synthetic, source));
    QVERIFY(state.setHeadphoneTapPoint(
        flow8::model::RoutingTapPoint::PreFader,
        flow8::model::EvidenceStatus::Synthetic, source));

    QCOMPARE(state.routing().headphones.source.value,
             std::optional(flow8::model::HeadphoneSource::Monitor));
    QCOMPARE(state.routing().headphones.tapPoint.value,
             std::optional(flow8::model::RoutingTapPoint::PreFader));
    QCOMPARE(state.bus(0)->fader.value, std::optional(0.73));
    QCOMPARE(state.bus(1)->fader.value, std::optional(0.41));
    QCOMPARE(state.routeLevel(0, flow8::model::RoutingDestination::Main)
                 ->confirmed.value, std::optional(0.62));
}

void Flow8StateTest::routeMatrixKeepsDestinationsIndependent()
{
    flow8::Flow8State state;
    state.replaceChannels(flow8::model::createOfficialInputProfile());
    state.replaceRouting(flow8::model::createRoutingProfile());
    const QString source = QStringLiteral("SYNTHETIC route matrix test");
    constexpr std::array destinations {
        flow8::model::RoutingDestination::Main,
        flow8::model::RoutingDestination::Monitor1,
        flow8::model::RoutingDestination::Monitor2,
        flow8::model::RoutingDestination::Fx1,
        flow8::model::RoutingDestination::Fx2,
    };
    constexpr std::array values {0.91, 0.72, 0.53, 0.34, 0.15};
    for (std::size_t index = 0; index < destinations.size(); ++index) {
        QVERIFY(state.setRouteLevel(0, destinations[index], values[index],
                                    flow8::model::EvidenceStatus::Synthetic, source));
    }
    for (std::size_t index = 0; index < destinations.size(); ++index) {
        const auto* route = state.routeLevel(0, destinations[index]);
        QVERIFY(route != nullptr);
        QCOMPARE(route->confirmed.value, std::optional(values[index]));
        QCOMPARE(route->confirmed.evidence, flow8::model::EvidenceStatus::Synthetic);
    }
    QVERIFY(state.setRouteLevelPending(
        0, flow8::model::RoutingDestination::Monitor1, 0.44));
    QCOMPARE(state.routeLevel(0, flow8::model::RoutingDestination::Monitor1)
                 ->effectiveValue(), 0.44);
    QCOMPARE(state.routeLevel(0, flow8::model::RoutingDestination::Main)
                 ->effectiveValue(), 0.91);
    QVERIFY(state.failRouteLevel(
        0, flow8::model::RoutingDestination::Monitor1,
        QStringLiteral("synthetic rejection")));
    QCOMPARE(state.routeLevel(0, flow8::model::RoutingDestination::Monitor1)
                 ->effectiveValue(), 0.72);
}

void Flow8StateTest::meterUpdatesStayOutsideControlSignals()
{
    flow8::Flow8State state;
    state.replaceChannels(flow8::model::createOfficialInputProfile());
    QSignalSpy channelSpy(&state, &flow8::Flow8State::channelChanged);
    QSignalSpy meterSpy(&state, &flow8::Flow8State::inputMeterChanged);
    QVERIFY(state.setChannelMeter(
        0, 0.4, 0.5, false, flow8::model::EvidenceStatus::Synthetic,
        QStringLiteral("SYNTHETIC meter test")));
    QCOMPARE(channelSpy.size(), 0);
    QCOMPARE(meterSpy.size(), 1);
    QCOMPARE(state.inputMeter(0)->level.value, std::optional(0.4));
}

QTEST_GUILESS_MAIN(Flow8StateTest)

#include "flow8_state_test.moc"
