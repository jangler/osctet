use std::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion};
use osctet::{fx::GlobalFX, module::{Event, EventData, Module}, playback::render};

fn empty_module(c: &mut Criterion) {
    let mut module = Module::new(GlobalFX::new_dummy());
    module.tracks[0].channels[0].push(Event {
        tick: 480,
        data: EventData::End,
    });
    c.bench_function("render", |b| b.iter(|| black_box(render(&module))));
}

// TODO: benchmark doing more DSP

criterion_group!(benches, empty_module);
criterion_main!(benches);