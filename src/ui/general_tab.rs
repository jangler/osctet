use crate::fx::GlobalFX;

use super::*;

pub fn draw(ui: &mut UI, fx: &mut GlobalFX) {
    ui.layout = Layout::Vertical;

    ui.label("Reverb");
    ui.shared_slider("reverb_level",
        "Level", &fx.settings.reverb_amount.0, 0.0..=1.0, None);

    if ui.slider("predelay",
        "Predelay time", &mut fx.settings.predelay_time, 0.0..=0.1, Some("s")) {
        fx.commit_predelay();
    }
    if ui.slider("room_size",
        "Room size", &mut fx.settings.reverb_room_size, 5.0..=100.0, Some("m")) {
        fx.commit_reverb();
    }
    if ui.slider("decay_time",
        "Decay time", &mut fx.settings.reverb_time, 0.0..=5.0, Some("s")) {
        fx.commit_reverb();
    }
}