#pragma once

#include <QObject>
#include <QString>

#include <memory>

class QTranslator;

namespace flow8::ui {

enum class UiLanguage {
    English,
    SimplifiedChinese,
};

class LanguageManager final : public QObject {
    Q_OBJECT

public:
    explicit LanguageManager(QObject* parent = nullptr);
    ~LanguageManager() override;

    [[nodiscard]] bool initialize();
    [[nodiscard]] bool setLanguage(UiLanguage language, bool persist = true);
    [[nodiscard]] UiLanguage language() const noexcept;

    [[nodiscard]] static UiLanguage systemLanguage();
    [[nodiscard]] static QString localeName(UiLanguage language);

signals:
    void languageChanged(flow8::ui::UiLanguage language);

private:
    [[nodiscard]] QString translationPath(UiLanguage language) const;

    UiLanguage language_ {UiLanguage::English};
    std::unique_ptr<QTranslator> translator_;
};

} // namespace flow8::ui

Q_DECLARE_METATYPE(flow8::ui::UiLanguage)
