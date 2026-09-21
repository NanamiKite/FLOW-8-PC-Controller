#include "core/flow8_state.h"

#include <QTest>

class Flow8StateTest final : public QObject {
    Q_OBJECT

private slots:
    void startsDisconnected();
    void storesConnectionState();
    void rejectsInvalidChannelValues();
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

QTEST_GUILESS_MAIN(Flow8StateTest)

#include "flow8_state_test.moc"
