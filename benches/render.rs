use std::{hint::black_box, path::PathBuf};
use criterion::{criterion_group, criterion_main, Criterion};
use osctet::{module::Module, playback::render};

fn render_module(c: &mut Criterion, filename: &str) {
    let path: PathBuf = ["./testdata", filename].iter().collect();
    let module = Module::load(&path).unwrap();
    c.bench_function(&format!("render {}", filename),
        |b| b.iter(|| black_box({
            let rx = render(module.clone(), path.clone());
            while let Ok(_) = rx.recv() {}
        })));
}

fn scale(c: &mut Criterion) {
    render_module(c, "scale.osctet");
}

fn interpolation(c: &mut Criterion) {
    render_module(c, "interpolation.osctet");
}

criterion_group!(benches, scale, interpolation);
criterion_main!(benches);