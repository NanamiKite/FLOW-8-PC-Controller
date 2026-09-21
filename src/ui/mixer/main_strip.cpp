#include "ui/mixer/main_strip.h"

#include "core/flow8_device.h"
#include "ui/widgets/fader_widget.h"
#include "ui/widgets/meter_widget.h"
#include "ui/ui_text.h"

#include <QHBoxLayout>
#include <QLabel>
#include <QMouseEvent>
#include <QSignalBlocker>
#include <QToolButton>
#include <QVBoxLayout>

namespace flow8::ui {

MainStrip::MainStrip(Flow8Device& device, QWidget* parent)
    : QWidget(parent)
    , device_(device)
    , fader_(new FaderWidget(this))
    , meter_(new MeterWidget(this))
    , mute_(new QToolButton(this))
    , title_(new QLabel(this))
    , badge_(new QLabel(this))
    , processors_(new QLabel(this))
{
    setObjectName(QStringLiteral("mainStrip"));
    setProperty("class", QStringLiteral("mainStrip"));
    setMinimumWidth(142);
    setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Expanding);
    fader_->setObjectName(QStringLiteral("mainFader"));
    mute_->setCheckable(true);
    mute_->setObjectName(QStringLiteral("mainMute"));
    mute_->setProperty("class", QStringLiteral("muteButton"));

    title_->setProperty("class", QStringLiteral("channelName"));
    title_->setAlignment(Qt::AlignCenter);
    badge_->setProperty("class", QStringLiteral("inputBadge"));
    badge_->setAlignment(Qt::AlignCenter);
    processors_->setProperty("class", QStringLiteral("stripIndicator"));
    processors_->setAlignment(Qt::AlignCenter);

    auto* faderRow = new QHBoxLayout;
    faderRow->setContentsMargins(8, 0, 8, 0);
    faderRow->setSpacing(8);
    faderRow->addWidget(meter_);
    faderRow->addWidget(fader_, 1, Qt::AlignHCenter);

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(10, 12, 10, 10);
    layout->setSpacing(8);
    layout->addWidget(title_);
    layout->addWidget(badge_);
    layout->addWidget(processors_);
    layout->addSpacing(44);
    layout->addLayout(faderRow, 1);
    layout->addWidget(mute_);

    connect(fader_, &FaderWidget::valueChanged, this, [this](const double value) {
        emit selected();
        (void)device_.setMainFader(value);
    });
    connect(mute_, &QToolButton::toggled, this, [this](const bool checked) {
        emit selected();
        (void)device_.setMainMuted(checked);
    });
    retranslateUi();
}

void MainStrip::refresh()
{
    const auto* main = device_.state().bus(0);
    setEnabled(main != nullptr
        && device_.state().connectionState() == ConnectionState::Ready);
    if (main == nullptr) {
        return;
    }
    const QSignalBlocker faderBlocker(fader_);
    const QSignalBlocker muteBlocker(mute_);
    fader_->setValue(main->fader.value.value_or(0.0));
    mute_->setChecked(main->muted.value.value_or(false));
    meter_->setLevel(main->fader.value.value_or(0.0) * 0.78);
    meter_->setPeak(main->fader.value.value_or(0.0) * 0.84);
}

void MainStrip::retranslateUi()
{
    title_->setText(uiText("Main"));
    badge_->setText(uiText("Stereo Bus"));
    processors_->setText(uiText("9-band EQ · Limiter"));
    mute_->setText(uiText("Mute"));
    mute_->setToolTip(uiText("Mute"));
    fader_->retranslateUi();
}

void MainStrip::mouseDoubleClickEvent(QMouseEvent* event)
{
    if (event->button() == Qt::LeftButton) {
        emit editRequested();
        event->accept();
        return;
    }
    QWidget::mouseDoubleClickEvent(event);
}

} // namespace flow8::ui
