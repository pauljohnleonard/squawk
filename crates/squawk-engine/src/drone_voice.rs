//! Playable Resonator / Waveshape / Tambura voices for live tuning.
//!
//! Same pad layout as Handpan. Strike sets pitch; the tone holds with a slow
//! fade so knobs can be auditioned. Accompaniment still uses Support's wash
//! path; these modules are the main-instrument face of each voice.

use crate::tambura::{DEFAULT_TAMBURA_JAWARI, TAMBURA_JAWARI_RANGE, TamburaBank};
use crate::voice::StrikeVoice;
use core::f32::consts::{LN_2, TAU};

pub const DEFAULT_DRONE_SHAPE: f32 = 0.25;
pub const DRONE_SHAPE_RANGE: (f32, f32) = (0.0, 1.0);
pub const DEFAULT_DRONE_SPREAD: f32 = 12.0;
pub const DRONE_SPREAD_RANGE: (f32, f32) = (0.0, 40.0);
pub const DEFAULT_DRONE_BRIGHTNESS: f32 = 0.3;
pub const DRONE_BRIGHTNESS_RANGE: (f32, f32) = (0.0, 1.0);
pub const DEFAULT_DRONE_STRINGS: f32 = 6.0;
pub const DRONE_STRINGS_RANGE: (f32, f32) = (1.0, 6.0);
pub const DEFAULT_DRONE_FORMANT_1: f32 = 180.0;
pub const DEFAULT_DRONE_FORMANT_2: f32 = 700.0;
pub const DEFAULT_DRONE_FORMANT_3: f32 = 1_800.0;
pub const DEFAULT_DRONE_RESONANCE: f32 = 4.0;
pub const DRONE_FORMANT_RANGE: (f32, f32) = (80.0, 4_000.0);
/// Signed Q: positive boosts a bandpass peak, negative scoops it out (notch-like).
pub const DRONE_RESONANCE_RANGE: (f32, f32) = (-24.0, 24.0);
pub const DEFAULT_DRONE_FORMANT_RATE: f32 = 0.2;
pub const DEFAULT_DRONE_FORMANT_DEPTH: f32 = 0.0;
pub const DRONE_FORMANT_RATE_RANGE: (f32, f32) = (0.0, 4.0);
pub const DRONE_FORMANT_DEPTH_RANGE: (f32, f32) = (0.0, 1.0);
pub const DEFAULT_DRONE_LEVEL: f32 = 1.0;
pub const DRONE_LEVEL_RANGE: (f32, f32) = (0.0, 2.0);

const CENTS_TO_RATIO: f32 = LN_2 / 1200.0;
const WAVE_GAIN: f32 = 0.22;
const UNISON_VOICES: usize = 3;
const BRIGHT_LO: f32 = 400.0;
const BRIGHT_HI: f32 = 6_000.0;
const HOLD_S: f32 = 2.5;
const FADE_S: f32 = 3.5;
const RISE_S: f32 = 0.08;
const RES_ZETA_LO: f32 = 0.008;
const RES_ZETA_HI: f32 = 0.012;
const RES_DRIVE: f32 = 0.045;
const QUIET: f32 = 1.0e-4;

/// Which continuous wash engine is sounding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DroneVoiceKind {
    Resonator = 0,
    Waveshape = 1,
    Tambura = 2,
}

impl DroneVoiceKind {
    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => DroneVoiceKind::Waveshape,
            2 => DroneVoiceKind::Tambura,
            _ => DroneVoiceKind::Resonator,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn name(self) -> &'static str {
        match self {
            DroneVoiceKind::Resonator => "Resonator",
            DroneVoiceKind::Waveshape => "Waveshape",
            DroneVoiceKind::Tambura => "Tambura",
        }
    }
}

#[derive(Clone, Copy)]
struct Formant {
    z1: f32,
    z2: f32,
}

impl Formant {
    fn bandpass(&mut self, sample: f32, hz: f32, q: f32, dt: f32) -> f32 {
        let w0 = TAU * hz * dt;
        let sine = w0.sin();
        let q = q.max(0.05);
        let alpha = sine / (2.0 * q);
        let a0 = 1.0 + alpha;
        let b0 = (sine * 0.5) / a0;
        let b2 = -b0;
        let a1 = (-2.0 * w0.cos()) / a0;
        let a2 = (1.0 - alpha) / a0;
        let y = b0 * sample + self.z1;
        self.z1 = self.z2 - a1 * y;
        self.z2 = b2 * sample - a2 * y;
        if self.z1.abs() < 1.0e-15 {
            self.z1 = 0.0;
        }
        if self.z2.abs() < 1.0e-15 {
            self.z2 = 0.0;
        }
        y
    }
}

/// One playable drone voice (resonator band, waveshape unison, or tambura chord).
pub struct DroneVoice {
    kind: DroneVoiceKind,
    dt: f32,
    hz: f32,
    target_hz: f32,
    presence: f32,
    hold_left: f32,
    shape: f32,
    spread: f32,
    brightness: f32,
    strings: u8,
    jawari: f32,
    formant_hz: [f32; 3],
    formant_q: [f32; 3],
    formant_rate: f32,
    formant_depth: f32,
    formant_phase: f32,
    formants: [Formant; 3],
    // Waveshape oscillators
    unison_phase: [f32; UNISON_VOICES],
    lowpass: f32,
    tambura: TamburaBank,
    // Resonator mode
    position: f32,
    velocity: f32,
    lfo_phase: f32,
    lfo_inc: f32,
    rng: u32,
    fifth_ratio: f32,
}

impl DroneVoice {
    pub fn new(sample_rate: f32, kind: DroneVoiceKind) -> Self {
        let sample_rate = sample_rate.max(1.0);
        let dt = 1.0 / sample_rate;
        Self {
            kind,
            dt,
            hz: 110.0,
            target_hz: 110.0,
            presence: 0.0,
            hold_left: 0.0,
            shape: DEFAULT_DRONE_SHAPE,
            spread: DEFAULT_DRONE_SPREAD,
            brightness: DEFAULT_DRONE_BRIGHTNESS,
            strings: DEFAULT_DRONE_STRINGS as u8,
            jawari: DEFAULT_TAMBURA_JAWARI,
            formant_hz: [
                DEFAULT_DRONE_FORMANT_1,
                DEFAULT_DRONE_FORMANT_2,
                DEFAULT_DRONE_FORMANT_3,
            ],
            formant_q: [DEFAULT_DRONE_RESONANCE; 3],
            formant_rate: DEFAULT_DRONE_FORMANT_RATE,
            formant_depth: DEFAULT_DRONE_FORMANT_DEPTH,
            formant_phase: 0.0,
            formants: [Formant { z1: 0.0, z2: 0.0 }; 3],
            unison_phase: [0.17, 0.41, 0.73],
            lowpass: 0.0,
            tambura: {
                let mut bank = TamburaBank::new(sample_rate);
                // Free-running slow strum while the note is sounding.
                bank.set_auto_pluck(true);
                bank.set_shape(DEFAULT_DRONE_SHAPE);
                bank.set_spread(DEFAULT_DRONE_SPREAD);
                bank.set_jawari(DEFAULT_TAMBURA_JAWARI);
                bank.set_strings(DEFAULT_DRONE_STRINGS);
                bank
            },
            position: 0.0,
            velocity: 0.0,
            lfo_phase: 0.0,
            lfo_inc: 0.35 * dt,
            rng: 0xC0FFEE,
            fifth_ratio: 1.5,
        }
    }

    pub fn kind(&self) -> DroneVoiceKind {
        self.kind
    }

    pub fn set_shape(&mut self, value: f32) {
        self.shape = value.clamp(DRONE_SHAPE_RANGE.0, DRONE_SHAPE_RANGE.1);
        self.tambura.set_shape(self.shape);
    }

    pub fn set_spread(&mut self, value: f32) {
        self.spread = value.clamp(DRONE_SPREAD_RANGE.0, DRONE_SPREAD_RANGE.1);
        self.tambura.set_spread(self.spread);
    }

    pub fn set_brightness(&mut self, value: f32) {
        self.brightness = value.clamp(DRONE_BRIGHTNESS_RANGE.0, DRONE_BRIGHTNESS_RANGE.1);
    }

    pub fn set_strings(&mut self, value: f32) {
        self.strings = value
            .clamp(DRONE_STRINGS_RANGE.0, DRONE_STRINGS_RANGE.1)
            .round() as u8;
        self.tambura.set_strings(self.strings as f32);
    }

    pub fn set_jawari(&mut self, value: f32) {
        self.jawari = value.clamp(TAMBURA_JAWARI_RANGE.0, TAMBURA_JAWARI_RANGE.1);
        self.tambura.set_jawari(self.jawari);
    }

    pub fn set_formant(&mut self, index: usize, hz: f32) {
        if let Some(slot) = self.formant_hz.get_mut(index) {
            *slot = hz.clamp(DRONE_FORMANT_RANGE.0, DRONE_FORMANT_RANGE.1);
        }
    }

    pub fn set_formant_q(&mut self, index: usize, q: f32) {
        if let Some(slot) = self.formant_q.get_mut(index) {
            *slot = q.clamp(DRONE_RESONANCE_RANGE.0, DRONE_RESONANCE_RANGE.1);
        }
    }

    pub fn set_formant_rate(&mut self, value: f32) {
        self.formant_rate = value.clamp(DRONE_FORMANT_RATE_RANGE.0, DRONE_FORMANT_RATE_RANGE.1);
    }

    pub fn set_formant_depth(&mut self, value: f32) {
        self.formant_depth = value.clamp(DRONE_FORMANT_DEPTH_RANGE.0, DRONE_FORMANT_DEPTH_RANGE.1);
    }

    /// Tuning fifth relative to the sounding root (Just or 12-TET from layout).
    pub fn set_fifth_ratio(&mut self, ratio: f32) {
        self.fifth_ratio = ratio.clamp(1.2, 1.9);
        self.tambura.set_fifth_ratio(self.fifth_ratio);
    }

    fn colour(&mut self, body: f32, excite: f32, body_mix: f32, peak_mix: f32) -> f32 {
        let nyquist = 0.45 / self.dt;
        self.formant_phase = (self.formant_phase + self.formant_rate * self.dt).fract();
        let scale = 2.0f32.powf((self.formant_phase * TAU).sin() * self.formant_depth);
        let mut peaks = 0.0;
        for (index, formant) in self.formants.iter_mut().enumerate() {
            let center = (self.formant_hz[index] * scale).clamp(20.0, nyquist);
            let signed = self.formant_q[index];
            let mag = signed.abs().max(0.05);
            let sign = if signed >= 0.0 { 1.0 } else { -1.0 };
            peaks += sign * formant.bandpass(excite, center, mag, self.dt);
        }
        let y = body * body_mix + peaks * peak_mix;
        if y.abs() < 1.0e-15 { 0.0 } else { y }
    }

    fn step_resonator(&mut self) -> (f32, f32) {
        self.hz += (self.target_hz - self.hz) * (1.0 - (-self.dt / 0.12).exp());
        self.lfo_phase = (self.lfo_phase + self.lfo_inc).fract();
        let open = 0.5 + 0.5 * (self.lfo_phase * TAU).cos();
        let zeta = RES_ZETA_LO + (RES_ZETA_HI - RES_ZETA_LO) * open;
        let omega = TAU * self.hz.max(20.0);
        let stiffness = omega * omega;
        let damping = 2.0 * zeta * omega;
        let drive = noise(&mut self.rng) * RES_DRIVE;
        let acceleration = drive * stiffness - stiffness * self.position - damping * self.velocity;
        self.velocity = (self.velocity + acceleration * self.dt).clamp(-8_000.0, 8_000.0);
        self.position = (self.position + self.velocity * self.dt).clamp(-2.0, 2.0);
        let band = self.velocity / omega;
        (band, band)
    }

    fn step_waveshape(&mut self) -> (f32, f32) {
        self.hz += (self.target_hz - self.hz) * (1.0 - (-self.dt / 0.08).exp());
        let half = self.spread * 0.5;
        let mut sum = 0.0;
        let mut excite = 0.0;
        for (index, offset) in [-half, 0.0, half].into_iter().enumerate() {
            let hz = (self.hz * (offset * CENTS_TO_RATIO).exp()).max(20.0);
            self.unison_phase[index] = (self.unison_phase[index] + hz * self.dt).fract();
            let sine = (self.unison_phase[index] * TAU).sin();
            sum += waveshape(sine, self.shape);
            excite += string_buzz(sine);
        }
        let scale = WAVE_GAIN / UNISON_VOICES as f32;
        let mixed = sum * scale;
        let cutoff = BRIGHT_LO * (BRIGHT_HI / BRIGHT_LO).powf(self.brightness);
        let coeff = (-TAU * cutoff * self.dt).exp();
        self.lowpass += (1.0 - coeff) * (mixed - self.lowpass);
        if self.lowpass.abs() < 1.0e-15 {
            self.lowpass = 0.0;
        }
        (self.lowpass, excite * scale)
    }

    fn step_tambura(&mut self) -> (f32, f32) {
        self.hz += (self.target_hz - self.hz) * (1.0 - (-self.dt / 0.08).exp());
        self.tambura.set_pitch_hz(self.hz);
        self.tambura.set_fifth_ratio(self.fifth_ratio);
        self.tambura.set_shape(self.shape);
        self.tambura.set_spread(self.spread);
        self.tambura.set_jawari(self.jawari);
        self.tambura.set_strings(self.strings as f32);
        self.tambura.set_auto_pluck(true);
        self.tambura.next_sample()
    }

    fn render(&mut self) -> f32 {
        let (body, excite, body_mix, peak_mix) = match self.kind {
            DroneVoiceKind::Resonator => {
                let (b, e) = self.step_resonator();
                (b, e, 1.0, 0.22)
            }
            DroneVoiceKind::Waveshape => {
                let (b, e) = self.step_waveshape();
                (b, e, 1.0, 0.12)
            }
            DroneVoiceKind::Tambura => {
                let (b, e) = self.step_tambura();
                (b, e, 0.35, 0.28)
            }
        };
        self.colour(body, excite, body_mix, peak_mix)
    }
}

impl StrikeVoice for DroneVoice {
    fn strike_at(&mut self, note: u8, amplitude: f32, _place: f32) {
        let amplitude = amplitude.clamp(0.0, 1.0);
        if amplitude <= 0.0 {
            return;
        }
        self.target_hz = midi_hz(note);
        self.hz = self.target_hz;
        self.tambura.set_pitch_hz(self.target_hz);
        self.hold_left = HOLD_S;
        // Soft re-attack toward the new note level.
        let target = 0.55 + 0.45 * amplitude;
        self.presence = self.presence.max(target * 0.35);
        self.presence = (self.presence + (target - self.presence) * 0.65).clamp(0.0, 1.0);
        if self.kind == DroneVoiceKind::Tambura {
            self.tambura.pluck(amplitude);
        }
    }

    fn next_sample(&mut self) -> f32 {
        if self.kind == DroneVoiceKind::Tambura {
            // Waveguides carry the ring — keep the amp open until the bank is spent.
            if self.hold_left > 0.0 {
                self.hold_left -= self.dt;
                let rise = 1.0 - (-self.dt / RISE_S).exp();
                self.presence += (1.0 - self.presence) * rise * 0.35;
            }
            let energy = self.tambura.energy();
            if energy > 1.0e-8 {
                self.presence = self.presence.max(0.9);
            } else if self.presence > QUIET {
                let fall = 1.0 - (-self.dt / (FADE_S * 0.5)).exp();
                self.presence += (0.0 - self.presence) * fall;
            } else {
                self.presence = 0.0;
                return 0.0;
            }
            let sample = self.render() * self.presence;
            return if sample.abs() < 1.0e-15 {
                0.0
            } else {
                sample.clamp(-1.0, 1.0)
            };
        }

        if self.hold_left > 0.0 {
            self.hold_left -= self.dt;
            let rise = 1.0 - (-self.dt / RISE_S).exp();
            self.presence += (1.0 - self.presence) * rise * 0.35;
        } else if self.presence > QUIET {
            let fall = 1.0 - (-self.dt / FADE_S).exp();
            self.presence += (0.0 - self.presence) * fall;
        } else {
            self.presence = 0.0;
            return 0.0;
        }
        let sample = self.render() * self.presence;
        if sample.abs() < 1.0e-15 {
            0.0
        } else {
            sample.clamp(-1.0, 1.0)
        }
    }
}

fn midi_hz(note: u8) -> f32 {
    440.0 * (2.0_f32).powf((note as f32 - 69.0) / 12.0)
}

fn string_buzz(sine: f32) -> f32 {
    (sine * 5.0).tanh()
}

fn waveshape(sine: f32, shape: f32) -> f32 {
    let poly = (shape / 0.6).clamp(0.0, 1.0);
    let cubic = sine * sine * sine;
    let fifth = cubic * sine * sine;
    let odd = (sine + poly * 0.35 * cubic + poly * poly * 0.15 * fifth) / (1.0 + 0.5 * poly);
    if shape <= 0.6 {
        return odd;
    }
    let amount = ((shape - 0.6) / 0.4).clamp(0.0, 1.0);
    let threshold = 1.0 - 0.55 * amount;
    fold_once(fold_once(odd, threshold), threshold)
}

fn fold_once(sample: f32, threshold: f32) -> f32 {
    if sample.abs() <= threshold {
        sample
    } else {
        sample.signum() * (2.0 * threshold - sample.abs())
    }
}

fn noise(rng: &mut u32) -> f32 {
    *rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    let unit = ((*rng >> 8) & 0x00FF_FFFF) as f32 / 16_777_216.0;
    unit * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    #[test]
    fn silent_until_struck() {
        let mut voice = DroneVoice::new(RATE, DroneVoiceKind::Tambura);
        for _ in 0..2_000 {
            assert_eq!(voice.next_sample(), 0.0);
        }
    }

    #[test]
    fn each_kind_rings() {
        for kind in [
            DroneVoiceKind::Resonator,
            DroneVoiceKind::Waveshape,
            DroneVoiceKind::Tambura,
        ] {
            let mut voice = DroneVoice::new(RATE, kind);
            voice.strike_at(50, 0.8, 0.0);
            let mut peak = 0.0_f32;
            for _ in 0..(RATE as usize / 4) {
                peak = peak.max(voice.next_sample().abs());
            }
            assert!(peak > 0.01, "{:?} peak {peak}", kind);
        }
    }

    #[test]
    fn output_stays_finite() {
        let mut voice = DroneVoice::new(RATE, DroneVoiceKind::Waveshape);
        for note in [48u8, 55, 62, 69] {
            voice.strike_at(note, 1.0, 0.5);
            for _ in 0..(RATE as usize / 10) {
                let s = voice.next_sample();
                assert!(s.is_finite());
                assert!((-1.0..=1.0).contains(&s));
            }
        }
    }
}
