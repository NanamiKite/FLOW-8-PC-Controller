#include "core/flow8_device.h"
#include "simulator/fake_transport.h"
#include "ui/language_manager.h"
#include "ui/main_window.h"

#include <QApplication>

#include <memory>

#ifndef FLOW8_APP_VERSION
#define FLOW8_APP_VERSION "development"
#endif

int main(int argc, char* argv[])
{
    QApplication application(argc, argv);
    QApplication::setApplicationName(QStringLiteral("FLOW 8 PC Controller"));
    QApplication::setOrganizationName(QStringLiteral("FLOW 8 PC Controller"));
    QApplication::setApplicationVersion(QStringLiteral(FLOW8_APP_VERSION));

    flow8::ui::LanguageManager languageManager;
    if (!languageManager.initialize()) {
        qWarning("FLOW 8 UI translations could not be loaded; using source text");
    }

    flow8::Flow8Device device;
    device.setTransport(std::make_unique<flow8::simulator::FakeTransport>());
    flow8::ui::MainWindow window(device, languageManager);
    window.show();

    return application.exec();
}
