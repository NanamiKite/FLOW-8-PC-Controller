#include "core/semantic_command_queue.h"

#include <QTest>

class SemanticCommandQueueTest final : public QObject {
    Q_OBJECT

private slots:
    void coalescesOnlyMatchingRouteFaders();
    void preservesDiscreteActionsAndOneInFlightRule();
};

void SemanticCommandQueueTest::coalescesOnlyMatchingRouteFaders()
{
    flow8::SemanticCommandQueue queue;
    using Destination = flow8::model::RoutingDestination;
    QVERIFY(queue.enqueueRouteLevel(0, Destination::Main, 0.1));
    QVERIFY(queue.enqueueRouteLevel(0, Destination::Main, 0.2));
    QVERIFY(queue.enqueueRouteLevel(0, Destination::Main, 0.9));
    QVERIFY(queue.enqueueRouteLevel(0, Destination::Monitor1, 0.4));
    QVERIFY(queue.enqueueRouteLevel(1, Destination::Main, 0.6));
    QCOMPARE(queue.pendingCount(), 3);

    const auto first = queue.beginNext();
    QVERIFY(first.has_value());
    QCOMPARE(first->sourceIndex, 0);
    QCOMPARE(first->destination, Destination::Main);
    QCOMPARE(first->normalized, 0.9);
    QVERIFY(!queue.beginNext().has_value());
    QVERIFY(queue.completeInFlight());
}

void SemanticCommandQueueTest::preservesDiscreteActionsAndOneInFlightRule()
{
    flow8::SemanticCommandQueue queue;
    QVERIFY(queue.enqueueDiscrete(QStringLiteral("mute:0:on")));
    QVERIFY(queue.enqueueDiscrete(QStringLiteral("mute:0:off")));
    QVERIFY(queue.enqueueDiscrete(QStringLiteral("snapshot:load:3")));
    QCOMPARE(queue.pendingCount(), 3);
    QCOMPARE(queue.beginNext()->action, QStringLiteral("mute:0:on"));
    QVERIFY(!queue.beginNext().has_value());
    QVERIFY(queue.completeInFlight());
    QCOMPARE(queue.beginNext()->action, QStringLiteral("mute:0:off"));
    QVERIFY(queue.completeInFlight());
    QCOMPARE(queue.beginNext()->action, QStringLiteral("snapshot:load:3"));
    QVERIFY(queue.completeInFlight());
    QVERIFY(!queue.completeInFlight());
}

QTEST_GUILESS_MAIN(SemanticCommandQueueTest)

#include "semantic_command_queue_test.moc"
