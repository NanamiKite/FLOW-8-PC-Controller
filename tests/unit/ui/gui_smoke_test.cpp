#include "core/flow8_device.h"
#include "simulator/fake_transport.h"
#include "ui/main_window.h"
#include "ui/mixer/channel_strip.h"

#include <QPushButton>
#include <QSlider>
#include <QTest>

#include <cmath>
#include <memory>

class GuiSmokeTest final : public QObject {
    Q_OBJECT

private slots:
    void simulatorConnectsAndBuildsMixer();
};

void GuiSmokeTest::simulatorConnectsAndBuildsMixer()
{
    flow8::Flow8Device device;
    auto transport = std::make_unique<flow8::simulator::FakeTransport>();
    transport->setRemoteChangesEnabled(false);
    device.setTransport(std::move(transport));
    flow8::ui::MainWindow window(device);
    window.show();

    auto* connectButton = window.findChild<QPushButton*>(QStringLiteral("connectButton"));
    QVERIFY(connectButton != nullptr);
    QTest::mouseClick(connectButton, Qt::LeftButton);
    QTRY_COMPARE(device.state().connectionState(), flow8::ConnectionState::Ready);
    QCOMPARE(device.state().channels().size(), 7);
    QCOMPARE(window.findChildren<flow8::ui::ChannelStrip*>().size(), 7);

    auto* fader = window.findChild<QSlider*>(QStringLiteral("fader0"));
    QVERIFY(fader != nullptr);
    fader->setSliderPosition(830);
    QTRY_VERIFY(device.state().channel(0)->fader.value.has_value());
    QTRY_VERIFY(std::abs(*device.state().channel(0)->fader.value - 0.83) <= (1.0 / 255.0));
}

QTEST_MAIN(GuiSmokeTest)
#include "gui_smoke_test.moc"
