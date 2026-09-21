#include "core/flow8_device.h"
#include "simulator/fake_transport.h"
#include "ui/language_manager.h"
#include "ui/main_window.h"
#include "ui/settings/settings_dialog.h"
#include "ui/ui_text.h"

#include <QAbstractButton>
#include <QComboBox>
#include <QLabel>
#include <QPushButton>
#include <QSettings>
#include <QTest>
#include <QTemporaryDir>

#include <array>
#include <memory>

class I18nTest final : public QObject {
    Q_OBJECT

private slots:
    void translatorsLoadAndCriticalTextIsComplete();
    void mainWindowRetranslatesAtRuntime();
    void settingsDialogSwitchesLanguage();
    void visibleKeyControlsFitBothLanguages_data();
    void visibleKeyControlsFitBothLanguages();
};

void I18nTest::translatorsLoadAndCriticalTextIsComplete()
{
    flow8::ui::LanguageManager manager;
    const std::array languages {
        flow8::ui::UiLanguage::English,
        flow8::ui::UiLanguage::SimplifiedChinese,
    };
    const std::array<const char*, 20> critical {
        "Connection", "Disconnected", "Mixer", "Channel", "Main", "Monitor 1",
        "Compressor", "Snapshot", "Routing", "Settings", "Unknown",
        "Needs Hardware Verification", "Stage View", "Preferences", "Assisted Setup",
        "USB Recording", "Pre-Fader", "Post-Fader", "Start New", "EZ-GAIN ready",
    };
    for (const auto language : languages) {
        QVERIFY2(manager.setLanguage(language, false), "compiled translator did not load");
        for (const char* source : critical) {
            const QString translated = flow8::ui::uiText(source);
            QVERIFY2(!translated.trimmed().isEmpty(), source);
            QVERIFY2(!translated.contains(QChar::ReplacementCharacter), source);
        }
    }

    QString rejection;
    flow8::Flow8Device device;
    connect(&device, &flow8::Flow8Device::controlRejected, this,
            [&rejection](flow8::Flow8Device::Control, const QString& reason) {
                rejection = reason;
            });
    QVERIFY(!device.setChannelFader(0, 0.5));
    QCOMPARE(rejection, QString::fromUtf8("通道推子不可用。"));
}

void I18nTest::settingsDialogSwitchesLanguage()
{
    QTemporaryDir settingsDirectory;
    QVERIFY(settingsDirectory.isValid());
    QSettings::setDefaultFormat(QSettings::IniFormat);
    QSettings::setPath(QSettings::IniFormat, QSettings::UserScope, settingsDirectory.path());
    QCoreApplication::setOrganizationName(QStringLiteral("Flow8I18nTest"));
    QCoreApplication::setApplicationName(QStringLiteral("Flow8I18nTest"));

    flow8::ui::LanguageManager manager;
    QVERIFY(manager.setLanguage(flow8::ui::UiLanguage::English, false));
    flow8::Flow8Device device;
    flow8::ui::SettingsDialog dialog(device, manager);
    dialog.show();
    auto* selector = dialog.findChild<QComboBox*>(QStringLiteral("languageSelector"));
    QVERIFY(selector != nullptr);
    QCOMPARE(selector->count(), 2);
    selector->setCurrentIndex(selector->findData(
        static_cast<int>(flow8::ui::UiLanguage::SimplifiedChinese)));
    QCoreApplication::processEvents();
    QCOMPARE(manager.language(), flow8::ui::UiLanguage::SimplifiedChinese);
    QCOMPARE(dialog.windowTitle(), QString::fromUtf8("FLOW 8 偏好设置"));
    QCOMPARE(QSettings().value(QStringLiteral("ui/language")).toString(),
             QStringLiteral("zh_CN"));
}

void I18nTest::mainWindowRetranslatesAtRuntime()
{
    flow8::ui::LanguageManager manager;
    QVERIFY(manager.setLanguage(flow8::ui::UiLanguage::English, false));
    flow8::Flow8Device device;
    auto transport = std::make_unique<flow8::simulator::FakeTransport>();
    transport->setRemoteChangesEnabled(false);
    device.setTransport(std::move(transport));
    flow8::ui::MainWindow window(device, manager);
    window.show();
    QCoreApplication::processEvents();

    auto* connect = window.findChild<QPushButton*>(QStringLiteral("connectButton"));
    QVERIFY(connect != nullptr);
    QCOMPARE(connect->text(), QStringLiteral("Connect"));
    QTest::mouseClick(connect, Qt::LeftButton);
    QTRY_COMPARE(device.state().connectionState(), flow8::ConnectionState::Ready);
    auto* inputOne = window.findChild<QLabel*>(QStringLiteral("channelName0"));
    QVERIFY(inputOne != nullptr);

    QVERIFY(manager.setLanguage(flow8::ui::UiLanguage::SimplifiedChinese, false));
    QCoreApplication::processEvents();
    QCOMPARE(connect->text(), QString::fromUtf8("断开"));
    QCOMPARE(inputOne->text(), QString::fromUtf8("输入 1"));
    QVERIFY(!window.windowTitle().contains(QChar::ReplacementCharacter));

    QVERIFY(manager.setLanguage(flow8::ui::UiLanguage::English, false));
    QCoreApplication::processEvents();
    QCOMPARE(connect->text(), QStringLiteral("Disconnect"));
    QCOMPARE(inputOne->text(), QStringLiteral("Input 1"));
}

void I18nTest::visibleKeyControlsFitBothLanguages_data()
{
    QTest::addColumn<int>("language");
    QTest::newRow("en-US") << static_cast<int>(flow8::ui::UiLanguage::English);
    QTest::newRow("zh-CN") << static_cast<int>(flow8::ui::UiLanguage::SimplifiedChinese);
}

void I18nTest::visibleKeyControlsFitBothLanguages()
{
    QFETCH(int, language);
    flow8::ui::LanguageManager manager;
    QVERIFY(manager.setLanguage(static_cast<flow8::ui::UiLanguage>(language), false));
    flow8::Flow8Device device;
    device.setTransport(std::make_unique<flow8::simulator::FakeTransport>());
    flow8::ui::MainWindow window(device, manager);
    window.resize(1440, 920);
    window.show();
    QCoreApplication::processEvents();

    const QStringList names {
        QStringLiteral("connectButton"), QStringLiteral("settingsButton"),
        QStringLiteral("navigation0"), QStringLiteral("navigation1"),
        QStringLiteral("navigation7"), QStringLiteral("navigation8"),
    };
    for (const QString& name : names) {
        auto* control = window.findChild<QWidget*>(name);
        QVERIFY2(control != nullptr, qPrintable(name));
        QVERIFY2(control->width() >= control->minimumSizeHint().width(), qPrintable(name));
    }
}

QTEST_MAIN(I18nTest)
#include "i18n_test.moc"
