use palette::Lchuv;

use super::{theme::Theme, Layout, UI};

pub fn draw(ui: &mut UI) {
    ui.layout = Layout::Vertical;
    ui.header("COLOR THEME");

    ui.start_group();
    color_controls(ui, "Foreground", false, |t| &mut t.fg);
    color_controls(ui, "Background", false, |t| &mut t.bg);
    color_controls(ui, "Accent 1", true, |t| &mut t.accent1);
    color_controls(ui, "Accent 2", true, |t| &mut t.accent2);
    ui.layout = Layout::Vertical;
    ui.end_group();
    
    ui.layout = Layout::Horizontal;
    if ui.button("Reset (light)") {
        ui.style.theme = Theme::light();
    }
    if ui.button("Reset (dark)") {
        ui.style.theme = Theme::dark();
    }
}

fn color_controls(ui: &mut UI, label: &str, accent: bool,
    f: impl Fn(&mut Theme) -> &mut Lchuv) {
    ui.start_group();
    ui.layout = Layout::Vertical;

    ui.label(label);

    let lchuv = f(&mut ui.style.theme);
    let (mut l, mut chroma, _) = lchuv.into_components();
    let mut hue = lchuv.hue.into_degrees();

    if !accent {
        if ui.slider(&format!("{}_l", label), "Lightness", &mut l, 0.0..=100.0, None) {
            f(&mut ui.style.theme).l = l;
        }
    }
    if ui.slider(&format!("{}_chroma", label), "Chroma", &mut chroma, 0.0..=180.0, None) {
        f(&mut ui.style.theme).chroma = chroma;
    }
    if ui.slider(&format!("{}_hue", label), "Hue", &mut hue, -180.0..=180.0,
        Some("degrees")) {
        f(&mut ui.style.theme).hue = hue.into();
    }

    ui.layout = Layout::Horizontal;
    ui.end_group();
}