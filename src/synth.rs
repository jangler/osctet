use fundsp::hacker::*;

pub struct Synth {
    pub gate: Shared,
    pub oscs: [Oscillator; 1],
}

impl Synth {
    pub fn new() -> Self {
        let gate = shared(0.0);
        let oscs = [Oscillator::new(&gate)];
        Self {
            gate,
            oscs,
        }
    }
}

pub struct Oscillator {
    pub freq: Shared,
    pub gain: Shared,
    pub env: ADSR,
    pub unit: Box<dyn AudioUnit>,
}

impl Oscillator {
    fn new(gate: &Shared) -> Self {
        let freq = shared(440.0);
        let gain = shared(0.2);
        let env = ADSR::new();
        let unit = (var(&freq) >> saw()) * var(&gain) *
            (var(gate) >> adsr_live(env.attack, env.decay, env.sustain, env.release));
        Self {
            freq,
            gain,
            env,
            unit: Box::new(unit),
        }
    }
}

pub struct ADSR {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

impl ADSR {
    fn new() -> Self {
        Self {
            attack: 0.1,
            decay: 0.1,
            sustain: 0.5,
            release: 0.1,
        }
    }
}