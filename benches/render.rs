use std::{hint::black_box, path::PathBuf};
use criterion::{criterion_group, criterion_main, Criterion};
use osctet::{module::{Event, EventData, Module}, playback::render};

fn empty_module(c: &mut Criterion) {
    let mut module = Module::new(Default::default());
    module.tracks[0].channels[0].events.push(Event {
        tick: 480,
        data: EventData::End,
    });
    let path = PathBuf::default();
    // TODO: module cloning is probably costly here!
    c.bench_function("render",
        |b| b.iter(|| black_box(render(module.clone(), path.clone()))));
}

// TODO: benchmark doing more DSP

criterion_group!(benches, empty_module);
criterion_main!(benches);