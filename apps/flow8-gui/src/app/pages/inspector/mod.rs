use crate::app::{Flow8App, bus_id_for_destination};
use eframe::egui;
use flow8_core::{ChannelStateTarget, SemanticCommand};
use flow8_model::{FxId, MixDestination};

mod fx;
mod input;
mod output;

impl Flow8App {
    pub(in crate::app) fn output_inspector(&mut self, ui: &mut egui::Ui) {
        let destination = self.store.state.selected_destination;
        if ui
            .small_button(self.language.tr("Refresh channel", "刷新通道"))
            .clicked()
        {
            self.dispatch(SemanticCommand::RequestChannelState {
                target: ChannelStateTarget::Destination(destination),
            });
        }
        if let Some(bus_id) = bus_id_for_destination(destination)
            && let Some(bus) = self.store.state.bus_for_destination(destination).cloned()
        {
            self.bus_output_inspector(ui, bus_id, &bus);
        } else if matches!(destination, MixDestination::Fx1 | MixDestination::Fx2) {
            let (fx_id, index) = if destination == MixDestination::Fx1 {
                (FxId::Fx1, 0)
            } else {
                (FxId::Fx2, 1)
            };
            let effect = self.store.state.effects[index].clone();
            self.fx_output_inspector(ui, fx_id, index, &effect);
        }
    }
}

pub(super) fn fx_setup_values(effect: &flow8_model::FxState) -> Option<([u8; 3], [bool; 3])> {
    Some((
        [
            *effect.parameters[0].effective()?,
            *effect.parameters[1].effective()?,
            *effect.parameters[2].effective()?,
        ],
        [
            *effect.return_to_main.effective()?,
            *effect.return_to_mon1.effective()?,
            *effect.return_to_mon2.effective()?,
        ],
    ))
}
