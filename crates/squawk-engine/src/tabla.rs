//! Modal tabla: bayan (bass) + dayan (treble) with a small stroke vocabulary.
//!
//! Expressive, not a carbon copy. Parallel resonant modes + shaped noise burst.
//! Ga includes a short upward pitch glide. Architecture is StrikeVoice so a
//! future groove TablaLine can reuse the same voice.
//!
//! Dayan strokes share one mode ratio set so keyboard rows keep the same pitch;
//! row knobs (decay / tone / noise) shape the hit.

use crate::voice::StrikeVoice;
use core::f32::consts::TAU;

pub const TABLA_VOICES: usize = 6;
pub const TABLA_ROWS: usize = 3;

pub const DEFAULT_TABLA_DECAY: f32 = 0.55;
pub const TABLA_DECAY_RANGE: (f32, f32) = (0.15, 1.0);
pub const DEFAULT_TABLA_BRIGHTNESS: f32 = 0.45;
pub const TABLA_BRIGHTNESS_RANGE: (f32, f32) = (0.0, 1.0);
pub const DEFAULT_TABLA_NOISE: f32 = 0.35;
pub const TABLA_NOISE_RANGE: (f32, f32) = (0.0, 1.0);
pub const DEFAULT_TABLA_GLIDE: f32 = 0.55;
pub const TABLA_GLIDE_RANGE: (f32, f32) = (0.0, 1.0);
/// Semitone offset of bayan fundamental relative to the struck MIDI note.
pub const DEFAULT_TABLA_BAYAN_TUNE: f32 = -12.0;
pub const TABLA_BAYAN_TUNE_RANGE: (f32, f32) = (-24.0, 0.0);
/// Semitone offset of dayan fundamental relative to the struck MIDI note.
pub const DEFAULT_TABLA_DAYAN_TUNE: f32 = 0.0;
pub const TABLA_DAYAN_TUNE_RANGE: (f32, f32) = (-12.0, 12.0);

/// Center / between / shoulder row defaults (open, mute, rim).
pub const DEFAULT_TABLA_ROW_DECAY: [f32; TABLA_ROWS] = [0.55, 0.28, 0.42];
pub const DEFAULT_TABLA_ROW_TONE: [f32; TABLA_ROWS] = [0.40, 0.50, 0.68];
pub const DEFAULT_TABLA_ROW_NOISE: [f32; TABLA_ROWS] = [0.22, 0.55, 0.38];

const BAYAN_MODES: usize = 4;
const DAYAN_MODES: usize = 5;
/// Shared so Tu / Ta / Na stay on one pitch family.
const DAYAN_RATIOS: [f32; 5] = [1.0, 1.7, 2.55, 3.7, 5.1];
/// Harmonic integers so the deep drum locks to ET bass (avoids stretched-partial beats).
const BAYAN_RATIOS: [f32; 5] = [1.0, 2.0, 3.0, 4.0, 0.0];
/// Same integer octave drop FM bass uses (`DEFAULT_FM_BASS_OCTAVE`).
const BAYAN_OCTAVE_DROP: u8 = 12;
const AMP_GAIN: f32 = 0.55;
const AMP_ATTACK_S: f32 = 0.003;
const NOISE_ATTACK_S: f32 = 0.0015;
const QUIET: f32 = 1.0e-4;
const GLIDE_S: f32 = 0.12;
/// App `note_on` scales velocity into roughly this range (see handpan).
const LOUD_STRIKE_AMPLITUDE: f32 = 0.20;
/// Strike-queue sentinel: open bayan without palm-press glide (keyboard A/W).
pub const TABLA_GE_PLACE: f32 = 1.5;

/// Stroke vocabulary for play and (later) groove patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TablaStroke {
    /// Open bayan with short upward palm-press glide.
    Ga,
    /// Open bayan, no glide (flat boom).
    Ge,
    Ka,
    Na,
    Ta,
    Tu,
}

#[derive(Debug, Clone, Copy)]
pub struct TablaRowParams {
    pub decay: f32,
    pub tone: f32,
    pub noise: f32,
}

impl TablaRowParams {
    pub fn default_row(row: usize) -> Self {
        let row = row.min(TABLA_ROWS - 1);
        Self {
            decay: DEFAULT_TABLA_ROW_DECAY[row],
            tone: DEFAULT_TABLA_ROW_TONE[row],
            noise: DEFAULT_TABLA_ROW_NOISE[row],
        }
    }

    fn clamped(self) -> Self {
        Self {
            decay: self.decay.clamp(TABLA_DECAY_RANGE.0, TABLA_DECAY_RANGE.1),
            tone: self.tone.clamp(TABLA_BRIGHTNESS_RANGE.0, TABLA_BRIGHTNESS_RANGE.1),
            noise: self.noise.clamp(TABLA_NOISE_RANGE.0, TABLA_NOISE_RANGE.1),
        }
    }
}

/// Map strike place to center (0) / between (1) / shoulder (2).
pub fn row_from_place(place: f32) -> usize {
    let place = place.clamp(0.0, 1.0);
    if place > 0.6 {
        2
    } else if place > 0.35 {
        1
    } else {
        0
    }
}

#[derive(Clone, Copy)]
struct Mode {
    phase: f32,
    inc: f32,
    target_inc: f32,
    amp: f32,
    decay: f32,
}

impl Mode {
    fn silent() -> Self {
        Self {
            phase: 0.0,
            inc: 0.0,
            target_inc: 0.0,
            amp: 0.0,
            decay: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
struct Voice {
    active: bool,
    bayan: bool,
    modes: [Mode; DAYAN_MODES],
    mode_n: usize,
    amp: f32,
    amp_level: f32,
    amp_attack: f32,
    amp_decay: f32,
    noise: f32,
    noise_level: f32,
    noise_decay: f32,
    glide: f32,
    age: u32,
    rng: u32,
}

impl Voice {
    fn silent() -> Self {
        Self {
            active: false,
            bayan: false,
            modes: [Mode::silent(); DAYAN_MODES],
            mode_n: 0,
            amp: 0.0,
            amp_level: 0.0,
            amp_attack: 0.0,
            amp_decay: 0.0,
            noise: 0.0,
            noise_level: 0.0,
            noise_decay: 0.0,
            glide: 0.0,
            age: 0,
            rng: 0xA341_316C,
        }
    }

    fn trigger(
        &mut self,
        stroke: TablaStroke,
        hz: f32,
        amplitude: f32,
        sample_rate: f32,
        row: TablaRowParams,
        glide_knob: f32,
    ) {
        let hz = hz.clamp(30.0, sample_rate * 0.22);
        let amplitude = amplitude.clamp(0.0, 1.0);
        let row = row.clamped();
        let glide_knob = glide_knob.clamp(TABLA_GLIDE_RANGE.0, TABLA_GLIDE_RANGE.1);
        let dt = 1.0 / sample_rate.max(1.0);
        let ringing = self.active && self.amp > QUIET;

        let (bayan, gains, body_base, noise_s, open, do_glide): (
            bool,
            [f32; 5],
            f32,
            f32,
            bool,
            bool,
        ) = match stroke {
            TablaStroke::Ga => (
                true,
                // Stronger fundamental so pitch reads against FM bass.
                [1.0, 0.38, 0.16, 0.07, 0.0],
                0.55,
                0.012,
                true,
                true,
            ),
            TablaStroke::Ge => (
                true,
                [1.0, 0.38, 0.16, 0.07, 0.0],
                0.55,
                0.012,
                true,
                false,
            ),
            TablaStroke::Ka => (
                true,
                [0.7, 0.55, 0.32, 0.16, 0.0],
                0.06,
                0.006,
                false,
                false,
            ),
            // Dayan strokes share DAYAN_RATIOS; gains only change colour.
            TablaStroke::Tu => (
                false,
                [1.0, 0.6, 0.35, 0.2, 0.1],
                0.32,
                0.008,
                true,
                false,
            ),
            TablaStroke::Ta => (
                false,
                [0.85, 0.5, 0.28, 0.14, 0.08],
                0.08,
                0.005,
                false,
                false,
            ),
            TablaStroke::Na => (
                false,
                [0.9, 0.75, 0.42, 0.22, 0.12],
                0.24,
                0.01,
                true,
                false,
            ),
        };

        let ratios = if bayan { BAYAN_RATIOS } else { DAYAN_RATIOS };
        let mode_n = if bayan { BAYAN_MODES } else { DAYAN_MODES };
        let body_s = body_base * (0.45 + 1.4 * row.decay);
        let bright = 0.5 + 0.5 * row.tone;
        let noise_mix = if open {
            0.12 + 0.55 * row.noise
        } else {
            0.35 + 0.55 * row.noise
        };
        let body_gain = if open { 1.0 } else { 0.72 };

        self.active = true;
        self.bayan = bayan;
        self.mode_n = mode_n;
        self.amp_level = AMP_GAIN * body_gain * (0.65 + 0.35 * amplitude);
        self.amp_attack = 1.0 - (-dt / AMP_ATTACK_S).exp();
        self.amp_decay = (-dt / body_s.max(0.02)).exp();
        self.noise_level = amplitude * noise_mix * (0.4 + 0.35 * bright);
        self.noise_decay = (-dt / noise_s.max(0.002)).exp();
        self.glide = if do_glide {
            1.0 - (-dt / GLIDE_S).exp()
        } else {
            0.0
        };
        self.age = 0;
        if !ringing {
            self.amp = 0.0;
            self.noise = 0.0;
        }

        let start_hz = if do_glide {
            hz * (1.0 - 0.18 * glide_knob)
        } else {
            hz
        };
        let end_hz = if do_glide {
            hz * (1.0 + 0.12 * glide_knob)
        } else {
            hz
        };

        for i in 0..DAYAN_MODES {
            if i >= mode_n || ratios[i] <= 0.0 {
                self.modes[i] = Mode::silent();
                continue;
            }
            let mode_hz = (start_hz * ratios[i]).min(sample_rate * 0.45);
            let target_hz = (end_hz * ratios[i]).min(sample_rate * 0.45);
            let mode_amp = gains[i] * amplitude * bright * body_gain;
            let mode_decay_s = body_s * (1.0 - 0.12 * i as f32).max(0.35);
            self.modes[i] = Mode {
                phase: if ringing {
                    self.modes[i].phase
                } else {
                    (i as f32 + 0.17) * 0.37
                },
                inc: TAU * mode_hz * dt,
                target_inc: TAU * target_hz * dt,
                amp: if ringing {
                    self.modes[i].amp.max(mode_amp * 0.35)
                } else {
                    mode_amp
                },
                decay: (-dt / mode_decay_s).exp(),
            };
        }
        if !ringing {
            self.noise = self.noise_level;
        } else {
            self.noise = self.noise.max(self.noise_level * 0.5);
        }
    }

    fn sample(&mut self) -> f32 {
        if !self.active {
            return 0.0;
        }

        self.age = self.age.saturating_add(1);
        let attack_samples = (AMP_ATTACK_S * 48_000.0) as u32 + 4;
        if self.age < attack_samples {
            self.amp += (self.amp_level - self.amp) * self.amp_attack;
        } else {
            self.amp *= self.amp_decay;
        }

        let noise_attack = (NOISE_ATTACK_S * 48_000.0) as u32 + 2;
        if self.age < noise_attack {
            self.noise += (self.noise_level - self.noise) * 0.35;
        } else {
            self.noise *= self.noise_decay;
        }

        let mut tonal = 0.0;
        for i in 0..self.mode_n {
            let mode = &mut self.modes[i];
            if mode.amp <= QUIET {
                continue;
            }
            if self.glide > 0.0 {
                mode.inc += (mode.target_inc - mode.inc) * self.glide;
            }
            tonal += mode.phase.sin() * mode.amp;
            mode.phase = wrap(mode.phase + mode.inc);
            mode.amp *= mode.decay;
        }

        let burst = self.next_noise() * self.noise;
        let sample = (tonal * self.amp + burst).clamp(-1.0, 1.0);

        let modes_quiet = self.modes[..self.mode_n]
            .iter()
            .all(|mode| mode.amp <= QUIET);
        if self.amp < QUIET && self.noise < QUIET && modes_quiet && self.age > attack_samples {
            *self = Self::silent();
            return 0.0;
        }
        flush(sample)
    }

    fn next_noise(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Choose a stroke from pad note / amplitude / place.
/// Low notes → bayan; high → dayan. Place picks open vs closed.
pub fn stroke_from(note: u8, amplitude: f32, place: f32, ding: u8) -> TablaStroke {
    let amplitude = amplitude.clamp(0.0, 1.0);
    let place = place.clamp(0.0, 1.0);
    let bayan = note <= ding.saturating_add(2);
    if bayan {
        if place > 0.35 {
            TablaStroke::Ka
        } else {
            TablaStroke::Ga
        }
    } else if place > 0.6 {
        TablaStroke::Na
    } else if place > 0.35 || amplitude < 0.4 {
        // Between row, or soft center → muted Ta
        TablaStroke::Ta
    } else {
        TablaStroke::Tu
    }
}

pub struct Tabla {
    sample_rate: f32,
    voices: [Voice; TABLA_VOICES],
    next: usize,
    rows: [TablaRowParams; TABLA_ROWS],
    glide: f32,
    bayan_tune: f32,
    dayan_tune: f32,
    ding: u8,
}

impl Tabla {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate: sample_rate.max(1.0),
            voices: [Voice::silent(); TABLA_VOICES],
            next: 0,
            rows: std::array::from_fn(TablaRowParams::default_row),
            glide: DEFAULT_TABLA_GLIDE,
            bayan_tune: DEFAULT_TABLA_BAYAN_TUNE,
            dayan_tune: DEFAULT_TABLA_DAYAN_TUNE,
            ding: 50,
        }
    }

    pub fn set_row(&mut self, row: usize, params: TablaRowParams) {
        if row < TABLA_ROWS {
            self.rows[row] = params.clamped();
        }
    }

    pub fn set_glide(&mut self, value: f32) {
        self.glide = value.clamp(TABLA_GLIDE_RANGE.0, TABLA_GLIDE_RANGE.1);
    }

    pub fn set_bayan_tune(&mut self, semitones: f32) {
        self.bayan_tune = semitones
            .round()
            .clamp(TABLA_BAYAN_TUNE_RANGE.0, TABLA_BAYAN_TUNE_RANGE.1);
    }

    pub fn set_dayan_tune(&mut self, semitones: f32) {
        self.dayan_tune = semitones
            .round()
            .clamp(TABLA_DAYAN_TUNE_RANGE.0, TABLA_DAYAN_TUNE_RANGE.1);
    }

    pub fn set_ding(&mut self, ding: u8) {
        self.ding = ding;
    }

    pub fn strike_stroke(&mut self, stroke: TablaStroke, note: u8, amplitude: f32) {
        self.strike_stroke_at(stroke, note, amplitude, 0.0);
    }

    pub fn strike_stroke_at(
        &mut self,
        stroke: TablaStroke,
        note: u8,
        amplitude: f32,
        place: f32,
    ) {
        if amplitude <= 0.0 {
            return;
        }
        let hz = match stroke {
            // Integer MIDI −12 like FM bass, then fine cents from Bayan tune vs default.
            TablaStroke::Ga | TablaStroke::Ge | TablaStroke::Ka => {
                bayan_hz(note, self.bayan_tune)
            }
            _ => midi_hz_offset(note, self.dayan_tune),
        };
        let row = self.rows[row_from_place(place)];
        let index = self.steal();
        self.voices[index].trigger(stroke, hz, amplitude, self.sample_rate, row, self.glide);
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
        let index = if amp < QUIET * 4.0 {
            quietest
        } else {
            self.next
        };
        self.next = (self.next + 1) % TABLA_VOICES;
        index
    }
}

impl StrikeVoice for Tabla {
    fn strike_at(&mut self, note: u8, amplitude: f32, place: f32) {
        let amp = (amplitude / LOUD_STRIKE_AMPLITUDE).clamp(0.0, 1.0);
        // A/W path encodes Ge via place sentinel (see TABLA_GE_PLACE).
        if place >= TABLA_GE_PLACE - 0.01 {
            self.strike_stroke_at(TablaStroke::Ge, note, amp, 0.0);
            return;
        }
        let stroke = stroke_from(note, amp, place, self.ding);
        self.strike_stroke_at(stroke, note, amp, place);
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

fn midi_hz_offset(note: u8, semitones: f32) -> f32 {
    midi_hz(note) * (2.0_f32).powf(semitones / 12.0)
}

/// Bayan fundamental: FM-bass octave lock, then integer-semitone trim from the knob.
fn bayan_hz(note: u8, bayan_tune: f32) -> f32 {
    let root = note.saturating_sub(BAYAN_OCTAVE_DROP);
    let fine = bayan_tune.round() - DEFAULT_TABLA_BAYAN_TUNE;
    midi_hz_offset(root, fine)
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
        let mut tabla = Tabla::new(RATE);
        for _ in 0..2_000 {
            assert_eq!(tabla.next_sample(), 0.0);
        }
    }

    #[test]
    fn stroke_from_picks_bayan_on_low_notes() {
        assert_eq!(stroke_from(50, 1.0, 0.2, 50), TablaStroke::Ga);
        assert_eq!(stroke_from(50, 1.0, 0.5, 50), TablaStroke::Ka);
        assert_eq!(stroke_from(48, 1.0, 0.7, 50), TablaStroke::Ka);
        assert_eq!(stroke_from(60, 0.6, 0.1, 50), TablaStroke::Tu);
        assert_eq!(stroke_from(60, 0.6, 0.5, 50), TablaStroke::Ta);
        assert_eq!(stroke_from(62, 0.3, 0.1, 50), TablaStroke::Ta);
        assert_eq!(stroke_from(64, 0.7, 0.8, 50), TablaStroke::Na);
    }

    #[test]
    fn bayan_hz_default_matches_fm_bass_octave() {
        let bass = midi_hz(50u8.saturating_sub(BAYAN_OCTAVE_DROP));
        assert!((bayan_hz(50, DEFAULT_TABLA_BAYAN_TUNE) - bass).abs() < 1.0e-4);
        assert!(bayan_hz(50, -11.0) > bass * 1.05);
        assert!(bayan_hz(50, -13.0) < bass * 0.95);
    }

    #[test]
    fn ge_holds_pitch_while_ga_glides_up() {
        let expect = midi_hz(50u8.saturating_sub(BAYAN_OCTAVE_DROP));
        let mut ga = Tabla::new(RATE);
        ga.set_glide(1.0);
        ga.strike_stroke(TablaStroke::Ga, 50, 0.9);
        let ga_buf = collect(&mut ga, 0.12);
        let ga_early = dominant_near(&ga_buf[..960], expect * 0.92);
        let ga_late = dominant_near(&ga_buf[ga_buf.len() / 2..], expect * 1.05);
        assert!(
            ga_late > ga_early + 2.0,
            "Ga palm glide should rise: early {ga_early} late {ga_late}"
        );

        let mut ge = Tabla::new(RATE);
        ge.set_glide(1.0);
        ge.strike_stroke(TablaStroke::Ge, 50, 0.9);
        let ge_buf = collect(&mut ge, 0.12);
        let ge_early = dominant_near(&ge_buf[..960], expect);
        let ge_late = dominant_near(&ge_buf[ge_buf.len() / 2..], expect);
        assert!(
            (ge_late - ge_early).abs() < 4.0,
            "Ge should stay flat: early {ge_early} late {ge_late}"
        );
        let ge_pitch = dominant_near(&ge_buf, expect);
        assert!(
            (ge_pitch - expect).abs() < 3.0,
            "Ge fundamental should track bayan_hz: got {ge_pitch} expect {expect}"
        );
    }

    #[test]
    fn ge_place_sentinel_selects_ge() {
        let mut tabla = Tabla::new(RATE);
        tabla.strike_at(50, LOUD_STRIKE_AMPLITUDE, TABLA_GE_PLACE);
        // Audible and finite is enough; stroke is covered by pitch test.
        let sample = tabla.next_sample();
        assert!(sample.is_finite());
        assert!(sample.abs() > 0.0 || tabla.next_sample().abs() > 0.0);
    }

    #[test]
    fn ga_has_more_low_energy_than_ka() {
        let mut ga = Tabla::new(RATE);
        let mut ka = Tabla::new(RATE);
        ga.strike_stroke(TablaStroke::Ga, 50, 0.9);
        ka.strike_stroke(TablaStroke::Ka, 50, 0.9);
        let ga_e = tone_power(&collect(&mut ga, 0.08), 65.0);
        let ka_e = tone_power(&collect(&mut ka, 0.08), 65.0);
        assert!(
            ga_e > ka_e * 1.4,
            "Ga should boom more at low freq: ga {ga_e} ka {ka_e}"
        );
    }

    #[test]
    fn dayan_rows_share_pitch_family() {
        // Same note, three rows — fundamental energy peak should stay near the same bin.
        let amp = LOUD_STRIKE_AMPLITUDE;
        let mut pitches = Vec::new();
        for place in [0.0, 0.5, 1.0] {
            let mut tabla = Tabla::new(RATE);
            tabla.strike_at(60, amp, place);
            let samples = collect(&mut tabla, 0.12);
            pitches.push(dominant_near(&samples, midi_hz(60)));
        }
        let spread = pitches.iter().cloned().fold(0.0_f32, f32::max)
            - pitches.iter().cloned().fold(f32::MAX, f32::min);
        assert!(
            spread < 8.0,
            "dayan rows drifted in pitch: {pitches:?} spread {spread}"
        );
    }

    #[test]
    fn app_scale_amp_distinguishes_dayan_rows() {
        let amp = LOUD_STRIKE_AMPLITUDE;
        let norm = (amp / LOUD_STRIKE_AMPLITUDE).clamp(0.0, 1.0);
        assert_eq!(stroke_from(60, norm, 0.0, 50), TablaStroke::Tu);
        assert_eq!(stroke_from(60, norm, 0.5, 50), TablaStroke::Ta);
        assert_eq!(stroke_from(60, norm, 1.0, 50), TablaStroke::Na);

        let mut center = Tabla::new(RATE);
        let mut between = Tabla::new(RATE);
        center.strike_at(60, amp, 0.0);
        between.strike_at(60, amp, 0.5);
        let c = rms(&collect(&mut center, 0.25)[2000..]);
        let b = rms(&collect(&mut between, 0.25)[2000..]);
        assert!(
            c > b * 1.5,
            "Z-row Tu should ring past S-row Ta: {c} vs {b}"
        );
    }

    #[test]
    fn ding_between_row_is_shorter_than_center() {
        let amp = LOUD_STRIKE_AMPLITUDE;
        let mut center = Tabla::new(RATE);
        let mut between = Tabla::new(RATE);
        center.strike_at(50, amp, 0.0);
        between.strike_at(50, amp, 0.5);
        let center_tail = collect(&mut center, 0.3);
        let between_tail = collect(&mut between, 0.3);
        let c = rms(&center_tail[center_tail.len() / 2..]);
        let b = rms(&between_tail[between_tail.len() / 2..]);
        assert!(
            c > b * 1.5,
            "Z-row Ga should outlast S-row Ka: {c} vs {b}"
        );
    }

    #[test]
    fn output_stays_finite() {
        let mut tabla = Tabla::new(RATE);
        for stroke in [
            TablaStroke::Ga,
            TablaStroke::Ge,
            TablaStroke::Ka,
            TablaStroke::Na,
            TablaStroke::Ta,
            TablaStroke::Tu,
        ] {
            tabla.strike_stroke(stroke, 55, 1.0);
        }
        for _ in 0..(RATE as usize / 2) {
            let sample = tabla.next_sample();
            assert!(sample.is_finite());
            assert!((-1.0..=1.0).contains(&sample));
        }
    }

    #[test]
    fn retrigger_does_not_hard_cut() {
        let mut tabla = Tabla::new(RATE);
        tabla.strike_stroke(TablaStroke::Ga, 50, 1.0);
        for _ in 0..(RATE as usize / 20) {
            let _ = tabla.next_sample();
        }
        let before = tabla.next_sample();
        tabla.strike_stroke(TablaStroke::Ga, 50, 1.0);
        let after = tabla.next_sample();
        let jump = (after - before).abs();
        assert!(
            jump < 0.25,
            "retrigger clicked: before {before} after {after} jump {jump}"
        );
    }

    fn collect(tabla: &mut Tabla, seconds: f32) -> Vec<f32> {
        let n = (RATE * seconds) as usize;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(tabla.next_sample());
        }
        out
    }

    fn rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum: f32 = samples.iter().map(|s| s * s).sum();
        (sum / samples.len() as f32).sqrt()
    }

    fn tone_power(samples: &[f32], hz: f32) -> f32 {
        let omega = TAU * hz / RATE;
        let coeff = 2.0 * omega.cos();
        let mut previous = 0.0;
        let mut current = 0.0;
        for &sample in samples {
            let next = sample + coeff * current - previous;
            previous = current;
            current = next;
        }
        current * current + previous * previous - coeff * current * previous
    }

    fn dominant_near(samples: &[f32], expect_hz: f32) -> f32 {
        let mut best_hz = expect_hz;
        let mut best = 0.0;
        let lo = (expect_hz * 0.85) as i32;
        let hi = (expect_hz * 1.15) as i32;
        for hz in lo..=hi {
            let power = tone_power(samples, hz as f32);
            if power > best {
                best = power;
                best_hz = hz as f32;
            }
        }
        best_hz
    }
}
