//! Monophonic 2-operator FM bass for the main instrument role.
//!
//! Last-note priority: each strike replaces the sounding voice. Tuned for low
//! harmonic bass — integer-friendly ratio, moderate index, optional feedback.

use crate::voice::StrikeVoice;
use core::f32::consts::TAU;

/// Play handpan-range MIDI an octave down so the pads sit in bass territory.
pub const DEFAULT_FM_BASS_OCTAVE: i8 = -12;

pub const DEFAULT_FM_BASS_DECAY: f32 = 0.55;
pub const FM_BASS_DECAY_RANGE: (f32, f32) = (0.15, 1.0);
pub const DEFAULT_FM_BASS_RATIO: f32 = 1.0;
pub const FM_BASS_RATIO_RANGE: (f32, f32) = (1.0, 2.0);
pub const DEFAULT_FM_BASS_INDEX: f32 = 1.2;
pub const FM_BASS_INDEX_RANGE: (f32, f32) = (0.0, 4.0);
pub const DEFAULT_FM_BASS_INDEX_DECAY: f32 = 0.35;
pub const FM_BASS_INDEX_DECAY_RANGE: (f32, f32) = (0.0, 1.0);
pub const DEFAULT_FM_BASS_FEEDBACK: f32 = 0.08;
pub const FM_BASS_FEEDBACK_RANGE: (f32, f32) = (0.0, 1.0);

const AMP_GAIN: f32 = 0.42;
const AMP_ATTACK_S: f32 = 0.015;
const DECAY_MIN_S: f32 = 0.45;
const DECAY_MAX_S: f32 = 2.4;
const INDEX_FLOOR: f32 = 0.04;
const INDEX_DECAY_MIN_S: f32 = 0.12;
const INDEX_DECAY_MAX_S: f32 = 1.2;
const FEEDBACK_MAX: f32 = 0.7;
const QUIET: f32 = 1.0e-4;

/// Snapshot of the FM knobs (and strip level) for presets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FmBassParams {
    pub decay: f32,
    pub ratio: f32,
    pub index: f32,
    pub index_decay: f32,
    pub feedback: f32,
    pub level: f32,
}

impl Default for FmBassParams {
    fn default() -> Self {
        Self {
            decay: DEFAULT_FM_BASS_DECAY,
            ratio: DEFAULT_FM_BASS_RATIO,
            index: DEFAULT_FM_BASS_INDEX,
            index_decay: DEFAULT_FM_BASS_INDEX_DECAY,
            feedback: DEFAULT_FM_BASS_FEEDBACK,
            level: 1.0,
        }
    }
}

impl FmBassParams {
    pub fn clamp(self) -> Self {
        Self {
            decay: self
                .decay
                .clamp(FM_BASS_DECAY_RANGE.0, FM_BASS_DECAY_RANGE.1),
            ratio: self
                .ratio
                .clamp(FM_BASS_RATIO_RANGE.0, FM_BASS_RATIO_RANGE.1),
            index: self
                .index
                .clamp(FM_BASS_INDEX_RANGE.0, FM_BASS_INDEX_RANGE.1),
            index_decay: self
                .index_decay
                .clamp(FM_BASS_INDEX_DECAY_RANGE.0, FM_BASS_INDEX_DECAY_RANGE.1),
            feedback: self
                .feedback
                .clamp(FM_BASS_FEEDBACK_RANGE.0, FM_BASS_FEEDBACK_RANGE.1),
            level: self.level.clamp(0.0, 2.0),
        }
    }
}

/// Built-in bass recipes (not user-editable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FmBassRecipe {
    Round,
    Growl,
    Pluck,
}

impl FmBassRecipe {
    pub const ALL: [FmBassRecipe; 3] = [
        FmBassRecipe::Round,
        FmBassRecipe::Growl,
        FmBassRecipe::Pluck,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FmBassRecipe::Round => "Round",
            FmBassRecipe::Growl => "Growl",
            FmBassRecipe::Pluck => "Pluck",
        }
    }

    pub fn params(self) -> FmBassParams {
        match self {
            FmBassRecipe::Round => FmBassParams::default(),
            FmBassRecipe::Growl => FmBassParams {
                decay: 0.62,
                ratio: 2.0,
                index: 1.8,
                index_decay: 0.45,
                feedback: 0.45,
                level: 1.0,
            },
            FmBassRecipe::Pluck => FmBassParams {
                decay: 0.28,
                ratio: 1.0,
                index: 2.4,
                index_decay: 0.12,
                feedback: 0.15,
                level: 1.0,
            },
        }
        .clamp()
    }
}

#[derive(Clone, Copy)]
struct Shape {
    decay: f32,
    ratio: f32,
    index: f32,
    index_decay: f32,
    feedback: f32,
}

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
    feedback: f32,
    mod_prev: f32,
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
            feedback: 0.0,
            mod_prev: 0.0,
            age: 0,
        }
    }

    fn trigger(&mut self, hz: f32, amplitude: f32, place: f32, sample_rate: f32, shape: Shape) {
        let hz = hz.clamp(20.0, sample_rate * 0.22);
        let amplitude = amplitude.clamp(0.0, 1.0);
        let place = place.clamp(0.0, 1.0);
        let decay = shape
            .decay
            .clamp(FM_BASS_DECAY_RANGE.0, FM_BASS_DECAY_RANGE.1);
        let ratio = shape
            .ratio
            .clamp(FM_BASS_RATIO_RANGE.0, FM_BASS_RATIO_RANGE.1);
        let index = shape
            .index
            .clamp(FM_BASS_INDEX_RANGE.0, FM_BASS_INDEX_RANGE.1);
        let index_decay_amt = shape
            .index_decay
            .clamp(FM_BASS_INDEX_DECAY_RANGE.0, FM_BASS_INDEX_DECAY_RANGE.1);
        let feedback = shape
            .feedback
            .clamp(FM_BASS_FEEDBACK_RANGE.0, FM_BASS_FEEDBACK_RANGE.1);
        let dt = 1.0 / sample_rate.max(1.0);
        let amp_decay_s = DECAY_MIN_S + (DECAY_MAX_S - DECAY_MIN_S) * decay;
        // Low knob = fast index fall (more bass body); high = longer brightness.
        let index_decay_s =
            INDEX_DECAY_MIN_S + (INDEX_DECAY_MAX_S - INDEX_DECAY_MIN_S) * index_decay_amt;
        let brightness = 0.85 + 0.15 * place;

        self.active = true;
        self.carrier_phase = 0.0;
        self.mod_phase = 0.0;
        self.carrier_inc = TAU * hz * dt;
        self.mod_inc = TAU * hz * ratio * dt;
        self.amp = 0.0;
        self.amp_level = amplitude * AMP_GAIN;
        self.amp_attack = 1.0 - (-dt / AMP_ATTACK_S).exp();
        self.amp_decay = (-dt / amp_decay_s).exp();
        self.index_peak = index * brightness * (0.65 + 0.35 * amplitude);
        self.index = self.index_peak;
        self.index_decay = (-dt / index_decay_s).exp();
        self.feedback = FEEDBACK_MAX * feedback;
        self.mod_prev = 0.0;
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

        let modulator = (self.mod_phase + self.feedback * self.mod_prev).sin();
        self.mod_prev = modulator;
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

pub struct FmBass {
    sample_rate: f32,
    voice: Voice,
    decay: f32,
    ratio: f32,
    index: f32,
    index_decay: f32,
    feedback: f32,
}

impl FmBass {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate: sample_rate.max(1.0),
            voice: Voice::silent(),
            decay: DEFAULT_FM_BASS_DECAY,
            ratio: DEFAULT_FM_BASS_RATIO,
            index: DEFAULT_FM_BASS_INDEX,
            index_decay: DEFAULT_FM_BASS_INDEX_DECAY,
            feedback: DEFAULT_FM_BASS_FEEDBACK,
        }
    }

    pub fn set_decay(&mut self, value: f32) {
        self.decay = value.clamp(FM_BASS_DECAY_RANGE.0, FM_BASS_DECAY_RANGE.1);
    }

    pub fn set_ratio(&mut self, value: f32) {
        self.ratio = value.clamp(FM_BASS_RATIO_RANGE.0, FM_BASS_RATIO_RANGE.1);
    }

    pub fn set_index(&mut self, value: f32) {
        self.index = value.clamp(FM_BASS_INDEX_RANGE.0, FM_BASS_INDEX_RANGE.1);
    }

    pub fn set_index_decay(&mut self, value: f32) {
        self.index_decay = value.clamp(FM_BASS_INDEX_DECAY_RANGE.0, FM_BASS_INDEX_DECAY_RANGE.1);
    }

    pub fn set_feedback(&mut self, value: f32) {
        self.feedback = value.clamp(FM_BASS_FEEDBACK_RANGE.0, FM_BASS_FEEDBACK_RANGE.1);
    }

    pub fn apply_params(&mut self, params: FmBassParams) {
        let p = params.clamp();
        self.set_decay(p.decay);
        self.set_ratio(p.ratio);
        self.set_index(p.index);
        self.set_index_decay(p.index_decay);
        self.set_feedback(p.feedback);
    }
}

impl StrikeVoice for FmBass {
    fn strike_at(&mut self, note: u8, amplitude: f32, place: f32) {
        if amplitude <= 0.0 {
            return;
        }
        let transposed = if DEFAULT_FM_BASS_OCTAVE < 0 {
            note.saturating_sub((-DEFAULT_FM_BASS_OCTAVE) as u8)
        } else {
            note.saturating_add(DEFAULT_FM_BASS_OCTAVE as u8).min(127)
        };
        let hz = midi_hz(transposed);
        self.voice.trigger(
            hz,
            amplitude,
            place,
            self.sample_rate,
            Shape {
                decay: self.decay,
                ratio: self.ratio,
                index: self.index,
                index_decay: self.index_decay,
                feedback: self.feedback,
            },
        );
    }

    fn next_sample(&mut self) -> f32 {
        flush(self.voice.sample().clamp(-1.0, 1.0))
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
        let mut bass = FmBass::new(RATE);
        for _ in 0..2_000 {
            assert_eq!(bass.next_sample(), 0.0);
        }
    }

    #[test]
    fn strike_rings_and_decays() {
        let mut bass = FmBass::new(RATE);
        bass.strike_at(48, 0.7, 0.0);
        let mut early: f32 = 0.0;
        for _ in 0..(RATE as usize / 15) {
            early = early.max(bass.next_sample().abs());
        }
        assert!(early > 0.02, "early peak {early}");
        for _ in 0..(RATE as usize * 5) {
            let _ = bass.next_sample();
        }
        let mut late: f32 = 0.0;
        for _ in 0..(RATE as usize / 10) {
            late = late.max(bass.next_sample().abs());
        }
        assert!(late < early * 0.08, "early {early} late {late}");
    }

    #[test]
    fn mono_retrigger_replaces_pitch() {
        let mut bass = FmBass::new(RATE);
        bass.strike_at(36, 0.8, 0.0);
        for _ in 0..200 {
            let _ = bass.next_sample();
        }
        bass.strike_at(48, 0.8, 0.0);
        let mut peak = 0.0_f32;
        for _ in 0..(RATE as usize / 20) {
            let sample = bass.next_sample();
            assert!(sample.is_finite());
            peak = peak.max(sample.abs());
        }
        assert!(peak > 0.02, "retrigger should sound, peak {peak}");
    }

    #[test]
    fn output_stays_finite() {
        let mut bass = FmBass::new(RATE);
        bass.set_index(FM_BASS_INDEX_RANGE.1);
        bass.set_feedback(1.0);
        bass.set_ratio(2.0);
        for note in [36u8, 40, 43, 48, 52] {
            bass.strike_at(note, 1.0, 1.0);
            for _ in 0..(RATE as usize / 8) {
                let sample = bass.next_sample();
                assert!(sample.is_finite());
                assert!((-1.0..=1.0).contains(&sample));
            }
        }
    }

    #[test]
    fn default_octave_drops_one_octave() {
        let open = midi_hz(60);
        let bass = midi_hz(60u8.saturating_sub((-DEFAULT_FM_BASS_OCTAVE) as u8));
        assert!((open / bass - 2.0).abs() < 1.0e-5);
    }

    #[test]
    fn recipes_are_clamped() {
        for recipe in FmBassRecipe::ALL {
            let p = recipe.params();
            assert_eq!(p, p.clamp());
            assert!(p.ratio >= 1.0 && p.ratio <= 2.0);
        }
    }
}
