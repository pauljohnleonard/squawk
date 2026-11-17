//! Sweet 2-operator FM bells for delayed hits and other strike roles.
//!
//! Fixed glassy preset: inharmonic ratio, decaying modulation index, short
//! amplitude envelope. Cheap polyphony, realtime-safe.

use crate::voice::StrikeVoice;
use core::f32::consts::TAU;

/// Voices that can ring at once. Echoes rarely need more.
pub const BELLS_VOICES: usize = 8;
/// Default transposition applied by the Echo role (one octave up).
pub const DEFAULT_BELLS_OCTAVE: i8 = 12;

const RATIO: f32 = 3.5;
const INDEX_PEAK: f32 = 4.5;
const INDEX_FLOOR: f32 = 0.15;
const AMP_GAIN: f32 = 0.35;
const AMP_ATTACK_S: f32 = 0.004;
const AMP_DECAY_S: f32 = 1.6;
const INDEX_DECAY_S: f32 = 0.55;
const QUIET: f32 = 1.0e-4;

#[derive(Clone, Copy)]
struct Voice {
    active: bool,
    carrier_phase: f32,
    mod_phase: f32,
    carrier_inc: f32,
    mod_inc: f32,
    amp: f32,
    amp_level: f32,
    amp_attack: f32,
    amp_decay: f32,
    index: f32,
    index_peak: f32,
    index_decay: f32,
    age: u32,
}

impl Voice {
    fn silent() -> Self {
        Self {
            active: false,
            carrier_phase: 0.0,
            mod_phase: 0.0,
            carrier_inc: 0.0,
            mod_inc: 0.0,
            amp: 0.0,
            amp_level: 0.0,
            amp_attack: 0.0,
            amp_decay: 0.0,
            index: 0.0,
            index_peak: 0.0,
            index_decay: 0.0,
            age: 0,
        }
    }

    fn trigger(&mut self, hz: f32, amplitude: f32, place: f32, sample_rate: f32) {
        let hz = hz.clamp(20.0, sample_rate * 0.45);
        let amplitude = amplitude.clamp(0.0, 1.0);
        let place = place.clamp(0.0, 1.0);
        let brightness = 0.65 + 0.35 * place;
        let dt = 1.0 / sample_rate.max(1.0);

        self.active = true;
        self.carrier_phase = 0.0;
        self.mod_phase = 0.0;
        self.carrier_inc = TAU * hz * dt;
        self.mod_inc = TAU * hz * RATIO * dt;
        self.amp = 0.0;
        self.amp_level = amplitude * AMP_GAIN;
        self.amp_attack = 1.0 - (-dt / AMP_ATTACK_S).exp();
        self.amp_decay = (-dt / AMP_DECAY_S).exp();
        self.index_peak = INDEX_PEAK * brightness * (0.55 + 0.45 * amplitude);
        self.index = self.index_peak;
        self.index_decay = (-dt / INDEX_DECAY_S).exp();
        self.age = 0;
    }

    fn sample(&mut self) -> f32 {
        if !self.active {
            return 0.0;
        }

        self.age = self.age.saturating_add(1);
        let attack_samples = (AMP_ATTACK_S * 48_000.0) as u32 + 8;
        if self.age < attack_samples {
            self.amp += (self.amp_level - self.amp) * self.amp_attack;
        } else {
            self.amp *= self.amp_decay;
        }
        self.index = INDEX_FLOOR + (self.index - INDEX_FLOOR) * self.index_decay;

        let modulator = self.mod_phase.sin();
        let sample = (self.carrier_phase + self.index * modulator).sin() * self.amp;

        self.carrier_phase = wrap(self.carrier_phase + self.carrier_inc);
        self.mod_phase = wrap(self.mod_phase + self.mod_inc);

        if self.amp < QUIET && self.index <= INDEX_FLOOR + QUIET && self.age > attack_samples {
            *self = Self::silent();
            return 0.0;
        }
        flush(sample)
    }
}

pub struct Bells {
    sample_rate: f32,
    voices: [Voice; BELLS_VOICES],
    next: usize,
}

impl Bells {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate: sample_rate.max(1.0),
            voices: [Voice::silent(); BELLS_VOICES],
            next: 0,
        }
    }

    fn steal(&mut self) -> usize {
        if let Some(index) = self.voices.iter().position(|voice| !voice.active) {
            return index;
        }
        let mut quietest = 0usize;
        let mut amp = f32::MAX;
        for (index, voice) in self.voices.iter().enumerate() {
            if voice.amp < amp {
                amp = voice.amp;
                quietest = index;
            }
        }
        // Prefer round-robin when amps are close so rapid echoes spread out.
        let index = if amp < QUIET * 4.0 {
            quietest
        } else {
            self.next
        };
        self.next = (self.next + 1) % BELLS_VOICES;
        index
    }
}

impl StrikeVoice for Bells {
    fn strike_at(&mut self, note: u8, amplitude: f32, place: f32) {
        if amplitude <= 0.0 {
            return;
        }
        let hz = midi_hz(note);
        let index = self.steal();
        self.voices[index].trigger(hz, amplitude, place, self.sample_rate);
    }

    fn next_sample(&mut self) -> f32 {
        let mut mix = 0.0;
        for voice in &mut self.voices {
            mix += voice.sample();
        }
        flush(mix.clamp(-1.0, 1.0))
    }
}

fn midi_hz(note: u8) -> f32 {
    440.0 * (2.0_f32).powf((note as f32 - 69.0) / 12.0)
}

fn wrap(phase: f32) -> f32 {
    let mut phase = phase;
    while phase >= TAU {
        phase -= TAU;
    }
    while phase < 0.0 {
        phase += TAU;
    }
    phase
}

fn flush(sample: f32) -> f32 {
    if sample.abs() < 1.0e-15 { 0.0 } else { sample }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    #[test]
    fn silent_until_struck() {
        let mut bells = Bells::new(RATE);
        for _ in 0..2_000 {
            assert_eq!(bells.next_sample(), 0.0);
        }
    }

    #[test]
    fn strike_rings_and_decays() {
        let mut bells = Bells::new(RATE);
        bells.strike_at(72, 0.6, 0.0);
        let mut early: f32 = 0.0;
        for _ in 0..(RATE as usize / 20) {
            early = early.max(bells.next_sample().abs());
        }
        assert!(early > 0.02, "early peak {early}");

        for _ in 0..(RATE as usize * 6) {
            let _ = bells.next_sample();
        }
        let mut late: f32 = 0.0;
        for _ in 0..(RATE as usize / 10) {
            late = late.max(bells.next_sample().abs());
        }
        assert!(late < early * 0.05, "early {early} late {late}");
    }

    #[test]
    fn output_stays_finite() {
        let mut bells = Bells::new(RATE);
        for note in [48u8, 60, 72, 84, 96] {
            bells.strike_at(note, 1.0, 1.0);
        }
        for _ in 0..(RATE as usize) {
            let sample = bells.next_sample();
            assert!(sample.is_finite());
            assert!((-1.0..=1.0).contains(&sample));
        }
    }

    #[test]
    fn octave_up_is_higher() {
        let low = midi_hz(60);
        let high = midi_hz(60u8.saturating_add(DEFAULT_BELLS_OCTAVE as u8));
        assert!((high / low - 2.0).abs() < 1.0e-5);
    }
}
