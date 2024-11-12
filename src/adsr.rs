use fundsp::prelude::*;

// slightly different implementation of adsr_live. inputs are 1) gate and 2) scale.
pub fn adsr_scalable(
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
) -> An<EnvelopeIn<f32, impl FnMut(f32, &Frame<f32, U2>) -> f32 + Clone, U2, f32>> {
    let neg1 = -1.0;
    let zero = 0.0;
    let a = shared(zero);
    let b = shared(neg1);
    let attack_start = var(&a);
    let release_start = var(&b);
    envelope3(move |time, control, scale| {
        if release_start.value() >= zero && control > zero {
            attack_start.set_value(time);
            release_start.set_value(neg1);
        } else if release_start.value() < zero && control <= zero {
            release_start.set_value(time);
        }
        let ads_value = ads(attack * scale, decay * scale, sustain, time - attack_start.value());
        if release_start.value() < zero {
            ads_value
        } else {
            ads_value
                * clamp01(delerp(
                    release_start.value() + release * scale,
                    release_start.value(),
                    time,
                ))
        }
    })
}

fn ads<F: Real>(attack: F, decay: F, sustain: F, time: F) -> F {
    if time < attack {
        lerp(F::from_f64(0.0), F::from_f64(1.0), time / attack)
    } else {
        let decay_time = time - attack;
        if decay_time < decay {
            lerp(F::from_f64(1.0), sustain, decay_time / decay)
        } else {
            sustain
        }
    }
}