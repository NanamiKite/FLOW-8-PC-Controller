#include "ui/mixer/main_strip.h"

#include "core/flow8_device.h"

#include <QCheckBox>
#include <QLabel>
#include <QSignalBlocker>
#include <QSlider>
#include <QVBoxLayout>

#include <cmath>

namespace flow8::ui {

MainStrip::MainStrip(Flow8Device& device, QWidget* parent)
    : QWidget(parent)
    , device_(device)
    , fader_(new QSlider(Qt::Vertical, this))
    , mute_(new QCheckBox(QStringLiteral("Mute"), this))
{
    setObjectName(QStringLiteral("mainStrip"));
    setMinimumWidth(118);
    fader_->setRange(0, 1000);
    fader_->setMinimumHeight(260);
    fader_->setObjectName(QStringLiteral("mainFader"));
    mute_->setObjectName(QStringLiteral("mainMute"));

    auto* layout = new QVBoxLayout(this);
    auto* label = new QLabel(QStringLiteral("Main"), this);
    label->setAlignment(Qt::AlignCenter);
    layout->addWidget(label);
    layout->addWidget(fader_, 1, Qt::AlignHCenter);
    layout->addWidget(mute_);

    connect(fader_, &QSlider::valueChanged, this,
            [this](const int value) { (void)device_.setMainFader(value / 1000.0); });
    connect(mute_, &QCheckBox::toggled, this,
            [this](const bool checked) { (void)device_.setMainMuted(checked); });
    refresh();
}

void MainStrip::refresh()
{
    const auto& main = device_.state().main();
    setEnabled(device_.state().connectionState() == ConnectionState::Ready);
    const QSignalBlocker faderBlocker(fader_);
    const QSignalBlocker muteBlocker(mute_);
    fader_->setValue(static_cast<int>(std::lround(main.fader.value.value_or(0.0) * 1000.0)));
    mute_->setChecked(main.muted.value.value_or(false));
}

} // namespace flow8::ui
