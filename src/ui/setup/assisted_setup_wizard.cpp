#include "ui/setup/assisted_setup_wizard.h"

#include "core/flow8_device.h"
#include "ui/ui_text.h"

#include <QComboBox>
#include <QEvent>
#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QStackedWidget>
#include <QVBoxLayout>

namespace flow8::ui {
namespace {

QWidget* wizardPage(QStackedWidget* stack, QLabel*& heading, QLabel*& body)
{
    auto* page = new QWidget(stack);
    heading = new QLabel(page);
    heading->setProperty("class", QStringLiteral("inspectorTitle"));
    body = new QLabel(page);
    body->setWordWrap(true);
    body->setProperty("class", QStringLiteral("secondaryText"));
    auto* layout = new QVBoxLayout(page);
    layout->setContentsMargins(20, 18, 20, 18);
    layout->addWidget(heading);
    layout->addWidget(body);
    return page;
}

} // namespace

AssistedSetupWizard::AssistedSetupWizard(Flow8Device& device, QWidget* parent)
    : QDialog(parent)
    , device_(device)
    , pages_(new QStackedWidget(this))
    , input_(new QComboBox(this))
    , sourceType_(new QComboBox(this))
    , recommendation_(new QLabel(this))
    , instruction_(new QLabel(this))
    , evidence_(new QLabel(this))
    , back_(new QPushButton(this))
    , next_(new QPushButton(this))
    , apply_(new QPushButton(this))
    , cancel_(new QPushButton(this))
{
    setObjectName(QStringLiteral("assistedSetupWizard"));
    setMinimumSize(620, 410);
    input_->setObjectName(QStringLiteral("assistedInput"));
    sourceType_->setObjectName(QStringLiteral("assistedSourceType"));
    apply_->setObjectName(QStringLiteral("applyAssistedSetup"));
    back_->setObjectName(QStringLiteral("assistedBack"));
    next_->setObjectName(QStringLiteral("assistedNext"));
    cancel_->setObjectName(QStringLiteral("assistedCancel"));

    QLabel* inputHeading {};
    QLabel* inputBody {};
    auto* inputPage = wizardPage(pages_, inputHeading, inputBody);
    inputHeading->setObjectName(QStringLiteral("assistedInputHeading"));
    inputBody->setObjectName(QStringLiteral("assistedInputBody"));
    static_cast<QVBoxLayout*>(inputPage->layout())->addWidget(input_);
    static_cast<QVBoxLayout*>(inputPage->layout())->addStretch();

    QLabel* sourceHeading {};
    QLabel* sourceBody {};
    auto* sourcePage = wizardPage(pages_, sourceHeading, sourceBody);
    sourceHeading->setObjectName(QStringLiteral("assistedSourceHeading"));
    sourceBody->setObjectName(QStringLiteral("assistedSourceBody"));
    static_cast<QVBoxLayout*>(sourcePage->layout())->addWidget(sourceType_);
    static_cast<QVBoxLayout*>(sourcePage->layout())->addStretch();

    QLabel* recommendationHeading {};
    QLabel* recommendationBody {};
    auto* recommendationPage = wizardPage(
        pages_, recommendationHeading, recommendationBody);
    recommendationHeading->setObjectName(QStringLiteral("assistedRecommendationHeading"));
    recommendationBody->setObjectName(QStringLiteral("assistedRecommendationBody"));
    recommendation_->setWordWrap(true);
    recommendation_->setProperty("class", QStringLiteral("detailCard"));
    static_cast<QVBoxLayout*>(recommendationPage->layout())->addWidget(recommendation_);
    static_cast<QVBoxLayout*>(recommendationPage->layout())->addStretch();

    QLabel* instructionHeading {};
    QLabel* instructionBody {};
    auto* instructionPage = wizardPage(pages_, instructionHeading, instructionBody);
    instructionHeading->setObjectName(QStringLiteral("assistedInstructionHeading"));
    instructionBody->setObjectName(QStringLiteral("assistedInstructionBody"));
    instruction_->setWordWrap(true);
    instruction_->setProperty("class", QStringLiteral("detailCard"));
    evidence_->setWordWrap(true);
    evidence_->setProperty("class", QStringLiteral("secondaryText"));
    static_cast<QVBoxLayout*>(instructionPage->layout())->addWidget(instruction_);
    static_cast<QVBoxLayout*>(instructionPage->layout())->addWidget(evidence_);
    static_cast<QVBoxLayout*>(instructionPage->layout())->addStretch();

    pages_->addWidget(inputPage);
    pages_->addWidget(sourcePage);
    pages_->addWidget(recommendationPage);
    pages_->addWidget(instructionPage);

    auto* actions = new QHBoxLayout;
    actions->addWidget(cancel_);
    actions->addStretch();
    actions->addWidget(back_);
    actions->addWidget(next_);
    actions->addWidget(apply_);
    auto* layout = new QVBoxLayout(this);
    layout->addWidget(pages_, 1);
    layout->addLayout(actions);

    connect(back_, &QPushButton::clicked, this,
            [this] { moveTo(pages_->currentIndex() - 1); });
    connect(next_, &QPushButton::clicked, this, [this] {
        if (pages_->currentIndex() == 1) {
            refreshRecommendation();
        }
        moveTo(pages_->currentIndex() + 1);
    });
    connect(apply_, &QPushButton::clicked, this, [this] {
        refreshRecommendation();
        if (device_.applyAssistedSetup()) {
            accept();
        }
    });
    connect(cancel_, &QPushButton::clicked, this, &QDialog::reject);
    retranslateUi();
    moveTo(0);
}

void AssistedSetupWizard::changeEvent(QEvent* event)
{
    QDialog::changeEvent(event);
    if (event->type() == QEvent::LanguageChange) {
        retranslateUi();
    }
}

void AssistedSetupWizard::moveTo(const int page)
{
    pages_->setCurrentIndex(qBound(0, page, pages_->count() - 1));
    back_->setEnabled(pages_->currentIndex() > 0);
    next_->setVisible(pages_->currentIndex() < pages_->count() - 1);
    apply_->setVisible(pages_->currentIndex() == pages_->count() - 1);
}

void AssistedSetupWizard::refreshRecommendation()
{
    const auto input = static_cast<model::InputId>(input_->currentData().toInt());
    const auto source = static_cast<model::AssistedSourceType>(sourceType_->currentData().toInt());
    if (!device_.configureAssistedSetup(input, source)) {
        recommendation_->setText(uiText("Unavailable"));
        instruction_->setText(uiText("Needs Hardware Verification"));
        return;
    }
    switch (source) {
    case model::AssistedSourceType::DynamicMicrophone:
        recommendation_->setText(uiText("Dynamic microphone starting preset"));
        instruction_->setText(uiText("Connect the microphone with an XLR cable."));
        break;
    case model::AssistedSourceType::CondenserMicrophone:
        recommendation_->setText(uiText("Condenser microphone starting preset"));
        instruction_->setText(uiText(
            "Connect with an XLR cable. Phantom Power applies only to Input 1 or Input 2."));
        break;
    case model::AssistedSourceType::LineInstrument:
        recommendation_->setText(uiText("Line instrument starting preset"));
        instruction_->setText(uiText("Connect the line-level source to a compatible input."));
        break;
    case model::AssistedSourceType::GuitarBass:
        recommendation_->setText(uiText("Guitar / Bass starting preset"));
        instruction_->setText(uiText("Connect the instrument through the appropriate input path."));
        break;
    }
}

void AssistedSetupWizard::retranslateUi()
{
    setWindowTitle(uiText("Assisted Setup"));
    findChild<QLabel*>(QStringLiteral("assistedInputHeading"))->setText(
        uiText("Step 1 · Select Input"));
    findChild<QLabel*>(QStringLiteral("assistedInputBody"))->setText(
        uiText("Choose the input you want to prepare."));
    findChild<QLabel*>(QStringLiteral("assistedSourceHeading"))->setText(
        uiText("Step 2 · Select Source Type"));
    findChild<QLabel*>(QStringLiteral("assistedSourceBody"))->setText(
        uiText("Choose the source connected to this input."));
    findChild<QLabel*>(QStringLiteral("assistedRecommendationHeading"))->setText(
        uiText("Step 3 · Recommended Preset"));
    findChild<QLabel*>(QStringLiteral("assistedRecommendationBody"))->setText(
        uiText("Review the synthetic starting point."));
    findChild<QLabel*>(QStringLiteral("assistedInstructionHeading"))->setText(
        uiText("Step 4 · Connection and Apply"));
    findChild<QLabel*>(QStringLiteral("assistedInstructionBody"))->setText(
        uiText("Check the connection before applying."));

    const int selectedInput = input_->currentData().toInt();
    input_->clear();
    for (int index = 0; index < 6; ++index) {
        const auto inputId = static_cast<model::InputId>(index);
        input_->addItem(inputDisplayName(inputId), index);
    }
    input_->setCurrentIndex(qMax(0, input_->findData(selectedInput)));
    const int selectedSource = sourceType_->currentData().toInt();
    sourceType_->clear();
    sourceType_->addItem(uiText("Dynamic Microphone"),
                         static_cast<int>(model::AssistedSourceType::DynamicMicrophone));
    sourceType_->addItem(uiText("Condenser Microphone"),
                         static_cast<int>(model::AssistedSourceType::CondenserMicrophone));
    sourceType_->addItem(uiText("Line Instrument"),
                         static_cast<int>(model::AssistedSourceType::LineInstrument));
    sourceType_->addItem(uiText("Guitar / Bass"),
                         static_cast<int>(model::AssistedSourceType::GuitarBass));
    sourceType_->setCurrentIndex(qMax(0, sourceType_->findData(selectedSource)));
    evidence_->setText(uiText(
        "Simulator applies a SYNTHETIC starting point. Hardware commands remain unavailable."));
    back_->setText(uiText("Back"));
    next_->setText(uiText("Next"));
    apply_->setText(uiText("Apply in Simulator"));
    cancel_->setText(uiText("Cancel"));
}

} // namespace flow8::ui
