use cpal::StreamConfig;
use macroquad::time::get_frame_time;

use crate::playback::PlayerShell;

use super::{info::Info, Layout, Ui};

/// Update FPS display at this frequency.
const FPS_UPDATE_INTERVAL: f32 = 0.1;

pub struct DevState {
    frame_times: Vec<f32>,
    fps: f32,
    scroll: f32,
    stream_config: Option<StreamConfig>,
    pub only_draw_on_input: bool,
}

impl DevState {
    pub fn new(stream_config: Option<StreamConfig>) -> Self {
        Self {
            frame_times: Vec::new(),
            fps: 0.0,
            scroll: 0.0,
            stream_config,
            only_draw_on_input: false,
        }
    }
}

pub fn draw(ui: &mut Ui, state: &mut DevState, player: &PlayerShell) {
    ui.layout = Layout::Horizontal;
    let old_y = ui.cursor_y;
    ui.cursor_y -= state.scroll;
    ui.cursor_z -= 1;
    ui.start_group();

    draw_diagnostics(ui, state, player);
    ui.vertical_space();
    draw_options(ui, state);

    let scroll_h = ui.end_group().unwrap().h + ui.style.margin;
    ui.cursor_z += 1;
    ui.cursor_y = old_y;
    ui.vertical_scrollbar(&mut state.scroll,
        scroll_h, ui.bounds.y + ui.bounds.h - ui.cursor_y, true);
}

fn draw_diagnostics(ui: &mut Ui, state: &mut DevState, player: &PlayerShell) {
    ui.header("DIAGNOSTICS", Info::None);

    // FPS
    state.frame_times.push(get_frame_time());
    if state.frame_times.iter().sum::<f32>() >= FPS_UPDATE_INTERVAL {
        state.fps = state.frame_times.len() as f32 / state.frame_times.iter().sum::<f32>();
        state.frame_times.clear();
    }
    ui.label(&format!("FPS: {}", state.fps.round() as i32), Info::None);

    // stream config
    if let Some(conf) = &state.stream_config {
        ui.label(&format!("{conf:?}"), Info::None);
    }

    ui.label(&format!("Buffer size: {}", player.buffer_size()), Info::None);
}

fn draw_options(ui: &mut Ui, state: &mut DevState) {
    ui.header("OPTIONS", Info::None);
    ui.checkbox("Skip UI if no input", &mut state.only_draw_on_input, true, Info::None);
}