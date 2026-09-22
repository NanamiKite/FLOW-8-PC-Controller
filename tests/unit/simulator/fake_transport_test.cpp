#include "simulator/fake_transport.h"

#include <QSignalSpy>
#include <QTest>

class FakeTransportTest final : public QObject {
    Q_OBJECT

private slots:
    void connectsAndEchoesBytes();
    void rejectsWritesWhileDisconnected();
};

void FakeTransportTest::connectsAndEchoesBytes()
{
    flow8::simulator::FakeTransport transport;
    QSignalSpy receivedSpy(&transport, &flow8::Flow8Transport::bytesReceived);

    transport.connectTransport();
    QTRY_COMPARE(transport.state(), flow8::Flow8Transport::State::Connected);
    const QByteArray payload = QByteArray::fromHex("370138");
    QVERIFY(transport.send(payload));
    QTRY_COMPARE(receivedSpy.count(), 1);
    QCOMPARE(receivedSpy.first().first().toByteArray(), payload);
}

void FakeTransportTest::rejectsWritesWhileDisconnected()
{
    flow8::simulator::FakeTransport transport;
    QVERIFY(!transport.send(QByteArray::fromHex("370138")));
}

QTEST_GUILESS_MAIN(FakeTransportTest)

#include "fake_transport_test.moc"
