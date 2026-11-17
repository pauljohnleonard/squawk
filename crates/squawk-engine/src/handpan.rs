//! Fixed D-Kurd handpan: one mass-spring cluster per tone field, a shared
//! Helmholtz cavity, and sympathetic springs only between partials that
//! already sit close in frequency.

use crate::StrikeExciter;
use crate::voice::StrikeVoice;

pub const HANDPAN_LAYOUT_LABEL: &str =
    "D Kurd: Ding D3, A3 Bb3 C4 D4 E4 F4 A4. Notes keep ringing.";
pub const DEFAULT_HANDPAN_DECAY: f32 = 1.0;
pub const DEFAULT_HANDPAN_DETUNE_CENTS: f32 = 6.0;
pub const DEFAULT_HANDPAN_COUPLING: f32 = 0.4;
pub const DEFAULT_HANDPAN_HIT: f32 = 0.6;
/// Just under the level where the glide becomes obvious on its own.
pub const DEFAULT_HANDPAN_GLIDE: f32 = 0.15;
pub const DEFAULT_HANDPAN_FUNDAMENTAL: f32 = 1.0;
pub const DEFAULT_HANDPAN_OCTAVE: f32 = 1.0;
pub const DEFAULT_HANDPAN_FIFTH: f32 = 1.0;
pub const DEFAULT_HANDPAN_HIGHER_DECAY: f32 = 1.0;
pub const HANDPAN_DECAY_RANGE: (f32, f32) = (0.25, 4.0);
pub const HANDPAN_DETUNE_CENTS_RANGE: (f32, f32) = (0.0, 30.0);
pub const HANDPAN_COUPLING_RANGE: (f32, f32) = (0.0, 1.0);
pub const HANDPAN_HIT_RANGE: (f32, f32) = (0.0, 1.0);
pub const HANDPAN_GLIDE_RANGE: (f32, f32) = (0.0, 0.4);
pub const HANDPAN_PARTIAL_RANGE: (f32, f32) = (0.0, 2.0);
pub const DEFAULT_HANDPAN_SCALE: u8 = 0;
pub const DEFAULT_HANDPAN_DING: u8 = 50;
pub const DEFAULT_HANDPAN_TUNING: u8 = 0;
pub const HANDPAN_DING_MIN: u8 = 48;
pub const HANDPAN_DING_MAX: u8 = 60;
pub const HANDPAN_SCALE_COUNT: u8 = 5;

const PAD_COUNT: usize = 10;
const MODES_PER_PAD: usize = 8;
const PARTIAL_COUNT: usize = 4;
const HIT_MS: f32 = 4.5;
const HIT_HIGHPASS_HZ: f32 = 2200.0;
/// Loud keyboard strikes arrive near this amplitude. Softer rows fall off faster than linear.
const LOUD_STRIKE_AMPLITUDE: f32 = 0.20;
/// Full-strength sharpening of the fundamental. A real handpan is closer to
/// 18 cents, mostly on the higher partials, gone in about 100 ms. These
/// values are exaggerated so the pitch fall is obvious.
const GLIDE_MAX_CENTS: f32 = 80.0;
const GLIDE_TAU_S: f32 = 0.40;
const MASS: f32 = 1.0;
const MAX_LINKS: usize = 160;
const SYMPATHETIC_CENTS: f32 = 45.0;
const SYMPATHETIC_STIFFNESS: f32 = 0.015;
const STRIKE_GAIN: f32 = 6.0;
const OUTPUT_GAIN: f32 = 0.55;
const CAVITY_MIX: f32 = 0.45;

/// Semitone offsets from the ding. Kurd is the layout the instrument started with.
const SCALE_INTERVALS: [[u8; PAD_COUNT]; 5] = [
    [0, 7, 8, 10, 12, 14, 15, 19, 20, 22],
    [0, 7, 10, 12, 14, 15, 17, 19, 22, 24],
    [0, 7, 12, 14, 15, 19, 22, 24, 26, 27],
    [0, 7, 8, 11, 12, 14, 15, 19, 20, 23],
    [0, 4, 7, 11, 12, 16, 18, 19, 23, 24],
];
const SCALE_NAMES: [&str; 5] = ["Kurd", "Celtic", "Pygmy", "Hijaz", "Aegean"];
struct Partial {
    /// Left and right frequencies, as multiples of the pad fundamental.
    ratios: [f32; 2],
    /// Tuned pairs take the cents split. The high pair stays at 7.2f and 8.1f.
    split_by_detune: bool,
    zeta: f32,
    drive: f32,
    mix: f32,
    /// How much of the strike glide this partial takes. Higher partials sharpen more.
    glide: f32,
}

const PARTIALS: [Partial; PARTIAL_COUNT] = [
    Partial {
        ratios: [1.0, 1.0],
        split_by_detune: true,
        zeta: 0.0014,
        drive: 1.0,
        mix: 0.34,
        glide: 1.0,
    },
    Partial {
        ratios: [2.0, 2.0],
        split_by_detune: true,
        zeta: 0.0028,
        drive: 0.42,
        mix: 0.22,
        glide: 1.2,
    },
    Partial {
        ratios: [3.0, 3.0],
        split_by_detune: true,
        zeta: 0.0055,
        drive: 0.20,
        mix: 0.12,
        glide: 1.5,
    },
    Partial {
        ratios: [7.2, 8.1],
        split_by_detune: false,
        zeta: 0.02,
        drive: 0.0,
        mix: 0.04,
        glide: 1.8,
    },
];

#[derive(Debug, Clone, Copy)]
struct SympatheticLink {
    a_pad: u8,
    a_mode: u8,
    b_pad: u8,
    b_mode: u8,
    k: f32,
    c: f32,
    closeness: f32,
}

impl SympatheticLink {
    const fn empty() -> Self {
        Self {
            a_pad: 0,
            a_mode: 0,
            b_pad: 0,
            b_mode: 0,
            k: 0.0,
            c: 0.0,
            closeness: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Pad {
    frequency_hz: f32,
    exciter: StrikeExciter,
    positions: [f32; MODES_PER_PAD],
    velocities: [f32; MODES_PER_PAD],
    stiffness: [f32; MODES_PER_PAD],
    damping: [f32; MODES_PER_PAD],
    drive_weight: [f32; MODES_PER_PAD],
    mix_weight: [f32; MODES_PER_PAD],
    mode_hz: [f32; MODES_PER_PAD],
    glide_weight: [f32; MODES_PER_PAD],
    /// Fraction of the full strike sharpening still left, 0 to 1.
    glide: f32,
    /// Where this strike landed, 0 at the center and 1 at the shoulder.
    place: f32,
    hit_remaining: u32,
    hit_len: u32,
    hit_level: f32,
    noise_state: u32,
    hit_lp: f32,
}

#[derive(Debug, Clone, Copy)]
struct Cavity {
    position: f32,
    velocity: f32,
    stiffness: f32,
    damping: f32,
}

/// Ten tone fields plus the shell cavity. State is only cleared on the pad
/// that was just struck; the others keep ringing.
#[derive(Debug, Clone)]
pub struct Handpan {
    inv_sample_rate: f32,
    max_hz: f32,
    decay: f32,
    detune_cents: f32,
    coupling: f32,
    hit: f32,
    fundamental: f32,
    octave: f32,
    fifth: f32,
    higher_decay: f32,
    scale: u8,
    ding: u8,
    tuning: u8,
    notes: [u8; PAD_COUNT],
    glide_amount: f32,
    glide_retain: f32,
    hit_len: u32,
    hit_hp_alpha: f32,
    pads: [Pad; PAD_COUNT],
    cavity: Cavity,
    links: [SympatheticLink; MAX_LINKS],
    link_count: usize,
}

impl Handpan {
    pub fn new(sample_rate_hz: f32) -> Self {
        let sample_rate_hz = sample_rate_hz.max(1.0);
        let notes = handpan_notes(DEFAULT_HANDPAN_SCALE, DEFAULT_HANDPAN_DING);
        let ding_hz = midi_note_to_hz(DEFAULT_HANDPAN_DING);
        let pads = std::array::from_fn(|index| Pad {
            frequency_hz: pad_hz(ding_hz, SCALE_INTERVALS[0][index], DEFAULT_HANDPAN_TUNING),
            exciter: StrikeExciter::new(sample_rate_hz, 1.5, 16.0),
            positions: [0.0; MODES_PER_PAD],
            velocities: [0.0; MODES_PER_PAD],
            stiffness: [0.0; MODES_PER_PAD],
            damping: [0.0; MODES_PER_PAD],
            drive_weight: [0.0; MODES_PER_PAD],
            mix_weight: [0.0; MODES_PER_PAD],
            mode_hz: [0.0; MODES_PER_PAD],
            glide_weight: [0.0; MODES_PER_PAD],
            glide: 0.0,
            place: 0.0,
            hit_remaining: 0,
            hit_len: 1,
            hit_level: 0.0,
            noise_state: 0x1234_5678 ^ ((index as u32 + 1).wrapping_mul(0x9E37_79B9)),
            hit_lp: 0.0,
        });

        let hit_len = ((HIT_MS / 1000.0) * sample_rate_hz).round().max(1.0) as u32;
        let hit_hp_alpha = 1.0 - (-core::f32::consts::TAU * HIT_HIGHPASS_HZ / sample_rate_hz).exp();
        let mut handpan = Self {
            inv_sample_rate: 1.0 / sample_rate_hz,
            max_hz: 0.45 * sample_rate_hz,
            decay: DEFAULT_HANDPAN_DECAY,
            detune_cents: DEFAULT_HANDPAN_DETUNE_CENTS,
            coupling: DEFAULT_HANDPAN_COUPLING,
            hit: DEFAULT_HANDPAN_HIT,
            fundamental: DEFAULT_HANDPAN_FUNDAMENTAL,
            octave: DEFAULT_HANDPAN_OCTAVE,
            fifth: DEFAULT_HANDPAN_FIFTH,
            higher_decay: DEFAULT_HANDPAN_HIGHER_DECAY,
            scale: DEFAULT_HANDPAN_SCALE,
            ding: DEFAULT_HANDPAN_DING,
            tuning: DEFAULT_HANDPAN_TUNING,
            notes,
            glide_amount: DEFAULT_HANDPAN_GLIDE,
            glide_retain: (-sample_rate_hz.recip() / GLIDE_TAU_S).exp(),
            hit_len,
            hit_hp_alpha: hit_hp_alpha.clamp(0.001, 1.0),
            pads,
            cavity: Cavity {
                position: 0.0,
                velocity: 0.0,
                stiffness: 0.0,
                damping: 0.0,
            },
            links: [SympatheticLink::empty(); MAX_LINKS],
            link_count: 0,
        };
        handpan.rebuild();
        handpan
    }

    pub fn set_decay(&mut self, decay: f32) {
        let (min, max) = HANDPAN_DECAY_RANGE;
        let decay = decay.clamp(min, max);
        if (decay - self.decay).abs() < 1.0e-5 {
            return;
        }
        self.decay = decay;
        self.rebuild();
    }

    pub fn set_detune_cents(&mut self, cents: f32) {
        let (min, max) = HANDPAN_DETUNE_CENTS_RANGE;
        let cents = cents.clamp(min, max);
        if (cents - self.detune_cents).abs() < 1.0e-4 {
            return;
        }
        self.detune_cents = cents;
        self.rebuild();
    }

    pub fn set_coupling(&mut self, coupling: f32) {
        let (min, max) = HANDPAN_COUPLING_RANGE;
        let coupling = coupling.clamp(min, max);
        if (coupling - self.coupling).abs() < 1.0e-5 {
            return;
        }
        self.coupling = coupling;
        self.rebuild();
    }

    pub fn set_hit(&mut self, hit: f32) {
        let (min, max) = HANDPAN_HIT_RANGE;
        self.hit = hit.clamp(min, max);
    }

    pub fn set_glide(&mut self, glide: f32) {
        let (min, max) = HANDPAN_GLIDE_RANGE;
        self.glide_amount = glide.clamp(min, max);
    }

    pub fn set_fundamental(&mut self, fundamental: f32) {
        let (min, max) = HANDPAN_PARTIAL_RANGE;
        let fundamental = fundamental.clamp(min, max);
        if (fundamental - self.fundamental).abs() < 1.0e-5 {
            return;
        }
        self.fundamental = fundamental;
        self.rebuild();
    }

    pub fn set_octave(&mut self, octave: f32) {
        let (min, max) = HANDPAN_PARTIAL_RANGE;
        let octave = octave.clamp(min, max);
        if (octave - self.octave).abs() < 1.0e-5 {
            return;
        }
        self.octave = octave;
        self.rebuild();
    }

    pub fn set_fifth(&mut self, fifth: f32) {
        let (min, max) = HANDPAN_PARTIAL_RANGE;
        let fifth = fifth.clamp(min, max);
        if (fifth - self.fifth).abs() < 1.0e-5 {
            return;
        }
        self.fifth = fifth;
        self.rebuild();
    }

    pub fn set_higher_decay(&mut self, higher_decay: f32) {
        let (min, max) = HANDPAN_PARTIAL_RANGE;
        let higher_decay = higher_decay.clamp(min, max);
        if (higher_decay - self.higher_decay).abs() < 1.0e-5 {
            return;
        }
        self.higher_decay = higher_decay;
        self.rebuild();
    }

    /// Retune the ten pads. Ringing pads keep their energy and move to the new pitch.
    pub fn set_layout(&mut self, scale: u8, ding: u8, tuning: u8) {
        let scale = scale.min(HANDPAN_SCALE_COUNT - 1);
        let ding = ding.clamp(HANDPAN_DING_MIN, HANDPAN_DING_MAX);
        let tuning = u8::from(tuning != 0);
        if scale == self.scale && ding == self.ding && tuning == self.tuning {
            return;
        }
        self.scale = scale;
        self.ding = ding;
        self.tuning = tuning;
        self.notes = handpan_notes(scale, ding);
        let ding_hz = midi_note_to_hz(ding);
        let intervals = SCALE_INTERVALS[scale as usize];
        for (index, pad) in self.pads.iter_mut().enumerate() {
            pad.frequency_hz = pad_hz(ding_hz, intervals[index], tuning);
        }
        self.rebuild();
    }

    /// Strike the scale degree nearest to `note`. Other pads are left running.
    pub fn strike_midi(&mut self, note: u8, amplitude: f32) {
        self.strike_at(note, amplitude, 0.0);
    }

    /// Strike `note` at `place`, from 0 at the center of the pad to 1 at the shoulder.
    pub fn strike_at(&mut self, note: u8, amplitude: f32, place: f32) {
        let index = pad_index_for_notes(&self.notes, note);
        let amplitude = amplitude.clamp(0.0, 1.0);
        let place = place.clamp(0.0, 1.0);
        let pulse = amplitude * STRIKE_GAIN * place_level(place);
        let pad = &mut self.pads[index];
        pad.exciter.trigger(pulse);
        pad.place = place;
        let brightness = strike_brightness(amplitude);
        pad.glide = brightness * self.glide_amount;
        pad.hit_level = brightness * self.hit;
        pad.hit_len = self.hit_len;
        pad.hit_remaining = if brightness > 0.0 { self.hit_len } else { 0 };
        pad.hit_lp = 0.0;
    }

    #[inline]
    pub fn next_sample(&mut self) -> f32 {
        let mut exciter = [0.0; PAD_COUNT];
        let mut clicks = [0.0; PAD_COUNT];
        let hp_alpha = self.hit_hp_alpha;
        for (index, pad) in self.pads.iter_mut().enumerate() {
            exciter[index] = pad.exciter.sample();
            clicks[index] = pad.sample_click(hp_alpha);
        }

        let mut pos = [[0.0; MODES_PER_PAD]; PAD_COUNT];
        let mut vel = [[0.0; MODES_PER_PAD]; PAD_COUNT];
        for (index, pad) in self.pads.iter().enumerate() {
            pos[index] = pad.positions;
            vel[index] = pad.velocities;
        }
        let cavity_position = self.cavity.position;
        let cavity_velocity = self.cavity.velocity;

        let mut force = [[0.0; MODES_PER_PAD]; PAD_COUNT];
        let mut cavity_force = 0.0;

        for link in &self.links[..self.link_count] {
            let a_pad = link.a_pad as usize;
            let b_pad = link.b_pad as usize;
            let a_mode = link.a_mode as usize;
            let b_mode = link.b_mode as usize;
            let pos_diff = pos[b_pad][b_mode] - pos[a_pad][a_mode];
            let vel_diff = vel[b_pad][b_mode] - vel[a_pad][a_mode];
            let coupled = link.k * pos_diff + link.c * vel_diff;
            force[a_pad][a_mode] += coupled;
            force[b_pad][b_mode] -= coupled;
        }

        if self.coupling > 0.0 {
            for pad_index in 0..PAD_COUNT {
                for mode in 0..2 {
                    let damp_scale =
                        0.5 * (self.cavity.damping + self.pads[pad_index].damping[mode]);
                    let drive = self.coupling * 0.45 * damp_scale;
                    cavity_force += drive * vel[pad_index][mode];
                    let pressure = self.coupling * 0.04 * self.cavity.stiffness * cavity_position;
                    force[pad_index][mode] += pressure;
                }
            }
        }

        let dt = self.inv_sample_rate;
        let glide_retain = self.glide_retain;
        let fifth_level = self.fifth;
        for pad_index in 0..PAD_COUNT {
            let pad = &mut self.pads[pad_index];
            pad.glide *= glide_retain;
            let place = pad.place;
            let click_place = 1.0 + 3.0 * place_amount(place);
            for mode in 0..MODES_PER_PAD {
                let cents = pad.glide * GLIDE_MAX_CENTS * pad.glide_weight[mode];
                let restoring =
                    pad.stiffness[mode] * stiffness_scale_for_cents(cents) * pos[pad_index][mode];
                let damping_force = pad.damping[mode] * vel[pad_index][mode];
                let mut drive_cmd = exciter[pad_index]
                    * pad.drive_weight[mode]
                    * place_drive_scale(mode / 2, place);
                if mode >= 6 {
                    drive_cmd += clicks[pad_index] * 0.55 * click_place;
                } else if mode >= 4 {
                    drive_cmd += clicks[pad_index] * 0.28 * fifth_level * click_place;
                }
                let drive = drive_cmd * pad.stiffness[mode];
                let acceleration =
                    (drive - restoring - damping_force + force[pad_index][mode]) / MASS;
                pad.velocities[mode] = vel[pad_index][mode] + acceleration * dt;
            }
            for mode in 0..MODES_PER_PAD {
                pad.positions[mode] += pad.velocities[mode] * dt;
                pad.positions[mode] = pad.positions[mode].clamp(-4.0, 4.0);
            }
        }

        let cavity_restoring = self.cavity.stiffness * cavity_position;
        let cavity_damping_force = self.cavity.damping * cavity_velocity;
        let cavity_acceleration = (cavity_force - cavity_restoring - cavity_damping_force) / MASS;
        self.cavity.velocity = cavity_velocity + cavity_acceleration * dt;
        self.cavity.position += self.cavity.velocity * dt;
        self.cavity.position = self.cavity.position.clamp(-4.0, 4.0);

        let mut sample = self.cavity.position * CAVITY_MIX;
        for (index, pad) in self.pads.iter().enumerate() {
            for mode in 0..MODES_PER_PAD {
                sample += pad.positions[mode] * pad.mix_weight[mode];
            }
            sample += clicks[index] * 0.28;
        }
        (sample * OUTPUT_GAIN).clamp(-1.0, 1.0)
    }

    fn rebuild(&mut self) {
        let decay = self.decay.max(0.05);
        let fundamental = self.fundamental;
        let octave = self.octave;
        let fifth = self.fifth;
        let higher_decay = self.higher_decay;
        let mix_sum: f32 = PARTIALS.iter().map(|partial| partial.mix * 2.0).sum();
        let mix_norm = mix_sum.max(0.000_1);

        for pad in &mut self.pads {
            for (partial_index, partial) in PARTIALS.iter().enumerate() {
                for side in 0..2 {
                    let mode = partial_index * 2 + side;
                    let mut ratio = partial.ratios[side];
                    if partial.split_by_detune {
                        let sign = if side == 0 { -0.5 } else { 0.5 };
                        ratio *= 2.0_f32.powf((sign * self.detune_cents) / 1200.0);
                    }
                    let frequency = (pad.frequency_hz * ratio).clamp(1.0, self.max_hz);
                    let zeta = partial_zeta(
                        partial_index,
                        partial.zeta,
                        higher_decay,
                        frequency,
                        pad.frequency_hz,
                    ) / decay;
                    pad.mode_hz[mode] = frequency;
                    pad.stiffness[mode] = stiffness_for_frequency(frequency);
                    pad.damping[mode] = damping_for_frequency(frequency, zeta);
                    let level = partial_level(partial_index, fundamental, octave, fifth);
                    pad.drive_weight[mode] = partial.drive * level;
                    pad.mix_weight[mode] = partial.mix / mix_norm;
                    pad.glide_weight[mode] = partial.glide;
                }
            }
        }

        let ding_hz = self.pads[0].frequency_hz;
        let cavity_hz = (ding_hz * 0.8).clamp(1.0, self.max_hz);
        self.cavity.stiffness = stiffness_for_frequency(cavity_hz);
        self.cavity.damping = damping_for_frequency(cavity_hz, 0.004 / decay);

        self.rebuild_links();
    }

    fn rebuild_links(&mut self) {
        self.link_count = 0;
        if self.coupling <= 0.0 {
            return;
        }

        for a_pad in 0..PAD_COUNT {
            for b_pad in (a_pad + 1)..PAD_COUNT {
                for a_mode in 0..MODES_PER_PAD {
                    for b_mode in 0..MODES_PER_PAD {
                        let a_hz = self.pads[a_pad].mode_hz[a_mode];
                        let b_hz = self.pads[b_pad].mode_hz[b_mode];
                        let cents = cents_between(a_hz, b_hz);
                        if cents > SYMPATHETIC_CENTS {
                            continue;
                        }
                        let closeness = 1.0 - cents / SYMPATHETIC_CENTS;
                        let stiffness_scale = 0.5
                            * (self.pads[a_pad].stiffness[a_mode]
                                + self.pads[b_pad].stiffness[b_mode]);
                        let damping_scale = 0.5
                            * (self.pads[a_pad].damping[a_mode] + self.pads[b_pad].damping[b_mode]);
                        let link = SympatheticLink {
                            a_pad: a_pad as u8,
                            a_mode: a_mode as u8,
                            b_pad: b_pad as u8,
                            b_mode: b_mode as u8,
                            k: self.coupling * SYMPATHETIC_STIFFNESS * closeness * stiffness_scale,
                            c: self.coupling * 0.08 * closeness * damping_scale,
                            closeness,
                        };
                        self.insert_link(link);
                    }
                }
            }
        }
    }

    fn insert_link(&mut self, link: SympatheticLink) {
        if self.link_count < MAX_LINKS {
            self.links[self.link_count] = link;
            self.link_count += 1;
            return;
        }

        let mut worst = 0;
        let mut worst_closeness = self.links[0].closeness;
        for index in 1..MAX_LINKS {
            if self.links[index].closeness < worst_closeness {
                worst_closeness = self.links[index].closeness;
                worst = index;
            }
        }
        if link.closeness > worst_closeness {
            self.links[worst] = link;
        }
    }
}

impl StrikeVoice for Handpan {
    fn strike_at(&mut self, note: u8, amplitude: f32, place: f32) {
        Handpan::strike_at(self, note, amplitude, place);
    }

    fn next_sample(&mut self) -> f32 {
        Handpan::next_sample(self)
    }
}

impl Pad {
    fn sample_click(&mut self, hp_alpha: f32) -> f32 {
        if self.hit_remaining == 0 || self.hit_level <= 0.0 {
            return 0.0;
        }
        self.hit_remaining -= 1;
        let t = self.hit_remaining as f32 / self.hit_len.max(1) as f32;
        let env = t * t;

        let mut x = self.noise_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        if x == 0 {
            x = 0xA5A5_A5A5;
        }
        self.noise_state = x;
        let noise = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;
        self.hit_lp += (noise - self.hit_lp) * hp_alpha;
        let highpass = noise - self.hit_lp;
        highpass * env * self.hit_level
    }
}

fn partial_level(partial_index: usize, fundamental: f32, octave: f32, fifth: f32) -> f32 {
    match partial_index {
        0 => fundamental,
        1 => octave,
        2 => fifth,
        _ => 1.0,
    }
}

/// Damping of partials above the fundamental. `1` keeps the built-in ratios.
/// `0` gives each partial the same ring time as the fundamental.
fn partial_zeta(
    partial_index: usize,
    zeta: f32,
    higher_decay: f32,
    frequency_hz: f32,
    fundamental_hz: f32,
) -> f32 {
    if partial_index == 0 {
        return zeta;
    }
    let fund_zeta = PARTIALS[0].zeta.max(1.0e-8);
    let freq_ratio = (frequency_hz / fundamental_hz.max(1.0)).max(1.0);
    let recipe_rate = zeta * freq_ratio;
    let matched_rate = fund_zeta;
    let rate = matched_rate * (recipe_rate / matched_rate).powf(higher_decay);
    rate / freq_ratio
}

fn stiffness_scale_for_cents(cents: f32) -> f32 {
    let ratio = 2.0_f32.powf(cents / 1200.0);
    ratio * ratio
}

fn strike_brightness(amplitude: f32) -> f32 {
    let norm = (amplitude / LOUD_STRIKE_AMPLITUDE).clamp(0.0, 1.0);
    norm * norm
}

/// 0 stays the center. The middle of the range is already well toward the shoulder,
/// so the three keyboard rows do not blur together.
fn place_amount(place: f32) -> f32 {
    place.clamp(0.0, 1.0).sqrt()
}

/// How this partial is struck at `place`. Center keeps the recipe. The shoulder
/// nearly drops the fundamental and rings on the octave and the fifth.
fn place_drive_scale(partial_index: usize, place: f32) -> f32 {
    let place = place_amount(place);
    let at_shoulder = match partial_index {
        0 => 0.05,
        1 => 2.4,
        2 => 4.0,
        _ => 1.0,
    };
    1.0 + (at_shoulder - 1.0) * place
}

/// Brings a shoulder strike back up to the struck energy of a center strike.
fn place_level(place: f32) -> f32 {
    let place = place.clamp(0.0, 1.0);
    let base = 1.0 + 0.42 + 0.20;
    let shaped = place_drive_scale(0, place)
        + 0.42 * place_drive_scale(1, place)
        + 0.20 * place_drive_scale(2, place);
    base / shaped.max(0.000_1)
}

pub fn handpan_scale_name(scale: u8) -> &'static str {
    SCALE_NAMES[scale.min(HANDPAN_SCALE_COUNT - 1) as usize]
}

pub fn handpan_interval_hz(ding: u8, semitones: u8, tuning: u8) -> f32 {
    pad_hz(midi_note_to_hz(ding), semitones, tuning)
}

pub fn handpan_notes(scale: u8, ding: u8) -> [u8; PAD_COUNT] {
    let scale = scale.min(HANDPAN_SCALE_COUNT - 1) as usize;
    let ding = ding.clamp(HANDPAN_DING_MIN, HANDPAN_DING_MAX);
    std::array::from_fn(|index| ding.saturating_add(SCALE_INTERVALS[scale][index]))
}

fn pad_hz(ding_hz: f32, semitones: u8, tuning: u8) -> f32 {
    if tuning == 0 {
        return ding_hz * 2.0_f32.powf(semitones as f32 / 12.0);
    }
    ding_hz * just_ratio(semitones)
}

fn just_ratio(semitones: u8) -> f32 {
    let octaves = semitones / 12;
    let degree = match semitones % 12 {
        0 => 1.0,
        1 => 16.0 / 15.0,
        2 => 9.0 / 8.0,
        3 => 6.0 / 5.0,
        4 => 5.0 / 4.0,
        5 => 4.0 / 3.0,
        // Augmented fourth. A unison here dropped the Aegean M below N.
        6 => 45.0 / 32.0,
        7 => 3.0 / 2.0,
        8 => 8.0 / 5.0,
        9 => 5.0 / 3.0,
        10 => 9.0 / 5.0,
        11 => 15.0 / 8.0,
        _ => 1.0,
    };
    degree * 2.0_f32.powi(octaves as i32)
}

fn pad_index_for_notes(notes: &[u8; PAD_COUNT], note: u8) -> usize {
    let mut best = 0;
    let mut best_distance = i16::MAX;
    for (index, pad_note) in notes.iter().enumerate() {
        let distance = (note as i16 - *pad_note as i16).abs();
        if distance < best_distance {
            best_distance = distance;
            best = index;
        }
    }
    best
}

fn midi_note_to_hz(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

fn cents_between(a_hz: f32, b_hz: f32) -> f32 {
    if a_hz <= 1.0 || b_hz <= 1.0 {
        return 10_000.0;
    }
    (1200.0 * (a_hz / b_hz).ln() / core::f32::consts::LN_2).abs()
}

fn stiffness_for_frequency(frequency_hz: f32) -> f32 {
    let omega = core::f32::consts::TAU * frequency_hz;
    MASS * omega * omega
}

fn damping_for_frequency(frequency_hz: f32, zeta: f32) -> f32 {
    let omega = core::f32::consts::TAU * frequency_hz.max(1.0);
    2.0 * zeta * MASS * omega
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad_energy(handpan: &Handpan, index: usize) -> f32 {
        let pad = &handpan.pads[index];
        let mut energy = 0.0;
        for mode in 0..MODES_PER_PAD {
            energy += pad.positions[mode] * pad.positions[mode];
            energy += pad.velocities[mode] * pad.velocities[mode];
        }
        energy
    }

    fn settle(handpan: &mut Handpan, samples: usize) -> f32 {
        let mut peak = 0.0_f32;
        for _ in 0..samples {
            let sample = handpan.next_sample();
            assert!(sample.is_finite());
            peak = peak.max(sample.abs());
        }
        peak
    }

    #[test]
    fn strike_stays_finite_and_rings() {
        let mut handpan = Handpan::new(48_000.0);
        handpan.set_coupling(0.0);
        handpan.strike_midi(50, 0.8);
        let early = settle(&mut handpan, 2_000);
        assert!(early > 0.01, "expected an audible strike, peak was {early}");
        let ding = pad_energy(&handpan, 0);
        let later = settle(&mut handpan, 4_000);
        assert!(later.is_finite());
        assert!(
            pad_energy(&handpan, 0) > ding * 0.05,
            "the ding should still be ringing after note-off is never sent"
        );
    }

    #[test]
    fn second_strike_does_not_clear_the_first_pad() {
        let mut handpan = Handpan::new(48_000.0);
        handpan.set_coupling(0.0);
        handpan.strike_midi(50, 0.8);
        settle(&mut handpan, 1_500);
        let before = pad_energy(&handpan, 0);
        assert!(before > 1.0);
        handpan.strike_midi(69, 0.8);
        let after = pad_energy(&handpan, 0);
        assert!(
            after > before * 0.5,
            "striking A4 cleared the ding ({before} -> {after})"
        );
    }

    #[test]
    fn zero_coupling_leaves_other_pads_quiet() {
        let mut handpan = Handpan::new(48_000.0);
        handpan.set_coupling(0.0);
        handpan.strike_midi(50, 0.8);
        settle(&mut handpan, 4_000);
        assert!(pad_energy(&handpan, 0) > 1.0);
        assert!(
            pad_energy(&handpan, 7) < 1.0e-4,
            "A4 rang without coupling: {}",
            pad_energy(&handpan, 7)
        );
    }

    #[test]
    fn coupling_wakes_a_shared_partial() {
        let mut quiet = Handpan::new(48_000.0);
        quiet.set_coupling(0.0);
        quiet.strike_midi(50, 0.8);
        settle(&mut quiet, 6_000);
        let leaked = pad_energy(&quiet, 7);

        let mut coupled = Handpan::new(48_000.0);
        coupled.set_coupling(1.0);
        coupled.strike_midi(50, 0.8);
        settle(&mut coupled, 6_000);
        let sympathetic = pad_energy(&coupled, 7);
        assert!(
            sympathetic > leaked * 20.0 + 1.0e-3,
            "expected the A4 field to pick up the ding's fifth, got {sympathetic} vs leak {leaked}"
        );
        assert!(coupled.link_count > 0);
    }

    fn high_mode_energy(handpan: &Handpan, index: usize) -> f32 {
        let pad = &handpan.pads[index];
        let mut energy = 0.0;
        for mode in 6..MODES_PER_PAD {
            energy += pad.positions[mode] * pad.positions[mode];
            energy += pad.velocities[mode] * pad.velocities[mode];
        }
        energy
    }

    #[test]
    fn high_pair_stays_near_seven_and_eight() {
        let handpan = Handpan::new(48_000.0);
        let fundamental = handpan.pads[0].frequency_hz;
        let low = handpan.pads[0].mode_hz[6] / fundamental;
        let high = handpan.pads[0].mode_hz[7] / fundamental;
        assert!((low - 7.2).abs() < 0.02, "low high-mode ratio was {low}");
        assert!((high - 8.1).abs() < 0.02, "high high-mode ratio was {high}");
        assert!(handpan.pads[0].drive_weight[6] == 0.0);
        assert!(handpan.pads[0].mix_weight[6] < handpan.pads[0].mix_weight[0] * 0.2);
    }

    #[test]
    fn hit_zero_leaves_the_high_pair_quiet() {
        let mut handpan = Handpan::new(48_000.0);
        handpan.set_hit(0.0);
        handpan.set_coupling(0.0);
        handpan.strike_midi(50, 0.25);
        settle(&mut handpan, 800);
        assert!(
            high_mode_energy(&handpan, 0) < 1.0e-4,
            "high pair rang without a hit: {}",
            high_mode_energy(&handpan, 0)
        );
        assert!(pad_energy(&handpan, 0) > 1.0);
    }

    #[test]
    fn loud_hit_is_brighter_than_a_soft_one() {
        let mut loud = Handpan::new(48_000.0);
        loud.set_hit(1.0);
        loud.set_coupling(0.0);
        loud.strike_midi(50, 0.25);
        let mut loud_peak = 0.0_f32;
        for _ in 0..400 {
            loud_peak = loud_peak.max(loud.next_sample().abs());
        }
        let loud_high = high_mode_energy(&loud, 0);

        let mut soft = Handpan::new(48_000.0);
        soft.set_hit(1.0);
        soft.set_coupling(0.0);
        soft.strike_midi(50, 0.05);
        let mut soft_peak = 0.0_f32;
        for _ in 0..400 {
            soft_peak = soft_peak.max(soft.next_sample().abs());
        }
        let soft_high = high_mode_energy(&soft, 0);

        assert!(
            loud_high > soft_high * 4.0 + 1.0e-3,
            "expected the hard strike to wake the high pair, {loud_high} vs {soft_high}"
        );
        assert!(loud_peak > soft_peak);
    }

    #[test]
    fn detune_splits_each_pair() {
        let mut handpan = Handpan::new(48_000.0);
        handpan.set_detune_cents(12.0);
        let low = handpan.pads[0].mode_hz[0];
        let high = handpan.pads[0].mode_hz[1];
        let cents = cents_between(high, low);
        assert!((cents - 12.0).abs() < 0.05, "pair split was {cents} cents");
        let octave = handpan.pads[0].mode_hz[2];
        assert!((octave / low - 2.0).abs() < 0.02);
    }

    #[test]
    fn glide_follows_strike_strength_and_settles() {
        let mut loud = Handpan::new(48_000.0);
        let max_glide = HANDPAN_GLIDE_RANGE.1;
        loud.set_glide(max_glide);
        loud.set_coupling(0.0);
        assert!((loud.pads[0].glide_weight[0] - 1.0).abs() < 0.01);
        assert!((loud.pads[0].glide_weight[2] - 1.2).abs() < 0.01);
        assert!((loud.pads[0].glide_weight[4] - 1.5).abs() < 0.01);
        assert!((loud.pads[0].glide_weight[6] - 1.8).abs() < 0.01);

        loud.strike_midi(50, 0.25);
        let start = loud.pads[0].glide;
        assert!((start - max_glide).abs() < 0.05, "loud glide was {start}");
        for _ in 0..19_200 {
            loud.next_sample();
        }
        let mid = loud.pads[0].glide;
        assert!(
            mid < start * 0.5 && mid > start * 0.2,
            "glide after one time constant was {mid}"
        );
        for _ in 0..57_600 {
            loud.next_sample();
        }
        assert!(loud.pads[0].glide < 0.05);

        let mut soft = Handpan::new(48_000.0);
        soft.set_glide(max_glide);
        soft.strike_midi(50, 0.05);
        assert!(
            soft.pads[0].glide < 0.1,
            "soft glide was {}",
            soft.pads[0].glide
        );

        let mut off = Handpan::new(48_000.0);
        off.set_glide(0.0);
        off.strike_midi(50, 0.25);
        assert_eq!(off.pads[0].glide, 0.0);
        assert!((stiffness_scale_for_cents(18.0) - 1.021).abs() < 0.002);
    }

    #[test]
    fn glide_raises_the_sounding_pitch() {
        let early = {
            let off = mode_hz_over(0.0, 0, 1_200, 4_000);
            let on = mode_hz_over(HANDPAN_GLIDE_RANGE.1, 0, 1_200, 4_000);
            1200.0 * (on / off).ln() / core::f32::consts::LN_2
        };
        let settled = {
            let off = mode_hz_over(0.0, 0, 96_000, 112_000);
            let on = mode_hz_over(HANDPAN_GLIDE_RANGE.1, 0, 96_000, 112_000);
            1200.0 * (on / off).ln() / core::f32::consts::LN_2
        };
        assert!(
            early > 20.0,
            "fundamental was only {early:.1} cents sharp just after the strike"
        );
        assert!(
            settled.abs() < 8.0,
            "fundamental stayed {settled:.1} cents off after the glide died"
        );
    }

    fn mode_hz_over(glide: f32, mode: usize, start: usize, end: usize) -> f32 {
        let mut handpan = Handpan::new(48_000.0);
        handpan.set_glide(glide);
        handpan.set_coupling(0.0);
        handpan.set_hit(0.0);
        handpan.strike_midi(50, 0.25);
        let mut previous = 0.0;
        let mut crossings = [0usize; 128];
        let mut count = 0;
        for sample in 0..end {
            let position = {
                let _ = handpan.next_sample();
                handpan.pads[0].positions[mode]
            };
            if sample >= start && previous <= 0.0 && position > 0.0 && count < crossings.len() {
                crossings[count] = sample;
                count += 1;
            }
            previous = position;
        }
        assert!(
            count >= 4,
            "mode {mode} glide {glide} only had {count} zero crossings in {start}-{end}"
        );
        let periods = count - 1;
        let samples: usize = crossings[periods] - crossings[0];
        48_000.0 * periods as f32 / samples as f32
    }

    #[test]
    fn partial_knobs_scale_the_strike() {
        let handpan = Handpan::new(48_000.0);
        assert!((handpan.pads[0].drive_weight[2] - 0.42).abs() < 0.01);
        assert!((handpan.pads[0].drive_weight[4] - 0.20).abs() < 0.01);
        let fund = handpan.pads[0].mix_weight[0];
        assert!((handpan.pads[0].mix_weight[2] / fund - 0.22 / 0.34).abs() < 0.01);

        let mut quiet_octave = Handpan::new(48_000.0);
        quiet_octave.strike_midi(50, 0.25);
        for _ in 0..1_000 {
            quiet_octave.next_sample();
        }
        let ringing = quiet_octave.pads[0].positions[0];
        assert!(ringing.abs() > 1.0e-4);
        quiet_octave.set_octave(0.0);
        assert!((quiet_octave.pads[0].positions[0] - ringing).abs() < 1.0e-6);
        assert_eq!(quiet_octave.pads[0].drive_weight[2], 0.0);
        assert_eq!(quiet_octave.pads[0].drive_weight[3], 0.0);

        let mut quiet_fundamental = Handpan::new(48_000.0);
        quiet_fundamental.set_fundamental(0.0);
        assert_eq!(quiet_fundamental.pads[0].drive_weight[0], 0.0);
        assert_eq!(quiet_fundamental.pads[0].drive_weight[1], 0.0);
        assert!(quiet_fundamental.pads[0].drive_weight[2] > 0.4);

        let mut loud_fifth = Handpan::new(48_000.0);
        loud_fifth.set_fifth(2.0);
        assert!((loud_fifth.pads[0].drive_weight[4] - 0.40).abs() < 0.01);

        let mut fifth_only = Handpan::new(48_000.0);
        fifth_only.set_coupling(1.0);
        fifth_only.set_fundamental(0.0);
        fifth_only.set_octave(0.0);
        fifth_only.set_hit(0.0);
        fifth_only.strike_midi(57, 0.25);
        for _ in 0..2_000 {
            fifth_only.next_sample();
        }
        let octave_energy = fifth_only.pads[1].positions[2].abs()
            + fifth_only.pads[1].positions[3].abs()
            + fifth_only.pads[0].positions[4].abs()
            + fifth_only.pads[7].positions[0].abs();
        assert!(
            octave_energy < 1.0e-4,
            "the fifth strike woke the octave pitch, energy {octave_energy}"
        );
    }

    #[test]
    fn higher_decay_bends_partials_around_the_fundamental() {
        let mut together = Handpan::new(48_000.0);
        together.set_higher_decay(0.0);
        let fund_rate = ring_rate(&together, 0);
        for mode in [2, 4, 6] {
            let rate = ring_rate(&together, mode);
            assert!(
                (rate - fund_rate).abs() < fund_rate * 0.05,
                "mode {mode} ring rate {rate} vs fundamental {fund_rate}"
            );
        }

        let stock = Handpan::new(48_000.0);
        let stock_fund = zeta_of(&stock, 0);
        assert!((zeta_of(&stock, 2) / stock_fund - 2.0).abs() < 0.05);
        assert!((zeta_of(&stock, 0) - zeta_of(&together, 0)).abs() < stock_fund * 0.05);

        let mut fast = Handpan::new(48_000.0);
        fast.set_higher_decay(2.0);
        assert!((zeta_of(&fast, 2) / zeta_of(&fast, 0) - 8.0).abs() < 0.5);
    }

    fn ring_rate(handpan: &Handpan, mode: usize) -> f32 {
        zeta_of(handpan, mode) * handpan.pads[0].mode_hz[mode]
    }

    fn zeta_of(handpan: &Handpan, mode: usize) -> f32 {
        let pad = &handpan.pads[0];
        let omega = core::f32::consts::TAU * pad.mode_hz[mode];
        pad.damping[mode] / (2.0 * omega)
    }

    #[test]
    fn shoulder_strike_thins_the_fundamental() {
        let mut center = Handpan::new(48_000.0);
        center.set_coupling(0.0);
        center.set_hit(0.0);
        center.set_glide(0.0);
        center.strike_at(50, 0.25, 0.0);
        let mut shoulder = Handpan::new(48_000.0);
        shoulder.set_coupling(0.0);
        shoulder.set_hit(0.0);
        shoulder.set_glide(0.0);
        shoulder.strike_at(50, 0.25, 1.0);
        let mut center_fund = 0.0f32;
        let mut shoulder_fund = 0.0f32;
        let mut center_fifth = 0.0f32;
        let mut shoulder_fifth = 0.0f32;
        for sample in 0..4_000 {
            center.next_sample();
            shoulder.next_sample();
            if sample > 800 {
                center_fund = center_fund.max(center.pads[0].positions[0].abs());
                shoulder_fund = shoulder_fund.max(shoulder.pads[0].positions[0].abs());
                center_fifth = center_fifth.max(center.pads[0].positions[4].abs());
                shoulder_fifth = shoulder_fifth.max(shoulder.pads[0].positions[4].abs());
            }
        }
        assert!(
            shoulder_fund < center_fund * 0.25,
            "shoulder fundamental {shoulder_fund} vs center {center_fund}"
        );
        let center_ratio = center_fifth / center_fund.max(1.0e-6);
        let shoulder_ratio = shoulder_fifth / shoulder_fund.max(1.0e-6);
        assert!(
            shoulder_ratio > center_ratio * 4.0,
            "fifth/fundamental shoulder {shoulder_ratio} vs center {center_ratio}"
        );
    }

    #[test]
    fn midi_note_selects_nearest_pad() {
        let notes = handpan_notes(DEFAULT_HANDPAN_SCALE, DEFAULT_HANDPAN_DING);
        assert_eq!(notes, [50, 57, 58, 60, 62, 64, 65, 69, 70, 72]);
        assert_eq!(pad_index_for_notes(&notes, 50), 0);
        assert_eq!(pad_index_for_notes(&notes, 69), 7);
        assert_eq!(pad_index_for_notes(&notes, 62), 4);
        assert_eq!(pad_index_for_notes(&notes, 61), 3);
    }

    #[test]
    fn each_scale_rises_across_ten_pads() {
        for scale in 0..HANDPAN_SCALE_COUNT {
            let notes = handpan_notes(scale, 50);
            for pair in notes.windows(2) {
                assert!(
                    pair[1] > pair[0],
                    "scale {scale} notes {notes:?} do not rise"
                );
            }
            let mut handpan = Handpan::new(48_000.0);
            handpan.set_layout(scale, 50, 1);
            for pair in handpan.pads.windows(2) {
                assert!(
                    pair[1].frequency_hz > pair[0].frequency_hz,
                    "just scale {scale} dips from {} Hz to {} Hz",
                    pair[0].frequency_hz,
                    pair[1].frequency_hz
                );
            }
        }
    }

    #[test]
    fn just_fifth_locks_to_the_ding() {
        let ding = midi_note_to_hz(DEFAULT_HANDPAN_DING);
        let just = pad_hz(ding, 19, 1);
        let equal = pad_hz(ding, 19, 0);
        assert!((just - ding * 3.0).abs() < 0.01);
        let cents = 1200.0 * (just / equal).ln() / core::f32::consts::LN_2;
        assert!(
            (cents - 2.0).abs() < 0.2,
            "just twelfth was {cents} cents from equal"
        );

        let mut handpan = Handpan::new(48_000.0);
        handpan.strike_midi(50, 0.25);
        for _ in 0..500 {
            handpan.next_sample();
        }
        let ringing = handpan.pads[0].positions[0];
        assert!(ringing.abs() > 1.0e-5);
        handpan.set_layout(1, 52, 1);
        assert!((handpan.pads[0].positions[0] - ringing).abs() < 1.0e-6);
        assert_eq!(handpan_notes(1, 50)[2], 60);

        let aegean_n = pad_hz(ding, 16, 1);
        let aegean_m = pad_hz(ding, 18, 1);
        assert!(
            aegean_m > aegean_n,
            "Aegean M {aegean_m} Hz was below N {aegean_n} Hz"
        );
    }
}
