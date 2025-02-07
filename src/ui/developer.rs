use macroquad::time::get_frame_time;

use super::{info::Info, Layout, Ui};

/// Update FPS display at this frequency.
const FPS_UPDATE_INTERVAL: f32 = 0.1;

#[derive(Default)]
pub struct DevState {
    frame_times: Vec<f32>,
    fps: f32,
    scroll: f32,
}

pub fn draw(ui: &mut Ui, state: &mut DevState) {
    ui.layout = Layout::Horizontal;
    let old_y = ui.cursor_y;
    ui.cursor_y -= state.scroll;
    ui.cursor_z -= 1;
    ui.start_group();

    state.frame_times.push(get_frame_time());
    if state.frame_times.iter().sum::<f32>() >= FPS_UPDATE_INTERVAL {
        state.fps = state.frame_times.len() as f32 / state.frame_times.iter().sum::<f32>();
        state.frame_times.clear();
    }
    ui.label(&format!("FPS: {}", state.fps.round() as i32), Info::None);

    let scroll_h = ui.end_group().unwrap().h + ui.style.margin;
    ui.cursor_z += 1;
    ui.cursor_y = old_y;
    ui.vertical_scrollbar(&mut state.scroll,
        scroll_h, ui.bounds.y + ui.bounds.h - ui.cursor_y, true);
}