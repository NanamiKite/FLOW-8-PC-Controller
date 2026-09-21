#include "core/semantic_command_queue.h"

#include <QTest>

class SemanticCommandQueueTest final : public QObject {
    Q_OBJECT

private slots:
    void coalescesOnlyMatchingRouteFaders();
    void representsDestinationMasterWithEqualEndpoints();
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
    QCOMPARE(first->sourceEndpoint, flow8::model::EndpointId::Input1);
    QCOMPARE(first->destinationEndpoint, flow8::model::EndpointId::MainLr);
    QCOMPARE(first->normalized, 0.9);
    QVERIFY(!queue.beginNext().has_value());
    QVERIFY(queue.completeInFlight());
}

void SemanticCommandQueueTest::representsDestinationMasterWithEqualEndpoints()
{
    flow8::SemanticCommandQueue queue;
    QVERIFY(queue.enqueueDestinationMaster(
        flow8::model::RoutingDestination::Monitor1, 0.52));
    const auto command = queue.beginNext();
    QVERIFY(command.has_value());
    QCOMPARE(command->sourceEndpoint, flow8::model::EndpointId::Monitor1);
    QCOMPARE(command->destinationEndpoint, flow8::model::EndpointId::Monitor1);
    QVERIFY(command->isDestinationMaster());
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
