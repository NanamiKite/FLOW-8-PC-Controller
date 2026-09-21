#include "ui/language_manager.h"

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QLocale>
#include <QSettings>
#include <QTranslator>
#include <QStringList>

namespace flow8::ui {
namespace {

constexpr auto settingsKey = "ui/language";

QString translationFile(const UiLanguage language)
{
    return language == UiLanguage::SimplifiedChinese
        ? QStringLiteral("flow8_zh_CN.qm") : QStringLiteral("flow8_en_US.qm");
}

} // namespace

LanguageManager::LanguageManager(QObject* parent)
    : QObject(parent)
{
}

LanguageManager::~LanguageManager()
{
    if (translator_ && QCoreApplication::instance() != nullptr) {
        QCoreApplication::removeTranslator(translator_.get());
    }
}

bool LanguageManager::initialize()
{
    const QString configured = QSettings().value(QString::fromLatin1(settingsKey)).toString();
    UiLanguage initial = systemLanguage();
    if (configured == QStringLiteral("en_US")) {
        initial = UiLanguage::English;
    } else if (configured == QStringLiteral("zh_CN")) {
        initial = UiLanguage::SimplifiedChinese;
    }
    return setLanguage(initial, false);
}

bool LanguageManager::setLanguage(const UiLanguage language, const bool persist)
{
    auto replacement = std::make_unique<QTranslator>();
    const QString path = translationPath(language);
    if (path.isEmpty() || !replacement->load(path)) {
        return false;
    }

    if (translator_) {
        QCoreApplication::removeTranslator(translator_.get());
    }
    translator_ = std::move(replacement);
    QCoreApplication::installTranslator(translator_.get());
    language_ = language;
    QLocale::setDefault(QLocale(localeName(language)));
    if (persist) {
        QSettings().setValue(QString::fromLatin1(settingsKey), localeName(language));
    }
    emit languageChanged(language_);
    return true;
}

UiLanguage LanguageManager::language() const noexcept
{
    return language_;
}

UiLanguage LanguageManager::systemLanguage()
{
    return QLocale::system().language() == QLocale::Chinese
        ? UiLanguage::SimplifiedChinese : UiLanguage::English;
}

QString LanguageManager::localeName(const UiLanguage language)
{
    return language == UiLanguage::SimplifiedChinese
        ? QStringLiteral("zh_CN") : QStringLiteral("en_US");
}

QString LanguageManager::translationPath(const UiLanguage language) const
{
    const QString fileName = translationFile(language);
    const QStringList directories {
        QStringLiteral(FLOW8_TRANSLATIONS_DIR),
        QCoreApplication::applicationDirPath() + QStringLiteral("/translations"),
    };
    for (const QString& directory : directories) {
        const QString candidate = QDir(directory).filePath(fileName);
        if (QFileInfo::exists(candidate)) {
            return candidate;
        }
    }
    return {};
}

} // namespace flow8::ui
