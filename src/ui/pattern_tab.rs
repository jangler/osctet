use crate::pattern::Pattern;

use super::*;

pub fn draw(ui: &mut UI, pattern: &mut Pattern) {
    if ui.button("Test button") {
        ui.open_dialog(Dialog::Alert("Button clicked".to_owned()));
    }
}