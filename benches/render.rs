use std::{hint::black_box, path::PathBuf};
use criterion::{criterion_group, criterion_main, Criterion};
use osctet::{module::Module, playback::render};

fn scale(c: &mut Criterion) {
    let path = PathBuf::from("./testdata/scale.osctet");
    let module = Module::load(&path).unwrap();
    c.bench_function("render",
        |b| b.iter(|| black_box({
            let rx = render(module.clone(), path.clone());
            while let Ok(_) = rx.recv() {}
        })));
}

criterion_group!(benches, scale);
criterion_main!(benches);