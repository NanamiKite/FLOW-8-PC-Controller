#include "core/flow8_state.h"
#include "model/flow8_capabilities.h"

#include <QTest>

class Flow8StateTest final : public QObject {
    Q_OBJECT

private slots:
    void startsDisconnected();
    void storesConnectionState();
    void rejectsInvalidChannelValues();
    void preservesStrongerEvidence();
    void keepsMixBusesAndFxRoutesIndependent();
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
    QCOMPARE(state.routing().monitorLink.stereoLinked.value, std::optional(true));
    QCOMPARE(state.bus(1)->fader.value, std::optional(0.40));
    QCOMPARE(state.bus(2)->fader.value, std::optional(0.20));
}

QTEST_GUILESS_MAIN(Flow8StateTest)

#include "flow8_state_test.moc"
