#include "ui/layers/layer_views.h"

#include "core/flow8_device.h"
#include "model/flow8_capabilities.h"
#include "ui/inspector/detail_panel.h"
#include "ui/mixer/mix_send_strip.h"
#include "ui/ui_text.h"
#include "ui/widgets/fader_widget.h"
#include "ui/widgets/meter_widget.h"

#include <QCheckBox>
#include <QFrame>
#include <QHBoxLayout>
#include <QLabel>
#include <QLocale>
#include <QScrollArea>
#include <QSignalBlocker>
#include <QSplitter>
#include <QToolButton>
#include <QVBoxLayout>
#include <QVariant>

#include <array>

namespace flow8::ui {
namespace {

QWidget* createLayerHeader(QLabel* title, QLabel* note, QWidget* parent)
{
    auto* header = new QWidget(parent);
    auto* layout = new QVBoxLayout(header);
    layout->setContentsMargins(18, 14, 18, 10);
    title->setProperty("class", QStringLiteral("sessionTitle"));
    note->setProperty("class", QStringLiteral("secondaryText"));
    layout->addWidget(title);
    layout->addWidget(note);
    return header;
}

QScrollArea* createSendSurface(Flow8Device& device,
                               const model::RoutingDestination destination,
                               QVector<MixSendStrip*>& strips, QWidget* parent)
{
    auto* container = new QWidget(parent);
    container->setObjectName(QStringLiteral("layerStripsContainer"));
    auto* layout = new QHBoxLayout(container);
    layout->setContentsMargins(12, 12, 12, 12);
    layout->setSpacing(7);
    layout->setAlignment(Qt::AlignLeft);
    for (int input = 0; input < model::conventionalMixerInputCount; ++input) {
        auto* strip = new MixSendStrip(device, input, destination, container);
        strips.append(strip);
        layout->addWidget(strip);
    }
    auto* scroll = new QScrollArea(parent);
    scroll->setFrameShape(QFrame::NoFrame);
    scroll->setWidgetResizable(true);
    scroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAsNeeded);
    scroll->setVerticalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    scroll->setWidget(container);
    return scroll;
}

} // namespace

FxView::FxView(Flow8Device& device, const int engineIndex, QWidget* parent)
    : QWidget(parent)
    , device_(device)
    , engineIndex_(engineIndex)
    , title_(new QLabel(this))
    , note_(new QLabel(this))
    , masterLabel_(new QLabel(this))
    , master_(new FaderWidget(this))
    , details_(new DetailPanel(device_, this))
{
    setObjectName(QStringLiteral("fx%1View").arg(engineIndex + 1));
    auto* header = createLayerHeader(title_, note_, this);
    auto* sends = createSendSurface(
        device_, engineIndex_ == 0 ? model::RoutingDestination::Fx1
                                   : model::RoutingDestination::Fx2,
        sends_, this);

    auto* engine = new QWidget(this);
    engine->setProperty("class", QStringLiteral("detailCard"));
    auto* engineLayout = new QVBoxLayout(engine);
    engineLayout->setContentsMargins(12, 12, 12, 12);
    masterLabel_->setProperty("class", QStringLiteral("sectionLabel"));
    master_->setObjectName(QStringLiteral("fx%1Master").arg(engineIndex_ + 1));
    engineLayout->addWidget(masterLabel_);
    engineLayout->addWidget(master_, 1, Qt::AlignHCenter);
    auto* routes = new QWidget(engine);
    auto* routeLayout = new QHBoxLayout(routes);
    routeLayout->setContentsMargins(0, 0, 0, 0);
    for (int destination = 0; destination < 3; ++destination) {
        auto* route = new QCheckBox(routes);
        route->setObjectName(QStringLiteral("fx%1Output%2")
            .arg(engineIndex_ + 1).arg(destination));
        outputRoutes_.append(route);
        routeLayout->addWidget(route);
        connect(route, &QCheckBox::toggled, this,
                [this, destination](const bool enabled) {
                    (void)device_.setFxOutputRouteEnabled(
                        engineIndex_,
                        static_cast<model::FxOutputDestination>(destination), enabled);
                });
    }
    engineLayout->addWidget(routes);

    auto* controls = new QWidget(this);
    auto* controlsLayout = new QVBoxLayout(controls);
    controlsLayout->setContentsMargins(0, 0, 0, 0);
    controlsLayout->addWidget(engine, 1);
    controlsLayout->addWidget(details_, 2);

    auto* split = new QSplitter(Qt::Horizontal, this);
    split->addWidget(sends);
    split->addWidget(controls);
    split->setStretchFactor(0, 3);
    split->setStretchFactor(1, 2);
    split->setSizes({760, 500});

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->addWidget(header);
    layout->addWidget(split, 1);
    details_->showFx(engineIndex_);
    connect(master_, &FaderWidget::valueChanged, this, [this](const double value) {
        (void)device_.setFxMaster(engineIndex_, value);
    });
    connect(&device_.state(), &Flow8State::effectChanged, this, [this](const int effect) {
        if (effect == engineIndex_) refresh();
    });
    connect(&device_.state(), &Flow8State::routingChanged, this, &FxView::refresh);
    retranslateUi();
}

void FxView::refresh()
{
    for (auto* send : sends_) send->refresh();
    if (engineIndex_ >= 0 && engineIndex_ < device_.state().effects().size()) {
        const QSignalBlocker blocker(master_);
        master_->setValue(
            device_.state().effects().at(engineIndex_).master.value.value_or(0.0));
    }
    for (int destination = 0; destination < outputRoutes_.size(); ++destination) {
        const auto* route = device_.state().routing().fxOutputRoute(
            engineIndex_, static_cast<model::FxOutputDestination>(destination));
        const QSignalBlocker blocker(outputRoutes_[destination]);
        outputRoutes_[destination]->setChecked(
            route != nullptr && route->enabled.value.value_or(false));
    }
    details_->showFx(engineIndex_);
    details_->refresh();
}

void FxView::retranslateUi()
{
    title_->setText(uiText("FX %1").arg(engineIndex_ + 1));
    note_->setText(uiText(
        "Input sends · Independent engine · Output routing · SYNTHETIC"));
    masterLabel_->setText(uiText("Master"));
    const std::array<const char*, 3> destinations {"Main", "Monitor 1", "Monitor 2"};
    for (int destination = 0; destination < outputRoutes_.size(); ++destination) {
        outputRoutes_[destination]->setText(
            uiText(destinations[static_cast<std::size_t>(destination)]));
    }
    for (auto* send : sends_) send->retranslateUi();
    master_->retranslateUi();
    details_->retranslateUi();
    refresh();
}

MonitorView::MonitorView(Flow8Device& device, const int monitorIndex, QWidget* parent)
    : QWidget(parent)
    , device_(device)
    , monitorIndex_(monitorIndex)
    , title_(new QLabel(this))
    , note_(new QLabel(this))
    , stereoLink_(new QCheckBox(this))
    , details_(new DetailPanel(device_, this))
{
    setObjectName(QStringLiteral("monitor%1View").arg(monitorIndex + 1));
    auto* header = createLayerHeader(title_, note_, this);
    auto* headerLayout = qobject_cast<QVBoxLayout*>(header->layout());
    stereoLink_->setObjectName(QStringLiteral("monitorStereoLink"));
    headerLayout->addWidget(stereoLink_, 0, Qt::AlignLeft);
    auto* scroll = createSendSurface(
        device_, monitorIndex_ == 0 ? model::RoutingDestination::Monitor1
                                    : model::RoutingDestination::Monitor2,
        sends_, this);
    auto* split = new QSplitter(Qt::Horizontal, this);
    split->addWidget(scroll);
    split->addWidget(details_);
    split->setStretchFactor(0, 2);
    split->setStretchFactor(1, 3);
    split->setSizes({460, 760});

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->addWidget(header);
    layout->addWidget(split, 1);

    details_->showBus(monitorIndex_ + 1);
    connect(&device_.state(), &Flow8State::channelChanged,
            this, [this](int) { refresh(); });
    connect(&device_.state(), &Flow8State::routingChanged,
            this, &MonitorView::refresh);
    connect(stereoLink_, &QCheckBox::toggled, this,
            [this](const bool linked) { (void)device_.setMonitorStereoLink(linked); });
    retranslateUi();
}

void MonitorView::refresh()
{
    for (auto* send : sends_) send->refresh();
    const QSignalBlocker blocker(stereoLink_);
    stereoLink_->setChecked(
        device_.state().monitorLink().stereoLinked.value.value_or(false));
    details_->showBus(monitorIndex_ + 1);
    details_->refresh();
}

void MonitorView::retranslateUi()
{
    title_->setText(uiText("Monitor %1").arg(monitorIndex_ + 1));
    note_->setText(uiText("Channel sends · Pre/Post-Fader · 9-band EQ · Limiter"));
    stereoLink_->setText(uiText("Stereo Link: MON1 ↔ MON2"));
    stereoLink_->setToolTip(uiText(
        "Simulator links MON1/MON2 send and master faders; hardware propagation remains UNKNOWN."));
    for (auto* send : sends_) send->retranslateUi();
    details_->retranslateUi();
    refresh();
}

MainView::MainView(Flow8Device& device, QWidget* parent)
    : QWidget(parent)
    , device_(device)
    , title_(new QLabel(this))
    , note_(new QLabel(this))
    , details_(new DetailPanel(device_, this))
{
    setObjectName(QStringLiteral("mainView"));
    auto* header = createLayerHeader(title_, note_, this);

    auto* channels = createSendSurface(
        device_, model::RoutingDestination::Main, sends_, this);
    auto* split = new QSplitter(Qt::Horizontal, this);
    split->addWidget(channels);
    split->addWidget(details_);
    split->setStretchFactor(0, 2);
    split->setStretchFactor(1, 3);
    split->setSizes({460, 760});

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->addWidget(header);
    layout->addWidget(split, 1);
    details_->showBus(0);
    connect(&device_.state(), &Flow8State::channelChanged,
            this, [this](int) { refresh(); });
    retranslateUi();
}

void MainView::refresh()
{
    for (auto* send : sends_) send->refresh();
    details_->showBus(0);
    details_->refresh();
}

void MainView::retranslateUi()
{
    title_->setText(uiText("Main"));
    note_->setText(uiText("Main mix sends · Master · Balance · 9-band EQ · Limiter"));
    for (auto* send : sends_) send->retranslateUi();
    details_->retranslateUi();
    refresh();
}

MainOutView::MainOutView(Flow8Device& device, QWidget* parent)
    : QWidget(parent)
    , device_(device)
    , title_(new QLabel(this))
    , subtitle_(new QLabel(this))
    , outputState_(new QLabel(this))
    , delayState_(new QLabel(this))
    , outputLevelMode_(new QLabel(this))
    , fader_(new FaderWidget(this))
    , meter_(new MeterWidget(this))
    , mute_(new QToolButton(this))
{
    setObjectName(QStringLiteral("mainOutView"));
    title_->setProperty("class", QStringLiteral("sessionTitle"));
    subtitle_->setProperty("class", QStringLiteral("secondaryText"));
    outputState_->setProperty("class", QStringLiteral("syntheticBadge"));
    delayState_->setProperty("class", QStringLiteral("secondaryText"));
    outputLevelMode_->setProperty("class", QStringLiteral("secondaryText"));
    fader_->setObjectName(QStringLiteral("mainOutFader"));
    mute_->setObjectName(QStringLiteral("mainOutMute"));
    mute_->setProperty("class", QStringLiteral("muteButton"));
    mute_->setCheckable(true);

    auto* card = new QWidget(this);
    card->setProperty("class", QStringLiteral("detailCard"));
    auto* faderRow = new QHBoxLayout;
    faderRow->addWidget(meter_);
    faderRow->addWidget(fader_);
    auto* cardLayout = new QVBoxLayout(card);
    cardLayout->setContentsMargins(28, 24, 28, 24);
    cardLayout->addWidget(outputState_, 0, Qt::AlignHCenter);
    cardLayout->addLayout(faderRow, 1);
    cardLayout->addWidget(mute_);
    cardLayout->addWidget(delayState_);
    cardLayout->addWidget(outputLevelMode_);

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(28, 22, 28, 24);
    layout->addWidget(title_);
    layout->addWidget(subtitle_);
    layout->addWidget(card, 1, Qt::AlignHCenter);

    connect(fader_, &FaderWidget::valueChanged,
            this, [this](const double value) { (void)device_.setMainFader(value); });
    connect(mute_, &QToolButton::toggled,
            this, [this](const bool muted) { (void)device_.setMainMuted(muted); });
    connect(&device_.state(), &Flow8State::busChanged,
            this, [this](int index) { if (index == 0) refresh(); });
    connect(&device_.state(), &Flow8State::outputMeterChanged,
            this, [this](const model::RoutingDestination destination) {
                if (destination == model::RoutingDestination::Main) refresh();
            });
    connect(&device_.state(), &Flow8State::preferencesChanged,
            this, &MainOutView::refresh);
    connect(&device_.state(), &Flow8State::physicalOutputChanged,
            this, [this](const model::PhysicalOutputId output) {
                if (output == model::PhysicalOutputId::MainOut) {
                    refresh();
                }
            });
    retranslateUi();
}

void MainOutView::refresh()
{
    const auto* main = device_.state().bus(0);
    if (main == nullptr) {
        setEnabled(false);
        return;
    }
    setEnabled(device_.state().connectionState() == ConnectionState::Ready);
    const QSignalBlocker faderBlocker(fader_);
    const QSignalBlocker muteBlocker(mute_);
    const double level = main->fader.value.value_or(0.0);
    fader_->setValue(level);
    const auto* meter = device_.state().outputMeter(model::RoutingDestination::Main);
    meter_->setLevel(meter == nullptr ? 0.0 : meter->level.value.value_or(0.0));
    meter_->setPeak(meter == nullptr ? 0.0 : meter->peak.value.value_or(0.0));
    meter_->setClipping(meter != nullptr && meter->clipping.value.value_or(false));
    mute_->setChecked(main->muted.has_value()
        && main->muted->value.value_or(false));
    outputState_->setText(uiText("Simulator Output · SYNTHETIC"));
    if (main->outputDelay.has_value()
        && main->outputDelay->milliseconds.value.has_value()) {
        delayState_->setText(uiText("Output Delay: %1 ms").arg(
            QLocale().toString(*main->outputDelay->milliseconds.value, 'f', 1)));
    } else {
        delayState_->setText(uiText("Output Delay: Unknown · Hardware Required"));
    }
    const auto* mainOutput = device_.state().physicalOutput(
        model::PhysicalOutputId::MainOut);
    const bool padEnabled = mainOutput != nullptr
        && mainOutput->padMinus10Dbv.has_value()
        && mainOutput->padMinus10Dbv->value.value_or(false);
    outputLevelMode_->setText(padEnabled
        ? uiText("10 dBV Output Level: On") : uiText("10 dBV Output Level: Off"));
}

void MainOutView::retranslateUi()
{
    title_->setText(uiText("Main Out"));
    subtitle_->setText(uiText("Final output level and output state"));
    mute_->setText(uiText("Mute"));
    fader_->retranslateUi();
    refresh();
}

} // namespace flow8::ui
