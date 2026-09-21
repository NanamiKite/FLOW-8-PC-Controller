#include "core/semantic_command_queue.h"
#include "protocol/gain_codec.h"
#include "protocol/packet.h"
#include "protocol/route_level_codec.h"
#include "simulator/fake_transport.h"

#include <QTest>

#include <limits>

class SemanticCommandQueueTest final : public QObject {
    Q_OBJECT

private slots:
    void coalescesOnlyMatchingRouteFaders();
    void coalescesGainByCommandAndInput();
    void representsDestinationMasterWithEqualEndpoints();
    void writesOnlySuccessfulCodecPacketsToTransport();
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
    const auto codecCommand = first->routeLevelCommand();
    QVERIFY(codecCommand.has_value());
    QCOMPARE(codecCommand->semanticEvidence,
             flow8::model::EvidenceStatus::VerifiedFromApk);
    const auto encoded = first->encodePacket();
    QVERIFY(encoded.has_value());
    const auto* routeResult = std::get_if<flow8::protocol::RouteLevelEncodeResult>(
        &*encoded);
    QVERIFY(routeResult != nullptr);
    QVERIFY(routeResult->ok());
    QCOMPARE(routeResult->error, flow8::protocol::RouteLevelCodecError::None);
    const auto parsed = flow8::protocol::parsePacket(*routeResult->packet);
    QVERIFY(parsed.ok());
    QCOMPARE(parsed.packet->payload.first(2), QByteArray::fromHex("000f"));
    QVERIFY(!queue.beginNext().has_value());
    QVERIFY(queue.completeInFlight());
}

void SemanticCommandQueueTest::coalescesGainByCommandAndInput()
{
    flow8::SemanticCommandQueue queue;
    QVERIFY(queue.enqueueGain(0, -20.0));
    QVERIFY(queue.enqueueGain(0, 0.0));
    QVERIFY(queue.enqueueGain(0, 20.0));
    QVERIFY(queue.enqueueRouteLevel(
        0, flow8::model::RoutingDestination::Main, 0.5));
    QVERIFY(queue.enqueueGain(1, 10.0));
    QCOMPARE(queue.pendingCount(), 3);

    const auto first = queue.beginNext();
    QVERIFY(first.has_value());
    QCOMPARE(first->kind, flow8::SemanticCommandQueue::Kind::Gain);
    QCOMPARE(first->sourceEndpoint, flow8::model::EndpointId::Input1);
    QCOMPARE(first->gainDb, 20.0);
    const auto gainCommand = first->gainCommand();
    QVERIFY(gainCommand.has_value());
    QCOMPARE(gainCommand->gainDb, 20.0);

    const auto encoded = first->encodePacket();
    QVERIFY(encoded.has_value());
    const auto* gainResult = std::get_if<flow8::protocol::GainEncodeResult>(&*encoded);
    QVERIFY(gainResult != nullptr);
    QVERIFY(gainResult->ok());
    QCOMPARE(*gainResult->packet, QByteArray::fromHex("020100a0a3"));
    QVERIFY(queue.completeInFlight());

    const auto route = queue.beginNext();
    QVERIFY(route.has_value());
    QCOMPARE(route->kind, flow8::SemanticCommandQueue::Kind::RouteLevel);
    QVERIFY(queue.completeInFlight());

    const auto secondGain = queue.beginNext();
    QVERIFY(secondGain.has_value());
    QCOMPARE(secondGain->kind, flow8::SemanticCommandQueue::Kind::Gain);
    QCOMPARE(secondGain->sourceEndpoint, flow8::model::EndpointId::Input2);
    QCOMPARE(secondGain->gainDb, 10.0);
    QVERIFY(queue.completeInFlight());

    QVERIFY(!queue.enqueueGain(6, 0.0)); // BT/USB has no Gain capability.
    QVERIFY(!queue.enqueueGain(7, 0.0));
    QVERIFY(!queue.enqueueGain(0, -20.1));
    QVERIFY(!queue.enqueueGain(0, 60.1));
    QVERIFY(!queue.enqueueGain(0, std::numeric_limits<double>::infinity()));
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
    const auto codecCommand = command->routeLevelCommand();
    QVERIFY(codecCommand.has_value());
    QVERIFY(codecCommand->isDestinationMaster());
    const auto encoded = command->encodePacket();
    QVERIFY(encoded.has_value());
    const auto* routeResult = std::get_if<flow8::protocol::RouteLevelEncodeResult>(
        &*encoded);
    QVERIFY(routeResult != nullptr);
    QVERIFY(routeResult->ok());
    const auto parsed = flow8::protocol::parsePacket(*routeResult->packet);
    QVERIFY(parsed.ok());
    QCOMPARE(parsed.packet->payload.first(2), QByteArray::fromHex("0a0a"));
}

void SemanticCommandQueueTest::writesOnlySuccessfulCodecPacketsToTransport()
{
    flow8::simulator::FakeTransport transport;
    transport.connectTransport();
    QTRY_COMPARE(transport.state(), flow8::Flow8Transport::State::Connected);

    flow8::SemanticCommandQueue queue;
    QVERIFY(queue.enqueueRouteLevel(
        0, flow8::model::RoutingDestination::Main, 0.5));
    const auto command = queue.beginNext();
    QVERIFY(command.has_value());
    const auto encoded = command->encodePacket();
    QVERIFY(encoded.has_value());
    const auto* routeResult = std::get_if<flow8::protocol::RouteLevelEncodeResult>(
        &*encoded);
    QVERIFY(routeResult != nullptr);
    QVERIFY(routeResult->ok());
    QVERIFY(transport.send(*routeResult->packet));
    QCOMPARE(transport.sentPackets().size(), 1);
    QCOMPARE(transport.sentPackets().constFirst(), QByteArray::fromHex("0601000f7f95"));
    QVERIFY(queue.completeInFlight());

    QVERIFY(queue.enqueueGain(0, 0.0));
    const auto gain = queue.beginNext();
    QVERIFY(gain.has_value());
    const auto gainEncoded = gain->encodePacket();
    QVERIFY(gainEncoded.has_value());
    const auto* gainResult = std::get_if<flow8::protocol::GainEncodeResult>(
        &*gainEncoded);
    QVERIFY(gainResult != nullptr);
    QVERIFY(gainResult->ok());
    QVERIFY(transport.send(*gainResult->packet));
    QCOMPARE(transport.sentPackets().size(), 2);
    QCOMPARE(transport.sentPackets().at(1), QByteArray::fromHex("020100787b"));
    QVERIFY(queue.completeInFlight());

    QVERIFY(queue.enqueueDiscrete(QStringLiteral("mute:0:on")));
    const auto discrete = queue.beginNext();
    QVERIFY(discrete.has_value());
    QVERIFY(!discrete->encodePacket().has_value());
    QCOMPARE(transport.sentPackets().size(), 2);
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
    QVERIFY(!queue.inFlight()->routeLevelCommand().has_value());
    QVERIFY(!queue.inFlight()->encodePacket().has_value());
    QVERIFY(queue.completeInFlight());
    QVERIFY(!queue.completeInFlight());
}

QTEST_GUILESS_MAIN(SemanticCommandQueueTest)

#include "semantic_command_queue_test.moc"
