//! A quiet layer under the handpan. The drones are wide resonant bands around
//! notes of the current scale. The band slowly opens and closes, and its center
//! wanders a little, so the pad does not sit on one sine.

use core::f32::consts::TAU;

pub use crate::tambura::{
    DEFAULT_TAMBURA_CYCLE, DEFAULT_TAMBURA_JAWARI, DEFAULT_TAMBURA_PICK, DEFAULT_TAMBURA_STRUM,
    TAMBURA_CYCLE_RANGE, TAMBURA_JAWARI_RANGE, TAMBURA_PICK_RANGE, TAMBURA_STRUM_RANGE,
};
use crate::{
    HANDPAN_DING_MAX, HANDPAN_DING_MIN, HANDPAN_SCALE_COUNT, TamburaBank, handpan_interval_hz,
    handpan_notes,
};

pub const DEFAULT_SUPPORT_LEVEL: f32 = 0.35;
pub const DEFAULT_SUPPORT_PULSE: f32 = 0.55;
pub const DEFAULT_SUPPORT_WIDTH: f32 = 1.0;
pub const DEFAULT_SUPPORT_WANDER: f32 = 14.0;
pub const DEFAULT_SUPPORT_DRIFT: f32 = 26.0;
pub const DEFAULT_SUPPORT_MOTION: f32 = 1.0;
pub const DEFAULT_SUPPORT_FADE: f32 = 4.0;
pub const SUPPORT_LEVEL_RANGE: (f32, f32) = (0.0, 1.0);
pub const SUPPORT_PULSE_RANGE: (f32, f32) = (0.0, 1.0);
pub const SUPPORT_WIDTH_RANGE: (f32, f32) = (0.0, 1.0);
pub const SUPPORT_WANDER_RANGE: (f32, f32) = (0.0, 40.0);
pub const SUPPORT_DRIFT_RANGE: (f32, f32) = (0.0, 60.0);
pub const SUPPORT_MOTION_RANGE: (f32, f32) = (0.25, 3.0);
pub const SUPPORT_FADE_RANGE: (f32, f32) = (0.5, 12.0);
/// 1 keeps each part at the level it has today. 0 is silent and 2 is twice as loud.
pub const DEFAULT_MIX_LEVEL: f32 = 1.0;
pub const MIX_LEVEL_RANGE: (f32, f32) = (0.0, 2.0);
/// 0.5 is the center. Both speakers keep today's level.
pub const DEFAULT_PAN: f32 = 0.5;
pub const PAN_RANGE: (f32, f32) = (0.0, 1.0);
pub const DEFAULT_FORMANT_RATE: f32 = 0.2;
pub const DEFAULT_FORMANT_DEPTH: f32 = 0.0;
pub const FORMANT_RATE_RANGE: (f32, f32) = (0.0, 4.0);
pub const FORMANT_DEPTH_RANGE: (f32, f32) = (0.0, 1.0);
pub const DEFAULT_DING_WASH: u8 = 0;
pub const DEFAULT_DING_SHAPE: f32 = 0.25;
pub const DEFAULT_DING_SPREAD: f32 = 12.0;
pub const DEFAULT_DING_BRIGHTNESS: f32 = 0.3;
pub const DING_SHAPE_RANGE: (f32, f32) = (0.0, 1.0);
pub const DING_SPREAD_RANGE: (f32, f32) = (0.0, 40.0);
pub const DING_BRIGHTNESS_RANGE: (f32, f32) = (0.0, 1.0);
pub const DEFAULT_DING_STRINGS: f32 = 6.0;
pub const DEFAULT_DING_FORMANT_1: f32 = 180.0;
pub const DEFAULT_DING_FORMANT_2: f32 = 700.0;
pub const DEFAULT_DING_FORMANT_3: f32 = 1_800.0;
pub const DEFAULT_DING_RESONANCE: f32 = 4.0;
pub const DING_STRINGS_RANGE: (f32, f32) = (1.0, 6.0);
pub const DING_FORMANT_RANGE: (f32, f32) = (80.0, 4_000.0);
/// Signed Q: positive boosts a bandpass peak, negative scoops it out (notch-like).
pub const DING_RESONANCE_RANGE: (f32, f32) = (-24.0, 24.0);
const DING_BRIGHT_LO: f32 = 400.0;
const DING_BRIGHT_HI: f32 = 6_000.0;
/// Puts a full waveshaped Ding near the level of the noise resonator.
const DING_WAVE_GAIN: f32 = 0.22;
const DING_VOICES: usize = 3;

const DEGREE_COUNT: usize = 10;
const HISTORY: usize = 16;
const DRONE_WINDOW: usize = 8;
const BASS_WINDOW: usize = 16;
const INTERVALS: usize = 6;
const DRONE_COUNT: usize = 3;
pub const SUPPORT_PARTS: usize = 5;
pub const SUPPORT_PART_NAMES: [&str; SUPPORT_PARTS] = ["Root", "Recent", "Second", "Bass", "Echo"];
const SUPPORT_PART_MASK: u8 = (1 << SUPPORT_PARTS) - 1;
/// Resonator / Waveshape / Tambura mute and solo bits.
pub const WASH_COUNT: usize = 3;
const WASH_PART_MASK: u8 = (1 << WASH_COUNT) - 1;
const FLAM_SECS: f32 = 0.080;
const PAUSE_SECS: f32 = 2.5;
const GLIDE_TAU_S: f32 = 0.45;
const AGREE_BEATS: u8 = 4;
const DRONE_ZETA: f32 = 0.010;
const BASS_ZETA: f32 = 0.006;
/// Slow random drift around the scale note. Small enough that the note stays put.
const WANDER_CENTS: f32 = DEFAULT_SUPPORT_WANDER;
/// A width of 0 sits here: a narrow tone with a little breath.
const TONAL_ZETA_LO: f32 = 0.008;
const TONAL_ZETA_HI: f32 = 0.012;
const WANDER_TAU_S: f32 = 1.8;
/// Rest that still counts as playing. A breath between notes keeps the pad up.
const PRESENCE_HOLD_S: f32 = 2.0;
const PRESENCE_RISE_S: f32 = 0.5;
const PULSE_FALL_S: f32 = 1.0;
const TONE_FADE_S: f32 = 4.0;
const CENTS_TO_RATIO: f32 = core::f32::consts::LN_2 / 1200.0;

#[derive(Clone, Copy)]
struct Formant {
    z1: f32,
    z2: f32,
}

struct Mode {
    position: f32,
    velocity: f32,
    hz: f32,
    target_hz: f32,
    zeta: f32,
    /// Drive phase in cycles, used by the tonal bass.
    phase: f32,
    wander: f32,
    wander_goal: f32,
    wander_left: u32,
    lfo_phase: f32,
    lfo_inc: f32,
    lfo_hz: f32,
    zeta_lo: f32,
    zeta_hi: f32,
    zeta_wide_lo: f32,
    zeta_wide_hi: f32,
    drift_cents: f32,
    drift_base: f32,
    wander_limit: f32,
    motion: f32,
    drive: f32,
    rng: u32,
}

#[derive(Clone, Copy)]
struct Heard {
    degree: u8,
    weight: f32,
}

pub struct Support {
    dt: f32,
    sample: u64,
    level: f32,
    pulse: f32,
    scale: u8,
    ding: u8,
    tuning: u8,
    ready: bool,
    notes: [u8; DEGREE_COUNT],
    history: [Heard; HISTORY],
    history_write: usize,
    history_count: usize,
    last_degree: u8,
    last_tempo_sample: Option<u64>,
    samples_since_strike: u64,
    intervals: [f32; INTERVALS],
    interval_n: usize,
    interval_i: usize,
    intervals_seen: u8,
    have_beat: bool,
    beat_samples: f32,
    phase: f32,
    glide: f32,
    wander_glide: f32,
    /// Tone level. Zero until the first strike, then fades after a rest.
    presence: f32,
    /// Swell depth. Falls ahead of the tone so the fade is not still pulsing.
    awake: f32,
    last_activity: Option<u64>,
    rise: f32,
    pulse_fall: f32,
    tone_fade: f32,
    bass_degree: u8,
    bass_pending: u8,
    bass_votes: u8,
    drones: [Mode; DRONE_COUNT],
    bass: Mode,
    /// Bit set means that part is muted. Mute wins over solo on the same part.
    muted: u8,
    /// Bit set means that part is soloed. Any solo silences the parts that are not.
    solo: u8,
    /// True when another desk channel (e.g. Handpan) is soloed. Silences parts
    /// that are not themselves soloed, same as a non-empty solo mask.
    foreign_solo: bool,
    /// Level of each part relative to today's balance. 1 leaves that balance alone.
    part_level: [f32; SUPPORT_PARTS],
    /// Soft focus hint for the UI (0 Resonator, 1 Waveshape, 2 Tambura) — not exclusive audio.
    ding_wash: u8,
    /// Mix gains for Resonator / Waveshape / Tambura into the Root bus.
    wash_level: [f32; 3],
    /// Bit set mutes that wash (0 Resonator, 1 Waveshape, 2 Tambura).
    wash_muted: u8,
    /// Bit set solos that wash; any solo silences non-soloed washes.
    wash_solo: u8,
    ding_shape: f32,
    ding_spread: f32,
    ding_brightness: f32,
    ding_strings: u8,
    tambura_jawari: f32,
    tambura_pick: f32,
    tambura_strum: f32,
    tambura_cycle: f32,
    ding_formant_hz: [f32; 3],
    ding_formant_q: [f32; 3],
    ding_phase: [f32; DING_VOICES],
    ding_low: f32,
    tambura: TamburaBank,
    ding_formant: [Formant; 3],
    formant_rate: f32,
    formant_depth: f32,
    formant_phase: f32,
}

impl Support {
    pub fn new(sample_rate_hz: f32) -> Self {
        let sample_rate_hz = sample_rate_hz.max(1.0);
        let dt = 1.0 / sample_rate_hz;
        let ding_hz = handpan_interval_hz(50, 0, 0);
        let mut support = Self {
            dt,
            sample: 0,
            level: DEFAULT_SUPPORT_LEVEL,
            pulse: DEFAULT_SUPPORT_PULSE,
            scale: 0,
            ding: 50,
            tuning: 0,
            ready: false,
            notes: [50; DEGREE_COUNT],
            history: [Heard {
                degree: 0,
                weight: 0.0,
            }; HISTORY],
            history_write: 0,
            history_count: 0,
            last_degree: 0,
            last_tempo_sample: None,
            samples_since_strike: 0,
            intervals: [0.0; INTERVALS],
            interval_n: 0,
            interval_i: 0,
            intervals_seen: 0,
            have_beat: false,
            beat_samples: 0.55 * sample_rate_hz,
            phase: 0.0,
            glide: 1.0 - (-dt / GLIDE_TAU_S).exp(),
            wander_glide: 1.0 - (-dt / WANDER_TAU_S).exp(),
            presence: 0.0,
            awake: 0.0,
            last_activity: None,
            rise: 1.0 - (-dt / PRESENCE_RISE_S).exp(),
            pulse_fall: 1.0 - (-dt / PULSE_FALL_S).exp(),
            tone_fade: 1.0 - (-dt / TONE_FADE_S).exp(),
            bass_degree: 0,
            bass_pending: 0,
            bass_votes: 0,
            drones: [
                mode(ding_hz, DRONE_ZETA),
                mode(ding_hz, DRONE_ZETA),
                mode(ding_hz, DRONE_ZETA * 1.3),
            ],
            bass: mode(ding_hz * 0.5, BASS_ZETA),
            muted: 0,
            solo: 0,
            foreign_solo: false,
            part_level: [DEFAULT_MIX_LEVEL; SUPPORT_PARTS],
            ding_wash: DEFAULT_DING_WASH,
            // Default matches exclusive Resonator (ding_wash 0); other washes start silent.
            wash_level: [DEFAULT_MIX_LEVEL, 0.0, 0.0],
            wash_muted: 0,
            wash_solo: 0,
            ding_shape: DEFAULT_DING_SHAPE,
            ding_spread: DEFAULT_DING_SPREAD,
            ding_brightness: DEFAULT_DING_BRIGHTNESS,
            ding_strings: DEFAULT_DING_STRINGS as u8,
            tambura_jawari: DEFAULT_TAMBURA_JAWARI,
            tambura_pick: DEFAULT_TAMBURA_PICK,
            tambura_strum: DEFAULT_TAMBURA_STRUM,
            tambura_cycle: DEFAULT_TAMBURA_CYCLE,
            ding_formant_hz: [
                DEFAULT_DING_FORMANT_1,
                DEFAULT_DING_FORMANT_2,
                DEFAULT_DING_FORMANT_3,
            ],
            ding_formant_q: [DEFAULT_DING_RESONANCE; 3],
            ding_phase: [0.0, 0.37, 0.71],
            ding_low: 0.0,
            tambura: {
                let mut bank = TamburaBank::new(sample_rate_hz);
                bank.set_auto_pluck(true);
                bank.set_shape(DEFAULT_DING_SHAPE);
                bank.set_spread(DEFAULT_DING_SPREAD);
                bank.set_jawari(DEFAULT_TAMBURA_JAWARI);
                bank.set_pick(DEFAULT_TAMBURA_PICK);
                bank.set_strum(DEFAULT_TAMBURA_STRUM);
                bank.set_cycle(DEFAULT_TAMBURA_CYCLE);
                bank.set_strings(DEFAULT_DING_STRINGS);
                bank
            },
            ding_formant: [Formant { z1: 0.0, z2: 0.0 }; 3],
            formant_rate: DEFAULT_FORMANT_RATE,
            formant_depth: DEFAULT_FORMANT_DEPTH,
            formant_phase: 0.0,
        };
        support.set_layout(0, 50, 0);
        for drone in &mut support.drones {
            drone.hz = drone.target_hz;
        }
        support.bass.hz = support.bass.target_hz;
        support.bass.phase = 0.17;
        tune_band(
            &mut support.drones[0],
            dt,
            BandShape {
                seed: 0xA341_316C,
                lfo_hz: 0.030,
                lfo_phase: 0.00,
                zeta_lo: 0.055,
                zeta_hi: 0.17,
                drift_cents: 26.0,
                drive: 0.28,
            },
        );
        tune_band(
            &mut support.drones[1],
            dt,
            BandShape {
                seed: 0x17E3_B2A1,
                lfo_hz: 0.046,
                lfo_phase: 0.37,
                zeta_lo: 0.040,
                zeta_hi: 0.09,
                drift_cents: 16.0,
                drive: 0.17,
            },
        );
        tune_band(
            &mut support.drones[2],
            dt,
            BandShape {
                seed: 0x00C0_FFEE,
                lfo_hz: 0.068,
                lfo_phase: 0.71,
                zeta_lo: 0.045,
                zeta_hi: 0.10,
                drift_cents: 18.0,
                drive: 0.14,
            },
        );
        support
    }

    pub fn set_ding_wash(&mut self, wash: u8) {
        self.ding_wash = wash.min(2);
    }

    pub fn set_wash_level(&mut self, index: usize, level: f32) {
        if let Some(slot) = self.wash_level.get_mut(index) {
            *slot = level.clamp(MIX_LEVEL_RANGE.0, MIX_LEVEL_RANGE.1);
        }
    }

    pub fn set_ding_shape(&mut self, shape: f32) {
        self.ding_shape = shape.clamp(DING_SHAPE_RANGE.0, DING_SHAPE_RANGE.1);
        self.tambura.set_shape(self.ding_shape);
    }

    pub fn set_ding_spread(&mut self, cents: f32) {
        self.ding_spread = cents.clamp(DING_SPREAD_RANGE.0, DING_SPREAD_RANGE.1);
        self.tambura.set_spread(self.ding_spread);
    }

    pub fn set_ding_brightness(&mut self, brightness: f32) {
        self.ding_brightness = brightness.clamp(DING_BRIGHTNESS_RANGE.0, DING_BRIGHTNESS_RANGE.1);
    }

    pub fn set_ding_strings(&mut self, strings: f32) {
        let strings = strings
            .round()
            .clamp(DING_STRINGS_RANGE.0, DING_STRINGS_RANGE.1) as u8;
        self.ding_strings = strings.clamp(1, 6);
        self.tambura.set_strings(self.ding_strings as f32);
    }

    pub fn set_tambura_jawari(&mut self, amount: f32) {
        self.tambura_jawari = amount.clamp(TAMBURA_JAWARI_RANGE.0, TAMBURA_JAWARI_RANGE.1);
        self.tambura.set_jawari(self.tambura_jawari);
    }

    pub fn set_tambura_pick(&mut self, amount: f32) {
        self.tambura_pick = amount.clamp(TAMBURA_PICK_RANGE.0, TAMBURA_PICK_RANGE.1);
        self.tambura.set_pick(self.tambura_pick);
    }

    pub fn set_tambura_strum(&mut self, seconds: f32) {
        self.tambura_strum = seconds.clamp(TAMBURA_STRUM_RANGE.0, TAMBURA_STRUM_RANGE.1);
        self.tambura.set_strum(self.tambura_strum);
    }

    pub fn set_tambura_cycle(&mut self, seconds: f32) {
        self.tambura_cycle = seconds.clamp(TAMBURA_CYCLE_RANGE.0, TAMBURA_CYCLE_RANGE.1);
        self.tambura.set_cycle(self.tambura_cycle);
    }

    pub fn set_ding_formant(&mut self, index: usize, hz: f32) {
        if let Some(slot) = self.ding_formant_hz.get_mut(index) {
            *slot = hz.clamp(DING_FORMANT_RANGE.0, DING_FORMANT_RANGE.1);
        }
    }

    pub fn set_ding_formant_q(&mut self, index: usize, q: f32) {
        if let Some(slot) = self.ding_formant_q.get_mut(index) {
            *slot = q.clamp(DING_RESONANCE_RANGE.0, DING_RESONANCE_RANGE.1);
        }
    }

    pub fn set_formant_rate(&mut self, hz: f32) {
        self.formant_rate = hz.clamp(FORMANT_RATE_RANGE.0, FORMANT_RATE_RANGE.1);
    }

    pub fn set_formant_depth(&mut self, octaves: f32) {
        self.formant_depth = octaves.clamp(FORMANT_DEPTH_RANGE.0, FORMANT_DEPTH_RANGE.1);
    }

    pub fn set_ding_resonance(&mut self, resonance: f32) {
        let q = resonance.clamp(DING_RESONANCE_RANGE.0, DING_RESONANCE_RANGE.1);
        self.ding_formant_q = [q; 3];
    }

    pub fn set_parts(&mut self, muted: u8, solo: u8) {
        self.muted = muted & SUPPORT_PART_MASK;
        self.solo = solo & SUPPORT_PART_MASK;
    }

    pub fn set_wash_parts(&mut self, muted: u8, solo: u8) {
        self.wash_muted = muted & WASH_PART_MASK;
        self.wash_solo = solo & WASH_PART_MASK;
    }

    pub fn set_foreign_solo(&mut self, active: bool) {
        self.foreign_solo = active;
    }

    pub fn set_part_level(&mut self, part: usize, level: f32) {
        if let Some(slot) = self.part_level.get_mut(part) {
            *slot = level.clamp(MIX_LEVEL_RANGE.0, MIX_LEVEL_RANGE.1);
        }
    }

    /// How loud an echoed pad hit should be. Zero while Echo is muted or another part is soloed.
    pub fn echo_gain(&self) -> f32 {
        part_gain(
            self.muted,
            self.solo,
            self.foreign_solo,
            &self.part_level,
            4,
        )
    }

    pub fn set_level(&mut self, level: f32) {
        self.level = level.clamp(SUPPORT_LEVEL_RANGE.0, SUPPORT_LEVEL_RANGE.1);
    }

    pub fn set_pulse(&mut self, pulse: f32) {
        self.pulse = pulse.clamp(SUPPORT_PULSE_RANGE.0, SUPPORT_PULSE_RANGE.1);
    }

    /// 0 is a narrow tone. 1 is the wide band the layer started with.
    pub fn set_width(&mut self, width: f32) {
        let width = width.clamp(SUPPORT_WIDTH_RANGE.0, SUPPORT_WIDTH_RANGE.1);
        for drone in &mut self.drones {
            drone.zeta_lo = TONAL_ZETA_LO + (drone.zeta_wide_lo - TONAL_ZETA_LO) * width;
            drone.zeta_hi = TONAL_ZETA_HI + (drone.zeta_wide_hi - TONAL_ZETA_HI) * width;
        }
    }

    pub fn set_wander(&mut self, cents: f32) {
        let cents = cents.clamp(SUPPORT_WANDER_RANGE.0, SUPPORT_WANDER_RANGE.1);
        for drone in &mut self.drones {
            drone.wander_limit = cents;
        }
    }

    /// Ding-band drift in cents. The color bands keep their share of that amount.
    pub fn set_drift(&mut self, cents: f32) {
        let cents = cents.clamp(SUPPORT_DRIFT_RANGE.0, SUPPORT_DRIFT_RANGE.1);
        let scale = if DEFAULT_SUPPORT_DRIFT > 0.0 {
            cents / DEFAULT_SUPPORT_DRIFT
        } else {
            0.0
        };
        for drone in &mut self.drones {
            drone.drift_cents = drone.drift_base * scale;
        }
    }

    pub fn set_motion(&mut self, motion: f32) {
        let motion = motion.clamp(SUPPORT_MOTION_RANGE.0, SUPPORT_MOTION_RANGE.1);
        for drone in &mut self.drones {
            let previous = drone.motion.max(0.001);
            drone.motion = motion;
            drone.lfo_inc *= motion / previous;
        }
    }

    pub fn set_fade(&mut self, seconds: f32) {
        let seconds = seconds.clamp(SUPPORT_FADE_RANGE.0, SUPPORT_FADE_RANGE.1);
        self.tone_fade = 1.0 - (-self.dt / seconds).exp();
    }

    pub fn set_layout(&mut self, scale: u8, ding: u8, tuning: u8) {
        let scale = scale.min(HANDPAN_SCALE_COUNT - 1);
        let ding = ding.clamp(HANDPAN_DING_MIN, HANDPAN_DING_MAX);
        let tuning = u8::from(tuning != 0);
        if self.ready && scale == self.scale && ding == self.ding && tuning == self.tuning {
            return;
        }
        self.scale = scale;
        self.ding = ding;
        self.tuning = tuning;
        self.notes = handpan_notes(scale, ding);
        self.ready = true;
        self.retarget_drones();
        self.bass.target_hz = self.bass_hz(self.bass_degree);
    }

    /// A handpan strike. `strength` is the same amplitude the handpan received.
    pub fn hear(&mut self, note: u8, strength: f32) {
        let degree = self.nearest_degree(note);
        let now = self.sample;
        self.last_activity = Some(now);
        self.push_heard(degree, strength);
        self.last_degree = degree;
        self.retarget_drones();

        if let Some(previous) = self.last_tempo_sample {
            let gap = now.saturating_sub(previous) as f32 * self.dt;
            if gap < FLAM_SECS {
                return;
            }
            if gap < PAUSE_SECS {
                self.push_interval(gap);
            }
        }
        self.last_tempo_sample = Some(now);
        self.phase = 0.0;
        self.samples_since_strike = 0;
        self.on_downbeat();
    }

    #[inline]
    pub fn next_sample(&mut self) -> f32 {
        self.next_parts().sum()
    }

    /// Ding, Recent, Second, and Bass after their faders, the swell, and Support level.
    pub fn next_parts(&mut self) -> SupportParts {
        self.sample = self.sample.wrapping_add(1);
        self.samples_since_strike = self.samples_since_strike.saturating_add(1);

        self.phase += 1.0 / self.beat_samples.max(1.0);
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.on_downbeat();
        }

        let root = step_band(&mut self.drones[0], self.dt, self.glide, self.wander_glide);
        let washes = self.step_ding(band_hz(&self.drones[0]), root);
        let color_a = step_band(&mut self.drones[1], self.dt, self.glide, self.wander_glide);
        let color_b = step_band(&mut self.drones[2], self.dt, self.glide, self.wander_glide);
        step(&mut self.bass, self.dt, self.glide, 0.0024);
        self.follow_presence();

        if self.level <= 0.0 || self.presence <= 1.0e-4 {
            return SupportParts::default();
        }
        let pulse = if self.have_beat {
            self.pulse * self.awake
        } else {
            0.0
        };
        let breath = 1.0 - pulse * 0.325 * (1.0 - (self.phase * TAU).cos());
        let bass_breath = 0.7 + 0.3 * breath;
        let levels = self.part_level;
        let presence = self.level * self.presence;
        let body = breath * 0.55 * presence;
        // Following pads duck when a wash is soloed or every wash is muted.
        let pads_open = self.wash_solo == 0 && (self.wash_muted & WASH_PART_MASK) != WASH_PART_MASK;
        let recent_gain = if pads_open {
            part_gain(self.muted, self.solo, self.foreign_solo, &levels, 1)
        } else {
            0.0
        };
        let second_gain = if pads_open {
            part_gain(self.muted, self.solo, self.foreign_solo, &levels, 2)
        } else {
            0.0
        };
        let root_gain = part_gain(self.muted, self.solo, self.foreign_solo, &levels, 0) * body;
        SupportParts {
            resonator: washes[0] * root_gain,
            waveshape: washes[1] * root_gain,
            tambura: washes[2] * root_gain,
            recent: color_a * 0.72 * recent_gain * body,
            second: color_b * 0.5 * second_gain * body,
            bass: self.bass.position
                * bass_breath
                * 0.4
                * part_gain(self.muted, self.solo, self.foreign_solo, &levels, 3)
                * presence,
        }
    }

    fn follow_presence(&mut self) {
        let Some(last) = self.last_activity else {
            return;
        };
        let silent = self.sample.saturating_sub(last) as f32 * self.dt;
        let resting = silent >= PRESENCE_HOLD_S;
        let tone_glide = if resting { self.tone_fade } else { self.rise };
        let pulse_glide = if resting { self.pulse_fall } else { self.rise };
        let target = if resting { 0.0 } else { 1.0 };
        self.presence += (target - self.presence) * tone_glide;
        self.awake += (target - self.awake) * pulse_glide;
    }

    fn nearest_degree(&self, note: u8) -> u8 {
        let mut best = 0usize;
        let mut best_distance = i32::MAX;
        for (index, scale_note) in self.notes.iter().enumerate() {
            let distance = i32::from(*scale_note) - i32::from(note);
            let distance = distance.abs();
            if distance < best_distance {
                best_distance = distance;
                best = index;
            }
        }
        best as u8
    }

    fn degree_hz(&self, degree: u8) -> f32 {
        let index = (degree as usize).min(DEGREE_COUNT - 1);
        let semitones = self.notes[index].saturating_sub(self.ding);
        handpan_interval_hz(self.ding, semitones, self.tuning)
    }

    /// One octave under the degree, or two when one octave would sit above the drone.
    fn bass_hz(&self, degree: u8) -> f32 {
        let fundamental = self.degree_hz(degree);
        let once = fundamental * 0.5;
        if once > 160.0 {
            fundamental * 0.25
        } else {
            once
        }
    }

    fn push_heard(&mut self, degree: u8, strength: f32) {
        self.history[self.history_write] = Heard {
            degree,
            weight: strength.clamp(0.0, 1.0).max(0.02),
        };
        self.history_write = (self.history_write + 1) % HISTORY;
        self.history_count = (self.history_count + 1).min(HISTORY);
    }

    fn push_interval(&mut self, gap: f32) {
        if self.interval_n < INTERVALS {
            self.intervals[self.interval_n] = gap;
            self.interval_n += 1;
        } else {
            self.intervals[self.interval_i] = gap;
            self.interval_i = (self.interval_i + 1) % INTERVALS;
        }
        self.intervals_seen = self.intervals_seen.saturating_add(1);
        self.have_beat = self.intervals_seen >= 3;
        let median = self.median_interval();
        let target = (median / self.dt).clamp(0.25 / self.dt, 1.6 / self.dt);
        if self.intervals_seen == 1 {
            self.beat_samples = target;
        } else {
            self.beat_samples += (target - self.beat_samples) * 0.2;
        }
    }

    fn median_interval(&self) -> f32 {
        let n = self.interval_n.max(1);
        let mut values = self.intervals;
        values[..n].sort_by(|a, b| a.total_cmp(b));
        if n % 2 == 1 {
            values[n / 2]
        } else {
            0.5 * (values[n / 2 - 1] + values[n / 2])
        }
    }

    fn counts(&self, window: usize, recency: bool) -> [f32; DEGREE_COUNT] {
        let n = self.history_count.min(window);
        let mut counts = [0.0; DEGREE_COUNT];
        for age in 0..n {
            let index = (self.history_write + HISTORY - 1 - age) % HISTORY;
            let heard = &self.history[index];
            let recency_weight = if recency {
                1.0 - age as f32 / window as f32
            } else {
                1.0
            };
            counts[heard.degree as usize] += heard.weight * recency_weight;
        }
        counts
    }

    fn retarget_drones(&mut self) {
        self.drones[0].target_hz = self.degree_hz(0);
        let counts = self.counts(DRONE_WINDOW, true);
        let (first, second) = leaders(&counts);
        self.drones[1].target_hz = self.degree_hz(first as u8);
        self.drones[2].target_hz = self.degree_hz(second.unwrap_or(first) as u8);
    }

    fn on_downbeat(&mut self) {
        if self.history_count == 0 {
            return;
        }
        let counts = self.counts(BASS_WINDOW, false);
        let (best, _) = leaders(&counts);
        let best = best as u8;
        if best == self.bass_pending {
            self.bass_votes = self.bass_votes.saturating_add(1);
        } else {
            self.bass_pending = best;
            self.bass_votes = 1;
        }
        if self.bass_votes >= AGREE_BEATS && self.bass_degree != self.bass_pending {
            self.bass_degree = self.bass_pending;
            self.bass.target_hz = self.bass_hz(self.bass_degree);
        }
    }

    /// All three Root washes keep running and mix by `wash_level`, then share formants.
    /// Returns Resonator / Waveshape / Tambura so each can be panned in the desk.
    fn step_ding(&mut self, center_hz: f32, root: f32) -> [f32; 3] {
        let (unison, unison_buzz) = self.step_unison(center_hz);
        let (tambura, tambura_buzz) = self.step_tambura(center_hz);
        let levels = [
            self.wash_level[0] * wash_gain(self.wash_muted, self.wash_solo, self.foreign_solo, 0),
            self.wash_level[1] * wash_gain(self.wash_muted, self.wash_solo, self.foreign_solo, 1),
            self.wash_level[2] * wash_gain(self.wash_muted, self.wash_solo, self.foreign_solo, 2),
        ];
        // Body / excite mixes match the old exclusive paths so each voice keeps its colour.
        let body = root * levels[0] + unison * levels[1] + tambura * (0.35 * levels[2]);
        let excite = root * (0.22 * levels[0])
            + unison_buzz * (0.12 * levels[1])
            + tambura_buzz * (0.28 * levels[2]);
        // Shared formants on the sum (same as before); split by wash weight for stereo pan.
        let coloured = self.colour_ding(body, excite, 1.0, 1.0);
        let weight = levels[0] + levels[1] + levels[2];
        if weight <= 1.0e-12 || coloured == 0.0 {
            return [0.0; 3];
        }
        let inv = 1.0 / weight;
        [
            coloured * levels[0] * inv,
            coloured * levels[1] * inv,
            coloured * levels[2] * inv,
        ]
    }

    /// Three sines around the Ding's moving center, through a soft shaper and a lowpass.
    /// The buzz is what the formants carve.
    fn step_unison(&mut self, center_hz: f32) -> (f32, f32) {
        let half = self.ding_spread * 0.5;
        let mut sum = 0.0;
        let mut excite = 0.0;
        for (index, offset) in [-half, 0.0, half].into_iter().enumerate() {
            let hz = (center_hz * (offset * CENTS_TO_RATIO).exp()).max(20.0);
            self.ding_phase[index] = (self.ding_phase[index] + hz * self.dt).fract();
            let sine = (self.ding_phase[index] * TAU).sin();
            sum += waveshape(sine, self.ding_shape);
            excite += string_buzz(sine);
        }
        let scale = DING_WAVE_GAIN / DING_VOICES as f32;
        let mixed = sum * scale;
        let cutoff = DING_BRIGHT_LO * (DING_BRIGHT_HI / DING_BRIGHT_LO).powf(self.ding_brightness);
        let coeff = (-TAU * cutoff * self.dt).exp();
        self.ding_low += (1.0 - coeff) * (mixed - self.ding_low);
        if self.ding_low.abs() < 1.0e-15 {
            self.ding_low = 0.0;
        }
        (self.ding_low, excite * scale)
    }

    /// Waveguide tambura: staggered auto-plucks with settling jawari buzz.
    fn step_tambura(&mut self, center_hz: f32) -> (f32, f32) {
        let fifth = handpan_interval_hz(self.ding, 7, self.tuning)
            / handpan_interval_hz(self.ding, 0, self.tuning).max(1.0);
        self.tambura.set_pitch_hz(center_hz);
        self.tambura.set_fifth_ratio(fifth);
        self.tambura.set_shape(self.ding_shape);
        self.tambura.set_spread(self.ding_spread);
        self.tambura.set_jawari(self.tambura_jawari);
        self.tambura.set_pick(self.tambura_pick);
        self.tambura.set_strum(self.tambura_strum);
        self.tambura.set_cycle(self.tambura_cycle);
        self.tambura.set_strings(self.ding_strings as f32);
        self.tambura.set_auto_pluck(true);
        self.tambura.next_sample()
    }

    /// The dry wash stays. The three bands colour whichever Ding is selected.
    fn colour_ding(&mut self, body: f32, excite: f32, body_mix: f32, peak_mix: f32) -> f32 {
        let dt = self.dt;
        let q = self.ding_formant_q;
        let nyquist = 0.45 / dt;
        self.formant_phase = (self.formant_phase + self.formant_rate * dt).fract();
        let scale = 2.0f32.powf((self.formant_phase * TAU).sin() * self.formant_depth);
        let hz = self.ding_formant_hz;
        let mut peaks = 0.0;
        for (index, formant) in self.ding_formant.iter_mut().enumerate() {
            let center = (hz[index] * scale).clamp(20.0, nyquist);
            let signed = q[index];
            let mag = signed.abs().max(0.05);
            let sign = if signed >= 0.0 { 1.0 } else { -1.0 };
            peaks += sign * formant.bandpass(excite, center, mag, dt);
        }
        let y = body * body_mix + peaks * peak_mix;
        if y.abs() < 1.0e-15 { 0.0 } else { y }
    }
}

/// One sample of the accompaniment, split so each part can be panned.
#[derive(Clone, Copy, Default)]
pub struct SupportParts {
    pub resonator: f32,
    pub waveshape: f32,
    pub tambura: f32,
    pub recent: f32,
    pub second: f32,
    pub bass: f32,
}

impl SupportParts {
    /// Root wash bus (Resonator + Waveshape + Tambura) for the shared reverb send.
    #[inline]
    pub fn ding(self) -> f32 {
        self.resonator + self.waveshape + self.tambura
    }

    pub fn sum(self) -> f32 {
        (self.ding() + self.recent + self.second + self.bass).clamp(-1.0, 1.0)
    }

    /// The same parts, pulled back together when their sum would have been limited.
    pub fn limited(self) -> Self {
        let raw = self.ding() + self.recent + self.second + self.bass;
        if raw.abs() <= 1.0 {
            return self;
        }
        let scale = raw.clamp(-1.0, 1.0) / raw;
        Self {
            resonator: self.resonator * scale,
            waveshape: self.waveshape * scale,
            tambura: self.tambura * scale,
            recent: self.recent * scale,
            second: self.second * scale,
            bass: self.bass * scale,
        }
    }
}

/// Center is full level on both speakers. Either end is that speaker alone.
pub fn stereo_pan(pan: f32) -> (f32, f32) {
    let pan = pan.clamp(PAN_RANGE.0, PAN_RANGE.1);
    (((1.0 - pan) * 2.0).min(1.0), (pan * 2.0).min(1.0))
}

impl Formant {
    fn bandpass(&mut self, sample: f32, hz: f32, q: f32, dt: f32) -> f32 {
        let w0 = TAU * hz * dt;
        let sine = w0.sin();
        let q = q.max(0.05);
        let alpha = sine / (2.0 * q);
        let a0 = 1.0 + alpha;
        // Constant skirt, so the peak grows with Resonance.
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

pub fn support_part_mask(flags: [bool; SUPPORT_PARTS]) -> u8 {
    let mut mask = 0u8;
    for (index, on) in flags.into_iter().enumerate() {
        if on {
            mask |= 1 << index;
        }
    }
    mask
}

pub fn support_part_flags(mask: u8) -> [bool; SUPPORT_PARTS] {
    std::array::from_fn(|index| (mask & (1 << index)) != 0)
}

/// Mute silences a part. A solo (local or foreign) silences every part that is
/// not itself soloed. A part that is heard plays at its own level.
fn part_gain(
    muted: u8,
    solo: u8,
    foreign_solo: bool,
    levels: &[f32; SUPPORT_PARTS],
    part: usize,
) -> f32 {
    let bit = 1u8 << part;
    if (muted & bit) != 0 {
        return 0.0;
    }
    if (solo != 0 || foreign_solo) && (solo & bit) == 0 {
        return 0.0;
    }
    levels[part]
}

fn wash_gain(muted: u8, solo: u8, foreign_solo: bool, wash: usize) -> f32 {
    let bit = 1u8 << wash;
    if (muted & bit) != 0 {
        return 0.0;
    }
    if (solo != 0 || foreign_solo) && (solo & bit) == 0 {
        return 0.0;
    }
    1.0
}

fn mode(hz: f32, zeta: f32) -> Mode {
    Mode {
        position: 0.0,
        velocity: 0.0,
        hz,
        target_hz: hz,
        zeta,
        phase: 0.0,
        wander: 0.0,
        wander_goal: 0.0,
        wander_left: 0,
        lfo_phase: 0.0,
        lfo_inc: 0.0,
        lfo_hz: 0.0,
        zeta_lo: zeta,
        zeta_hi: zeta,
        zeta_wide_lo: zeta,
        zeta_wide_hi: zeta,
        drift_cents: 0.0,
        drift_base: 0.0,
        wander_limit: WANDER_CENTS,
        motion: DEFAULT_SUPPORT_MOTION,
        drive: 0.0,
        rng: 1,
    }
}

struct BandShape {
    seed: u32,
    lfo_hz: f32,
    lfo_phase: f32,
    zeta_lo: f32,
    zeta_hi: f32,
    drift_cents: f32,
    drive: f32,
}

fn tune_band(mode: &mut Mode, dt: f32, shape: BandShape) {
    mode.rng = shape.seed;
    mode.lfo_hz = shape.lfo_hz;
    mode.lfo_inc = shape.lfo_hz * dt;
    mode.lfo_phase = shape.lfo_phase;
    mode.zeta_lo = shape.zeta_lo;
    mode.zeta_hi = shape.zeta_hi;
    mode.zeta_wide_lo = shape.zeta_lo;
    mode.zeta_wide_hi = shape.zeta_hi;
    mode.drift_cents = shape.drift_cents;
    mode.drift_base = shape.drift_cents;
    mode.wander_limit = WANDER_CENTS;
    mode.motion = DEFAULT_SUPPORT_MOTION;
    mode.drive = shape.drive;
}

fn leaders(counts: &[f32; DEGREE_COUNT]) -> (usize, Option<usize>) {
    let mut best = 0usize;
    let mut second = None;
    for index in 0..DEGREE_COUNT {
        if counts[index] > counts[best] {
            second = Some(best);
            best = index;
        } else if index != best
            && counts[index] > 0.0
            && second.is_none_or(|other| counts[index] > counts[other])
        {
            second = Some(index);
        }
    }
    (best, second)
}

/// A noise-driven resonance. Velocity is the band-pass output, so the mode
/// sounds like a width of pitches around the note rather than a sine.
fn step_band(mode: &mut Mode, dt: f32, glide: f32, wander_glide: f32) -> f32 {
    mode.hz += (mode.target_hz - mode.hz) * glide;
    if mode.wander_left == 0 {
        mode.wander_goal = noise(&mut mode.rng) * mode.wander_limit;
        let gap = 0.9 + (noise(&mut mode.rng) * 0.5 + 0.5) * 1.2;
        mode.wander_left = ((gap / dt) as u32).max(1);
        let rate = 0.85 + 0.30 * (noise(&mut mode.rng) * 0.5 + 0.5);
        mode.lfo_inc = mode.lfo_hz * mode.motion * dt * rate;
    } else {
        mode.wander_left -= 1;
    }
    mode.wander += (mode.wander_goal - mode.wander) * wander_glide;
    mode.lfo_phase = (mode.lfo_phase + mode.lfo_inc).fract();
    let hz = band_hz(mode);
    let open = 0.5 + 0.5 * (mode.lfo_phase * TAU).cos();
    let zeta = mode.zeta_lo + (mode.zeta_hi - mode.zeta_lo) * open;
    let omega = TAU * hz;
    let stiffness = omega * omega;
    let damping = 2.0 * zeta * omega;
    let drive = noise(&mut mode.rng) * mode.drive;
    let acceleration = drive * stiffness - stiffness * mode.position - damping * mode.velocity;
    mode.velocity = (mode.velocity + acceleration * dt).clamp(-8_000.0, 8_000.0);
    mode.position = (mode.position + mode.velocity * dt).clamp(-2.0, 2.0);
    mode.velocity / omega
}

fn band_hz(mode: &Mode) -> f32 {
    let cents = mode.wander + (mode.lfo_phase * TAU).sin() * mode.drift_cents;
    (mode.hz * (cents * CENTS_TO_RATIO).exp()).max(20.0)
}

/// Odd harmonics of the string, so the formants have a bright buzz to carve.
fn string_buzz(sine: f32) -> f32 {
    (sine * 5.0).tanh()
}

/// A sine, then a soft odd-harmonic curve, then a gentle fold. Never a saw.
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

fn step(mode: &mut Mode, dt: f32, glide: f32, drive_amp: f32) {
    mode.hz += (mode.target_hz - mode.hz) * glide;
    let hz = mode.hz.max(20.0);
    mode.phase = (mode.phase + dt * hz).fract();
    let omega = TAU * hz;
    let stiffness = omega * omega;
    let damping = 2.0 * mode.zeta * omega;
    let drive = drive_amp * (mode.phase * TAU).sin();
    let acceleration = drive * stiffness - stiffness * mode.position - damping * mode.velocity;
    mode.velocity = (mode.velocity + acceleration * dt).clamp(-8_000.0, 8_000.0);
    mode.position = (mode.position + mode.velocity * dt).clamp(-2.0, 2.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advance(support: &mut Support, samples: usize) {
        for _ in 0..samples {
            support.next_sample();
        }
    }

    #[test]
    fn steady_strikes_settle_on_that_interval() {
        let mut support = Support::new(48_000.0);
        support.set_layout(0, 50, 0);
        for _ in 0..10 {
            advance(&mut support, 24_000);
            support.hear(50, 0.5);
        }
        let beat = support.beat_samples * support.dt;
        assert!(
            (beat - 0.5).abs() < 0.04,
            "beat settled at {beat}s, expected 0.5s"
        );

        let settled = support.beat_samples;
        support.hear(50, 0.4);
        advance(&mut support, 960);
        support.hear(52, 0.4);
        assert!(
            (support.beat_samples - settled).abs() < 1.0,
            "a flam should not move the beat"
        );

        advance(&mut support, 144_000);
        support.hear(50, 0.5);
        assert!(
            (support.beat_samples - settled).abs() < 1.0,
            "a pause should not become the beat"
        );
    }

    #[test]
    fn drone_pitches_stay_on_the_scale() {
        let mut support = Support::new(48_000.0);
        support.set_layout(0, 50, 1);
        let notes = handpan_notes(0, 50);
        for note in [notes[0], notes[1], notes[3], notes[1], notes[0]] {
            advance(&mut support, 12_000);
            support.hear(note, 0.6);
        }
        let scale_hz: [f32; DEGREE_COUNT] =
            core::array::from_fn(|index| handpan_interval_hz(50, notes[index] - 50, 1));
        for drone in &support.drones {
            assert!(
                scale_hz
                    .iter()
                    .any(|hz| (drone.target_hz - hz).abs() < 0.05),
                "drone target {} Hz is not a scale note",
                drone.target_hz
            );
        }
        let bass_class = support.bass.target_hz;
        assert!(
            scale_hz.iter().any(|hz| {
                let ratio = hz / bass_class;
                (ratio - 2.0).abs() < 0.02 || (ratio - 4.0).abs() < 0.02
            }),
            "bass {} Hz is not one or two octaves under a scale note",
            bass_class
        );
    }

    #[test]
    fn the_drone_sounds_until_level_is_zero() {
        let mut support = Support::new(48_000.0);
        support.set_level(1.0);
        support.set_pulse(0.0);
        support.hear(50, 0.5);
        advance(&mut support, 24_000);
        let mut energy = 0.0;
        for _ in 0..48_000 {
            let sample = support.next_sample();
            energy += sample * sample;
        }
        assert!(energy > 8.0 && energy < 2_000.0, "energy {energy}");
        support.set_level(0.0);
        for _ in 0..1_000 {
            assert_eq!(support.next_sample(), 0.0);
        }
    }

    fn energy(support: &mut Support, samples: usize) -> f32 {
        let mut total = 0.0;
        for _ in 0..samples {
            let sample = support.next_sample();
            total += sample * sample;
        }
        total
    }

    /// Energy near the strike over energy later in the beat, before the offbeat tap.
    /// The phase must just have been reset, or be sitting near zero.
    fn swell_ratio(support: &mut Support) -> f32 {
        let beat = support.beat_samples.round() as usize;
        let window = ((beat as f32) * 0.08) as usize;
        let trough_at = ((beat as f32) * 0.38) as usize;
        let mut peak = 0.0;
        let mut trough = 0.0;
        for _ in 0..3 {
            peak += energy(support, window);
            advance(support, trough_at.saturating_sub(window));
            trough += energy(support, window);
            advance(support, beat.saturating_sub(trough_at + window));
        }
        peak / trough.max(1.0e-12)
    }

    #[test]
    fn echoes_leave_the_beat_where_the_player_set_it() {
        let mut support = Support::new(1_000.0);
        support.set_layout(0, 50, 0);
        let mut echo = crate::Echo::new(1_000.0);
        for _ in 0..4 {
            support.hear(50, 0.5);
            echo.record(50, 0.5, 0.0);
            for _ in 0..500 {
                support.next_sample();
                echo.poll(|_, _, _| {});
            }
        }
        let beat = support.beat_samples;
        let mut echoes = 0;
        for _ in 0..12_000 {
            support.next_sample();
            echo.poll(|_, _, _| echoes += 1);
        }
        assert!(echoes > 0, "the phrase should have come back");
        assert_eq!(
            support.beat_samples, beat,
            "an echo is not a new strike and must not move the beat"
        );
    }

    #[test]
    fn echo_gain_follows_the_echo_row() {
        let mut support = Support::new(48_000.0);
        assert!((support.echo_gain() - 1.0).abs() < 1.0e-6);
        support.set_part_level(4, 0.0);
        assert_eq!(support.echo_gain(), 0.0);
        support.set_part_level(4, 0.5);
        assert!((support.echo_gain() - 0.5).abs() < 1.0e-6);
        support.set_parts(1 << 4, 0);
        assert_eq!(support.echo_gain(), 0.0);
        support.set_parts(0, 1);
        assert_eq!(support.echo_gain(), 0.0);
        support.set_parts(0, 1 << 4);
        assert!((support.echo_gain() - 0.5).abs() < 1.0e-6);
    }

    fn silence_for(support: &Support) -> f32 {
        match support.last_activity {
            Some(last) => support.sample.saturating_sub(last) as f32 * support.dt,
            None => f32::MAX,
        }
    }

    fn advance_until_silent(support: &mut Support, seconds: f32) {
        let remain = seconds - silence_for(support);
        if remain > 0.0 {
            advance(support, (remain / support.dt) as usize);
        }
    }

    #[test]
    fn accompaniment_waits_and_then_fades() {
        let mut support = Support::new(48_000.0);
        support.set_level(1.0);
        support.set_pulse(1.0);
        assert_eq!(
            energy(&mut support, 48_000),
            0.0,
            "the layer is silent until a strike"
        );

        for _ in 0..8 {
            advance(&mut support, 24_000);
            support.hear(50, 0.6);
        }
        let ratio = swell_ratio(&mut support);
        assert!(
            ratio > 1.4,
            "the swell should be audible while playing, peak/trough {ratio}"
        );

        advance_until_silent(&mut support, PRESENCE_HOLD_S - 0.05);
        let held = energy(&mut support, 2_400);
        assert!(
            held > 0.2 && support.presence > 0.85,
            "a rest under two seconds keeps the pad, energy {held} presence {}",
            support.presence
        );

        advance_until_silent(&mut support, 5.0);
        let fading = energy(&mut support, 2_400);
        assert!(
            support.awake < 0.12 && support.presence > 0.25 && fading > 0.02,
            "the pulse stops while the tone is still there, awake {} presence {} energy {fading}",
            support.awake,
            support.presence
        );
        let to_peak = if support.phase < 0.02 {
            0
        } else {
            ((1.0 - support.phase) * support.beat_samples) as usize
        };
        advance(&mut support, to_peak);
        let resting_ratio = swell_ratio(&mut support);
        assert!(
            resting_ratio < 1.35,
            "the fade should not still be pulsing, peak/trough {resting_ratio}"
        );

        advance_until_silent(&mut support, 18.0);
        let gone = energy(&mut support, 48_000);
        assert!(
            gone < 0.5 && support.presence < 0.05,
            "the tone fades out, energy {gone} presence {}",
            support.presence
        );
    }

    #[test]
    fn width_and_drift_follow_the_controls() {
        let mut support = Support::new(48_000.0);
        assert!(support.drones[0].zeta_hi >= 0.15);
        support.set_width(0.0);
        assert!(support.drones[0].zeta_hi < 0.02);
        assert!(support.drones[1].zeta_hi < 0.02);
        support.set_width(1.0);
        assert!(support.drones[0].zeta_hi >= 0.15);
        support.set_drift(0.0);
        assert!(support.drones[0].drift_cents.abs() < 0.01);
        support.set_drift(DEFAULT_SUPPORT_DRIFT);
        assert!((support.drones[0].drift_cents - 26.0).abs() < 0.01);
        assert!((support.drones[1].drift_cents - 16.0).abs() < 0.01);
        assert!((support.drones[2].drift_cents - 18.0).abs() < 0.01);
    }

    #[test]
    fn solo_and_mute_choose_who_is_heard() {
        let unity = [1.0; SUPPORT_PARTS];
        assert_eq!(part_gain(0, 0, false, &unity, 0), 1.0);
        assert_eq!(part_gain(0, 0, false, &unity, 4), 1.0);
        assert_eq!(part_gain(1, 0, false, &unity, 0), 0.0);
        assert_eq!(part_gain(1, 0, false, &unity, 1), 1.0);
        assert_eq!(part_gain(0, 0b1000, false, &unity, 0), 0.0);
        assert_eq!(part_gain(0, 0b1000, false, &unity, 3), 1.0);
        assert_eq!(part_gain(0b1000, 0b1000, false, &unity, 3), 0.0);
        assert_eq!(part_gain(0, 0b1001, false, &unity, 0), 1.0);
        assert_eq!(part_gain(0, 0b1001, false, &unity, 3), 1.0);
        assert_eq!(part_gain(0, 0b1001, false, &unity, 1), 0.0);
        assert_eq!(part_gain(0, 0, true, &unity, 0), 0.0);
        assert_eq!(part_gain(0, 1, true, &unity, 0), 1.0);
        assert_eq!(part_gain(0, 1, true, &unity, 1), 0.0);
        let boosted = [2.0; SUPPORT_PARTS];
        assert_eq!(part_gain(1, 0, false, &boosted, 0), 0.0);
        assert!((part_gain(0, 0, false, &[0.5, 1.0, 1.0, 1.0, 1.0], 0) - 0.5).abs() < 1.0e-6);

        let mut support = Support::new(48_000.0);
        support.set_level(1.0);
        support.set_parts(SUPPORT_PART_MASK, 0);
        support.hear(50, 1.0);
        advance(&mut support, 24_000);
        let mut energy = 0.0;
        for _ in 0..24_000 {
            let sample = support.next_sample();
            energy += sample * sample;
        }
        assert!(
            energy < 1.0e-8,
            "muted accompaniment still sounded: {energy}"
        );
    }

    #[test]
    fn part_levels_scale_the_ding() {
        let full = ding_mix_energy(1.0);
        let half = ding_mix_energy(0.5);
        let silent = ding_mix_energy(0.0);
        assert!(full > 1.0e-4, "ding was silent at full level: {full}");
        assert!(silent < 1.0e-8, "ding still sounded at level 0: {silent}");
        let ratio = half / full;
        assert!(
            (0.20..0.30).contains(&ratio),
            "half level should be about a quarter of the energy, got {ratio}"
        );
    }

    fn ding_mix_energy(level: f32) -> f32 {
        let mut support = ding_only(0, DEFAULT_DING_SHAPE);
        support.set_level(0.2);
        support.set_part_level(0, level);
        let mut energy = 0.0;
        for _ in 0..48_000 {
            let sample = support.next_sample();
            energy += sample * sample;
        }
        energy
    }

    fn ding_only(wash: u8, shape: f32) -> Support {
        let mut support = Support::new(48_000.0);
        support.set_level(1.0);
        support.set_pulse(0.0);
        support.set_wander(0.0);
        support.set_drift(0.0);
        support.set_parts(0, 1);
        support.set_ding_wash(wash);
        // Exclusive solo of one wash — mirrors the old ding_wash switch for tests.
        support.set_wash_level(0, if wash == 0 { DEFAULT_MIX_LEVEL } else { 0.0 });
        support.set_wash_level(1, if wash == 1 { DEFAULT_MIX_LEVEL } else { 0.0 });
        support.set_wash_level(2, if wash == 2 { DEFAULT_MIX_LEVEL } else { 0.0 });
        support.set_ding_shape(shape);
        support.set_ding_spread(0.0);
        support.set_ding_brightness(1.0);
        support.hear(50, 1.0);
        advance(&mut support, 24_000);
        support
    }

    fn collect(support: &mut Support, seconds: f32) -> Vec<f32> {
        let mut samples = Vec::new();
        for _ in 0..(48_000.0 * seconds) as usize {
            samples.push(support.next_sample());
        }
        samples
    }

    fn tone_power(samples: &[f32], hz: f32) -> f32 {
        let omega = TAU * hz / 48_000.0;
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

    fn park_formants(support: &mut Support) {
        support.set_ding_resonance(DING_RESONANCE_RANGE.1);
        support.set_ding_formant(0, 3_500.0);
        support.set_ding_formant(1, 3_600.0);
        support.set_ding_formant(2, 3_700.0);
    }

    #[test]
    fn waveshape_is_a_tone_until_it_folds() {
        let ding = handpan_interval_hz(50, 0, 0);
        let mut sine = ding_only(1, 0.0);
        park_formants(&mut sine);
        let sine_samples = collect(&mut sine, 1.0);
        assert!(sine_samples.iter().all(|sample| sample.is_finite()));
        let fundamental = tone_power(&sine_samples, ding);
        let third = tone_power(&sine_samples, ding * 3.0);
        assert!(
            fundamental > third * 8.0,
            "a sine Ding still had a strong third: fund {fundamental} third {third}"
        );

        let mut folded = ding_only(1, 1.0);
        park_formants(&mut folded);
        let folded_samples = collect(&mut folded, 1.0);
        assert!(
            folded_samples
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() < 2.0)
        );
        let folded_fundamental = tone_power(&folded_samples, ding);
        let folded_third = tone_power(&folded_samples, ding * 3.0);
        let sine_ratio = third / fundamental.max(1.0e-12);
        let folded_ratio = folded_third / folded_fundamental.max(1.0e-12);
        assert!(
            folded_ratio > sine_ratio * 3.0,
            "the fold did not add harmonics: sine {sine_ratio} fold {folded_ratio}"
        );

        let mut muted = ding_only(1, 1.0);
        park_formants(&mut muted);
        muted.set_parts(1, 1);
        let silent = collect(&mut muted, 0.5);
        let energy: f32 = silent.iter().map(|sample| sample * sample).sum();
        assert!(
            energy < 1.0e-8,
            "a muted waveshape Ding still sounded: {energy}"
        );
    }

    #[test]
    fn formants_colour_each_ding_wash() {
        let ding = handpan_interval_hz(50, 0, 0);
        for wash in [0_u8, 1, 2] {
            let mut parked = ding_only(wash, DEFAULT_DING_SHAPE);
            park_formants(&mut parked);
            let parked_power = tone_power(&collect(&mut parked, 1.0), ding);
            let mut coloured = ding_only(wash, DEFAULT_DING_SHAPE);
            coloured.set_ding_resonance(8.0);
            coloured.set_ding_formant(0, ding);
            coloured.set_ding_formant(1, ding);
            coloured.set_ding_formant(2, ding);
            let coloured_samples = collect(&mut coloured, 1.0);
            assert!(
                coloured_samples
                    .iter()
                    .all(|sample| sample.is_finite() && sample.abs() < 2.0),
                "wash {wash} left the formant"
            );
            let coloured_power = tone_power(&coloured_samples, ding);
            assert!(
                coloured_power > parked_power * 2.0,
                "wash {wash} ignored the formant: coloured {coloured_power} parked {parked_power}"
            );
        }
    }

    #[test]
    fn wash_mute_and_solo_isolate_washes() {
        let ding = handpan_interval_hz(50, 0, 0);
        let mut mixed = ding_only(0, DEFAULT_DING_SHAPE);
        park_formants(&mut mixed);
        mixed.set_wash_level(0, 1.0);
        mixed.set_wash_level(1, 1.0);
        mixed.set_wash_level(2, 0.0);
        let both = tone_power(&collect(&mut mixed, 0.5), ding);

        let mut muted_wave = ding_only(0, DEFAULT_DING_SHAPE);
        park_formants(&mut muted_wave);
        muted_wave.set_wash_level(0, 1.0);
        muted_wave.set_wash_level(1, 1.0);
        muted_wave.set_wash_parts(1 << 1, 0);
        let res_only = tone_power(&collect(&mut muted_wave, 0.5), ding);
        assert!(
            res_only < both * 0.85,
            "muting waveshape should drop energy: both {both} muted {res_only}"
        );

        let mut solo_wave = ding_only(0, DEFAULT_DING_SHAPE);
        park_formants(&mut solo_wave);
        solo_wave.set_wash_level(0, 1.0);
        solo_wave.set_wash_level(1, 1.0);
        solo_wave.set_wash_parts(0, 1 << 1);
        solo_wave.set_part_level(1, 1.0);
        solo_wave.set_part_level(2, 1.0);
        // Following pads must stay silent while a wash is soloed.
        let parts = {
            let mut energy = [0.0f32; 3];
            for _ in 0..24_000 {
                let p = solo_wave.next_parts();
                let ding = p.ding();
                energy[0] += ding * ding;
                energy[1] += p.recent * p.recent;
                energy[2] += p.second * p.second;
            }
            energy
        };
        assert!(parts[0] > 1.0e-4, "soloed wash went silent: {}", parts[0]);
        assert!(
            parts[1] < 1.0e-8 && parts[2] < 1.0e-8,
            "following pads leaked during wash solo: recent {} second {}",
            parts[1],
            parts[2]
        );
    }

    #[test]
    fn wash_levels_mix_instead_of_exclusive_ding_wash() {
        let ding = handpan_interval_hz(50, 0, 0);
        // ding_wash focus alone must not silence other washes that have level.
        let mut focused = ding_only(0, DEFAULT_DING_SHAPE);
        park_formants(&mut focused);
        focused.set_ding_wash(0);
        focused.set_wash_level(0, 0.0);
        focused.set_wash_level(1, 1.0);
        focused.set_wash_level(2, 0.0);
        let wave_only = tone_power(&collect(&mut focused, 0.8), ding);
        assert!(
            wave_only > 1.0e-3,
            "waveshape stayed silent while ding_wash focused resonator: {wave_only}"
        );

        let mut wave = ding_only(1, 0.0);
        park_formants(&mut wave);
        let wave_energy: f32 = collect(&mut wave, 0.6).iter().map(|s| s * s).sum();

        let mut mixed = ding_only(1, 0.0);
        park_formants(&mut mixed);
        mixed.set_wash_level(0, 1.0);
        mixed.set_wash_level(1, 1.0);
        mixed.set_wash_level(2, 0.0);
        let mixed_energy: f32 = collect(&mut mixed, 0.6).iter().map(|s| s * s).sum();
        assert!(
            mixed_energy > wave_energy * 1.2,
            "adding resonator did not raise the Root mix: mixed {mixed_energy} wave {wave_energy}"
        );
    }

    #[test]
    fn waveshape_sits_near_the_noise_ding() {
        let mut noise = ding_only(0, DEFAULT_DING_SHAPE);
        noise.set_ding_brightness(DEFAULT_DING_BRIGHTNESS);
        let noise_samples = collect(&mut noise, 1.0);
        let mut wave = ding_only(1, DEFAULT_DING_SHAPE);
        wave.set_ding_brightness(DEFAULT_DING_BRIGHTNESS);
        wave.set_ding_spread(DEFAULT_DING_SPREAD);
        let wave_samples = collect(&mut wave, 1.0);
        let noise_rms = (noise_samples
            .iter()
            .map(|sample| sample * sample)
            .sum::<f32>()
            / noise_samples.len() as f32)
            .sqrt();
        let wave_rms = (wave_samples
            .iter()
            .map(|sample| sample * sample)
            .sum::<f32>()
            / wave_samples.len() as f32)
            .sqrt();
        assert!(
            wave_rms > noise_rms * 0.45 && wave_rms < noise_rms * 2.2,
            "waveshape rms {wave_rms} noise rms {noise_rms}"
        );
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
    }

    fn tambura_at(tuning: u8, strings: f32) -> Support {
        let mut support = ding_only(2, 0.0);
        support.set_layout(0, 50, tuning);
        support.set_ding_strings(strings);
        support.set_ding_resonance(0.7);
        support.set_ding_formant(0, 80.0);
        support.set_ding_formant(1, handpan_interval_hz(50, 0, tuning));
        support.set_ding_formant(2, handpan_interval_hz(50, 7, tuning));
        support
    }

    #[test]
    fn tambura_chord_follows_the_ding() {
        let ding = handpan_interval_hz(50, 0, 0);
        let low = ding * 0.5;
        let pa = handpan_interval_hz(50, 7, 0);
        let mut chord = tambura_at(0, 6.0);
        chord.set_tambura_jawari(0.2);
        let chord_samples = collect(&mut chord, 2.0);
        assert!(
            chord_samples
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() < 2.0)
        );
        let low_power = tone_power(&chord_samples, low);
        let ding_power = tone_power(&chord_samples, ding);
        let pa_power = tone_power(&chord_samples, pa);
        assert!(
            low_power > 1.0 && ding_power > 1.0 && pa_power > 1.0,
            "full chord missing tones: low {low_power} ding {ding_power} pa {pa_power}"
        );

        let mut high_only = tambura_at(0, 5.0);
        high_only.set_tambura_jawari(0.2);
        let high_samples = collect(&mut high_only, 2.0);
        assert!(
            high_samples
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() < 2.0)
        );
        assert!(
            tone_power(&high_samples, ding * 2.0) > 1.0,
            "high-Sa solo stayed silent"
        );

        let mut muted = tambura_at(0, 6.0);
        muted.set_parts(1, 1);
        let silent = collect(&mut muted, 0.5);
        let energy: f32 = silent.iter().map(|sample| sample * sample).sum();
        assert!(energy < 1.0e-8, "a muted tambura still sounded: {energy}");
    }

    #[test]
    fn tambura_fifth_follows_the_tuning() {
        // Layout fifth is applied into the waveguide; Pa should stay audible for both tunings.
        let equal_pa = handpan_interval_hz(50, 7, 0);
        let just_pa = handpan_interval_hz(50, 7, 1);
        let mut equal = tambura_at(0, 6.0);
        equal.set_tambura_jawari(0.15);
        equal.set_ding_spread(0.0);
        let equal_samples = collect(&mut equal, 2.0);
        assert!(
            tone_power(&equal_samples, equal_pa) > 1.0,
            "equal Pa missing: {}",
            tone_power(&equal_samples, equal_pa)
        );

        let mut just = tambura_at(1, 6.0);
        just.set_tambura_jawari(0.15);
        just.set_ding_spread(0.0);
        let just_samples = collect(&mut just, 2.0);
        assert!(
            tone_power(&just_samples, just_pa) > 1.0,
            "just Pa missing: {}",
            tone_power(&just_samples, just_pa)
        );
    }

    #[test]
    fn tambura_formant_picks_out_a_harmonic() {
        let ding = handpan_interval_hz(50, 0, 0);
        let partial = ding * 3.0;
        let mut on_ding = tambura_at(0, 6.0);
        on_ding.set_ding_shape(0.85);
        on_ding.set_tambura_jawari(0.55);
        on_ding.set_ding_resonance(14.0);
        on_ding.set_ding_formant(0, ding);
        on_ding.set_ding_formant(1, ding);
        on_ding.set_ding_formant(2, ding);
        let ding_samples = collect(&mut on_ding, 2.0);
        assert!(
            ding_samples
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() < 2.0)
        );
        let focused_low =
            tone_power(&ding_samples, partial) / tone_power(&ding_samples, ding).max(1.0e-12);

        let mut on_partial = tambura_at(0, 6.0);
        on_partial.set_ding_shape(0.85);
        on_partial.set_tambura_jawari(0.55);
        on_partial.set_ding_resonance(14.0);
        on_partial.set_ding_formant(0, 80.0);
        on_partial.set_ding_formant(1, partial);
        on_partial.set_ding_formant(2, 4_000.0);
        let partial_samples = collect(&mut on_partial, 2.0);
        assert!(
            partial_samples
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() < 2.0)
        );
        let focused_high =
            tone_power(&partial_samples, partial) / tone_power(&partial_samples, ding).max(1.0e-12);
        assert!(
            focused_high > focused_low * 1.8,
            "the formant did not favour its harmonic: on ding {focused_low} on partial {focused_high}"
        );
    }

    #[test]
    fn formant_controls_change_the_ding() {
        let ding = handpan_interval_hz(50, 0, 0);
        let mut parked = ding_only(2, DEFAULT_DING_SHAPE);
        parked.set_ding_spread(0.0);
        parked.set_ding_strings(DEFAULT_DING_STRINGS);
        parked.set_ding_resonance(DEFAULT_DING_RESONANCE);
        parked.set_ding_formant(0, 3_500.0);
        parked.set_ding_formant(1, 3_500.0);
        parked.set_ding_formant(2, 3_500.0);
        let parked_power = tone_power(&collect(&mut parked, 1.0), ding);

        let mut on_ding = ding_only(2, DEFAULT_DING_SHAPE);
        on_ding.set_ding_spread(0.0);
        on_ding.set_ding_strings(DEFAULT_DING_STRINGS);
        on_ding.set_ding_resonance(DEFAULT_DING_RESONANCE);
        on_ding.set_ding_formant(0, ding);
        on_ding.set_ding_formant(1, ding);
        on_ding.set_ding_formant(2, ding);
        let on_power = tone_power(&collect(&mut on_ding, 1.0), ding);
        assert!(
            on_power > parked_power * 4.0,
            "a formant on the Ding was too quiet: on {on_power} parked {parked_power}"
        );

        let mut soft = ding_only(2, DEFAULT_DING_SHAPE);
        soft.set_ding_spread(0.0);
        soft.set_ding_strings(DEFAULT_DING_STRINGS);
        soft.set_ding_resonance(0.7);
        soft.set_ding_formant(0, ding);
        soft.set_ding_formant(1, ding);
        soft.set_ding_formant(2, ding);
        let soft_power = tone_power(&collect(&mut soft, 1.0), ding);
        let mut sharp = ding_only(2, DEFAULT_DING_SHAPE);
        sharp.set_ding_spread(0.0);
        sharp.set_ding_strings(DEFAULT_DING_STRINGS);
        sharp.set_ding_resonance(DING_RESONANCE_RANGE.1);
        sharp.set_ding_formant(0, ding);
        sharp.set_ding_formant(1, ding);
        sharp.set_ding_formant(2, ding);
        let sharp_samples = collect(&mut sharp, 1.0);
        assert!(
            sharp_samples
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() < 2.0)
        );
        let sharp_power = tone_power(&sharp_samples, ding);
        assert!(
            sharp_power > soft_power * 3.0,
            "resonance did not raise the peak: soft {soft_power} sharp {sharp_power}"
        );

        let partial = ding * 9.0;
        let mut dull = ding_only(2, DEFAULT_DING_SHAPE);
        dull.set_ding_spread(0.0);
        dull.set_ding_strings(DEFAULT_DING_STRINGS);
        dull.set_ding_formant_q(0, 1.0);
        dull.set_ding_formant_q(1, 1.0);
        dull.set_ding_formant_q(2, 1.0);
        dull.set_ding_formant(0, 80.0);
        dull.set_ding_formant(1, ding);
        dull.set_ding_formant(2, 200.0);
        let dull_partial = tone_power(&collect(&mut dull, 1.0), partial);
        let mut bright = ding_only(2, DEFAULT_DING_SHAPE);
        bright.set_ding_spread(0.0);
        bright.set_ding_strings(DEFAULT_DING_STRINGS);
        bright.set_ding_formant_q(0, 1.0);
        bright.set_ding_formant_q(1, 1.0);
        bright.set_ding_formant_q(2, 10.0);
        bright.set_ding_formant(0, 80.0);
        bright.set_ding_formant(1, ding);
        bright.set_ding_formant(2, partial);
        let bright_partial = tone_power(&collect(&mut bright, 1.0), partial);
        assert!(
            bright_partial > dull_partial * 3.0,
            "the high band had nothing to filter: dull {dull_partial} bright {bright_partial}"
        );
    }

    #[test]
    fn tambura_sits_near_the_noise_ding() {
        let mut noise = ding_only(0, DEFAULT_DING_SHAPE);
        noise.set_ding_brightness(DEFAULT_DING_BRIGHTNESS);
        let noise_rms = rms(&collect(&mut noise, 2.0));
        let mut chord = ding_only(2, DEFAULT_DING_SHAPE);
        chord.set_ding_spread(DEFAULT_DING_SPREAD);
        chord.set_ding_strings(DEFAULT_DING_STRINGS);
        chord.set_tambura_jawari(DEFAULT_TAMBURA_JAWARI);
        // Faster cycle for a fair level check against the continuous noise wash.
        chord.set_tambura_strum(0.12);
        chord.set_tambura_cycle(1.6);
        chord.set_tambura_pick(0.45);
        chord.set_ding_resonance(DEFAULT_DING_RESONANCE);
        chord.set_ding_formant(0, DEFAULT_DING_FORMANT_1);
        chord.set_ding_formant(1, DEFAULT_DING_FORMANT_2);
        chord.set_ding_formant(2, DEFAULT_DING_FORMANT_3);
        let chord_samples = collect(&mut chord, 3.0);
        let chord_rms = rms(&chord_samples);
        assert!(
            chord_rms > noise_rms * 0.2 && chord_rms < noise_rms * 3.5,
            "tambura rms {chord_rms} noise rms {noise_rms}"
        );
        let ding = handpan_interval_hz(50, 0, 0);
        let low = tone_power(&chord_samples, ding * 0.5);
        let pa = tone_power(&chord_samples, handpan_interval_hz(50, 7, 0));
        assert!(
            low > 1.0 && pa > 1.0,
            "default formants hid the chord: low {low} pa {pa}"
        );
    }

    #[test]
    fn center_pan_keeps_both_speakers_and_the_ends_are_one_side() {
        let (left, right) = stereo_pan(0.5);
        assert_eq!((left, right), (1.0, 1.0));
        assert_eq!(stereo_pan(0.0), (1.0, 0.0));
        assert_eq!(stereo_pan(1.0), (0.0, 1.0));
    }

    #[test]
    fn a_soloed_ding_is_the_only_part() {
        let mut support = ding_only(0, DEFAULT_DING_SHAPE);
        let parts = support.next_parts();
        assert!(parts.ding().abs() > 1.0e-4, "the ding was silent");
        assert_eq!(parts.recent, 0.0);
        assert_eq!(parts.second, 0.0);
        assert_eq!(parts.bass, 0.0);
        assert!((parts.sum() - parts.ding().clamp(-1.0, 1.0)).abs() < 1.0e-5);
    }

    #[test]
    fn zero_wash_levels_leave_only_following_pads() {
        let mut support = ding_only(0, DEFAULT_DING_SHAPE);
        park_formants(&mut support);
        support.set_wash_level(0, 0.0);
        support.set_wash_level(1, 0.0);
        support.set_wash_level(2, 0.0);
        // Clear Root solo so Recent / Second can speak.
        support.set_parts(0, 0);
        support.set_part_level(1, 1.0);
        support.set_part_level(2, 1.0);
        let mut ding_e = 0.0;
        let mut pad_e = 0.0;
        for _ in 0..24_000 {
            let p = support.next_parts();
            ding_e += p.ding() * p.ding();
            pad_e += p.recent * p.recent + p.second * p.second;
        }
        assert!(ding_e < 1.0e-8, "washes at zero still sounded: {ding_e}");
        assert!(pad_e > 1.0e-4, "following pads were silent: {pad_e}");
    }

    #[test]
    fn formant_depth_zero_matches_a_still_filter() {
        let mut still = ding_only(0, DEFAULT_DING_SHAPE);
        let mut swept = ding_only(0, DEFAULT_DING_SHAPE);
        swept.set_formant_rate(3.0);
        swept.set_formant_depth(0.0);
        for _ in 0..4_000 {
            let plain = still.next_sample();
            let held = swept.next_sample();
            assert!(
                (plain - held).abs() < 1.0e-5,
                "depth 0 changed the tone: {plain} vs {held}"
            );
        }
    }

    #[test]
    fn formant_depth_moves_the_tone() {
        // Use the continuous waveshape wash so pluck timing does not dominate the diff.
        let ding = handpan_interval_hz(50, 0, 0);
        let mut still = ding_only(1, DEFAULT_DING_SHAPE);
        let mut moving = ding_only(1, DEFAULT_DING_SHAPE);
        for support in [&mut still, &mut moving] {
            support.set_ding_formant(0, ding);
            support.set_ding_formant(1, ding);
            support.set_ding_formant(2, ding);
            support.set_ding_formant_q(0, 8.0);
            support.set_ding_formant_q(1, 8.0);
            support.set_ding_formant_q(2, 8.0);
        }
        moving.set_formant_rate(1.0);
        moving.set_formant_depth(1.0);
        let mut held = Vec::new();
        let mut swept = Vec::new();
        for _ in 0..48_000 {
            held.push(still.next_sample());
            swept.push(moving.next_sample());
        }
        assert!(
            swept
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() < 2.0)
        );
        let mut diff = 0.0;
        let mut energy = 0.0;
        for (held_sample, swept_sample) in held.iter().zip(&swept) {
            energy += held_sample * held_sample;
            let delta = held_sample - swept_sample;
            diff += delta * delta;
        }
        assert!(
            diff > energy * 0.05,
            "the sweep should move the tone: difference {diff} held {energy}"
        );
    }

    #[test]
    fn drone_wander_stays_near_the_note() {
        let mut support = Support::new(48_000.0);
        advance(&mut support, 48_000 * 8);
        assert!(
            support.drones[0].zeta_hi >= 0.15 && support.drones[0].zeta_lo <= 0.07,
            "the root band should be wide"
        );
        for drone in &support.drones {
            assert!(
                drone.wander.abs() <= WANDER_CENTS + 0.01,
                "wander {} cents",
                drone.wander
            );
        }
    }

    #[test]
    fn bass_waits_for_the_history_to_agree() {
        let mut support = Support::new(48_000.0);
        let ding_bass = handpan_interval_hz(50, 0, 0) * 0.5;
        for _ in 0..12 {
            advance(&mut support, 24_000);
            support.hear(50, 0.5);
        }
        assert!(
            (support.bass.target_hz - ding_bass).abs() < 0.5,
            "bass locked at {} Hz",
            support.bass.target_hz
        );
        let locked = support.bass.target_hz;
        support.hear(57, 0.8);
        advance(&mut support, 24_000);
        assert!((support.bass.target_hz - locked).abs() < 0.01);

        let fifth_bass = handpan_interval_hz(50, 7, 0) * 0.5;
        for _ in 0..16 {
            advance(&mut support, 24_000);
            support.hear(57, 0.8);
        }
        assert!(
            (support.bass.target_hz - fifth_bass).abs() < 0.5,
            "bass moved to {} Hz",
            support.bass.target_hz
        );
    }
}
