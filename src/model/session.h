#pragma once

#include "model/channel.h"
#include "model/state_value.h"

#include <QString>
#include <QVector>

#include <optional>

namespace flow8::model {

enum class SessionStartAction {
    AssistedSetup,
    LoadSnapshot,
    StartNew,
    ContinueSession,
};

enum class AssistedSetupStep {
    SelectInput,
    SelectSourceType,
    RecommendedPreset,
    ConnectionInstruction,
    Apply,
    Complete,
};

enum class AssistedSourceType {
    DynamicMicrophone,
    CondenserMicrophone,
    LineInstrument,
    GuitarBass,
};

struct AssistedSetupState {
    AssistedSetupStep step {AssistedSetupStep::SelectInput};
    std::optional<InputId> input;
    std::optional<AssistedSourceType> sourceType;
    QString recommendedPreset;
    QString connectionInstruction;
    bool applied {};
    QString source;
};

struct EzGainChannelResult {
    InputId input {InputId::Input1};
    StateValue<bool> signalDetected;
    StateValue<double> gain;
    StateValue<double> headroomDb;
};

struct EzGainSession {
    QVector<InputId> targets;
    bool running {};
    int durationSeconds {};
    bool cancelled {};
    QVector<EzGainChannelResult> results;
    QString source;
};

} // namespace flow8::model
