use std::error::Error;

use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, Stream, StreamConfig};
use fundsp::hacker::{AudioUnit, BlockRateAdapter};

pub fn init_audio(mut backend: BlockRateAdapter) -> Result<Stream, Box<dyn Error>> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("could not open audio output device")?;

    let config: StreamConfig = device.supported_output_configs()?
        .next()
        .ok_or("could not find audio output config")?
        .with_max_sample_rate()
        .into();

    backend.set_sample_rate(config.sample_rate.0 as f64);

    let stream = device.build_output_stream(
        &config,move |data: &mut[f32], _: &cpal::OutputCallbackInfo| {
            // there's probably a better way to do this
            let mut i = 0;
            let len = data.len();
            while i < len {
                let (l, r) = backend.get_stereo();
                data[i] = l;
                data[i+1] = r;
                i += 2;
            }
        },
        move |err| {
            eprintln!("stream error: {}", err);
        },
        None
    )?;
    
    stream.play()?;
    Ok(stream)
}