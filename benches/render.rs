use std::{hint::black_box, path::PathBuf, sync::Arc};
use criterion::{criterion_group, criterion_main, Criterion};
use osctet::{module::Module, playback::render};

fn render_module(c: &mut Criterion, filename: &str) {
    let path: PathBuf = ["./testdata", filename].iter().collect();
    let module = Arc::new(Module::load(&path).unwrap());
    c.bench_function(&format!("render {}", filename),
        |b| b.iter(|| black_box({
            let rx = render(module.clone(), path.clone(), None);
            while let Ok(_) = rx.recv() {}
        })));
}

fn scale_dry(c: &mut Criterion) {
    render_module(c, "scale_dry.osctet");
}

fn scale_reverb(c: &mut Criterion) {
    render_module(c, "scale_reverb.osctet");
}

fn scale_delay(c: &mut Criterion) {
    render_module(c, "scale_delay.osctet");
}

fn interpolation(c: &mut Criterion) {
    render_module(c, "interpolation.osctet");
}

fn lfo(c: &mut Criterion) {
    render_module(c, "lfo.osctet");
}

fn noise(c: &mut Criterion) {
    render_module(c, "noise.osctet");
}

fn lfo_noise(c: &mut Criterion) {
    render_module(c, "lfo_noise.osctet");
}

fn undecad(c: &mut Criterion) {
    render_module(c, "undecad.osctet");
}

fn song(c: &mut Criterion) {
    render_module(c, "song.osctet");
}

criterion_group!(benches,
    scale_dry, scale_reverb, scale_delay, interpolation, lfo, noise, lfo_noise, undecad,
    song);
criterion_main!(benches);