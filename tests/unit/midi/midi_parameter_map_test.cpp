#include "midi/midi_parameter_map.h"

#include <QTest>

class MidiParameterMapTest final : public QObject {
    Q_OBJECT

private slots:
    void mapsOfficialChannels();
    void enforcesInputExceptions();
    void mapsBusFxAndGlobalControls();
};

void MidiParameterMapTest::mapsOfficialChannels()
{
    using flow8::midi::MidiParameterMap;
    QCOMPARE(MidiParameterMap::channelForInput(flow8::model::InputId::Input1), 0);
    QCOMPARE(MidiParameterMap::channelForInput(flow8::model::InputId::Input56), 4);
    QCOMPARE(MidiParameterMap::channelForInput(flow8::model::InputId::UsbBluetooth), 6);
    QCOMPARE(MidiParameterMap::channelForBus(flow8::model::BusId::Main), 7);
    QCOMPARE(MidiParameterMap::channelForBus(flow8::model::BusId::Monitor2), 9);
    QCOMPARE(MidiParameterMap::channelForBus(flow8::model::BusId::Fx2), 11);
    QCOMPARE(MidiParameterMap::channelForFxEngine(0), 13);
    QCOMPARE(MidiParameterMap::channelForFxEngine(1), 14);
    QCOMPARE(MidiParameterMap::globalChannel(), 15);
}

void MidiParameterMapTest::enforcesInputExceptions()
{
    using enum flow8::midi::Parameter;
    using flow8::midi::MidiParameterMap;
    const auto level = MidiParameterMap::input(flow8::model::InputId::UsbBluetooth, Level);
    QVERIFY(level.has_value());
    QCOMPARE(level->number, 7);
    QVERIFY(level->source.startsWith(QStringLiteral("OfficialManual")));
    QVERIFY(!MidiParameterMap::input(flow8::model::InputId::UsbBluetooth, Gain).has_value());
    QVERIFY(!MidiParameterMap::input(flow8::model::InputId::UsbBluetooth, Compressor).has_value());
    QVERIFY(MidiParameterMap::input(flow8::model::InputId::Input2, Phantom48V).has_value());
    QVERIFY(!MidiParameterMap::input(flow8::model::InputId::Input3, Phantom48V).has_value());
}

void MidiParameterMapTest::mapsBusFxAndGlobalControls()
{
    using enum flow8::midi::Parameter;
    using flow8::midi::MessageType;
    using flow8::midi::MidiParameterMap;
    const auto mainBalance = MidiParameterMap::bus(flow8::model::BusId::Main, Balance);
    QVERIFY(mainBalance.has_value());
    QCOMPARE(mainBalance->number, 10);
    QVERIFY(!MidiParameterMap::bus(flow8::model::BusId::Monitor1, Balance).has_value());
    QVERIFY(MidiParameterMap::bus(flow8::model::BusId::Monitor1, BusEq1kHz).has_value());
    QVERIFY(!MidiParameterMap::bus(flow8::model::BusId::Fx1, Limiter).has_value());

    const auto preset = MidiParameterMap::fx(1, FxPreset);
    QVERIFY(preset.has_value());
    QCOMPARE(preset->midiChannel, 14);
    QCOMPARE(preset->messageType, MessageType::ProgramChange);
    QCOMPARE(preset->minimum, 1);
    QCOMPARE(preset->maximum, 16);

    const auto snapshot = MidiParameterMap::global(SnapshotRecall);
    QVERIFY(snapshot.has_value());
    QCOMPARE(snapshot->midiChannel, 15);
    QCOMPARE(snapshot->maximum, 15);
    const auto reset = MidiParameterMap::global(MixerReset);
    QVERIFY(reset.has_value());
    QCOMPARE(reset->minimum, 16);
    QCOMPARE(reset->maximum, 16);
    const auto tap = MidiParameterMap::global(TapTempo);
    QVERIFY(tap.has_value());
    QCOMPARE(tap->messageType, MessageType::NoteOn);
    QCOMPARE(tap->number, 0);
}

QTEST_GUILESS_MAIN(MidiParameterMapTest)

#include "midi_parameter_map_test.moc"
