#include "ui/session/session_start_view.h"

#include "ui/ui_text.h"

#include <QGridLayout>
#include <QLabel>
#include <QPushButton>
#include <QVBoxLayout>

namespace flow8::ui {

SessionStartView::SessionStartView(QWidget* parent)
    : QWidget(parent)
    , title_(new QLabel(this))
    , subtitle_(new QLabel(this))
    , note_(new QLabel(this))
    , assistedSetup_(new QPushButton(this))
    , loadSnapshot_(new QPushButton(this))
    , startNew_(new QPushButton(this))
    , continueSession_(new QPushButton(this))
{
    setObjectName(QStringLiteral("sessionStartView"));
    title_->setProperty("class", QStringLiteral("sessionTitle"));
    title_->setAlignment(Qt::AlignCenter);
    subtitle_->setProperty("class", QStringLiteral("secondaryText"));
    subtitle_->setAlignment(Qt::AlignCenter);
    note_->setProperty("class", QStringLiteral("syntheticBadge"));
    note_->setAlignment(Qt::AlignCenter);
    for (auto* button : {assistedSetup_, loadSnapshot_, startNew_, continueSession_}) {
        button->setMinimumSize(240, 82);
        button->setProperty("class", QStringLiteral("sessionAction"));
    }
    assistedSetup_->setObjectName(QStringLiteral("startAssistedSetup"));
    loadSnapshot_->setObjectName(QStringLiteral("startLoadSnapshot"));
    startNew_->setObjectName(QStringLiteral("startNewSession"));
    continueSession_->setObjectName(QStringLiteral("continueSession"));

    auto* actions = new QGridLayout;
    actions->setHorizontalSpacing(14);
    actions->setVerticalSpacing(14);
    actions->addWidget(assistedSetup_, 0, 0);
    actions->addWidget(loadSnapshot_, 0, 1);
    actions->addWidget(startNew_, 1, 0);
    actions->addWidget(continueSession_, 1, 1);

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(80, 60, 80, 60);
    layout->addStretch();
    layout->addWidget(title_);
    layout->addWidget(subtitle_);
    layout->addSpacing(28);
    layout->addLayout(actions);
    layout->addSpacing(18);
    layout->addWidget(note_, 0, Qt::AlignHCenter);
    layout->addStretch();

    connect(assistedSetup_, &QPushButton::clicked,
            this, &SessionStartView::assistedSetupRequested);
    connect(loadSnapshot_, &QPushButton::clicked,
            this, &SessionStartView::loadSnapshotRequested);
    connect(startNew_, &QPushButton::clicked,
            this, &SessionStartView::startNewRequested);
    connect(continueSession_, &QPushButton::clicked,
            this, &SessionStartView::continueSessionRequested);
    retranslateUi();
}

void SessionStartView::retranslateUi()
{
    title_->setText(uiText("Start a FLOW 8 Session"));
    subtitle_->setText(uiText("Choose how you want to begin"));
    assistedSetup_->setText(uiText("Assisted Setup"));
    loadSnapshot_->setText(uiText("Load Snapshot"));
    startNew_->setText(uiText("Start New"));
    continueSession_->setText(uiText("Continue Session"));
    note_->setText(uiText("Simulator · SYNTHETIC"));
}

} // namespace flow8::ui
