mod analysis;
mod settings;
mod ui;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI8, AtomicU8, AtomicU32, Ordering};
use std::{env, sync::Arc};

use anyhow::Context as _;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use midir::{Ignore, MidiInput};
use parking_lot::Mutex;

use squawk_engine::{
    BASS_LINE_AMOUNT_RANGE, BASS_LINE_VARIATION_RANGE, BELLS_DECAY_RANGE, BELLS_INDEX_DECAY_RANGE,
    BELLS_INDEX_RANGE, BELLS_RATIO_RANGE, BassLine, Bells, DECO_DENSITY_RANGE,
    DECO_LAG_BEATS_RANGE, DEFAULT_BASS_LINE_AMOUNT as ENGINE_DEFAULT_BASS_LINE_AMOUNT,
    DEFAULT_BASS_LINE_VARIATION as ENGINE_DEFAULT_BASS_LINE_VARIATION,
    DEFAULT_BELLS_DECAY as ENGINE_DEFAULT_BELLS_DECAY,
    DEFAULT_BELLS_INDEX as ENGINE_DEFAULT_BELLS_INDEX,
    DEFAULT_BELLS_INDEX_DECAY as ENGINE_DEFAULT_BELLS_INDEX_DECAY, DEFAULT_BELLS_OCTAVE,
    DEFAULT_BELLS_RATIO as ENGINE_DEFAULT_BELLS_RATIO,
    DEFAULT_BREATH_AUTO_LEVEL as ENGINE_DEFAULT_BREATH_AUTO_LEVEL,
    DEFAULT_BREATH_FEEDBACK as ENGINE_DEFAULT_BREATH_FEEDBACK,
    DEFAULT_BREATH_NOISE_CUTOFF_HZ as ENGINE_DEFAULT_BREATH_NOISE_CUTOFF_HZ,
    DEFAULT_BREATH_PRESSURE as ENGINE_DEFAULT_BREATH_PRESSURE,
    DEFAULT_DECO_DENSITY as ENGINE_DEFAULT_DECO_DENSITY,
    DEFAULT_DECO_LAG_BEATS as ENGINE_DEFAULT_DECO_LAG_BEATS,
    DEFAULT_DING_BRIGHTNESS as ENGINE_DEFAULT_DING_BRIGHTNESS,
    DEFAULT_DING_FORMANT_1 as ENGINE_DEFAULT_DING_FORMANT_1,
    DEFAULT_DING_FORMANT_2 as ENGINE_DEFAULT_DING_FORMANT_2,
    DEFAULT_DING_FORMANT_3 as ENGINE_DEFAULT_DING_FORMANT_3,
    DEFAULT_DING_RESONANCE as ENGINE_DEFAULT_DING_RESONANCE,
    DEFAULT_DING_SHAPE as ENGINE_DEFAULT_DING_SHAPE,
    DEFAULT_DING_SPREAD as ENGINE_DEFAULT_DING_SPREAD,
    DEFAULT_DING_STRINGS as ENGINE_DEFAULT_DING_STRINGS,
    DEFAULT_DING_WASH as ENGINE_DEFAULT_DING_WASH,
    DEFAULT_EXCITATION_GAIN as ENGINE_DEFAULT_EXCITATION_GAIN,
    DEFAULT_FM_BASS_DECAY as ENGINE_DEFAULT_FM_BASS_DECAY,
    DEFAULT_FM_BASS_FEEDBACK as ENGINE_DEFAULT_FM_BASS_FEEDBACK,
    DEFAULT_FM_BASS_INDEX as ENGINE_DEFAULT_FM_BASS_INDEX,
    DEFAULT_FM_BASS_INDEX_DECAY as ENGINE_DEFAULT_FM_BASS_INDEX_DECAY,
    DEFAULT_FM_BASS_RATIO as ENGINE_DEFAULT_FM_BASS_RATIO,
    DEFAULT_FORMANT_DEPTH as ENGINE_DEFAULT_FORMANT_DEPTH,
    DEFAULT_FORMANT_RATE as ENGINE_DEFAULT_FORMANT_RATE,
    DEFAULT_GROOVE_AUTO_LOCK as ENGINE_DEFAULT_GROOVE_AUTO_LOCK,
    DEFAULT_GROOVE_CLICK_LEVEL as ENGINE_DEFAULT_GROOVE_CLICK_LEVEL,
    DEFAULT_GROOVE_PHASE_PULL as ENGINE_DEFAULT_GROOVE_PHASE_PULL,
    DEFAULT_GROOVE_SENSITIVITY as ENGINE_DEFAULT_GROOVE_SENSITIVITY,
    DEFAULT_HANDPAN_COUPLING as ENGINE_DEFAULT_HANDPAN_COUPLING,
    DEFAULT_HANDPAN_DECAY as ENGINE_DEFAULT_HANDPAN_DECAY,
    DEFAULT_HANDPAN_DETUNE_CENTS as ENGINE_DEFAULT_HANDPAN_DETUNE_CENTS,
    DEFAULT_HANDPAN_DING as ENGINE_DEFAULT_HANDPAN_DING,
    DEFAULT_HANDPAN_FIFTH as ENGINE_DEFAULT_HANDPAN_FIFTH,
    DEFAULT_HANDPAN_FUNDAMENTAL as ENGINE_DEFAULT_HANDPAN_FUNDAMENTAL,
    DEFAULT_HANDPAN_GLIDE as ENGINE_DEFAULT_HANDPAN_GLIDE,
    DEFAULT_HANDPAN_HIGHER_DECAY as ENGINE_DEFAULT_HANDPAN_HIGHER_DECAY,
    DEFAULT_HANDPAN_HIT as ENGINE_DEFAULT_HANDPAN_HIT,
    DEFAULT_HANDPAN_OCTAVE as ENGINE_DEFAULT_HANDPAN_OCTAVE,
    DEFAULT_HANDPAN_SCALE as ENGINE_DEFAULT_HANDPAN_SCALE,
    DEFAULT_HANDPAN_TUNING as ENGINE_DEFAULT_HANDPAN_TUNING,
    DEFAULT_INHARMONICITY as ENGINE_DEFAULT_INHARMONICITY,
    DEFAULT_MIX_LEVEL as ENGINE_DEFAULT_MIX_LEVEL,
    DEFAULT_OUTPUT_GAIN as ENGINE_DEFAULT_OUTPUT_GAIN, DEFAULT_PAN as ENGINE_DEFAULT_PAN,
    DEFAULT_POSITION_COUPLING as ENGINE_DEFAULT_POSITION_COUPLING,
    DEFAULT_REVERB_BASS as ENGINE_DEFAULT_REVERB_BASS,
    DEFAULT_REVERB_DECAY as ENGINE_DEFAULT_REVERB_DECAY,
    DEFAULT_REVERB_DING as ENGINE_DEFAULT_REVERB_DING,
    DEFAULT_REVERB_ECHO as ENGINE_DEFAULT_REVERB_ECHO,
    DEFAULT_REVERB_HANDPAN as ENGINE_DEFAULT_REVERB_HANDPAN,
    DEFAULT_REVERB_RECENT as ENGINE_DEFAULT_REVERB_RECENT,
    DEFAULT_REVERB_SECOND as ENGINE_DEFAULT_REVERB_SECOND,
    DEFAULT_REVERB_SUPPORT as ENGINE_DEFAULT_REVERB_SUPPORT,
    DEFAULT_REVERB_TONE as ENGINE_DEFAULT_REVERB_TONE,
    DEFAULT_SUPPORT_DRIFT as ENGINE_DEFAULT_SUPPORT_DRIFT,
    DEFAULT_SUPPORT_FADE as ENGINE_DEFAULT_SUPPORT_FADE,
    DEFAULT_SUPPORT_LEVEL as ENGINE_DEFAULT_SUPPORT_LEVEL,
    DEFAULT_SUPPORT_MOTION as ENGINE_DEFAULT_SUPPORT_MOTION,
    DEFAULT_SUPPORT_PULSE as ENGINE_DEFAULT_SUPPORT_PULSE,
    DEFAULT_SUPPORT_WANDER as ENGINE_DEFAULT_SUPPORT_WANDER,
    DEFAULT_SUPPORT_WIDTH as ENGINE_DEFAULT_SUPPORT_WIDTH,
    DEFAULT_TAMBURA_JAWARI as ENGINE_DEFAULT_TAMBURA_JAWARI,
    DEFAULT_VELOCITY_COUPLING as ENGINE_DEFAULT_VELOCITY_COUPLING, DING_BRIGHTNESS_RANGE,
    DING_FORMANT_RANGE, DING_RESONANCE_RANGE, DING_SHAPE_RANGE, DING_SPREAD_RANGE,
    DING_STRINGS_RANGE, DRONE_LEVEL_RANGE, Decorations, DroneVoice, DroneVoiceKind, Engine,
    EngineParams, ExciterMode, FM_BASS_DECAY_RANGE, FM_BASS_FEEDBACK_RANGE,
    FM_BASS_INDEX_DECAY_RANGE, FM_BASS_INDEX_RANGE, FM_BASS_RATIO_RANGE, FORMANT_DEPTH_RANGE,
    FORMANT_RATE_RANGE, FmBass, FmBassParams, GROOVE_CLICK_LEVEL_RANGE, GROOVE_PHASE_PULL_RANGE,
    GROOVE_SENSITIVITY_RANGE, GrooveStatus, GrooveTracker, HANDPAN_COUPLING_RANGE,
    HANDPAN_DECAY_RANGE, HANDPAN_DETUNE_CENTS_RANGE, HANDPAN_DING_MAX as ENGINE_HANDPAN_DING_MAX,
    HANDPAN_DING_MIN as ENGINE_HANDPAN_DING_MIN, HANDPAN_GLIDE_RANGE, HANDPAN_HIT_RANGE,
    HANDPAN_PARTIAL_RANGE, HANDPAN_SCALE_COUNT as ENGINE_HANDPAN_SCALE_COUNT, Handpan,
    MIX_LEVEL_RANGE, PAN_RANGE, PlayMemory, REVERB_BASS_RANGE, REVERB_DECAY_RANGE,
    REVERB_DING_RANGE, REVERB_ECHO_RANGE, REVERB_HANDPAN_RANGE, REVERB_RECENT_RANGE,
    REVERB_SECOND_RANGE, REVERB_SUPPORT_RANGE, REVERB_TONE_RANGE, Reverb, SUPPORT_DRIFT_RANGE,
    SUPPORT_FADE_RANGE, SUPPORT_LEVEL_RANGE, SUPPORT_MOTION_RANGE, SUPPORT_PULSE_RANGE,
    SUPPORT_WANDER_RANGE, SUPPORT_WIDTH_RANGE, StrikeVoice, Support, TAMBURA_JAWARI_RANGE,
    TamburaBank, handpan_interval_hz, stereo_pan,
};

use settings::SettingsStore;

use analysis::AudioTap;

pub(crate) const DEFAULT_ATTACK_MS: f32 = 2.0;
pub(crate) const DEFAULT_RELEASE_MS: f32 = 400.0;
pub(crate) const DEFAULT_DAMPING_RATIO: f32 = 0.002;
pub(crate) const DEFAULT_INHARMONICITY: f32 = ENGINE_DEFAULT_INHARMONICITY;
pub(crate) const DEFAULT_POSITION_COUPLING: f32 = ENGINE_DEFAULT_POSITION_COUPLING;
pub(crate) const DEFAULT_VELOCITY_COUPLING: f32 = ENGINE_DEFAULT_VELOCITY_COUPLING;
pub(crate) const DEFAULT_EXCITATION_GAIN: f32 = ENGINE_DEFAULT_EXCITATION_GAIN;
pub(crate) const DEFAULT_OUTPUT_GAIN: f32 = ENGINE_DEFAULT_OUTPUT_GAIN;
pub(crate) const DEFAULT_BREATH_PRESSURE: f32 = ENGINE_DEFAULT_BREATH_PRESSURE;
pub(crate) const DEFAULT_BREATH_FEEDBACK: f32 = ENGINE_DEFAULT_BREATH_FEEDBACK;
pub(crate) const DEFAULT_BREATH_AUTO_LEVEL: f32 = ENGINE_DEFAULT_BREATH_AUTO_LEVEL;
pub(crate) const DEFAULT_BREATH_NOISE_CUTOFF_HZ: f32 = ENGINE_DEFAULT_BREATH_NOISE_CUTOFF_HZ;
const ATTACK_RANGE_MS: (f32, f32) = (0.5, 200.0);
const RELEASE_RANGE_MS: (f32, f32) = (20.0, 2000.0);
const DAMPING_RATIO_RANGE: (f32, f32) = (0.001, 0.2);
const INHARMONICITY_RANGE: (f32, f32) = (0.8, 1.4);
const POSITION_COUPLING_RANGE: (f32, f32) = (-1.0, 1.0);
const VELOCITY_COUPLING_RANGE: (f32, f32) = (-1.0, 1.0);
const EXCITATION_GAIN_RANGE: (f32, f32) = (0.1, 20.0);
const OUTPUT_GAIN_RANGE: (f32, f32) = (0.05, 4.0);
const BREATH_PRESSURE_RANGE: (f32, f32) = (0.0, 4.0);
const BREATH_FEEDBACK_RANGE: (f32, f32) = (0.0, 1.5);
const BREATH_AUTO_LEVEL_RANGE: (f32, f32) = (0.0, 20.0);
const BREATH_NOISE_CUTOFF_HZ_RANGE: (f32, f32) = (50.0, 8000.0);
pub(crate) const DEFAULT_HANDPAN_DECAY: f32 = ENGINE_DEFAULT_HANDPAN_DECAY;
pub(crate) const DEFAULT_HANDPAN_DETUNE_CENTS: f32 = ENGINE_DEFAULT_HANDPAN_DETUNE_CENTS;
pub(crate) const DEFAULT_HANDPAN_COUPLING: f32 = ENGINE_DEFAULT_HANDPAN_COUPLING;
pub(crate) const DEFAULT_HANDPAN_HIT: f32 = ENGINE_DEFAULT_HANDPAN_HIT;
pub(crate) const DEFAULT_HANDPAN_GLIDE: f32 = ENGINE_DEFAULT_HANDPAN_GLIDE;
pub(crate) const DEFAULT_HANDPAN_SCALE: u8 = ENGINE_DEFAULT_HANDPAN_SCALE;
pub(crate) const DEFAULT_HANDPAN_DING: u8 = ENGINE_DEFAULT_HANDPAN_DING;
pub(crate) const DEFAULT_HANDPAN_TUNING: u8 = ENGINE_DEFAULT_HANDPAN_TUNING;
pub(crate) const HANDPAN_SCALE_COUNT: u8 = ENGINE_HANDPAN_SCALE_COUNT;
pub(crate) const HANDPAN_DING_MIN: u8 = ENGINE_HANDPAN_DING_MIN;
pub(crate) const HANDPAN_DING_MAX: u8 = ENGINE_HANDPAN_DING_MAX;
pub(crate) const DEFAULT_HANDPAN_FUNDAMENTAL: f32 = ENGINE_DEFAULT_HANDPAN_FUNDAMENTAL;
pub(crate) const DEFAULT_HANDPAN_OCTAVE: f32 = ENGINE_DEFAULT_HANDPAN_OCTAVE;
pub(crate) const DEFAULT_HANDPAN_FIFTH: f32 = ENGINE_DEFAULT_HANDPAN_FIFTH;
pub(crate) const DEFAULT_HANDPAN_HIGHER_DECAY: f32 = ENGINE_DEFAULT_HANDPAN_HIGHER_DECAY;
pub(crate) const DEFAULT_SUPPORT_LEVEL: f32 = ENGINE_DEFAULT_SUPPORT_LEVEL;
pub(crate) const DEFAULT_SUPPORT_PULSE: f32 = ENGINE_DEFAULT_SUPPORT_PULSE;
pub(crate) const DEFAULT_SUPPORT_WIDTH: f32 = ENGINE_DEFAULT_SUPPORT_WIDTH;
pub(crate) const DEFAULT_SUPPORT_WANDER: f32 = ENGINE_DEFAULT_SUPPORT_WANDER;
pub(crate) const DEFAULT_SUPPORT_DRIFT: f32 = ENGINE_DEFAULT_SUPPORT_DRIFT;
pub(crate) const DEFAULT_SUPPORT_MOTION: f32 = ENGINE_DEFAULT_SUPPORT_MOTION;
pub(crate) const DEFAULT_SUPPORT_FADE: f32 = ENGINE_DEFAULT_SUPPORT_FADE;
pub(crate) const DEFAULT_REVERB_HANDPAN: f32 = ENGINE_DEFAULT_REVERB_HANDPAN;
pub(crate) const DEFAULT_REVERB_SUPPORT: f32 = ENGINE_DEFAULT_REVERB_SUPPORT;
pub(crate) const DEFAULT_REVERB_DING: f32 = ENGINE_DEFAULT_REVERB_DING;
pub(crate) const DEFAULT_REVERB_RECENT: f32 = ENGINE_DEFAULT_REVERB_RECENT;
pub(crate) const DEFAULT_REVERB_SECOND: f32 = ENGINE_DEFAULT_REVERB_SECOND;
pub(crate) const DEFAULT_REVERB_BASS: f32 = ENGINE_DEFAULT_REVERB_BASS;
pub(crate) const DEFAULT_REVERB_ECHO: f32 = ENGINE_DEFAULT_REVERB_ECHO;
pub(crate) const DEFAULT_REVERB_DECAY: f32 = ENGINE_DEFAULT_REVERB_DECAY;
pub(crate) const DEFAULT_REVERB_TONE: f32 = ENGINE_DEFAULT_REVERB_TONE;
pub(crate) const DEFAULT_DING_WASH: u8 = ENGINE_DEFAULT_DING_WASH;
pub(crate) const DEFAULT_DING_SHAPE: f32 = ENGINE_DEFAULT_DING_SHAPE;
pub(crate) const DEFAULT_DING_SPREAD: f32 = ENGINE_DEFAULT_DING_SPREAD;
pub(crate) const DEFAULT_DING_BRIGHTNESS: f32 = ENGINE_DEFAULT_DING_BRIGHTNESS;
pub(crate) const DEFAULT_DING_STRINGS: f32 = ENGINE_DEFAULT_DING_STRINGS;
pub(crate) const DEFAULT_TAMBURA_JAWARI: f32 = ENGINE_DEFAULT_TAMBURA_JAWARI;
pub(crate) const DEFAULT_DING_FORMANT_1: f32 = ENGINE_DEFAULT_DING_FORMANT_1;
pub(crate) const DEFAULT_DING_FORMANT_2: f32 = ENGINE_DEFAULT_DING_FORMANT_2;
pub(crate) const DEFAULT_DING_FORMANT_3: f32 = ENGINE_DEFAULT_DING_FORMANT_3;
pub(crate) const DEFAULT_DING_RESONANCE: f32 = ENGINE_DEFAULT_DING_RESONANCE;
pub(crate) const DEFAULT_MIX_LEVEL: f32 = ENGINE_DEFAULT_MIX_LEVEL;
pub(crate) const DEFAULT_DECO_DENSITY: f32 = ENGINE_DEFAULT_DECO_DENSITY;
pub(crate) const DEFAULT_DECO_LAG_BEATS: f32 = ENGINE_DEFAULT_DECO_LAG_BEATS;
pub(crate) const DEFAULT_FM_BASS_DECAY: f32 = ENGINE_DEFAULT_FM_BASS_DECAY;
pub(crate) const DEFAULT_FM_BASS_RATIO: f32 = ENGINE_DEFAULT_FM_BASS_RATIO;
pub(crate) const DEFAULT_FM_BASS_INDEX: f32 = ENGINE_DEFAULT_FM_BASS_INDEX;
pub(crate) const DEFAULT_FM_BASS_INDEX_DECAY: f32 = ENGINE_DEFAULT_FM_BASS_INDEX_DECAY;
pub(crate) const DEFAULT_FM_BASS_FEEDBACK: f32 = ENGINE_DEFAULT_FM_BASS_FEEDBACK;
pub(crate) const DEFAULT_FM_BASS_LEVEL: f32 = 1.0;
pub(crate) const FM_BASS_LEVEL_RANGE: (f32, f32) = (0.0, 2.0);
pub(crate) const DEFAULT_BASS_LINE_AMOUNT: f32 = ENGINE_DEFAULT_BASS_LINE_AMOUNT;
pub(crate) const DEFAULT_BASS_LINE_VARIATION: f32 = ENGINE_DEFAULT_BASS_LINE_VARIATION;
pub(crate) const DEFAULT_BELLS_DECAY: f32 = ENGINE_DEFAULT_BELLS_DECAY;
pub(crate) const DEFAULT_BELLS_RATIO: f32 = ENGINE_DEFAULT_BELLS_RATIO;
pub(crate) const DEFAULT_BELLS_INDEX: f32 = ENGINE_DEFAULT_BELLS_INDEX;
pub(crate) const DEFAULT_BELLS_INDEX_DECAY: f32 = ENGINE_DEFAULT_BELLS_INDEX_DECAY;
pub(crate) const DEFAULT_PAN: f32 = ENGINE_DEFAULT_PAN;
pub(crate) const DEFAULT_FORMANT_RATE: f32 = ENGINE_DEFAULT_FORMANT_RATE;
pub(crate) const DEFAULT_FORMANT_DEPTH: f32 = ENGINE_DEFAULT_FORMANT_DEPTH;
pub(crate) const DEFAULT_GROOVE_SENSITIVITY: f32 = ENGINE_DEFAULT_GROOVE_SENSITIVITY;
pub(crate) const DEFAULT_GROOVE_PHASE_PULL: f32 = ENGINE_DEFAULT_GROOVE_PHASE_PULL;
pub(crate) const DEFAULT_GROOVE_CLICK_LEVEL: f32 = ENGINE_DEFAULT_GROOVE_CLICK_LEVEL;
pub(crate) const DEFAULT_GROOVE_CLICK_ENABLED: bool = true;
pub(crate) const DEFAULT_GROOVE_AUTO_LOCK: bool = ENGINE_DEFAULT_GROOVE_AUTO_LOCK;
const GROOVE_LOCK_CMD_NONE: u8 = 0;
const GROOVE_LOCK_CMD_LOCK: u8 = 1;
const GROOVE_LOCK_CMD_UNLOCK: u8 = 2;
pub(crate) const GROOVE_IOI_CAP: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Instrument {
    Modal = 0,
    Handpan = 1,
    Bass = 2,
    Bells = 3,
    Resonator = 4,
    Waveshape = 5,
    Tambura = 6,
}

impl Instrument {
    pub(crate) fn as_u8(self) -> u8 {
        self as u8
    }

    pub(crate) fn from_u8(value: u8) -> Self {
        match value {
            1 => Instrument::Handpan,
            2 => Instrument::Bass,
            3 => Instrument::Bells,
            4 => Instrument::Resonator,
            5 => Instrument::Waveshape,
            6 => Instrument::Tambura,
            _ => Instrument::Modal,
        }
    }

    /// Handpan-style pad keyboard / note layout.
    pub(crate) fn uses_handpan_keys(self) -> bool {
        matches!(
            self,
            Instrument::Handpan
                | Instrument::Bass
                | Instrument::Bells
                | Instrument::Resonator
                | Instrument::Waveshape
                | Instrument::Tambura
        )
    }

    /// Maps Resonator / Waveshape / Tambura to Support's ding_wash index.
    pub(crate) fn drone_wash(self) -> Option<u8> {
        match self {
            Instrument::Resonator => Some(0),
            Instrument::Waveshape => Some(1),
            Instrument::Tambura => Some(2),
            _ => None,
        }
    }
}

/// Strike voice used for delayed Echo hits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EchoInstrument {
    Handpan = 0,
    Bells = 1,
}

impl EchoInstrument {
    pub(crate) fn as_u8(self) -> u8 {
        self as u8
    }

    pub(crate) fn from_u8(value: u8) -> Self {
        match value {
            0 => EchoInstrument::Handpan,
            _ => EchoInstrument::Bells,
        }
    }
}

pub(crate) const DEFAULT_ECHO_INSTRUMENT: u8 = EchoInstrument::Bells as u8;
const STRIKE_QUEUE_LEN: usize = 16;
const PEAK_HALF_LIFE_S: f32 = 0.5;
const PEAK_DECAY_TAU: f32 = PEAK_HALF_LIFE_S / core::f32::consts::LN_2;

#[derive(Debug)]
struct ControlBlock {
    gate: AtomicBool,
    frequency_bits: AtomicU32,
    amplitude_bits: AtomicU32,
    active_note: AtomicU8,
    last_note_on_note: AtomicU8,
    last_note_on_hz_bits: AtomicU32,
    last_note_on_counter: AtomicU32,
    trigger_counter: AtomicU32,
    exciter_mode: AtomicU8,
    attack_ms_bits: AtomicU32,
    release_ms_bits: AtomicU32,
    damping_ratio_bits: AtomicU32,
    peak_bits: AtomicU32,
    inharmonicity_bits: AtomicU32,
    position_coupling_bits: AtomicU32,
    velocity_coupling_bits: AtomicU32,
    excitation_gain_bits: AtomicU32,
    output_gain_bits: AtomicU32,
    breath_pressure_bits: AtomicU32,
    breath_noise_cutoff_hz_bits: AtomicU32,
    breath_feedback_bits: AtomicU32,
    breath_auto_level_bits: AtomicU32,
    instrument: AtomicU8,
    handpan_scale: AtomicU8,
    handpan_ding: AtomicU8,
    handpan_tuning: AtomicU8,
    handpan_decay_bits: AtomicU32,
    handpan_detune_cents_bits: AtomicU32,
    handpan_coupling_bits: AtomicU32,
    handpan_hit_bits: AtomicU32,
    handpan_glide_bits: AtomicU32,
    handpan_fundamental_bits: AtomicU32,
    handpan_octave_bits: AtomicU32,
    handpan_fifth_bits: AtomicU32,
    handpan_higher_decay_bits: AtomicU32,
    support_level_bits: AtomicU32,
    support_pulse_bits: AtomicU32,
    support_width_bits: AtomicU32,
    support_wander_bits: AtomicU32,
    support_drift_bits: AtomicU32,
    support_motion_bits: AtomicU32,
    support_fade_bits: AtomicU32,
    reverb_handpan_bits: AtomicU32,
    reverb_support_bits: AtomicU32,
    reverb_ding_bits: AtomicU32,
    reverb_recent_bits: AtomicU32,
    reverb_second_bits: AtomicU32,
    reverb_bass_bits: AtomicU32,
    reverb_echo_bits: AtomicU32,
    reverb_decay_bits: AtomicU32,
    reverb_tone_bits: AtomicU32,
    echo_instrument: AtomicU8,
    support_muted: AtomicU8,
    support_solo: AtomicU8,
    handpan_muted: AtomicBool,
    handpan_solo: AtomicBool,
    ding_wash: AtomicU8,
    ding_shape_bits: AtomicU32,
    ding_spread_bits: AtomicU32,
    ding_brightness_bits: AtomicU32,
    ding_strings_bits: AtomicU32,
    tambura_jawari_bits: AtomicU32,
    ding_formant_1_bits: AtomicU32,
    ding_formant_2_bits: AtomicU32,
    ding_formant_3_bits: AtomicU32,
    ding_resonance_bits: AtomicU32,
    ding_q_1_bits: AtomicU32,
    ding_q_2_bits: AtomicU32,
    ding_q_3_bits: AtomicU32,
    mix_handpan_bits: AtomicU32,
    mix_ding_bits: AtomicU32,
    mix_recent_bits: AtomicU32,
    mix_second_bits: AtomicU32,
    mix_bass_bits: AtomicU32,
    mix_taps_bits: AtomicU32,
    deco_density_bits: AtomicU32,
    deco_lag_beats_bits: AtomicU32,
    pan_handpan_bits: AtomicU32,
    pan_ding_bits: AtomicU32,
    pan_recent_bits: AtomicU32,
    pan_second_bits: AtomicU32,
    pan_bass_bits: AtomicU32,
    pan_echo_bits: AtomicU32,
    formant_rate_bits: AtomicU32,
    formant_depth_bits: AtomicU32,
    groove_sensitivity_bits: AtomicU32,
    groove_phase_pull_bits: AtomicU32,
    groove_click_level_bits: AtomicU32,
    fm_bass_decay_bits: AtomicU32,
    fm_bass_ratio_bits: AtomicU32,
    fm_bass_index_bits: AtomicU32,
    fm_bass_index_decay_bits: AtomicU32,
    fm_bass_feedback_bits: AtomicU32,
    fm_bass_level_bits: AtomicU32,
    bass_line_amount_bits: AtomicU32,
    bass_line_variation_bits: AtomicU32,
    bells_decay_bits: AtomicU32,
    bells_ratio_bits: AtomicU32,
    bells_index_bits: AtomicU32,
    bells_index_decay_bits: AtomicU32,
    groove_click_enabled: AtomicBool,
    groove_auto_lock: AtomicBool,
    groove_lock_cmd: AtomicU8,
    groove_bpm_bits: AtomicU32,
    groove_confidence_bits: AtomicU32,
    groove_phase_bits: AtomicU32,
    groove_status: AtomicU8,
    groove_locked: AtomicBool,
    groove_ioi_ms: [AtomicU32; GROOVE_IOI_CAP],
    groove_ioi_count: AtomicU8,
    strike_notes: [AtomicU8; STRIKE_QUEUE_LEN],
    strike_amps: [AtomicU32; STRIKE_QUEUE_LEN],
    strike_places: [AtomicU32; STRIKE_QUEUE_LEN],
}

impl ControlBlock {
    fn new(initial: EngineParams) -> Self {
        Self {
            gate: AtomicBool::new(false),
            frequency_bits: AtomicU32::new(initial.frequency_hz.to_bits()),
            amplitude_bits: AtomicU32::new(initial.amplitude.to_bits()),
            active_note: AtomicU8::new(0),
            last_note_on_note: AtomicU8::new(0),
            last_note_on_hz_bits: AtomicU32::new(initial.frequency_hz.to_bits()),
            last_note_on_counter: AtomicU32::new(0),
            trigger_counter: AtomicU32::new(0),
            exciter_mode: AtomicU8::new(ExciterMode::Strike.as_u8()),
            attack_ms_bits: AtomicU32::new(DEFAULT_ATTACK_MS.to_bits()),
            release_ms_bits: AtomicU32::new(DEFAULT_RELEASE_MS.to_bits()),
            damping_ratio_bits: AtomicU32::new(DEFAULT_DAMPING_RATIO.to_bits()),
            peak_bits: AtomicU32::new(0.0f32.to_bits()),
            inharmonicity_bits: AtomicU32::new(DEFAULT_INHARMONICITY.to_bits()),
            position_coupling_bits: AtomicU32::new(DEFAULT_POSITION_COUPLING.to_bits()),
            velocity_coupling_bits: AtomicU32::new(DEFAULT_VELOCITY_COUPLING.to_bits()),
            excitation_gain_bits: AtomicU32::new(DEFAULT_EXCITATION_GAIN.to_bits()),
            output_gain_bits: AtomicU32::new(DEFAULT_OUTPUT_GAIN.to_bits()),
            breath_pressure_bits: AtomicU32::new(DEFAULT_BREATH_PRESSURE.to_bits()),
            breath_noise_cutoff_hz_bits: AtomicU32::new(DEFAULT_BREATH_NOISE_CUTOFF_HZ.to_bits()),
            breath_feedback_bits: AtomicU32::new(DEFAULT_BREATH_FEEDBACK.to_bits()),
            breath_auto_level_bits: AtomicU32::new(DEFAULT_BREATH_AUTO_LEVEL.to_bits()),
            instrument: AtomicU8::new(Instrument::Modal.as_u8()),
            handpan_scale: AtomicU8::new(DEFAULT_HANDPAN_SCALE),
            handpan_ding: AtomicU8::new(DEFAULT_HANDPAN_DING),
            handpan_tuning: AtomicU8::new(DEFAULT_HANDPAN_TUNING),
            handpan_decay_bits: AtomicU32::new(DEFAULT_HANDPAN_DECAY.to_bits()),
            handpan_detune_cents_bits: AtomicU32::new(DEFAULT_HANDPAN_DETUNE_CENTS.to_bits()),
            handpan_coupling_bits: AtomicU32::new(DEFAULT_HANDPAN_COUPLING.to_bits()),
            handpan_hit_bits: AtomicU32::new(DEFAULT_HANDPAN_HIT.to_bits()),
            handpan_glide_bits: AtomicU32::new(DEFAULT_HANDPAN_GLIDE.to_bits()),
            handpan_fundamental_bits: AtomicU32::new(DEFAULT_HANDPAN_FUNDAMENTAL.to_bits()),
            handpan_octave_bits: AtomicU32::new(DEFAULT_HANDPAN_OCTAVE.to_bits()),
            handpan_fifth_bits: AtomicU32::new(DEFAULT_HANDPAN_FIFTH.to_bits()),
            handpan_higher_decay_bits: AtomicU32::new(DEFAULT_HANDPAN_HIGHER_DECAY.to_bits()),
            support_level_bits: AtomicU32::new(DEFAULT_SUPPORT_LEVEL.to_bits()),
            support_pulse_bits: AtomicU32::new(DEFAULT_SUPPORT_PULSE.to_bits()),
            support_width_bits: AtomicU32::new(DEFAULT_SUPPORT_WIDTH.to_bits()),
            support_wander_bits: AtomicU32::new(DEFAULT_SUPPORT_WANDER.to_bits()),
            support_drift_bits: AtomicU32::new(DEFAULT_SUPPORT_DRIFT.to_bits()),
            support_motion_bits: AtomicU32::new(DEFAULT_SUPPORT_MOTION.to_bits()),
            support_fade_bits: AtomicU32::new(DEFAULT_SUPPORT_FADE.to_bits()),
            reverb_handpan_bits: AtomicU32::new(DEFAULT_REVERB_HANDPAN.to_bits()),
            reverb_support_bits: AtomicU32::new(DEFAULT_REVERB_SUPPORT.to_bits()),
            reverb_ding_bits: AtomicU32::new(DEFAULT_REVERB_DING.to_bits()),
            reverb_recent_bits: AtomicU32::new(DEFAULT_REVERB_RECENT.to_bits()),
            reverb_second_bits: AtomicU32::new(DEFAULT_REVERB_SECOND.to_bits()),
            reverb_bass_bits: AtomicU32::new(DEFAULT_REVERB_BASS.to_bits()),
            reverb_echo_bits: AtomicU32::new(DEFAULT_REVERB_ECHO.to_bits()),
            reverb_decay_bits: AtomicU32::new(DEFAULT_REVERB_DECAY.to_bits()),
            reverb_tone_bits: AtomicU32::new(DEFAULT_REVERB_TONE.to_bits()),
            echo_instrument: AtomicU8::new(DEFAULT_ECHO_INSTRUMENT),
            support_muted: AtomicU8::new(0),
            support_solo: AtomicU8::new(0),
            handpan_muted: AtomicBool::new(false),
            handpan_solo: AtomicBool::new(false),
            ding_wash: AtomicU8::new(DEFAULT_DING_WASH),
            ding_shape_bits: AtomicU32::new(DEFAULT_DING_SHAPE.to_bits()),
            ding_spread_bits: AtomicU32::new(DEFAULT_DING_SPREAD.to_bits()),
            ding_brightness_bits: AtomicU32::new(DEFAULT_DING_BRIGHTNESS.to_bits()),
            ding_strings_bits: AtomicU32::new(DEFAULT_DING_STRINGS.to_bits()),
            tambura_jawari_bits: AtomicU32::new(DEFAULT_TAMBURA_JAWARI.to_bits()),
            ding_formant_1_bits: AtomicU32::new(DEFAULT_DING_FORMANT_1.to_bits()),
            ding_formant_2_bits: AtomicU32::new(DEFAULT_DING_FORMANT_2.to_bits()),
            ding_formant_3_bits: AtomicU32::new(DEFAULT_DING_FORMANT_3.to_bits()),
            ding_resonance_bits: AtomicU32::new(DEFAULT_DING_RESONANCE.to_bits()),
            ding_q_1_bits: AtomicU32::new(DEFAULT_DING_RESONANCE.to_bits()),
            ding_q_2_bits: AtomicU32::new(DEFAULT_DING_RESONANCE.to_bits()),
            ding_q_3_bits: AtomicU32::new(DEFAULT_DING_RESONANCE.to_bits()),
            mix_handpan_bits: AtomicU32::new(DEFAULT_MIX_LEVEL.to_bits()),
            mix_ding_bits: AtomicU32::new(DEFAULT_MIX_LEVEL.to_bits()),
            mix_recent_bits: AtomicU32::new(DEFAULT_MIX_LEVEL.to_bits()),
            mix_second_bits: AtomicU32::new(DEFAULT_MIX_LEVEL.to_bits()),
            mix_bass_bits: AtomicU32::new(DEFAULT_MIX_LEVEL.to_bits()),
            mix_taps_bits: AtomicU32::new(DEFAULT_MIX_LEVEL.to_bits()),
            deco_density_bits: AtomicU32::new(DEFAULT_DECO_DENSITY.to_bits()),
            deco_lag_beats_bits: AtomicU32::new(DEFAULT_DECO_LAG_BEATS.to_bits()),
            pan_handpan_bits: AtomicU32::new(DEFAULT_PAN.to_bits()),
            pan_ding_bits: AtomicU32::new(DEFAULT_PAN.to_bits()),
            pan_recent_bits: AtomicU32::new(DEFAULT_PAN.to_bits()),
            pan_second_bits: AtomicU32::new(DEFAULT_PAN.to_bits()),
            pan_bass_bits: AtomicU32::new(DEFAULT_PAN.to_bits()),
            pan_echo_bits: AtomicU32::new(DEFAULT_PAN.to_bits()),
            formant_rate_bits: AtomicU32::new(DEFAULT_FORMANT_RATE.to_bits()),
            formant_depth_bits: AtomicU32::new(DEFAULT_FORMANT_DEPTH.to_bits()),
            groove_sensitivity_bits: AtomicU32::new(DEFAULT_GROOVE_SENSITIVITY.to_bits()),
            groove_phase_pull_bits: AtomicU32::new(DEFAULT_GROOVE_PHASE_PULL.to_bits()),
            groove_click_level_bits: AtomicU32::new(DEFAULT_GROOVE_CLICK_LEVEL.to_bits()),
            fm_bass_decay_bits: AtomicU32::new(DEFAULT_FM_BASS_DECAY.to_bits()),
            fm_bass_ratio_bits: AtomicU32::new(DEFAULT_FM_BASS_RATIO.to_bits()),
            fm_bass_index_bits: AtomicU32::new(DEFAULT_FM_BASS_INDEX.to_bits()),
            fm_bass_index_decay_bits: AtomicU32::new(DEFAULT_FM_BASS_INDEX_DECAY.to_bits()),
            fm_bass_feedback_bits: AtomicU32::new(DEFAULT_FM_BASS_FEEDBACK.to_bits()),
            fm_bass_level_bits: AtomicU32::new(DEFAULT_FM_BASS_LEVEL.to_bits()),
            bass_line_amount_bits: AtomicU32::new(DEFAULT_BASS_LINE_AMOUNT.to_bits()),
            bass_line_variation_bits: AtomicU32::new(DEFAULT_BASS_LINE_VARIATION.to_bits()),
            bells_decay_bits: AtomicU32::new(DEFAULT_BELLS_DECAY.to_bits()),
            bells_ratio_bits: AtomicU32::new(DEFAULT_BELLS_RATIO.to_bits()),
            bells_index_bits: AtomicU32::new(DEFAULT_BELLS_INDEX.to_bits()),
            bells_index_decay_bits: AtomicU32::new(DEFAULT_BELLS_INDEX_DECAY.to_bits()),
            groove_click_enabled: AtomicBool::new(DEFAULT_GROOVE_CLICK_ENABLED),
            groove_auto_lock: AtomicBool::new(DEFAULT_GROOVE_AUTO_LOCK),
            groove_lock_cmd: AtomicU8::new(GROOVE_LOCK_CMD_NONE),
            groove_bpm_bits: AtomicU32::new(75.0f32.to_bits()),
            groove_confidence_bits: AtomicU32::new(0.0f32.to_bits()),
            groove_phase_bits: AtomicU32::new(0.0f32.to_bits()),
            groove_status: AtomicU8::new(0),
            groove_locked: AtomicBool::new(false),
            groove_ioi_ms: std::array::from_fn(|_| AtomicU32::new(0)),
            groove_ioi_count: AtomicU8::new(0),
            strike_notes: std::array::from_fn(|_| AtomicU8::new(0)),
            strike_amps: std::array::from_fn(|_| AtomicU32::new(0)),
            strike_places: std::array::from_fn(|_| AtomicU32::new(0)),
        }
    }

    /// Notes struck since `seen`, oldest first. Each entry is `(note, amplitude, place)`.
    fn drain_strikes(&self, seen: u32) -> ([(u8, f32, f32); STRIKE_QUEUE_LEN], u32, usize) {
        let write = self.trigger_counter.load(Ordering::Acquire);
        let mut out = [(0u8, 0.0f32, 0.0f32); STRIKE_QUEUE_LEN];
        let mut count = 0usize;
        let mut cursor = seen;
        while cursor != write && count < STRIKE_QUEUE_LEN {
            let slot = (cursor as usize) % STRIKE_QUEUE_LEN;
            let note = self.strike_notes[slot].load(Ordering::Relaxed);
            let amp = f32::from_bits(self.strike_amps[slot].load(Ordering::Relaxed));
            let place = f32::from_bits(self.strike_places[slot].load(Ordering::Relaxed));
            out[count] = (note, amp, place);
            count += 1;
            cursor = cursor.wrapping_add(1);
        }
        (out, write, count)
    }

    #[inline]
    fn snapshot(&self) -> ControlSnapshot {
        ControlSnapshot {
            gate: self.gate.load(Ordering::Relaxed),
            frequency_hz: f32::from_bits(self.frequency_bits.load(Ordering::Relaxed)),
            amplitude: f32::from_bits(self.amplitude_bits.load(Ordering::Relaxed)),
            exciter_mode: self.exciter_mode.load(Ordering::Relaxed),
            attack_ms: f32::from_bits(self.attack_ms_bits.load(Ordering::Relaxed)),
            release_ms: f32::from_bits(self.release_ms_bits.load(Ordering::Relaxed)),
            damping_ratio: f32::from_bits(self.damping_ratio_bits.load(Ordering::Relaxed)),
            inharmonicity: f32::from_bits(self.inharmonicity_bits.load(Ordering::Relaxed)),
            position_coupling: f32::from_bits(self.position_coupling_bits.load(Ordering::Relaxed)),
            velocity_coupling: f32::from_bits(self.velocity_coupling_bits.load(Ordering::Relaxed)),
            excitation_gain: f32::from_bits(self.excitation_gain_bits.load(Ordering::Relaxed)),
            output_gain: f32::from_bits(self.output_gain_bits.load(Ordering::Relaxed)),
            breath_pressure: f32::from_bits(self.breath_pressure_bits.load(Ordering::Relaxed)),
            breath_noise_cutoff_hz: f32::from_bits(
                self.breath_noise_cutoff_hz_bits.load(Ordering::Relaxed),
            ),
            breath_feedback: f32::from_bits(self.breath_feedback_bits.load(Ordering::Relaxed)),
            breath_auto_level: f32::from_bits(self.breath_auto_level_bits.load(Ordering::Relaxed)),
            instrument: self.instrument.load(Ordering::Relaxed),
            handpan_scale: self.handpan_scale.load(Ordering::Relaxed),
            handpan_ding: self.handpan_ding.load(Ordering::Relaxed),
            handpan_tuning: self.handpan_tuning.load(Ordering::Relaxed),
            handpan_decay: f32::from_bits(self.handpan_decay_bits.load(Ordering::Relaxed)),
            handpan_detune_cents: f32::from_bits(
                self.handpan_detune_cents_bits.load(Ordering::Relaxed),
            ),
            handpan_coupling: f32::from_bits(self.handpan_coupling_bits.load(Ordering::Relaxed)),
            handpan_hit: f32::from_bits(self.handpan_hit_bits.load(Ordering::Relaxed)),
            handpan_glide: f32::from_bits(self.handpan_glide_bits.load(Ordering::Relaxed)),
            handpan_fundamental: f32::from_bits(
                self.handpan_fundamental_bits.load(Ordering::Relaxed),
            ),
            handpan_octave: f32::from_bits(self.handpan_octave_bits.load(Ordering::Relaxed)),
            handpan_fifth: f32::from_bits(self.handpan_fifth_bits.load(Ordering::Relaxed)),
            handpan_higher_decay: f32::from_bits(
                self.handpan_higher_decay_bits.load(Ordering::Relaxed),
            ),
            support_pulse: f32::from_bits(self.support_pulse_bits.load(Ordering::Relaxed)),
            support_width: f32::from_bits(self.support_width_bits.load(Ordering::Relaxed)),
            support_wander: f32::from_bits(self.support_wander_bits.load(Ordering::Relaxed)),
            support_drift: f32::from_bits(self.support_drift_bits.load(Ordering::Relaxed)),
            support_motion: f32::from_bits(self.support_motion_bits.load(Ordering::Relaxed)),
            support_fade: f32::from_bits(self.support_fade_bits.load(Ordering::Relaxed)),
            reverb_handpan: f32::from_bits(self.reverb_handpan_bits.load(Ordering::Relaxed)),
            reverb_ding: f32::from_bits(self.reverb_ding_bits.load(Ordering::Relaxed)),
            reverb_bass: f32::from_bits(self.reverb_bass_bits.load(Ordering::Relaxed)),
            reverb_echo: f32::from_bits(self.reverb_echo_bits.load(Ordering::Relaxed)),
            reverb_decay: f32::from_bits(self.reverb_decay_bits.load(Ordering::Relaxed)),
            reverb_tone: f32::from_bits(self.reverb_tone_bits.load(Ordering::Relaxed)),
            echo_instrument: self.echo_instrument.load(Ordering::Relaxed),
            support_muted: self.support_muted.load(Ordering::Relaxed),
            support_solo: self.support_solo.load(Ordering::Relaxed),
            handpan_muted: self.handpan_muted.load(Ordering::Relaxed),
            handpan_solo: self.handpan_solo.load(Ordering::Relaxed),
            ding_wash: self.ding_wash.load(Ordering::Relaxed),
            ding_shape: f32::from_bits(self.ding_shape_bits.load(Ordering::Relaxed)),
            ding_spread: f32::from_bits(self.ding_spread_bits.load(Ordering::Relaxed)),
            ding_brightness: f32::from_bits(self.ding_brightness_bits.load(Ordering::Relaxed)),
            ding_strings: f32::from_bits(self.ding_strings_bits.load(Ordering::Relaxed)),
            tambura_jawari: f32::from_bits(self.tambura_jawari_bits.load(Ordering::Relaxed)),
            ding_formant_1: f32::from_bits(self.ding_formant_1_bits.load(Ordering::Relaxed)),
            ding_formant_2: f32::from_bits(self.ding_formant_2_bits.load(Ordering::Relaxed)),
            ding_formant_3: f32::from_bits(self.ding_formant_3_bits.load(Ordering::Relaxed)),
            ding_q_1: f32::from_bits(self.ding_q_1_bits.load(Ordering::Relaxed)),
            ding_q_2: f32::from_bits(self.ding_q_2_bits.load(Ordering::Relaxed)),
            ding_q_3: f32::from_bits(self.ding_q_3_bits.load(Ordering::Relaxed)),
            mix_handpan: f32::from_bits(self.mix_handpan_bits.load(Ordering::Relaxed)),
            mix_ding: f32::from_bits(self.mix_ding_bits.load(Ordering::Relaxed)),
            mix_recent: f32::from_bits(self.mix_recent_bits.load(Ordering::Relaxed)),
            mix_second: f32::from_bits(self.mix_second_bits.load(Ordering::Relaxed)),
            mix_bass: f32::from_bits(self.mix_bass_bits.load(Ordering::Relaxed)),
            mix_taps: f32::from_bits(self.mix_taps_bits.load(Ordering::Relaxed)),
            deco_density: f32::from_bits(self.deco_density_bits.load(Ordering::Relaxed)),
            deco_lag_beats: f32::from_bits(self.deco_lag_beats_bits.load(Ordering::Relaxed)),
            pan_handpan: f32::from_bits(self.pan_handpan_bits.load(Ordering::Relaxed)),
            pan_ding: f32::from_bits(self.pan_ding_bits.load(Ordering::Relaxed)),
            pan_bass: f32::from_bits(self.pan_bass_bits.load(Ordering::Relaxed)),
            pan_echo: f32::from_bits(self.pan_echo_bits.load(Ordering::Relaxed)),
            formant_rate: f32::from_bits(self.formant_rate_bits.load(Ordering::Relaxed)),
            formant_depth: f32::from_bits(self.formant_depth_bits.load(Ordering::Relaxed)),
            groove_sensitivity: f32::from_bits(
                self.groove_sensitivity_bits.load(Ordering::Relaxed),
            ),
            groove_phase_pull: f32::from_bits(self.groove_phase_pull_bits.load(Ordering::Relaxed)),
            groove_click_level: f32::from_bits(
                self.groove_click_level_bits.load(Ordering::Relaxed),
            ),
            fm_bass_decay: f32::from_bits(self.fm_bass_decay_bits.load(Ordering::Relaxed)),
            fm_bass_ratio: f32::from_bits(self.fm_bass_ratio_bits.load(Ordering::Relaxed)),
            fm_bass_index: f32::from_bits(self.fm_bass_index_bits.load(Ordering::Relaxed)),
            fm_bass_index_decay: f32::from_bits(
                self.fm_bass_index_decay_bits.load(Ordering::Relaxed),
            ),
            fm_bass_feedback: f32::from_bits(self.fm_bass_feedback_bits.load(Ordering::Relaxed)),
            fm_bass_level: f32::from_bits(self.fm_bass_level_bits.load(Ordering::Relaxed)),
            bass_line_amount: f32::from_bits(self.bass_line_amount_bits.load(Ordering::Relaxed)),
            bass_line_variation: f32::from_bits(
                self.bass_line_variation_bits.load(Ordering::Relaxed),
            ),
            bells_decay: f32::from_bits(self.bells_decay_bits.load(Ordering::Relaxed)),
            bells_ratio: f32::from_bits(self.bells_ratio_bits.load(Ordering::Relaxed)),
            bells_index: f32::from_bits(self.bells_index_bits.load(Ordering::Relaxed)),
            bells_index_decay: f32::from_bits(self.bells_index_decay_bits.load(Ordering::Relaxed)),
            groove_click_enabled: self.groove_click_enabled.load(Ordering::Relaxed),
            groove_auto_lock: self.groove_auto_lock.load(Ordering::Relaxed),
            groove_lock_cmd: self
                .groove_lock_cmd
                .swap(GROOVE_LOCK_CMD_NONE, Ordering::Relaxed),
        }
    }

    pub(crate) fn echo_instrument(&self) -> u8 {
        self.echo_instrument.load(Ordering::Relaxed)
    }

    pub(crate) fn set_echo_instrument_u8(&self, instrument: u8) {
        self.echo_instrument.store(
            EchoInstrument::from_u8(instrument).as_u8(),
            Ordering::Relaxed,
        );
    }

    pub(crate) fn groove_click_enabled(&self) -> bool {
        self.groove_click_enabled.load(Ordering::Relaxed)
    }

    pub(crate) fn set_groove_click_enabled(&self, enabled: bool) {
        self.groove_click_enabled.store(enabled, Ordering::Relaxed);
    }

    pub(crate) fn groove_auto_lock(&self) -> bool {
        self.groove_auto_lock.load(Ordering::Relaxed)
    }

    pub(crate) fn set_groove_auto_lock(&self, enabled: bool) {
        self.groove_auto_lock.store(enabled, Ordering::Relaxed);
    }

    pub(crate) fn request_groove_lock(&self) {
        self.groove_lock_cmd
            .store(GROOVE_LOCK_CMD_LOCK, Ordering::Relaxed);
    }

    pub(crate) fn request_groove_unlock(&self) {
        self.groove_lock_cmd
            .store(GROOVE_LOCK_CMD_UNLOCK, Ordering::Relaxed);
    }

    pub(crate) fn groove_view(&self) -> GrooveView {
        let count = self.groove_ioi_count.load(Ordering::Relaxed) as usize;
        let mut iois_ms = [0.0; GROOVE_IOI_CAP];
        for (index, slot) in iois_ms
            .iter_mut()
            .enumerate()
            .take(count.min(GROOVE_IOI_CAP))
        {
            *slot = f32::from_bits(self.groove_ioi_ms[index].load(Ordering::Relaxed));
        }
        GrooveView {
            bpm: f32::from_bits(self.groove_bpm_bits.load(Ordering::Relaxed)),
            confidence: f32::from_bits(self.groove_confidence_bits.load(Ordering::Relaxed)),
            phase: f32::from_bits(self.groove_phase_bits.load(Ordering::Relaxed)),
            status: groove_status_from_u8(self.groove_status.load(Ordering::Relaxed)),
            locked: self.groove_locked.load(Ordering::Relaxed),
            iois_ms,
            ioi_count: count.min(GROOVE_IOI_CAP),
        }
    }

    fn publish_groove(&self, snap: &squawk_engine::GrooveSnapshot) {
        self.groove_bpm_bits
            .store(snap.bpm.to_bits(), Ordering::Relaxed);
        self.groove_confidence_bits
            .store(snap.confidence.to_bits(), Ordering::Relaxed);
        self.groove_phase_bits
            .store(snap.phase.to_bits(), Ordering::Relaxed);
        self.groove_status
            .store(groove_status_as_u8(snap.status), Ordering::Relaxed);
        self.groove_locked.store(snap.locked, Ordering::Relaxed);
        let count = snap.ioi_count.min(GROOVE_IOI_CAP);
        self.groove_ioi_count.store(count as u8, Ordering::Relaxed);
        for index in 0..count {
            self.groove_ioi_ms[index].store(snap.iois_ms[index].to_bits(), Ordering::Relaxed);
        }
    }

    #[inline]
    pub(crate) fn support_muted(&self) -> u8 {
        self.support_muted.load(Ordering::Relaxed)
    }

    pub(crate) fn support_solo(&self) -> u8 {
        self.support_solo.load(Ordering::Relaxed)
    }

    pub(crate) fn ding_wash(&self) -> u8 {
        self.ding_wash.load(Ordering::Relaxed)
    }

    pub(crate) fn set_ding_wash(&self, wash: u8) {
        self.ding_wash.store(wash.min(2), Ordering::Relaxed);
    }

    pub(crate) fn set_support_parts(&self, muted: u8, solo: u8) {
        self.support_muted.store(muted, Ordering::Relaxed);
        self.support_solo.store(solo, Ordering::Relaxed);
    }

    pub(crate) fn handpan_muted(&self) -> bool {
        self.handpan_muted.load(Ordering::Relaxed)
    }

    pub(crate) fn handpan_solo(&self) -> bool {
        self.handpan_solo.load(Ordering::Relaxed)
    }

    pub(crate) fn set_handpan_mute_solo(&self, muted: bool, solo: bool) {
        self.handpan_muted.store(muted, Ordering::Relaxed);
        self.handpan_solo.store(solo, Ordering::Relaxed);
    }

    pub(crate) fn instrument(&self) -> u8 {
        self.instrument.load(Ordering::Relaxed)
    }

    #[inline]
    pub(crate) fn handpan_scale(&self) -> u8 {
        self.handpan_scale.load(Ordering::Relaxed)
    }

    pub(crate) fn handpan_ding(&self) -> u8 {
        self.handpan_ding.load(Ordering::Relaxed)
    }

    pub(crate) fn handpan_tuning(&self) -> u8 {
        self.handpan_tuning.load(Ordering::Relaxed)
    }

    pub(crate) fn set_handpan_layout(&self, scale: u8, ding: u8, tuning: u8) {
        self.handpan_scale.store(scale, Ordering::Relaxed);
        self.handpan_ding.store(ding, Ordering::Relaxed);
        self.handpan_tuning.store(tuning, Ordering::Relaxed);
    }

    pub(crate) fn set_instrument_u8(&self, instrument: u8) {
        let instrument = Instrument::from_u8(instrument);
        self.instrument.store(instrument.as_u8(), Ordering::Relaxed);
        // Accompaniment bridge: keep Support wash synced to the voice being tuned.
        if let Some(wash) = instrument.drone_wash() {
            self.set_ding_wash(wash);
        }
    }

    pub(crate) fn fm_bass_params(&self) -> FmBassParams {
        FmBassParams {
            decay: self.parameter_value(ParameterId::FmBassDecay),
            ratio: self.parameter_value(ParameterId::FmBassRatio),
            index: self.parameter_value(ParameterId::FmBassIndex),
            index_decay: self.parameter_value(ParameterId::FmBassIndexDecay),
            feedback: self.parameter_value(ParameterId::FmBassFeedback),
            level: self.parameter_value(ParameterId::FmBassLevel),
        }
        .clamp()
    }

    pub(crate) fn apply_fm_bass_params(&self, params: FmBassParams) {
        let p = params.clamp();
        self.set_parameter_value(ParameterId::FmBassDecay, p.decay);
        self.set_parameter_value(ParameterId::FmBassRatio, p.ratio);
        self.set_parameter_value(ParameterId::FmBassIndex, p.index);
        self.set_parameter_value(ParameterId::FmBassIndexDecay, p.index_decay);
        self.set_parameter_value(ParameterId::FmBassFeedback, p.feedback);
        self.set_parameter_value(ParameterId::FmBassLevel, p.level);
    }

    #[inline]
    fn trigger_counter(&self) -> u32 {
        self.trigger_counter.load(Ordering::Relaxed)
    }

    #[inline]
    fn exciter_mode(&self) -> u8 {
        self.exciter_mode.load(Ordering::Relaxed)
    }

    #[inline]
    fn set_exciter_mode_u8(&self, mode: u8) {
        // Stable storage: 0 = strike, 1 = breath
        self.exciter_mode.store(mode.min(1), Ordering::Relaxed);
    }

    #[inline]
    fn attack_ms(&self) -> f32 {
        f32::from_bits(self.attack_ms_bits.load(Ordering::Relaxed))
    }

    #[inline]
    fn set_attack_ms(&self, attack_ms: f32) {
        self.attack_ms_bits
            .store(attack_ms.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    fn release_ms(&self) -> f32 {
        f32::from_bits(self.release_ms_bits.load(Ordering::Relaxed))
    }

    #[inline]
    fn set_release_ms(&self, release_ms: f32) {
        self.release_ms_bits
            .store(release_ms.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    fn damping_ratio(&self) -> f32 {
        f32::from_bits(self.damping_ratio_bits.load(Ordering::Relaxed))
    }

    #[inline]
    fn set_damping_ratio(&self, ratio: f32) {
        self.damping_ratio_bits
            .store(ratio.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    fn inharmonicity(&self) -> f32 {
        f32::from_bits(self.inharmonicity_bits.load(Ordering::Relaxed))
    }

    #[inline]
    fn set_inharmonicity(&self, value: f32) {
        self.inharmonicity_bits
            .store(value.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    fn position_coupling(&self) -> f32 {
        f32::from_bits(self.position_coupling_bits.load(Ordering::Relaxed))
    }

    #[inline]
    fn set_position_coupling(&self, value: f32) {
        self.position_coupling_bits
            .store(value.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    fn velocity_coupling(&self) -> f32 {
        f32::from_bits(self.velocity_coupling_bits.load(Ordering::Relaxed))
    }

    #[inline]
    fn set_velocity_coupling(&self, value: f32) {
        self.velocity_coupling_bits
            .store(value.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    fn excitation_gain(&self) -> f32 {
        f32::from_bits(self.excitation_gain_bits.load(Ordering::Relaxed))
    }

    #[inline]
    fn set_excitation_gain(&self, value: f32) {
        self.excitation_gain_bits
            .store(value.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    fn output_gain(&self) -> f32 {
        f32::from_bits(self.output_gain_bits.load(Ordering::Relaxed))
    }

    #[inline]
    fn set_output_gain(&self, value: f32) {
        self.output_gain_bits
            .store(value.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    fn breath_pressure(&self) -> f32 {
        f32::from_bits(self.breath_pressure_bits.load(Ordering::Relaxed))
    }

    #[inline]
    fn set_breath_pressure(&self, value: f32) {
        self.breath_pressure_bits
            .store(value.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    fn breath_noise_cutoff_hz(&self) -> f32 {
        f32::from_bits(self.breath_noise_cutoff_hz_bits.load(Ordering::Relaxed))
    }

    #[inline]
    fn set_breath_noise_cutoff_hz(&self, value: f32) {
        self.breath_noise_cutoff_hz_bits
            .store(value.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    fn breath_feedback(&self) -> f32 {
        f32::from_bits(self.breath_feedback_bits.load(Ordering::Relaxed))
    }

    #[inline]
    fn set_breath_feedback(&self, value: f32) {
        self.breath_feedback_bits
            .store(value.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    fn breath_auto_level(&self) -> f32 {
        f32::from_bits(self.breath_auto_level_bits.load(Ordering::Relaxed))
    }

    #[inline]
    fn set_breath_auto_level(&self, value: f32) {
        self.breath_auto_level_bits
            .store(value.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    fn peak_level(&self) -> f32 {
        f32::from_bits(self.peak_bits.load(Ordering::Relaxed))
    }

    #[inline]
    fn last_note_on_note(&self) -> Option<u8> {
        if self.last_note_on_counter.load(Ordering::Relaxed) == 0 {
            None
        } else {
            Some(self.last_note_on_note.load(Ordering::Relaxed))
        }
    }

    #[inline]
    fn last_note_on_counter(&self) -> u32 {
        self.last_note_on_counter.load(Ordering::Relaxed)
    }

    #[inline]
    fn last_note_on_hz(&self) -> Option<f32> {
        if self.last_note_on_counter.load(Ordering::Relaxed) == 0 {
            None
        } else {
            Some(f32::from_bits(
                self.last_note_on_hz_bits.load(Ordering::Relaxed),
            ))
        }
    }

    #[inline]
    fn update_peak(&self, block_peak: f32, block_duration_s: f32) {
        if !block_peak.is_finite() {
            return;
        }
        let decay = if block_duration_s <= 0.0 {
            0.0
        } else {
            (-block_duration_s / PEAK_DECAY_TAU).exp()
        };
        let current = self.peak_level();
        let decayed = current * decay;
        let next = block_peak.max(decayed);
        self.peak_bits
            .store(next.clamp(0.0, 1.5).to_bits(), Ordering::Relaxed);
    }

    #[inline]
    fn parameter_value(&self, id: ParameterId) -> f32 {
        match id {
            ParameterId::AttackMs => self.attack_ms(),
            ParameterId::ReleaseMs => self.release_ms(),
            ParameterId::DampingRatio => self.damping_ratio(),
            ParameterId::Inharmonicity => self.inharmonicity(),
            ParameterId::PositionCoupling => self.position_coupling(),
            ParameterId::VelocityCoupling => self.velocity_coupling(),
            ParameterId::ExcitationGain => self.excitation_gain(),
            ParameterId::OutputGain => self.output_gain(),
            ParameterId::BreathPressure => self.breath_pressure(),
            ParameterId::BreathNoiseCutoffHz => self.breath_noise_cutoff_hz(),
            ParameterId::BreathFeedback => self.breath_feedback(),
            ParameterId::BreathAutoLevel => self.breath_auto_level(),
            ParameterId::HandpanDecay => {
                f32::from_bits(self.handpan_decay_bits.load(Ordering::Relaxed))
            }
            ParameterId::HandpanDetuneCents => {
                f32::from_bits(self.handpan_detune_cents_bits.load(Ordering::Relaxed))
            }
            ParameterId::HandpanCoupling => {
                f32::from_bits(self.handpan_coupling_bits.load(Ordering::Relaxed))
            }
            ParameterId::HandpanHit => {
                f32::from_bits(self.handpan_hit_bits.load(Ordering::Relaxed))
            }
            ParameterId::HandpanGlide => {
                f32::from_bits(self.handpan_glide_bits.load(Ordering::Relaxed))
            }
            ParameterId::HandpanFundamental => {
                f32::from_bits(self.handpan_fundamental_bits.load(Ordering::Relaxed))
            }
            ParameterId::HandpanOctave => {
                f32::from_bits(self.handpan_octave_bits.load(Ordering::Relaxed))
            }
            ParameterId::HandpanFifth => {
                f32::from_bits(self.handpan_fifth_bits.load(Ordering::Relaxed))
            }
            ParameterId::HandpanHigherDecay => {
                f32::from_bits(self.handpan_higher_decay_bits.load(Ordering::Relaxed))
            }
            ParameterId::SupportLevel => {
                f32::from_bits(self.support_level_bits.load(Ordering::Relaxed))
            }
            ParameterId::SupportPulse => {
                f32::from_bits(self.support_pulse_bits.load(Ordering::Relaxed))
            }
            ParameterId::SupportWidth => {
                f32::from_bits(self.support_width_bits.load(Ordering::Relaxed))
            }
            ParameterId::SupportWander => {
                f32::from_bits(self.support_wander_bits.load(Ordering::Relaxed))
            }
            ParameterId::SupportDrift => {
                f32::from_bits(self.support_drift_bits.load(Ordering::Relaxed))
            }
            ParameterId::SupportMotion => {
                f32::from_bits(self.support_motion_bits.load(Ordering::Relaxed))
            }
            ParameterId::SupportFade => {
                f32::from_bits(self.support_fade_bits.load(Ordering::Relaxed))
            }
            ParameterId::ReverbHandpan => {
                f32::from_bits(self.reverb_handpan_bits.load(Ordering::Relaxed))
            }
            ParameterId::ReverbSupport => {
                f32::from_bits(self.reverb_support_bits.load(Ordering::Relaxed))
            }
            ParameterId::ReverbDing => {
                f32::from_bits(self.reverb_ding_bits.load(Ordering::Relaxed))
            }
            ParameterId::ReverbRecent => {
                f32::from_bits(self.reverb_recent_bits.load(Ordering::Relaxed))
            }
            ParameterId::ReverbSecond => {
                f32::from_bits(self.reverb_second_bits.load(Ordering::Relaxed))
            }
            ParameterId::ReverbBass => {
                f32::from_bits(self.reverb_bass_bits.load(Ordering::Relaxed))
            }
            ParameterId::ReverbEcho => {
                f32::from_bits(self.reverb_echo_bits.load(Ordering::Relaxed))
            }
            ParameterId::ReverbDecay => {
                f32::from_bits(self.reverb_decay_bits.load(Ordering::Relaxed))
            }
            ParameterId::ReverbTone => {
                f32::from_bits(self.reverb_tone_bits.load(Ordering::Relaxed))
            }
            ParameterId::DingShape => f32::from_bits(self.ding_shape_bits.load(Ordering::Relaxed)),
            ParameterId::DingSpread => {
                f32::from_bits(self.ding_spread_bits.load(Ordering::Relaxed))
            }
            ParameterId::DingBrightness => {
                f32::from_bits(self.ding_brightness_bits.load(Ordering::Relaxed))
            }
            ParameterId::DingStrings => {
                f32::from_bits(self.ding_strings_bits.load(Ordering::Relaxed))
            }
            ParameterId::TamburaJawari => {
                f32::from_bits(self.tambura_jawari_bits.load(Ordering::Relaxed))
            }
            ParameterId::DingFormant1 => {
                f32::from_bits(self.ding_formant_1_bits.load(Ordering::Relaxed))
            }
            ParameterId::DingFormant2 => {
                f32::from_bits(self.ding_formant_2_bits.load(Ordering::Relaxed))
            }
            ParameterId::DingFormant3 => {
                f32::from_bits(self.ding_formant_3_bits.load(Ordering::Relaxed))
            }
            ParameterId::DingResonance => {
                f32::from_bits(self.ding_resonance_bits.load(Ordering::Relaxed))
            }
            ParameterId::DingQ1 => f32::from_bits(self.ding_q_1_bits.load(Ordering::Relaxed)),
            ParameterId::DingQ2 => f32::from_bits(self.ding_q_2_bits.load(Ordering::Relaxed)),
            ParameterId::DingQ3 => f32::from_bits(self.ding_q_3_bits.load(Ordering::Relaxed)),
            ParameterId::MixHandpan => {
                f32::from_bits(self.mix_handpan_bits.load(Ordering::Relaxed))
            }
            ParameterId::MixDing => f32::from_bits(self.mix_ding_bits.load(Ordering::Relaxed)),
            ParameterId::MixRecent => f32::from_bits(self.mix_recent_bits.load(Ordering::Relaxed)),
            ParameterId::MixSecond => f32::from_bits(self.mix_second_bits.load(Ordering::Relaxed)),
            ParameterId::MixBass => f32::from_bits(self.mix_bass_bits.load(Ordering::Relaxed)),
            ParameterId::MixTaps => f32::from_bits(self.mix_taps_bits.load(Ordering::Relaxed)),
            ParameterId::DecoDensity => {
                f32::from_bits(self.deco_density_bits.load(Ordering::Relaxed))
            }
            ParameterId::DecoLagBeats => {
                f32::from_bits(self.deco_lag_beats_bits.load(Ordering::Relaxed))
            }
            ParameterId::PanHandpan => {
                f32::from_bits(self.pan_handpan_bits.load(Ordering::Relaxed))
            }
            ParameterId::PanDing => f32::from_bits(self.pan_ding_bits.load(Ordering::Relaxed)),
            ParameterId::PanRecent => f32::from_bits(self.pan_recent_bits.load(Ordering::Relaxed)),
            ParameterId::PanSecond => f32::from_bits(self.pan_second_bits.load(Ordering::Relaxed)),
            ParameterId::PanBass => f32::from_bits(self.pan_bass_bits.load(Ordering::Relaxed)),
            ParameterId::PanEcho => f32::from_bits(self.pan_echo_bits.load(Ordering::Relaxed)),
            ParameterId::FormantRate => {
                f32::from_bits(self.formant_rate_bits.load(Ordering::Relaxed))
            }
            ParameterId::FormantDepth => {
                f32::from_bits(self.formant_depth_bits.load(Ordering::Relaxed))
            }
            ParameterId::GrooveSensitivity => {
                f32::from_bits(self.groove_sensitivity_bits.load(Ordering::Relaxed))
            }
            ParameterId::GroovePhasePull => {
                f32::from_bits(self.groove_phase_pull_bits.load(Ordering::Relaxed))
            }
            ParameterId::GrooveClickLevel => {
                f32::from_bits(self.groove_click_level_bits.load(Ordering::Relaxed))
            }
            ParameterId::FmBassDecay => {
                f32::from_bits(self.fm_bass_decay_bits.load(Ordering::Relaxed))
            }
            ParameterId::FmBassRatio => {
                f32::from_bits(self.fm_bass_ratio_bits.load(Ordering::Relaxed))
            }
            ParameterId::FmBassIndex => {
                f32::from_bits(self.fm_bass_index_bits.load(Ordering::Relaxed))
            }
            ParameterId::FmBassIndexDecay => {
                f32::from_bits(self.fm_bass_index_decay_bits.load(Ordering::Relaxed))
            }
            ParameterId::FmBassFeedback => {
                f32::from_bits(self.fm_bass_feedback_bits.load(Ordering::Relaxed))
            }
            ParameterId::FmBassLevel => {
                f32::from_bits(self.fm_bass_level_bits.load(Ordering::Relaxed))
            }
            ParameterId::BassLineAmount => {
                f32::from_bits(self.bass_line_amount_bits.load(Ordering::Relaxed))
            }
            ParameterId::BassLineVariation => {
                f32::from_bits(self.bass_line_variation_bits.load(Ordering::Relaxed))
            }
            ParameterId::BellsDecay => {
                f32::from_bits(self.bells_decay_bits.load(Ordering::Relaxed))
            }
            ParameterId::BellsRatio => {
                f32::from_bits(self.bells_ratio_bits.load(Ordering::Relaxed))
            }
            ParameterId::BellsIndex => {
                f32::from_bits(self.bells_index_bits.load(Ordering::Relaxed))
            }
            ParameterId::BellsIndexDecay => {
                f32::from_bits(self.bells_index_decay_bits.load(Ordering::Relaxed))
            }
        }
    }

    #[inline]
    fn set_parameter_value(&self, id: ParameterId, value: f32) {
        match id {
            ParameterId::AttackMs => self.set_attack_ms(value),
            ParameterId::ReleaseMs => self.set_release_ms(value),
            ParameterId::DampingRatio => self.set_damping_ratio(value),
            ParameterId::Inharmonicity => self.set_inharmonicity(value),
            ParameterId::PositionCoupling => self.set_position_coupling(value),
            ParameterId::VelocityCoupling => self.set_velocity_coupling(value),
            ParameterId::ExcitationGain => self.set_excitation_gain(value),
            ParameterId::OutputGain => self.set_output_gain(value),
            ParameterId::BreathPressure => self.set_breath_pressure(value),
            ParameterId::BreathNoiseCutoffHz => self.set_breath_noise_cutoff_hz(value),
            ParameterId::BreathFeedback => self.set_breath_feedback(value),
            ParameterId::BreathAutoLevel => self.set_breath_auto_level(value),
            ParameterId::HandpanDecay => self
                .handpan_decay_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::HandpanDetuneCents => self
                .handpan_detune_cents_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::HandpanCoupling => self
                .handpan_coupling_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::HandpanHit => self
                .handpan_hit_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::HandpanGlide => self
                .handpan_glide_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::HandpanFundamental => self
                .handpan_fundamental_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::HandpanOctave => self
                .handpan_octave_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::HandpanFifth => self
                .handpan_fifth_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::HandpanHigherDecay => self
                .handpan_higher_decay_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::SupportLevel => self
                .support_level_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::SupportPulse => self
                .support_pulse_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::SupportWidth => self
                .support_width_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::SupportWander => self
                .support_wander_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::SupportDrift => self
                .support_drift_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::SupportMotion => self
                .support_motion_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::SupportFade => self
                .support_fade_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::ReverbHandpan => self
                .reverb_handpan_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::ReverbSupport => {
                // Old MIDI maps still land here; keep the four voice sends in step.
                let bits = value.to_bits();
                self.reverb_support_bits.store(bits, Ordering::Relaxed);
                self.reverb_ding_bits.store(bits, Ordering::Relaxed);
                self.reverb_recent_bits.store(bits, Ordering::Relaxed);
                self.reverb_second_bits.store(bits, Ordering::Relaxed);
                self.reverb_bass_bits.store(bits, Ordering::Relaxed);
            }
            ParameterId::ReverbDing => self
                .reverb_ding_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::ReverbRecent => self
                .reverb_recent_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::ReverbSecond => self
                .reverb_second_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::ReverbBass => self
                .reverb_bass_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::ReverbEcho => self
                .reverb_echo_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::ReverbDecay => self
                .reverb_decay_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::ReverbTone => self
                .reverb_tone_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::DingShape => self
                .ding_shape_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::DingSpread => self
                .ding_spread_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::DingBrightness => self
                .ding_brightness_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::DingStrings => self
                .ding_strings_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::TamburaJawari => self
                .tambura_jawari_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::DingFormant1 => self
                .ding_formant_1_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::DingFormant2 => self
                .ding_formant_2_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::DingFormant3 => self
                .ding_formant_3_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::DingResonance => self
                .ding_resonance_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::DingQ1 => self.ding_q_1_bits.store(value.to_bits(), Ordering::Relaxed),
            ParameterId::DingQ2 => self.ding_q_2_bits.store(value.to_bits(), Ordering::Relaxed),
            ParameterId::DingQ3 => self.ding_q_3_bits.store(value.to_bits(), Ordering::Relaxed),
            ParameterId::MixHandpan => self
                .mix_handpan_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::MixDing => self.mix_ding_bits.store(value.to_bits(), Ordering::Relaxed),
            ParameterId::MixRecent => self
                .mix_recent_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::MixSecond => self
                .mix_second_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::MixBass => self.mix_bass_bits.store(value.to_bits(), Ordering::Relaxed),
            ParameterId::MixTaps => self.mix_taps_bits.store(value.to_bits(), Ordering::Relaxed),
            ParameterId::DecoDensity => self
                .deco_density_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::DecoLagBeats => self
                .deco_lag_beats_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::PanHandpan => self
                .pan_handpan_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::PanDing => self.pan_ding_bits.store(value.to_bits(), Ordering::Relaxed),
            ParameterId::PanRecent => self
                .pan_recent_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::PanSecond => self
                .pan_second_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::PanBass => self.pan_bass_bits.store(value.to_bits(), Ordering::Relaxed),
            ParameterId::PanEcho => self.pan_echo_bits.store(value.to_bits(), Ordering::Relaxed),
            ParameterId::FormantRate => self
                .formant_rate_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::FormantDepth => self
                .formant_depth_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::GrooveSensitivity => self
                .groove_sensitivity_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::GroovePhasePull => self
                .groove_phase_pull_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::GrooveClickLevel => self
                .groove_click_level_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::FmBassDecay => self
                .fm_bass_decay_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::FmBassRatio => self
                .fm_bass_ratio_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::FmBassIndex => self
                .fm_bass_index_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::FmBassIndexDecay => self
                .fm_bass_index_decay_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::FmBassFeedback => self
                .fm_bass_feedback_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::FmBassLevel => self
                .fm_bass_level_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::BassLineAmount => self
                .bass_line_amount_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::BassLineVariation => self
                .bass_line_variation_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::BellsDecay => self
                .bells_decay_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::BellsRatio => self
                .bells_ratio_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::BellsIndex => self
                .bells_index_bits
                .store(value.to_bits(), Ordering::Relaxed),
            ParameterId::BellsIndexDecay => self
                .bells_index_decay_bits
                .store(value.to_bits(), Ordering::Relaxed),
        }
    }

    fn reset_parameters(&self) {
        self.set_exciter_mode_u8(ExciterMode::Strike.as_u8());
        self.set_attack_ms(DEFAULT_ATTACK_MS);
        self.set_release_ms(DEFAULT_RELEASE_MS);
        self.set_damping_ratio(DEFAULT_DAMPING_RATIO);
        self.set_inharmonicity(DEFAULT_INHARMONICITY);
        self.set_position_coupling(DEFAULT_POSITION_COUPLING);
        self.set_velocity_coupling(DEFAULT_VELOCITY_COUPLING);
        self.set_excitation_gain(DEFAULT_EXCITATION_GAIN);
        self.set_output_gain(DEFAULT_OUTPUT_GAIN);
        self.set_breath_pressure(DEFAULT_BREATH_PRESSURE);
        self.set_breath_noise_cutoff_hz(DEFAULT_BREATH_NOISE_CUTOFF_HZ);
        self.set_breath_feedback(DEFAULT_BREATH_FEEDBACK);
        self.set_breath_auto_level(DEFAULT_BREATH_AUTO_LEVEL);
        self.set_parameter_value(ParameterId::HandpanDecay, DEFAULT_HANDPAN_DECAY);
        self.set_parameter_value(
            ParameterId::HandpanDetuneCents,
            DEFAULT_HANDPAN_DETUNE_CENTS,
        );
        self.set_parameter_value(ParameterId::HandpanCoupling, DEFAULT_HANDPAN_COUPLING);
        self.set_parameter_value(ParameterId::HandpanHit, DEFAULT_HANDPAN_HIT);
        self.set_parameter_value(ParameterId::HandpanGlide, DEFAULT_HANDPAN_GLIDE);
        self.set_parameter_value(ParameterId::HandpanFundamental, DEFAULT_HANDPAN_FUNDAMENTAL);
        self.set_parameter_value(ParameterId::HandpanOctave, DEFAULT_HANDPAN_OCTAVE);
        self.set_parameter_value(ParameterId::HandpanFifth, DEFAULT_HANDPAN_FIFTH);
        self.set_parameter_value(
            ParameterId::HandpanHigherDecay,
            DEFAULT_HANDPAN_HIGHER_DECAY,
        );
        self.set_parameter_value(ParameterId::SupportLevel, DEFAULT_SUPPORT_LEVEL);
        self.set_parameter_value(ParameterId::SupportPulse, DEFAULT_SUPPORT_PULSE);
        self.set_parameter_value(ParameterId::SupportWidth, DEFAULT_SUPPORT_WIDTH);
        self.set_parameter_value(ParameterId::SupportWander, DEFAULT_SUPPORT_WANDER);
        self.set_parameter_value(ParameterId::SupportDrift, DEFAULT_SUPPORT_DRIFT);
        self.set_parameter_value(ParameterId::SupportMotion, DEFAULT_SUPPORT_MOTION);
        self.set_parameter_value(ParameterId::SupportFade, DEFAULT_SUPPORT_FADE);
        self.set_parameter_value(ParameterId::ReverbHandpan, DEFAULT_REVERB_HANDPAN);
        self.set_parameter_value(ParameterId::ReverbSupport, DEFAULT_REVERB_SUPPORT);
        self.set_parameter_value(ParameterId::ReverbDing, DEFAULT_REVERB_DING);
        self.set_parameter_value(ParameterId::ReverbRecent, DEFAULT_REVERB_RECENT);
        self.set_parameter_value(ParameterId::ReverbSecond, DEFAULT_REVERB_SECOND);
        self.set_parameter_value(ParameterId::ReverbBass, DEFAULT_REVERB_BASS);
        self.set_parameter_value(ParameterId::ReverbEcho, DEFAULT_REVERB_ECHO);
        self.set_parameter_value(ParameterId::ReverbDecay, DEFAULT_REVERB_DECAY);
        self.set_parameter_value(ParameterId::ReverbTone, DEFAULT_REVERB_TONE);
        self.set_echo_instrument_u8(DEFAULT_ECHO_INSTRUMENT);
        self.set_support_parts(0, 0);
        self.set_handpan_mute_solo(false, false);
        self.set_ding_wash(DEFAULT_DING_WASH);
        self.set_parameter_value(ParameterId::DingShape, DEFAULT_DING_SHAPE);
        self.set_parameter_value(ParameterId::DingSpread, DEFAULT_DING_SPREAD);
        self.set_parameter_value(ParameterId::DingBrightness, DEFAULT_DING_BRIGHTNESS);
        self.set_parameter_value(ParameterId::DingStrings, DEFAULT_DING_STRINGS);
        self.set_parameter_value(ParameterId::TamburaJawari, DEFAULT_TAMBURA_JAWARI);
        self.set_parameter_value(ParameterId::DingFormant1, DEFAULT_DING_FORMANT_1);
        self.set_parameter_value(ParameterId::DingFormant2, DEFAULT_DING_FORMANT_2);
        self.set_parameter_value(ParameterId::DingFormant3, DEFAULT_DING_FORMANT_3);
        self.set_parameter_value(ParameterId::DingResonance, DEFAULT_DING_RESONANCE);
        self.set_parameter_value(ParameterId::DingQ1, DEFAULT_DING_RESONANCE);
        self.set_parameter_value(ParameterId::DingQ2, DEFAULT_DING_RESONANCE);
        self.set_parameter_value(ParameterId::DingQ3, DEFAULT_DING_RESONANCE);
        self.set_parameter_value(ParameterId::MixHandpan, DEFAULT_MIX_LEVEL);
        self.set_parameter_value(ParameterId::MixDing, DEFAULT_MIX_LEVEL);
        self.set_parameter_value(ParameterId::MixRecent, DEFAULT_MIX_LEVEL);
        self.set_parameter_value(ParameterId::MixSecond, DEFAULT_MIX_LEVEL);
        self.set_parameter_value(ParameterId::MixBass, DEFAULT_MIX_LEVEL);
        self.set_parameter_value(ParameterId::MixTaps, DEFAULT_MIX_LEVEL);
        self.set_parameter_value(ParameterId::DecoDensity, DEFAULT_DECO_DENSITY);
        self.set_parameter_value(ParameterId::DecoLagBeats, DEFAULT_DECO_LAG_BEATS);
        self.set_parameter_value(ParameterId::PanHandpan, DEFAULT_PAN);
        self.set_parameter_value(ParameterId::PanDing, DEFAULT_PAN);
        self.set_parameter_value(ParameterId::PanRecent, DEFAULT_PAN);
        self.set_parameter_value(ParameterId::PanSecond, DEFAULT_PAN);
        self.set_parameter_value(ParameterId::PanBass, DEFAULT_PAN);
        self.set_parameter_value(ParameterId::PanEcho, DEFAULT_PAN);
        self.set_parameter_value(ParameterId::FormantRate, DEFAULT_FORMANT_RATE);
        self.set_parameter_value(ParameterId::FormantDepth, DEFAULT_FORMANT_DEPTH);
        self.set_parameter_value(ParameterId::GrooveSensitivity, DEFAULT_GROOVE_SENSITIVITY);
        self.set_parameter_value(ParameterId::GroovePhasePull, DEFAULT_GROOVE_PHASE_PULL);
        self.set_parameter_value(ParameterId::GrooveClickLevel, DEFAULT_GROOVE_CLICK_LEVEL);
        self.set_parameter_value(ParameterId::FmBassDecay, DEFAULT_FM_BASS_DECAY);
        self.set_parameter_value(ParameterId::FmBassRatio, DEFAULT_FM_BASS_RATIO);
        self.set_parameter_value(ParameterId::FmBassIndex, DEFAULT_FM_BASS_INDEX);
        self.set_parameter_value(ParameterId::FmBassIndexDecay, DEFAULT_FM_BASS_INDEX_DECAY);
        self.set_parameter_value(ParameterId::FmBassFeedback, DEFAULT_FM_BASS_FEEDBACK);
        self.set_parameter_value(ParameterId::FmBassLevel, DEFAULT_FM_BASS_LEVEL);
        self.set_parameter_value(ParameterId::BassLineAmount, DEFAULT_BASS_LINE_AMOUNT);
        self.set_parameter_value(ParameterId::BassLineVariation, DEFAULT_BASS_LINE_VARIATION);
        self.set_parameter_value(ParameterId::BellsDecay, DEFAULT_BELLS_DECAY);
        self.set_parameter_value(ParameterId::BellsRatio, DEFAULT_BELLS_RATIO);
        self.set_parameter_value(ParameterId::BellsIndex, DEFAULT_BELLS_INDEX);
        self.set_parameter_value(ParameterId::BellsIndexDecay, DEFAULT_BELLS_INDEX_DECAY);
        self.set_groove_click_enabled(DEFAULT_GROOVE_CLICK_ENABLED);
        self.set_groove_auto_lock(DEFAULT_GROOVE_AUTO_LOCK);
        self.request_groove_unlock();
        self.set_handpan_layout(
            DEFAULT_HANDPAN_SCALE,
            DEFAULT_HANDPAN_DING,
            DEFAULT_HANDPAN_TUNING,
        );
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GrooveView {
    pub bpm: f32,
    pub confidence: f32,
    pub phase: f32,
    pub status: GrooveStatus,
    pub locked: bool,
    pub iois_ms: [f32; GROOVE_IOI_CAP],
    pub ioi_count: usize,
}

fn groove_status_as_u8(status: GrooveStatus) -> u8 {
    match status {
        GrooveStatus::Listening => 0,
        GrooveStatus::Locking => 1,
        GrooveStatus::Locked => 2,
        GrooveStatus::Lost => 3,
    }
}

fn groove_status_from_u8(value: u8) -> GrooveStatus {
    match value {
        1 => GrooveStatus::Locking,
        2 => GrooveStatus::Locked,
        3 => GrooveStatus::Lost,
        _ => GrooveStatus::Listening,
    }
}

#[derive(Debug, Clone, Copy)]
struct ControlSnapshot {
    gate: bool,
    frequency_hz: f32,
    amplitude: f32,
    exciter_mode: u8,
    attack_ms: f32,
    release_ms: f32,
    damping_ratio: f32,
    inharmonicity: f32,
    position_coupling: f32,
    velocity_coupling: f32,
    excitation_gain: f32,
    output_gain: f32,
    breath_pressure: f32,
    breath_noise_cutoff_hz: f32,
    breath_feedback: f32,
    breath_auto_level: f32,
    instrument: u8,
    handpan_scale: u8,
    handpan_ding: u8,
    handpan_tuning: u8,
    handpan_decay: f32,
    handpan_detune_cents: f32,
    handpan_coupling: f32,
    handpan_hit: f32,
    handpan_glide: f32,
    handpan_fundamental: f32,
    handpan_octave: f32,
    handpan_fifth: f32,
    handpan_higher_decay: f32,
    support_pulse: f32,
    support_width: f32,
    support_wander: f32,
    support_drift: f32,
    support_motion: f32,
    support_fade: f32,
    reverb_handpan: f32,
    reverb_ding: f32,
    reverb_bass: f32,
    reverb_echo: f32,
    reverb_decay: f32,
    reverb_tone: f32,
    echo_instrument: u8,
    support_muted: u8,
    support_solo: u8,
    handpan_muted: bool,
    handpan_solo: bool,
    ding_wash: u8,
    ding_shape: f32,
    ding_spread: f32,
    ding_brightness: f32,
    ding_strings: f32,
    tambura_jawari: f32,
    ding_formant_1: f32,
    ding_formant_2: f32,
    ding_formant_3: f32,
    ding_q_1: f32,
    ding_q_2: f32,
    ding_q_3: f32,
    mix_handpan: f32,
    mix_ding: f32,
    mix_recent: f32,
    mix_second: f32,
    mix_bass: f32,
    mix_taps: f32,
    deco_density: f32,
    deco_lag_beats: f32,
    pan_handpan: f32,
    pan_ding: f32,
    pan_bass: f32,
    pan_echo: f32,
    formant_rate: f32,
    formant_depth: f32,
    groove_sensitivity: f32,
    groove_phase_pull: f32,
    groove_click_level: f32,
    fm_bass_decay: f32,
    fm_bass_ratio: f32,
    fm_bass_index: f32,
    fm_bass_index_decay: f32,
    fm_bass_feedback: f32,
    fm_bass_level: f32,
    bass_line_amount: f32,
    bass_line_variation: f32,
    bells_decay: f32,
    bells_ratio: f32,
    bells_index: f32,
    bells_index_decay: f32,
    groove_click_enabled: bool,
    groove_auto_lock: bool,
    groove_lock_cmd: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParameterId {
    AttackMs,
    ReleaseMs,
    DampingRatio,
    Inharmonicity,
    PositionCoupling,
    VelocityCoupling,
    ExcitationGain,
    OutputGain,
    BreathPressure,
    BreathNoiseCutoffHz,
    BreathFeedback,
    BreathAutoLevel,
    HandpanDecay,
    HandpanDetuneCents,
    HandpanCoupling,
    HandpanHit,
    HandpanGlide,
    HandpanFundamental,
    HandpanOctave,
    HandpanFifth,
    HandpanHigherDecay,
    SupportLevel,
    SupportPulse,
    SupportWidth,
    SupportWander,
    SupportDrift,
    SupportMotion,
    SupportFade,
    ReverbHandpan,
    ReverbSupport,
    ReverbDing,
    ReverbRecent,
    ReverbSecond,
    ReverbBass,
    ReverbEcho,
    ReverbDecay,
    ReverbTone,
    DingShape,
    DingSpread,
    DingBrightness,
    DingStrings,
    TamburaJawari,
    DingFormant1,
    DingFormant2,
    DingFormant3,
    DingResonance,
    DingQ1,
    DingQ2,
    DingQ3,
    MixHandpan,
    MixDing,
    MixRecent,
    MixSecond,
    MixBass,
    MixTaps,
    DecoDensity,
    DecoLagBeats,
    PanHandpan,
    PanDing,
    PanRecent,
    PanSecond,
    PanBass,
    PanEcho,
    FormantRate,
    FormantDepth,
    GrooveSensitivity,
    GroovePhasePull,
    GrooveClickLevel,
    FmBassDecay,
    FmBassRatio,
    FmBassIndex,
    FmBassIndexDecay,
    FmBassFeedback,
    FmBassLevel,
    BassLineAmount,
    BassLineVariation,
    BellsDecay,
    BellsRatio,
    BellsIndex,
    BellsIndexDecay,
}

const MODAL_PARAMETERS: [ParameterId; 12] = [
    ParameterId::AttackMs,
    ParameterId::ReleaseMs,
    ParameterId::DampingRatio,
    ParameterId::Inharmonicity,
    ParameterId::PositionCoupling,
    ParameterId::VelocityCoupling,
    ParameterId::ExcitationGain,
    ParameterId::OutputGain,
    ParameterId::BreathPressure,
    ParameterId::BreathNoiseCutoffHz,
    ParameterId::BreathFeedback,
    ParameterId::BreathAutoLevel,
];

const FM_BASS_PARAMETERS: [ParameterId; 5] = [
    ParameterId::FmBassDecay,
    ParameterId::FmBassRatio,
    ParameterId::FmBassIndex,
    ParameterId::FmBassIndexDecay,
    ParameterId::FmBassFeedback,
];

const BELLS_PARAMETERS: [ParameterId; 4] = [
    ParameterId::BellsDecay,
    ParameterId::BellsRatio,
    ParameterId::BellsIndex,
    ParameterId::BellsIndexDecay,
];

const HANDPAN_PARAMETERS: [ParameterId; 9] = [
    ParameterId::HandpanDecay,
    ParameterId::HandpanDetuneCents,
    ParameterId::HandpanCoupling,
    ParameterId::HandpanHit,
    ParameterId::HandpanGlide,
    ParameterId::HandpanFundamental,
    ParameterId::HandpanOctave,
    ParameterId::HandpanFifth,
    ParameterId::HandpanHigherDecay,
];

const ANALYSIS_PARAMETERS: [ParameterId; 2] = [ParameterId::SupportPulse, ParameterId::SupportFade];

const BAND_MOTION_PARAMETERS: [ParameterId; 4] = [
    ParameterId::SupportWidth,
    ParameterId::SupportWander,
    ParameterId::SupportDrift,
    ParameterId::SupportMotion,
];

const REVERB_PARAMETERS: [ParameterId; 2] = [ParameterId::ReverbDecay, ParameterId::ReverbTone];

const DING_WASH_PARAMETERS: [ParameterId; 3] = [
    ParameterId::DingShape,
    ParameterId::DingSpread,
    ParameterId::DingBrightness,
];

const TAMBURA_PARAMETERS: [ParameterId; 4] = [
    ParameterId::DingStrings,
    ParameterId::DingShape,
    ParameterId::DingSpread,
    ParameterId::TamburaJawari,
];

const FORMANT_PARAMETERS: [ParameterId; 6] = [
    ParameterId::DingFormant1,
    ParameterId::DingQ1,
    ParameterId::DingFormant2,
    ParameterId::DingQ2,
    ParameterId::DingFormant3,
    ParameterId::DingQ3,
];

impl ParameterId {
    fn as_i8(self) -> i8 {
        match self {
            ParameterId::AttackMs => 0,
            ParameterId::ReleaseMs => 1,
            ParameterId::DampingRatio => 2,
            ParameterId::Inharmonicity => 3,
            ParameterId::PositionCoupling => 4,
            ParameterId::VelocityCoupling => 5,
            ParameterId::ExcitationGain => 6,
            ParameterId::OutputGain => 7,
            ParameterId::BreathPressure => 8,
            ParameterId::BreathFeedback => 9,
            ParameterId::BreathAutoLevel => 10,
            ParameterId::BreathNoiseCutoffHz => 11,
            ParameterId::HandpanDecay => 12,
            ParameterId::HandpanDetuneCents => 13,
            ParameterId::HandpanCoupling => 14,
            ParameterId::HandpanHit => 15,
            ParameterId::HandpanGlide => 16,
            ParameterId::HandpanFundamental => 20,
            ParameterId::HandpanOctave => 17,
            ParameterId::HandpanFifth => 18,
            ParameterId::HandpanHigherDecay => 19,
            ParameterId::SupportLevel => 21,
            ParameterId::SupportPulse => 22,
            ParameterId::SupportWidth => 23,
            ParameterId::SupportWander => 24,
            ParameterId::SupportDrift => 25,
            ParameterId::SupportMotion => 26,
            ParameterId::SupportFade => 27,
            ParameterId::ReverbHandpan => 28,
            ParameterId::ReverbSupport => 29,
            ParameterId::ReverbDecay => 30,
            ParameterId::ReverbTone => 31,
            ParameterId::DingShape => 32,
            ParameterId::DingSpread => 33,
            ParameterId::DingBrightness => 34,
            ParameterId::DingStrings => 35,
            ParameterId::TamburaJawari => 79,
            ParameterId::DingFormant1 => 36,
            ParameterId::DingFormant2 => 37,
            ParameterId::DingFormant3 => 38,
            ParameterId::DingResonance => 39,
            ParameterId::DingQ1 => 40,
            ParameterId::DingQ2 => 41,
            ParameterId::DingQ3 => 42,
            ParameterId::MixHandpan => 43,
            ParameterId::MixDing => 44,
            ParameterId::MixRecent => 45,
            ParameterId::MixSecond => 46,
            ParameterId::MixBass => 47,
            ParameterId::MixTaps => 48,
            ParameterId::DecoDensity => 49,
            ParameterId::DecoLagBeats => 66,
            ParameterId::PanHandpan => 50,
            ParameterId::PanDing => 51,
            ParameterId::PanRecent => 52,
            ParameterId::PanSecond => 53,
            ParameterId::PanBass => 54,
            ParameterId::FormantRate => 55,
            ParameterId::FormantDepth => 56,
            ParameterId::ReverbDing => 57,
            ParameterId::ReverbRecent => 58,
            ParameterId::ReverbSecond => 59,
            ParameterId::ReverbBass => 60,
            ParameterId::PanEcho => 61,
            ParameterId::ReverbEcho => 62,
            ParameterId::GrooveSensitivity => 63,
            ParameterId::GroovePhasePull => 64,
            ParameterId::GrooveClickLevel => 65,
            ParameterId::FmBassDecay => 67,
            ParameterId::FmBassRatio => 68,
            ParameterId::FmBassIndex => 69,
            ParameterId::FmBassIndexDecay => 71,
            ParameterId::FmBassFeedback => 72,
            ParameterId::FmBassLevel => 70,
            ParameterId::BassLineAmount => 73,
            ParameterId::BassLineVariation => 74,
            ParameterId::BellsDecay => 75,
            ParameterId::BellsRatio => 76,
            ParameterId::BellsIndex => 77,
            ParameterId::BellsIndexDecay => 78,
        }
    }

    fn from_i8(value: i8) -> Option<Self> {
        match value {
            0 => Some(ParameterId::AttackMs),
            1 => Some(ParameterId::ReleaseMs),
            2 => Some(ParameterId::DampingRatio),
            3 => Some(ParameterId::Inharmonicity),
            4 => Some(ParameterId::PositionCoupling),
            5 => Some(ParameterId::VelocityCoupling),
            6 => Some(ParameterId::ExcitationGain),
            7 => Some(ParameterId::OutputGain),
            8 => Some(ParameterId::BreathPressure),
            9 => Some(ParameterId::BreathFeedback),
            10 => Some(ParameterId::BreathAutoLevel),
            11 => Some(ParameterId::BreathNoiseCutoffHz),
            12 => Some(ParameterId::HandpanDecay),
            13 => Some(ParameterId::HandpanDetuneCents),
            14 => Some(ParameterId::HandpanCoupling),
            15 => Some(ParameterId::HandpanHit),
            16 => Some(ParameterId::HandpanGlide),
            17 => Some(ParameterId::HandpanOctave),
            20 => Some(ParameterId::HandpanFundamental),
            18 => Some(ParameterId::HandpanFifth),
            19 => Some(ParameterId::HandpanHigherDecay),
            21 => Some(ParameterId::SupportLevel),
            22 => Some(ParameterId::SupportPulse),
            23 => Some(ParameterId::SupportWidth),
            24 => Some(ParameterId::SupportWander),
            25 => Some(ParameterId::SupportDrift),
            26 => Some(ParameterId::SupportMotion),
            27 => Some(ParameterId::SupportFade),
            28 => Some(ParameterId::ReverbHandpan),
            29 => Some(ParameterId::ReverbSupport),
            30 => Some(ParameterId::ReverbDecay),
            31 => Some(ParameterId::ReverbTone),
            32 => Some(ParameterId::DingShape),
            33 => Some(ParameterId::DingSpread),
            34 => Some(ParameterId::DingBrightness),
            35 => Some(ParameterId::DingStrings),
            79 => Some(ParameterId::TamburaJawari),
            36 => Some(ParameterId::DingFormant1),
            37 => Some(ParameterId::DingFormant2),
            38 => Some(ParameterId::DingFormant3),
            39 => Some(ParameterId::DingResonance),
            40 => Some(ParameterId::DingQ1),
            41 => Some(ParameterId::DingQ2),
            42 => Some(ParameterId::DingQ3),
            43 => Some(ParameterId::MixHandpan),
            44 => Some(ParameterId::MixDing),
            45 => Some(ParameterId::MixRecent),
            46 => Some(ParameterId::MixSecond),
            47 => Some(ParameterId::MixBass),
            48 => Some(ParameterId::MixTaps),
            49 => Some(ParameterId::DecoDensity),
            66 => Some(ParameterId::DecoLagBeats),
            50 => Some(ParameterId::PanHandpan),
            51 => Some(ParameterId::PanDing),
            52 => Some(ParameterId::PanRecent),
            53 => Some(ParameterId::PanSecond),
            54 => Some(ParameterId::PanBass),
            55 => Some(ParameterId::FormantRate),
            56 => Some(ParameterId::FormantDepth),
            57 => Some(ParameterId::ReverbDing),
            58 => Some(ParameterId::ReverbRecent),
            59 => Some(ParameterId::ReverbSecond),
            60 => Some(ParameterId::ReverbBass),
            61 => Some(ParameterId::PanEcho),
            62 => Some(ParameterId::ReverbEcho),
            63 => Some(ParameterId::GrooveSensitivity),
            64 => Some(ParameterId::GroovePhasePull),
            65 => Some(ParameterId::GrooveClickLevel),
            67 => Some(ParameterId::FmBassDecay),
            68 => Some(ParameterId::FmBassRatio),
            69 => Some(ParameterId::FmBassIndex),
            70 => Some(ParameterId::FmBassLevel),
            71 => Some(ParameterId::FmBassIndexDecay),
            72 => Some(ParameterId::FmBassFeedback),
            73 => Some(ParameterId::BassLineAmount),
            74 => Some(ParameterId::BassLineVariation),
            75 => Some(ParameterId::BellsDecay),
            76 => Some(ParameterId::BellsRatio),
            77 => Some(ParameterId::BellsIndex),
            78 => Some(ParameterId::BellsIndexDecay),
            _ => None,
        }
    }

    pub(crate) fn modal() -> &'static [ParameterId] {
        &MODAL_PARAMETERS
    }

    pub(crate) fn handpan() -> &'static [ParameterId] {
        &HANDPAN_PARAMETERS
    }

    pub(crate) fn fm_bass() -> &'static [ParameterId] {
        &FM_BASS_PARAMETERS
    }

    pub(crate) fn bells() -> &'static [ParameterId] {
        &BELLS_PARAMETERS
    }

    pub(crate) fn analysis() -> &'static [ParameterId] {
        &ANALYSIS_PARAMETERS
    }

    pub(crate) fn band_motion() -> &'static [ParameterId] {
        &BAND_MOTION_PARAMETERS
    }

    pub(crate) fn reverb() -> &'static [ParameterId] {
        &REVERB_PARAMETERS
    }

    pub(crate) fn ding_wash() -> &'static [ParameterId] {
        &DING_WASH_PARAMETERS
    }

    pub(crate) fn tambura() -> &'static [ParameterId] {
        &TAMBURA_PARAMETERS
    }

    pub(crate) fn formant() -> &'static [ParameterId] {
        &FORMANT_PARAMETERS
    }

    fn label(self) -> &'static str {
        match self {
            ParameterId::AttackMs => "Attack (ms)",
            ParameterId::ReleaseMs => "Release (ms)",
            ParameterId::DampingRatio => "Damping ratio",
            ParameterId::Inharmonicity => "Inharmonicity",
            ParameterId::PositionCoupling => "Position coupling",
            ParameterId::VelocityCoupling => "Velocity coupling",
            ParameterId::ExcitationGain => "Excitation gain",
            ParameterId::OutputGain => "Output gain",
            ParameterId::BreathPressure => "Breath pressure",
            ParameterId::BreathNoiseCutoffHz => "Breath brightness",
            ParameterId::BreathFeedback => "Breath feedback",
            ParameterId::BreathAutoLevel => "Breath auto-level",
            ParameterId::HandpanDecay => "Handpan decay",
            ParameterId::HandpanDetuneCents => "Pair detune",
            ParameterId::HandpanCoupling => "Pad coupling",
            ParameterId::HandpanHit => "Hit",
            ParameterId::HandpanGlide => "Glide",
            ParameterId::HandpanFundamental => "Fundamental",
            ParameterId::HandpanOctave => "Octave",
            ParameterId::HandpanFifth => "Fifth",
            ParameterId::HandpanHigherDecay => "Higher decay",
            ParameterId::SupportLevel => "Support level",
            ParameterId::SupportPulse => "Pulse",
            ParameterId::SupportWidth => "Width",
            ParameterId::SupportWander => "Wander",
            ParameterId::SupportDrift => "Drift",
            ParameterId::SupportMotion => "Motion",
            ParameterId::SupportFade => "Fade",
            ParameterId::ReverbHandpan => "Handpan send",
            ParameterId::ReverbSupport => "Accompaniment send",
            ParameterId::ReverbDing => "Ding send",
            ParameterId::ReverbRecent => "Recent send",
            ParameterId::ReverbSecond => "Second send",
            ParameterId::ReverbBass => "Bass send",
            ParameterId::ReverbEcho => "Echo send",
            ParameterId::ReverbDecay => "Decay",
            ParameterId::ReverbTone => "Tone",
            ParameterId::DingShape => "Shape",
            ParameterId::DingSpread => "Spread",
            ParameterId::DingBrightness => "Brightness",
            ParameterId::DingStrings => "Strings",
            ParameterId::TamburaJawari => "Jawari",
            ParameterId::DingFormant1 => "Formant 1",
            ParameterId::DingFormant2 => "Formant 2",
            ParameterId::DingFormant3 => "Formant 3",
            ParameterId::DingResonance => "Resonance",
            ParameterId::DingQ1 => "Q 1",
            ParameterId::DingQ2 => "Q 2",
            ParameterId::DingQ3 => "Q 3",
            ParameterId::MixHandpan => "Handpan",
            ParameterId::MixDing => "Root",
            ParameterId::MixRecent => "Recent",
            ParameterId::MixSecond => "Second",
            ParameterId::MixBass => "Bass",
            ParameterId::MixTaps => "Level",
            ParameterId::DecoDensity => "Density",
            ParameterId::DecoLagBeats => "Lag",
            ParameterId::PanHandpan => "Handpan pan",
            ParameterId::PanDing => "Ding pan",
            ParameterId::PanRecent => "Recent pan",
            ParameterId::PanSecond => "Second pan",
            ParameterId::PanBass => "Bass pan",
            ParameterId::PanEcho => "Echo pan",
            ParameterId::FormantRate => "Rate",
            ParameterId::FormantDepth => "Depth",
            ParameterId::GrooveSensitivity => "Sensitivity",
            ParameterId::GroovePhasePull => "Phase pull",
            ParameterId::GrooveClickLevel => "Click level",
            ParameterId::FmBassDecay => "Decay",
            ParameterId::FmBassRatio => "Ratio",
            ParameterId::FmBassIndex => "Index",
            ParameterId::FmBassIndexDecay => "Index decay",
            ParameterId::FmBassFeedback => "Feedback",
            ParameterId::FmBassLevel => "Level",
            ParameterId::BassLineAmount => "Line amount",
            ParameterId::BassLineVariation => "Line variation",
            ParameterId::BellsDecay => "Decay",
            ParameterId::BellsRatio => "Ratio",
            ParameterId::BellsIndex => "Index",
            ParameterId::BellsIndexDecay => "Index decay",
        }
    }

    fn range(self) -> (f32, f32) {
        match self {
            ParameterId::AttackMs => ATTACK_RANGE_MS,
            ParameterId::ReleaseMs => RELEASE_RANGE_MS,
            ParameterId::DampingRatio => DAMPING_RATIO_RANGE,
            ParameterId::Inharmonicity => INHARMONICITY_RANGE,
            ParameterId::PositionCoupling => POSITION_COUPLING_RANGE,
            ParameterId::VelocityCoupling => VELOCITY_COUPLING_RANGE,
            ParameterId::ExcitationGain => EXCITATION_GAIN_RANGE,
            ParameterId::OutputGain => OUTPUT_GAIN_RANGE,
            ParameterId::BreathPressure => BREATH_PRESSURE_RANGE,
            ParameterId::BreathNoiseCutoffHz => BREATH_NOISE_CUTOFF_HZ_RANGE,
            ParameterId::BreathFeedback => BREATH_FEEDBACK_RANGE,
            ParameterId::BreathAutoLevel => BREATH_AUTO_LEVEL_RANGE,
            ParameterId::HandpanDecay => HANDPAN_DECAY_RANGE,
            ParameterId::HandpanDetuneCents => HANDPAN_DETUNE_CENTS_RANGE,
            ParameterId::HandpanCoupling => HANDPAN_COUPLING_RANGE,
            ParameterId::HandpanHit => HANDPAN_HIT_RANGE,
            ParameterId::HandpanGlide => HANDPAN_GLIDE_RANGE,
            ParameterId::HandpanFundamental => HANDPAN_PARTIAL_RANGE,
            ParameterId::HandpanOctave => HANDPAN_PARTIAL_RANGE,
            ParameterId::HandpanFifth => HANDPAN_PARTIAL_RANGE,
            ParameterId::HandpanHigherDecay => HANDPAN_PARTIAL_RANGE,
            ParameterId::SupportLevel => SUPPORT_LEVEL_RANGE,
            ParameterId::SupportPulse => SUPPORT_PULSE_RANGE,
            ParameterId::SupportWidth => SUPPORT_WIDTH_RANGE,
            ParameterId::SupportWander => SUPPORT_WANDER_RANGE,
            ParameterId::SupportDrift => SUPPORT_DRIFT_RANGE,
            ParameterId::SupportMotion => SUPPORT_MOTION_RANGE,
            ParameterId::SupportFade => SUPPORT_FADE_RANGE,
            ParameterId::ReverbHandpan => REVERB_HANDPAN_RANGE,
            ParameterId::ReverbSupport => REVERB_SUPPORT_RANGE,
            ParameterId::ReverbDing => REVERB_DING_RANGE,
            ParameterId::ReverbRecent => REVERB_RECENT_RANGE,
            ParameterId::ReverbSecond => REVERB_SECOND_RANGE,
            ParameterId::ReverbBass => REVERB_BASS_RANGE,
            ParameterId::ReverbEcho => REVERB_ECHO_RANGE,
            ParameterId::ReverbDecay => REVERB_DECAY_RANGE,
            ParameterId::ReverbTone => REVERB_TONE_RANGE,
            ParameterId::DingShape => DING_SHAPE_RANGE,
            ParameterId::DingSpread => DING_SPREAD_RANGE,
            ParameterId::DingBrightness => DING_BRIGHTNESS_RANGE,
            ParameterId::DingStrings => DING_STRINGS_RANGE,
            ParameterId::TamburaJawari => TAMBURA_JAWARI_RANGE,
            ParameterId::DingFormant1 | ParameterId::DingFormant2 | ParameterId::DingFormant3 => {
                DING_FORMANT_RANGE
            }
            ParameterId::DingResonance
            | ParameterId::DingQ1
            | ParameterId::DingQ2
            | ParameterId::DingQ3 => DING_RESONANCE_RANGE,
            ParameterId::MixHandpan
            | ParameterId::MixDing
            | ParameterId::MixRecent
            | ParameterId::MixSecond
            | ParameterId::MixBass
            | ParameterId::MixTaps => MIX_LEVEL_RANGE,
            ParameterId::DecoDensity => DECO_DENSITY_RANGE,
            ParameterId::DecoLagBeats => DECO_LAG_BEATS_RANGE,
            ParameterId::PanHandpan
            | ParameterId::PanDing
            | ParameterId::PanRecent
            | ParameterId::PanSecond
            | ParameterId::PanBass
            | ParameterId::PanEcho => PAN_RANGE,
            ParameterId::FormantRate => FORMANT_RATE_RANGE,
            ParameterId::FormantDepth => FORMANT_DEPTH_RANGE,
            ParameterId::GrooveSensitivity => GROOVE_SENSITIVITY_RANGE,
            ParameterId::GroovePhasePull => GROOVE_PHASE_PULL_RANGE,
            ParameterId::GrooveClickLevel => GROOVE_CLICK_LEVEL_RANGE,
            ParameterId::FmBassDecay => FM_BASS_DECAY_RANGE,
            ParameterId::FmBassRatio => FM_BASS_RATIO_RANGE,
            ParameterId::FmBassIndex => FM_BASS_INDEX_RANGE,
            ParameterId::FmBassIndexDecay => FM_BASS_INDEX_DECAY_RANGE,
            ParameterId::FmBassFeedback => FM_BASS_FEEDBACK_RANGE,
            ParameterId::FmBassLevel => FM_BASS_LEVEL_RANGE,
            ParameterId::BassLineAmount => BASS_LINE_AMOUNT_RANGE,
            ParameterId::BassLineVariation => BASS_LINE_VARIATION_RANGE,
            ParameterId::BellsDecay => BELLS_DECAY_RANGE,
            ParameterId::BellsRatio => BELLS_RATIO_RANGE,
            ParameterId::BellsIndex => BELLS_INDEX_RANGE,
            ParameterId::BellsIndexDecay => BELLS_INDEX_DECAY_RANGE,
        }
    }

    fn clamp(self, value: f32) -> f32 {
        let (min, max) = self.range();
        value.clamp(min, max)
    }

    fn slider_range(self) -> std::ops::RangeInclusive<f32> {
        let (min, max) = self.range();
        min..=max
    }

    fn logarithmic(self) -> bool {
        matches!(
            self,
            ParameterId::AttackMs
                | ParameterId::ReleaseMs
                | ParameterId::DampingRatio
                | ParameterId::ExcitationGain
                | ParameterId::OutputGain
                | ParameterId::BreathPressure
                | ParameterId::BreathAutoLevel
                | ParameterId::BreathNoiseCutoffHz
                | ParameterId::HandpanDecay
                | ParameterId::DingFormant1
                | ParameterId::DingFormant2
                | ParameterId::DingFormant3
        )
    }

    fn format_value(self, value: f32) -> String {
        match self {
            ParameterId::AttackMs | ParameterId::ReleaseMs => format!("{value:.1} ms"),
            ParameterId::DampingRatio => format!("{value:.3}"),
            ParameterId::Inharmonicity => format!("{value:.3}×"),
            ParameterId::PositionCoupling | ParameterId::VelocityCoupling => {
                format!("{value:.3}")
            }
            ParameterId::ExcitationGain => format!("{value:.2}×"),
            ParameterId::OutputGain => format!("{value:.2}×"),
            ParameterId::BreathPressure => format!("{value:.2}×"),
            ParameterId::BreathNoiseCutoffHz => format!("{value:.0} Hz"),
            ParameterId::BreathFeedback => format!("{value:.2}"),
            ParameterId::BreathAutoLevel => format!("{value:.2}"),
            ParameterId::HandpanDecay => format!("{value:.2}×"),
            ParameterId::HandpanDetuneCents => format!("{value:.1} cents"),
            ParameterId::HandpanCoupling => format!("{value:.2}"),
            ParameterId::HandpanHit => format!("{value:.2}"),
            ParameterId::HandpanGlide => format!("{value:.2}"),
            ParameterId::HandpanFundamental => format!("{value:.2}"),
            ParameterId::HandpanOctave => format!("{value:.2}"),
            ParameterId::HandpanFifth => format!("{value:.2}"),
            ParameterId::HandpanHigherDecay => format!("{value:.2}"),
            ParameterId::SupportLevel => format!("{value:.2}"),
            ParameterId::SupportPulse => format!("{value:.2}"),
            ParameterId::SupportWidth => format!("{value:.2}"),
            ParameterId::SupportWander => format!("{value:.0} cents"),
            ParameterId::SupportDrift => format!("{value:.0} cents"),
            ParameterId::SupportMotion => format!("{value:.2}×"),
            ParameterId::SupportFade => format!("{value:.1} s"),
            ParameterId::ReverbHandpan
            | ParameterId::ReverbSupport
            | ParameterId::ReverbDing
            | ParameterId::ReverbRecent
            | ParameterId::ReverbSecond
            | ParameterId::ReverbBass
            | ParameterId::ReverbEcho => format!("{value:.2}"),
            ParameterId::ReverbDecay => format!("{value:.1} s"),
            ParameterId::ReverbTone => format!("{value:.2}"),
            ParameterId::DingShape => format!("{value:.2}"),
            ParameterId::DingSpread => format!("{value:.0} cents"),
            ParameterId::DingBrightness => format!("{value:.2}"),
            ParameterId::DingStrings => TamburaBank::string_mode_label(value).to_string(),
            ParameterId::TamburaJawari => format!("{value:.2}"),
            ParameterId::DingFormant1 | ParameterId::DingFormant2 | ParameterId::DingFormant3 => {
                format!("{value:.0} Hz")
            }
            ParameterId::DingResonance
            | ParameterId::DingQ1
            | ParameterId::DingQ2
            | ParameterId::DingQ3 => format!("{value:.1}"),
            ParameterId::MixHandpan
            | ParameterId::MixDing
            | ParameterId::MixRecent
            | ParameterId::MixSecond
            | ParameterId::MixBass
            | ParameterId::MixTaps
            | ParameterId::DecoDensity => format!("{value:.2}"),
            ParameterId::DecoLagBeats => format!("{value:.1} beats"),
            ParameterId::PanHandpan
            | ParameterId::PanDing
            | ParameterId::PanRecent
            | ParameterId::PanSecond
            | ParameterId::PanBass
            | ParameterId::PanEcho => format_pan(value),
            ParameterId::FormantRate => format!("{value:.2} Hz"),
            ParameterId::FormantDepth => format!("{value:.2} oct"),
            ParameterId::GrooveSensitivity
            | ParameterId::GroovePhasePull
            | ParameterId::GrooveClickLevel
            | ParameterId::FmBassDecay
            | ParameterId::FmBassRatio
            | ParameterId::FmBassIndex
            | ParameterId::FmBassIndexDecay
            | ParameterId::FmBassFeedback
            | ParameterId::FmBassLevel
            | ParameterId::BassLineAmount
            | ParameterId::BassLineVariation
            | ParameterId::BellsDecay
            | ParameterId::BellsRatio
            | ParameterId::BellsIndex
            | ParameterId::BellsIndexDecay => format!("{value:.2}"),
        }
    }

    fn value_from_cc(self, cc_value: u8) -> f32 {
        let norm = (cc_value as f32 / 127.0).clamp(0.0, 1.0);
        let (min, max) = self.range();
        min + (max - min) * norm
    }

    fn storage_key(self) -> &'static str {
        match self {
            ParameterId::AttackMs => "attack_ms",
            ParameterId::ReleaseMs => "release_ms",
            ParameterId::DampingRatio => "damping_ratio",
            ParameterId::Inharmonicity => "inharmonicity",
            ParameterId::PositionCoupling => "position_coupling",
            ParameterId::VelocityCoupling => "velocity_coupling",
            ParameterId::ExcitationGain => "excitation_gain",
            ParameterId::OutputGain => "output_gain",
            ParameterId::BreathPressure => "breath_pressure",
            ParameterId::BreathNoiseCutoffHz => "breath_noise_cutoff_hz",
            ParameterId::BreathFeedback => "breath_feedback",
            ParameterId::BreathAutoLevel => "breath_auto_level",
            ParameterId::HandpanDecay => "handpan_decay",
            ParameterId::HandpanDetuneCents => "handpan_detune_cents",
            ParameterId::HandpanCoupling => "handpan_coupling",
            ParameterId::HandpanHit => "handpan_hit",
            ParameterId::HandpanGlide => "handpan_glide",
            ParameterId::HandpanFundamental => "handpan_fundamental",
            ParameterId::HandpanOctave => "handpan_octave",
            ParameterId::HandpanFifth => "handpan_fifth",
            ParameterId::HandpanHigherDecay => "handpan_higher_decay",
            ParameterId::SupportLevel => "support_level",
            ParameterId::SupportPulse => "support_pulse",
            ParameterId::SupportWidth => "support_width",
            ParameterId::SupportWander => "support_wander",
            ParameterId::SupportDrift => "support_drift",
            ParameterId::SupportMotion => "support_motion",
            ParameterId::SupportFade => "support_fade",
            ParameterId::ReverbHandpan => "reverb_handpan",
            ParameterId::ReverbSupport => "reverb_support",
            ParameterId::ReverbDing => "reverb_ding",
            ParameterId::ReverbRecent => "reverb_recent",
            ParameterId::ReverbSecond => "reverb_second",
            ParameterId::ReverbBass => "reverb_bass",
            ParameterId::ReverbEcho => "reverb_echo",
            ParameterId::ReverbDecay => "reverb_decay",
            ParameterId::ReverbTone => "reverb_tone",
            ParameterId::DingShape => "ding_shape",
            ParameterId::DingSpread => "ding_spread",
            ParameterId::DingBrightness => "ding_brightness",
            ParameterId::DingStrings => "ding_strings",
            ParameterId::TamburaJawari => "tambura_jawari",
            ParameterId::DingFormant1 => "ding_formant_1",
            ParameterId::DingFormant2 => "ding_formant_2",
            ParameterId::DingFormant3 => "ding_formant_3",
            ParameterId::DingResonance => "ding_resonance",
            ParameterId::DingQ1 => "ding_q_1",
            ParameterId::DingQ2 => "ding_q_2",
            ParameterId::DingQ3 => "ding_q_3",
            ParameterId::MixHandpan => "mix_handpan",
            ParameterId::MixDing => "mix_ding",
            ParameterId::MixRecent => "mix_recent",
            ParameterId::MixSecond => "mix_second",
            ParameterId::MixBass => "mix_bass",
            ParameterId::MixTaps => "mix_taps",
            ParameterId::DecoDensity => "deco_density",
            ParameterId::DecoLagBeats => "deco_lag_beats",
            ParameterId::PanHandpan => "pan_handpan",
            ParameterId::PanDing => "pan_ding",
            ParameterId::PanRecent => "pan_recent",
            ParameterId::PanSecond => "pan_second",
            ParameterId::PanBass => "pan_bass",
            ParameterId::PanEcho => "pan_echo",
            ParameterId::FormantRate => "formant_rate",
            ParameterId::FormantDepth => "formant_depth",
            ParameterId::GrooveSensitivity => "groove_sensitivity",
            ParameterId::GroovePhasePull => "groove_phase_pull",
            ParameterId::GrooveClickLevel => "groove_click_level",
            ParameterId::FmBassDecay => "fm_bass_decay",
            ParameterId::FmBassRatio => "fm_bass_ratio",
            ParameterId::FmBassIndex => "fm_bass_index",
            ParameterId::FmBassIndexDecay => "fm_bass_index_decay",
            ParameterId::FmBassFeedback => "fm_bass_feedback",
            ParameterId::FmBassLevel => "fm_bass_level",
            ParameterId::BassLineAmount => "bass_line_amount",
            ParameterId::BassLineVariation => "bass_line_variation",
            ParameterId::BellsDecay => "bells_decay",
            ParameterId::BellsRatio => "bells_ratio",
            ParameterId::BellsIndex => "bells_index",
            ParameterId::BellsIndexDecay => "bells_index_decay",
        }
    }

    fn from_storage_key(key: &str) -> Option<Self> {
        match key {
            "attack_ms" => Some(ParameterId::AttackMs),
            "release_ms" => Some(ParameterId::ReleaseMs),
            "damping_ratio" => Some(ParameterId::DampingRatio),
            "inharmonicity" => Some(ParameterId::Inharmonicity),
            "position_coupling" => Some(ParameterId::PositionCoupling),
            "velocity_coupling" => Some(ParameterId::VelocityCoupling),
            "excitation_gain" => Some(ParameterId::ExcitationGain),
            "output_gain" => Some(ParameterId::OutputGain),
            "breath_pressure" => Some(ParameterId::BreathPressure),
            "breath_noise_cutoff_hz" => Some(ParameterId::BreathNoiseCutoffHz),
            "breath_feedback" => Some(ParameterId::BreathFeedback),
            "breath_auto_level" => Some(ParameterId::BreathAutoLevel),
            "handpan_decay" => Some(ParameterId::HandpanDecay),
            "handpan_detune_cents" => Some(ParameterId::HandpanDetuneCents),
            "handpan_coupling" => Some(ParameterId::HandpanCoupling),
            "handpan_hit" => Some(ParameterId::HandpanHit),
            "handpan_glide" => Some(ParameterId::HandpanGlide),
            "handpan_fundamental" => Some(ParameterId::HandpanFundamental),
            "handpan_octave" => Some(ParameterId::HandpanOctave),
            "handpan_fifth" => Some(ParameterId::HandpanFifth),
            "handpan_higher_decay" => Some(ParameterId::HandpanHigherDecay),
            "support_level" => Some(ParameterId::SupportLevel),
            "support_pulse" => Some(ParameterId::SupportPulse),
            "support_width" => Some(ParameterId::SupportWidth),
            "support_wander" => Some(ParameterId::SupportWander),
            "support_drift" => Some(ParameterId::SupportDrift),
            "support_motion" => Some(ParameterId::SupportMotion),
            "support_fade" => Some(ParameterId::SupportFade),
            "reverb_handpan" => Some(ParameterId::ReverbHandpan),
            "reverb_support" => Some(ParameterId::ReverbSupport),
            "reverb_ding" => Some(ParameterId::ReverbDing),
            "reverb_recent" => Some(ParameterId::ReverbRecent),
            "reverb_second" => Some(ParameterId::ReverbSecond),
            "reverb_bass" => Some(ParameterId::ReverbBass),
            "reverb_echo" => Some(ParameterId::ReverbEcho),
            "reverb_decay" => Some(ParameterId::ReverbDecay),
            "reverb_tone" => Some(ParameterId::ReverbTone),
            "ding_shape" => Some(ParameterId::DingShape),
            "ding_spread" => Some(ParameterId::DingSpread),
            "ding_brightness" => Some(ParameterId::DingBrightness),
            "ding_strings" => Some(ParameterId::DingStrings),
            "tambura_jawari" => Some(ParameterId::TamburaJawari),
            "ding_formant_1" => Some(ParameterId::DingFormant1),
            "ding_formant_2" => Some(ParameterId::DingFormant2),
            "ding_formant_3" => Some(ParameterId::DingFormant3),
            "ding_resonance" => Some(ParameterId::DingResonance),
            "ding_q_1" => Some(ParameterId::DingQ1),
            "ding_q_2" => Some(ParameterId::DingQ2),
            "ding_q_3" => Some(ParameterId::DingQ3),
            "mix_handpan" => Some(ParameterId::MixHandpan),
            "mix_ding" => Some(ParameterId::MixDing),
            "mix_recent" => Some(ParameterId::MixRecent),
            "mix_second" => Some(ParameterId::MixSecond),
            "mix_bass" => Some(ParameterId::MixBass),
            "mix_taps" => Some(ParameterId::MixTaps),
            "echo_feedback" | "deco_density" => Some(ParameterId::DecoDensity),
            "deco_lag_beats" => Some(ParameterId::DecoLagBeats),
            "pan_handpan" => Some(ParameterId::PanHandpan),
            "pan_ding" => Some(ParameterId::PanDing),
            "pan_recent" => Some(ParameterId::PanRecent),
            "pan_second" => Some(ParameterId::PanSecond),
            "pan_bass" => Some(ParameterId::PanBass),
            "pan_echo" => Some(ParameterId::PanEcho),
            "formant_rate" => Some(ParameterId::FormantRate),
            "formant_depth" => Some(ParameterId::FormantDepth),
            "groove_sensitivity" => Some(ParameterId::GrooveSensitivity),
            "groove_phase_pull" => Some(ParameterId::GroovePhasePull),
            "groove_click_level" => Some(ParameterId::GrooveClickLevel),
            "fm_bass_decay" => Some(ParameterId::FmBassDecay),
            "fm_bass_ratio" => Some(ParameterId::FmBassRatio),
            "fm_bass_index" => Some(ParameterId::FmBassIndex),
            "fm_bass_index_decay" => Some(ParameterId::FmBassIndexDecay),
            "fm_bass_feedback" => Some(ParameterId::FmBassFeedback),
            "fm_bass_level" => Some(ParameterId::FmBassLevel),
            "bass_line_amount" => Some(ParameterId::BassLineAmount),
            "bass_line_variation" => Some(ParameterId::BassLineVariation),
            "bells_decay" => Some(ParameterId::BellsDecay),
            "bells_ratio" => Some(ParameterId::BellsRatio),
            "bells_index" => Some(ParameterId::BellsIndex),
            "bells_index_decay" => Some(ParameterId::BellsIndexDecay),
            _ => None,
        }
    }
}

fn format_pan(value: f32) -> String {
    if (value - 0.5).abs() < 0.01 {
        "C".to_string()
    } else if value < 0.5 {
        format!("L {:.0}", (0.5 - value) * 200.0)
    } else {
        format!("R {:.0}", (value - 0.5) * 200.0)
    }
}

#[derive(Debug)]
pub struct MidiLearnState {
    pending: AtomicI8,
    assignments: Mutex<HashMap<ParameterId, u8>>,
}

impl MidiLearnState {
    fn new() -> Self {
        Self {
            pending: AtomicI8::new(-1),
            assignments: Mutex::new(HashMap::new()),
        }
    }

    fn load_assignments(&self, pairs: &[(ParameterId, u8)]) {
        let mut assignments = self.assignments.lock();
        assignments.clear();
        for (param, cc) in pairs {
            assignments.insert(*param, *cc);
        }
    }

    fn request(&self, param: ParameterId) {
        self.pending.store(param.as_i8(), Ordering::Relaxed);
    }

    fn pending(&self) -> Option<ParameterId> {
        ParameterId::from_i8(self.pending.load(Ordering::Relaxed))
    }

    fn take_pending(&self) -> Option<ParameterId> {
        ParameterId::from_i8(self.pending.swap(-1, Ordering::Relaxed))
    }

    fn set_mapping(&self, param: ParameterId, cc: u8) {
        let mut assignments = self.assignments.lock();
        assignments.insert(param, cc);
    }

    fn mapping_for(&self, param: ParameterId) -> Option<u8> {
        self.assignments.lock().get(&param).copied()
    }

    fn clear_mapping(&self, param: ParameterId) -> bool {
        let mut assignments = self.assignments.lock();
        assignments.remove(&param).is_some()
    }

    fn param_for_cc(&self, cc: u8) -> Option<ParameterId> {
        self.assignments
            .lock()
            .iter()
            .find_map(|(&param, &mapped_cc)| if mapped_cc == cc { Some(param) } else { None })
    }
}

struct MidiContext {
    control: Arc<ControlBlock>,
    midi_learn: Arc<MidiLearnState>,
    settings: Arc<SettingsStore>,
}

impl MidiContext {
    fn new(
        control: Arc<ControlBlock>,
        midi_learn: Arc<MidiLearnState>,
        settings: Arc<SettingsStore>,
    ) -> Self {
        Self {
            control,
            midi_learn,
            settings,
        }
    }
}

#[derive(Debug, Default)]
struct CliOptions {
    list_midi: bool,
    midi_port: Option<usize>,
    sample_rate_hz: Option<u32>,
    buffer_frames: Option<u32>,
}

fn parse_cli() -> anyhow::Result<CliOptions> {
    let mut opts = CliOptions::default();
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--list-midi" => opts.list_midi = true,
            "--midi-port" => {
                let value = args
                    .next()
                    .context("--midi-port requires an index value (e.g. --midi-port 1)")?;
                opts.midi_port = Some(parse_port_index(&value)?);
            }
            "--sample-rate" => {
                let value = args
                    .next()
                    .context("--sample-rate requires a value in Hz (e.g. --sample-rate 48000)")?;
                opts.sample_rate_hz = Some(parse_positive_u32("sample rate", &value)?);
            }
            "--buffer-frames" => {
                let value = args
                    .next()
                    .context("--buffer-frames requires a frame count (e.g. --buffer-frames 128)")?;
                opts.buffer_frames = Some(parse_positive_u32("buffer frames", &value)?);
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ if arg.starts_with("--midi-port=") => {
                let value = &arg["--midi-port=".len()..];
                opts.midi_port = Some(parse_port_index(value)?);
            }
            _ if arg.starts_with("--sample-rate=") => {
                let value = &arg["--sample-rate=".len()..];
                opts.sample_rate_hz = Some(parse_positive_u32("sample rate", value)?);
            }
            _ if arg.starts_with("--buffer-frames=") => {
                let value = &arg["--buffer-frames=".len()..];
                opts.buffer_frames = Some(parse_positive_u32("buffer frames", value)?);
            }
            other => {
                anyhow::bail!("Unknown argument: {other}. Run with --help for usage.");
            }
        }
    }

    Ok(opts)
}

fn parse_port_index(value: &str) -> anyhow::Result<usize> {
    value
        .parse::<usize>()
        .with_context(|| format!("Invalid MIDI port index: {value}"))
}

fn parse_positive_u32(label: &str, value: &str) -> anyhow::Result<u32> {
    let parsed = value
        .parse::<u32>()
        .with_context(|| format!("Invalid {label} value: {value}"))?;
    anyhow::ensure!(parsed > 0, "{label} must be > 0");
    Ok(parsed)
}

fn print_usage() {
    println!("squawk-app usage:");
    println!("  cargo run -p squawk-app --release -- [OPTIONS]\n");
    println!("Options:");
    println!("  --list-midi           List available MIDI inputs and exit");
    println!("  --midi-port <index>   Force a specific MIDI input (0-based)");
    println!("  --sample-rate <Hz>    Request a specific sample rate (default 48000)");
    println!("  --buffer-frames <N>   Request buffer size in frames (default 128)");
    println!("  -h, --help            Show this help text");
}

fn main() -> anyhow::Result<()> {
    let cli = parse_cli()?;

    if cli.list_midi {
        list_midi_ports()?;
        return Ok(());
    }

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .context("No default output audio device available")?;

    let requested_sample_rate = cli.sample_rate_hz.unwrap_or(48_000);
    let supported_config = choose_supported_config(&device, requested_sample_rate)
        .or_else(|_| device.default_output_config())
        .context("Failed to query output config")?;
    let sample_format = supported_config.sample_format();

    let mut config: cpal::StreamConfig = supported_config.into();
    let channels = config.channels as usize;

    // For sub-10ms targets, you'll typically want fixed buffers like 64/128.
    // Not all backends/devices will accept this; we fall back if stream creation fails.
    if let Some(buffer_frames) = cli.buffer_frames {
        config.buffer_size = cpal::BufferSize::Fixed(buffer_frames);
    } else {
        config.buffer_size = cpal::BufferSize::Fixed(128);
    }

    println!(
        "Output device: {}",
        device.name().unwrap_or_else(|_| "<unknown>".to_string())
    );
    println!(
        "Config request: {} Hz, {} channels, {:?}, buffer {:?}",
        config.sample_rate.0, config.channels, sample_format, config.buffer_size
    );

    let settings = Arc::new(SettingsStore::load()?);
    let snapshot = settings.snapshot();

    let params = EngineParams::default();
    let control = Arc::new(ControlBlock::new(params));
    control.set_attack_ms(snapshot.attack_ms);
    control.set_release_ms(snapshot.release_ms);
    control.set_damping_ratio(snapshot.damping_ratio);
    control.set_inharmonicity(snapshot.inharmonicity);
    control.set_position_coupling(snapshot.position_coupling);
    control.set_velocity_coupling(snapshot.velocity_coupling);
    control.set_excitation_gain(snapshot.excitation_gain);
    control.set_output_gain(snapshot.output_gain);
    control.set_exciter_mode_u8(snapshot.exciter_mode);
    control.set_breath_pressure(snapshot.breath_pressure);
    control.set_breath_noise_cutoff_hz(snapshot.breath_noise_cutoff_hz);
    control.set_breath_feedback(snapshot.breath_feedback);
    control.set_breath_auto_level(snapshot.breath_auto_level);
    control.set_instrument_u8(snapshot.instrument);
    control.set_handpan_layout(
        snapshot.handpan_scale,
        snapshot.handpan_ding,
        snapshot.handpan_tuning,
    );
    control.set_parameter_value(ParameterId::HandpanDecay, snapshot.handpan_decay);
    control.set_parameter_value(
        ParameterId::HandpanDetuneCents,
        snapshot.handpan_detune_cents,
    );
    control.set_parameter_value(ParameterId::HandpanCoupling, snapshot.handpan_coupling);
    control.set_parameter_value(ParameterId::HandpanHit, snapshot.handpan_hit);
    control.set_parameter_value(ParameterId::HandpanGlide, snapshot.handpan_glide);
    control.set_parameter_value(
        ParameterId::HandpanFundamental,
        snapshot.handpan_fundamental,
    );
    control.set_parameter_value(ParameterId::HandpanOctave, snapshot.handpan_octave);
    control.set_parameter_value(ParameterId::HandpanFifth, snapshot.handpan_fifth);
    control.set_parameter_value(
        ParameterId::HandpanHigherDecay,
        snapshot.handpan_higher_decay,
    );
    control.set_parameter_value(ParameterId::SupportLevel, snapshot.support_level);
    control.set_parameter_value(ParameterId::SupportPulse, snapshot.support_pulse);
    control.set_parameter_value(ParameterId::SupportWidth, snapshot.support_width);
    control.set_parameter_value(ParameterId::SupportWander, snapshot.support_wander);
    control.set_parameter_value(ParameterId::SupportDrift, snapshot.support_drift);
    control.set_parameter_value(ParameterId::SupportMotion, snapshot.support_motion);
    control.set_parameter_value(ParameterId::SupportFade, snapshot.support_fade);
    control.set_parameter_value(ParameterId::ReverbHandpan, snapshot.reverb_handpan);
    control.set_parameter_value(ParameterId::ReverbSupport, snapshot.reverb_support);
    control.set_parameter_value(ParameterId::ReverbDing, snapshot.reverb_ding);
    control.set_parameter_value(ParameterId::ReverbRecent, snapshot.reverb_recent);
    control.set_parameter_value(ParameterId::ReverbSecond, snapshot.reverb_second);
    control.set_parameter_value(ParameterId::ReverbBass, snapshot.reverb_bass);
    control.set_parameter_value(ParameterId::ReverbEcho, snapshot.reverb_echo);
    control.set_parameter_value(ParameterId::ReverbDecay, snapshot.reverb_decay);
    control.set_parameter_value(ParameterId::ReverbTone, snapshot.reverb_tone);
    control.set_echo_instrument_u8(snapshot.echo_instrument);
    control.set_support_parts(snapshot.support_muted, snapshot.support_solo);
    control.set_handpan_mute_solo(snapshot.handpan_muted, snapshot.handpan_solo);
    control.set_ding_wash(snapshot.ding_wash);
    control.set_parameter_value(ParameterId::DingShape, snapshot.ding_shape);
    control.set_parameter_value(ParameterId::DingSpread, snapshot.ding_spread);
    control.set_parameter_value(ParameterId::DingBrightness, snapshot.ding_brightness);
    control.set_parameter_value(ParameterId::DingStrings, snapshot.ding_strings);
    control.set_parameter_value(ParameterId::TamburaJawari, snapshot.tambura_jawari);
    control.set_parameter_value(ParameterId::DingFormant1, snapshot.ding_formant_1);
    control.set_parameter_value(ParameterId::DingFormant2, snapshot.ding_formant_2);
    control.set_parameter_value(ParameterId::DingFormant3, snapshot.ding_formant_3);
    control.set_parameter_value(ParameterId::DingResonance, snapshot.ding_resonance);
    control.set_parameter_value(ParameterId::DingQ1, snapshot.ding_q_1);
    control.set_parameter_value(ParameterId::DingQ2, snapshot.ding_q_2);
    control.set_parameter_value(ParameterId::DingQ3, snapshot.ding_q_3);
    control.set_parameter_value(ParameterId::MixHandpan, snapshot.mix_handpan);
    control.set_parameter_value(ParameterId::MixDing, snapshot.mix_ding);
    control.set_parameter_value(ParameterId::MixRecent, snapshot.mix_recent);
    control.set_parameter_value(ParameterId::MixSecond, snapshot.mix_second);
    control.set_parameter_value(ParameterId::MixBass, snapshot.mix_bass);
    control.set_parameter_value(ParameterId::MixTaps, snapshot.mix_taps);
    control.set_parameter_value(ParameterId::DecoDensity, snapshot.deco_density);
    control.set_parameter_value(ParameterId::DecoLagBeats, snapshot.deco_lag_beats);
    control.set_parameter_value(ParameterId::PanHandpan, snapshot.pan_handpan);
    control.set_parameter_value(ParameterId::PanDing, snapshot.pan_ding);
    control.set_parameter_value(ParameterId::PanRecent, snapshot.pan_recent);
    control.set_parameter_value(ParameterId::PanSecond, snapshot.pan_second);
    control.set_parameter_value(ParameterId::PanBass, snapshot.pan_bass);
    control.set_parameter_value(ParameterId::PanEcho, snapshot.pan_echo);
    control.set_parameter_value(ParameterId::FormantRate, snapshot.formant_rate);
    control.set_parameter_value(ParameterId::FormantDepth, snapshot.formant_depth);
    control.set_parameter_value(ParameterId::GrooveSensitivity, snapshot.groove_sensitivity);
    control.set_parameter_value(ParameterId::GroovePhasePull, snapshot.groove_phase_pull);
    control.set_parameter_value(ParameterId::GrooveClickLevel, snapshot.groove_click_level);
    control.set_parameter_value(ParameterId::FmBassDecay, snapshot.fm_bass_decay);
    control.set_parameter_value(ParameterId::FmBassRatio, snapshot.fm_bass_ratio);
    control.set_parameter_value(ParameterId::FmBassIndex, snapshot.fm_bass_index);
    control.set_parameter_value(ParameterId::FmBassIndexDecay, snapshot.fm_bass_index_decay);
    control.set_parameter_value(ParameterId::FmBassFeedback, snapshot.fm_bass_feedback);
    control.set_parameter_value(ParameterId::FmBassLevel, snapshot.fm_bass_level);
    control.set_parameter_value(ParameterId::BassLineAmount, snapshot.bass_line_amount);
    control.set_parameter_value(ParameterId::BassLineVariation, snapshot.bass_line_variation);
    control.set_parameter_value(ParameterId::BellsDecay, snapshot.bells_decay);
    control.set_parameter_value(ParameterId::BellsRatio, snapshot.bells_ratio);
    control.set_parameter_value(ParameterId::BellsIndex, snapshot.bells_index);
    control.set_parameter_value(ParameterId::BellsIndexDecay, snapshot.bells_index_decay);
    control.set_groove_click_enabled(snapshot.groove_click_enabled);
    control.set_groove_auto_lock(snapshot.groove_auto_lock);

    let midi_learn = Arc::new(MidiLearnState::new());
    midi_learn.load_assignments(&snapshot.midi_assignments);

    // Analysis tap: lock-free ring buffer for UI-side inspection (pitch tracking, etc).
    // ~0.25s is plenty for low-latency trackers and small plots.
    let sample_rate_hz = config.sample_rate.0 as f32;
    let tap_len = (sample_rate_hz * 0.25) as usize;
    let audio_tap = Arc::new(AudioTap::new(sample_rate_hz, tap_len));

    // Start MIDI input on a non-audio thread. The callback only touches atomics.
    // Keep the connection in scope so it stays active.
    let _midi_conn = match start_midi(
        control.clone(),
        midi_learn.clone(),
        settings.clone(),
        cli.midi_port,
    ) {
        Ok(conn) => conn,
        Err(err) => {
            eprintln!("MIDI disabled: {err}");
            None
        }
    };

    let stream = match sample_format {
        cpal::SampleFormat::F32 => build_stream::<f32>(
            &device,
            &config,
            channels,
            params,
            control.clone(),
            audio_tap.clone(),
        )?,
        cpal::SampleFormat::I16 => build_stream::<i16>(
            &device,
            &config,
            channels,
            params,
            control.clone(),
            audio_tap.clone(),
        )?,
        cpal::SampleFormat::U16 => build_stream::<u16>(
            &device,
            &config,
            channels,
            params,
            control.clone(),
            audio_tap.clone(),
        )?,
        other => anyhow::bail!("Unsupported sample format: {other:?}"),
    };

    stream.play().context("Failed to start audio stream")?;
    println!("Running. Close the UI window or hit Ctrl+C to quit.");
    println!("Tip: pass --list-midi / --midi-port / --buffer-frames / --sample-rate as needed.");

    ui::launch(
        control.clone(),
        midi_learn.clone(),
        settings.clone(),
        audio_tap.clone(),
    )?;

    Ok(())
}

fn list_midi_ports() -> anyhow::Result<()> {
    let mut midi_in = MidiInput::new("squawk")?;
    midi_in.ignore(Ignore::None);

    let in_ports = midi_in.ports();
    if in_ports.is_empty() {
        println!("No MIDI inputs found.");
        return Ok(());
    }

    println!("MIDI inputs:");
    for (idx, port) in in_ports.iter().enumerate() {
        let name = midi_in
            .port_name(port)
            .unwrap_or_else(|_| "<unknown>".to_string());
        println!("  [{idx}] {name}");
    }

    Ok(())
}

fn start_midi(
    control: Arc<ControlBlock>,
    midi_learn: Arc<MidiLearnState>,
    settings: Arc<SettingsStore>,
    midi_port_override: Option<usize>,
) -> anyhow::Result<Option<midir::MidiInputConnection<MidiContext>>> {
    let mut midi_in = MidiInput::new("squawk")?;
    midi_in.ignore(Ignore::None);

    let in_ports = midi_in.ports();
    if in_ports.is_empty() {
        println!("No MIDI input ports found (audio will still run).");
        println!("(Plug in a controller, or enable an IAC bus on macOS.)");
        return Ok(None);
    }

    if midi_port_override.is_none() {
        println!("MIDI inputs:");
        for (idx, port) in in_ports.iter().enumerate() {
            let name = midi_in
                .port_name(port)
                .unwrap_or_else(|_| "<unknown>".to_string());
            println!("  [{idx}] {name}");
        }
    }

    let selected_idx = midi_port_override
        .or_else(|| {
            env::var("MIDI_PORT")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
        })
        .unwrap_or(0)
        .min(in_ports.len() - 1);
    let selected_port = &in_ports[selected_idx];
    let selected_name = midi_in
        .port_name(selected_port)
        .unwrap_or_else(|_| "<unknown>".to_string());

    println!("Connecting MIDI input [{selected_idx}] {selected_name}");
    let ctx = MidiContext::new(control, midi_learn, settings);
    Ok(Some(midi_in.connect(
        selected_port,
        "squawk-midi-in",
        midi_callback,
        ctx,
    )?))
}

fn midi_callback(_timestamp_ms: u64, message: &[u8], ctx: &mut MidiContext) {
    // Basic channel voice messages only.
    if message.is_empty() {
        return;
    }

    let status = message[0] & 0xF0;

    // Note On: 0x90, Note Off: 0x80
    match status {
        0x90 => {
            if message.len() < 3 {
                return;
            }
            let note = message[1];
            let vel = message[2];
            if vel == 0 {
                note_off(&ctx.control, note);
            } else {
                note_on(&ctx.control, note, vel, 0.0);
            }
        }
        0x80 => {
            if message.len() < 3 {
                return;
            }
            let note = message[1];
            note_off(&ctx.control, note);
        }
        0xB0 => {
            if message.len() < 3 {
                return;
            }
            let controller = message[1];
            let value = message[2];
            handle_cc(ctx, controller, value);
        }
        _ => {}
    }
}

fn handle_cc(ctx: &MidiContext, controller: u8, value: u8) {
    if let Some(param) = ctx.midi_learn.take_pending() {
        ctx.midi_learn.set_mapping(param, controller);
        println!("Mapped {} to CC {controller}", param.label());
        if let Err(err) = ctx.settings.update_mapping(param, controller) {
            eprintln!("Warning: Failed to persist mapping: {err}");
        }
    }

    if let Some(param) = ctx.midi_learn.param_for_cc(controller) {
        let mapped_value = param.value_from_cc(value);
        ctx.control.set_parameter_value(param, mapped_value);
        if let Err(err) = ctx.settings.update_parameter(param, mapped_value) {
            eprintln!("Warning: Failed to persist parameter update: {err}");
        }
    }
}

#[inline]
fn note_on(control: &Arc<ControlBlock>, note: u8, velocity: u8, place: f32) {
    let freq = midi_note_to_hz(note);
    let amp = (velocity as f32 / 127.0) * 0.25;
    let place = place.clamp(0.0, 1.0);

    control.active_note.store(note, Ordering::Relaxed);
    control.last_note_on_note.store(note, Ordering::Relaxed);
    control
        .last_note_on_hz_bits
        .store(freq.to_bits(), Ordering::Relaxed);
    control.last_note_on_counter.fetch_add(1, Ordering::Relaxed);
    control
        .frequency_bits
        .store(freq.to_bits(), Ordering::Relaxed);
    control
        .amplitude_bits
        .store(amp.to_bits(), Ordering::Relaxed);
    control.gate.store(true, Ordering::Relaxed);
    let slot = control.trigger_counter.load(Ordering::Relaxed) as usize % STRIKE_QUEUE_LEN;
    control.strike_notes[slot].store(note, Ordering::Relaxed);
    control.strike_amps[slot].store(amp.to_bits(), Ordering::Relaxed);
    control.strike_places[slot].store(place.to_bits(), Ordering::Relaxed);
    control.trigger_counter.fetch_add(1, Ordering::Release);
}

#[inline]
fn note_off(control: &Arc<ControlBlock>, note: u8) {
    // Simple monophonic behavior: only release if this is the active note.
    let active = control.active_note.load(Ordering::Relaxed);
    if active == note {
        control.gate.store(false, Ordering::Relaxed);
    }
}

#[inline]
fn midi_note_to_hz(note: u8) -> f32 {
    // A4 = MIDI 69 = 440Hz
    let semitones = note as f32 - 69.0;
    440.0 * (2.0_f32).powf(semitones / 12.0)
}

fn choose_supported_config(
    device: &cpal::Device,
    sample_rate_hz: u32,
) -> anyhow::Result<cpal::SupportedStreamConfig> {
    let mut best: Option<(i32, cpal::SupportedStreamConfig)> = None;

    for range in device.supported_output_configs()? {
        if range.min_sample_rate().0 > sample_rate_hz || range.max_sample_rate().0 < sample_rate_hz
        {
            continue;
        }

        let cfg = range.with_sample_rate(cpal::SampleRate(sample_rate_hz));
        let score = config_score(&cfg);

        match &best {
            Some((best_score, _)) if *best_score >= score => {}
            _ => best = Some((score, cfg)),
        }
    }

    best.map(|(_, cfg)| cfg)
        .context("No supported output config matches requested sample rate")
}

fn config_score(cfg: &cpal::SupportedStreamConfig) -> i32 {
    let channels_score = match cfg.channels() {
        2 => 1000,
        1 => 500,
        _ => 0,
    };

    let format_score = match cfg.sample_format() {
        cpal::SampleFormat::F32 => 30,
        cpal::SampleFormat::I16 => 20,
        cpal::SampleFormat::U16 => 10,
        _ => 0,
    };

    channels_score + format_score
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    params: EngineParams,
    control: Arc<ControlBlock>,
    audio_tap: Arc<AudioTap>,
) -> anyhow::Result<cpal::Stream>
where
    T: FromF32 + cpal::SizedSample + Send + 'static,
{
    let err_fn = |err| eprintln!("stream error: {err}");

    let requested_buffer = config.buffer_size;

    let try_build = |cfg: &cpal::StreamConfig| {
        let mut engine = Engine::new(cfg.sample_rate.0 as f32, params);
        let mut handpan = Handpan::new(cfg.sample_rate.0 as f32);
        let mut fm_bass = FmBass::new(cfg.sample_rate.0 as f32);
        let mut bass_line = BassLine::new(cfg.sample_rate.0 as f32);
        let mut bells = Bells::new(cfg.sample_rate.0 as f32);
        let mut resonator = DroneVoice::new(cfg.sample_rate.0 as f32, DroneVoiceKind::Resonator);
        let mut waveshape = DroneVoice::new(cfg.sample_rate.0 as f32, DroneVoiceKind::Waveshape);
        let mut tambura = DroneVoice::new(cfg.sample_rate.0 as f32, DroneVoiceKind::Tambura);
        let mut support = Support::new(cfg.sample_rate.0 as f32);
        let mut groove = GrooveTracker::new(cfg.sample_rate.0 as f32);
        let mut play_memory = PlayMemory::new(cfg.sample_rate.0 as f32);
        let mut decorations = Decorations::new(cfg.sample_rate.0 as f32);
        let mut reverb = Reverb::new(cfg.sample_rate.0 as f32);
        let control = control.clone();
        let audio_tap = audio_tap.clone();
        let mut trigger_seen = control.trigger_counter();
        let mut rendered_instrument = control.instrument();
        let sample_rate_hz = cfg.sample_rate.0 as f32;
        device.build_output_stream(
            cfg,
            move |data: &mut [T], _info: &cpal::OutputCallbackInfo| {
                // Realtime-safe: just atomic loads + pure math.
                let snapshot = control.snapshot();
                let (strikes, write, strike_count) = control.drain_strikes(trigger_seen);
                let instrument_changed = snapshot.instrument != rendered_instrument;
                if instrument_changed {
                    rendered_instrument = snapshot.instrument;
                }
                let triggered = !instrument_changed && trigger_seen != write;
                trigger_seen = write;

                let block_peak = if snapshot.instrument == Instrument::Handpan.as_u8() {
                    handpan.set_decay(snapshot.handpan_decay);
                    handpan.set_detune_cents(snapshot.handpan_detune_cents);
                    handpan.set_coupling(snapshot.handpan_coupling);
                    handpan.set_hit(snapshot.handpan_hit);
                    handpan.set_glide(snapshot.handpan_glide);
                    handpan.set_layout(
                        snapshot.handpan_scale,
                        snapshot.handpan_ding,
                        snapshot.handpan_tuning,
                    );
                    handpan.set_fundamental(snapshot.handpan_fundamental);
                    handpan.set_octave(snapshot.handpan_octave);
                    handpan.set_fifth(snapshot.handpan_fifth);
                    handpan.set_higher_decay(snapshot.handpan_higher_decay);
                    // Bake today's Support level so part fader 1 keeps the same balance.
                    support.set_level(DEFAULT_SUPPORT_LEVEL);
                    support.set_pulse(snapshot.support_pulse);
                    support.set_width(snapshot.support_width);
                    support.set_wander(snapshot.support_wander);
                    support.set_drift(snapshot.support_drift);
                    support.set_motion(snapshot.support_motion);
                    support.set_fade(snapshot.support_fade);
                    support.set_parts(snapshot.support_muted, snapshot.support_solo);
                    support.set_foreign_solo(snapshot.handpan_solo);
                    support.set_ding_wash(snapshot.ding_wash);
                    support.set_ding_shape(snapshot.ding_shape);
                    support.set_ding_spread(snapshot.ding_spread);
                    support.set_ding_brightness(snapshot.ding_brightness);
                    support.set_ding_strings(snapshot.ding_strings);
                    support.set_tambura_jawari(snapshot.tambura_jawari);
                    support.set_ding_formant(0, snapshot.ding_formant_1);
                    support.set_ding_formant(1, snapshot.ding_formant_2);
                    support.set_ding_formant(2, snapshot.ding_formant_3);
                    support.set_ding_formant_q(0, snapshot.ding_q_1);
                    support.set_ding_formant_q(1, snapshot.ding_q_2);
                    support.set_ding_formant_q(2, snapshot.ding_q_3);
                    support.set_formant_rate(snapshot.formant_rate);
                    support.set_formant_depth(snapshot.formant_depth);
                    support.set_part_level(0, snapshot.mix_ding);
                    support.set_part_level(1, snapshot.mix_recent);
                    support.set_part_level(2, snapshot.mix_second);
                    support.set_part_level(3, snapshot.mix_bass);
                    support.set_part_level(4, snapshot.mix_taps);
                    decorations.set_density(snapshot.deco_density);
                    decorations.set_lag_beats(snapshot.deco_lag_beats);
                    bells.set_decay(snapshot.bells_decay);
                    bells.set_ratio(snapshot.bells_ratio);
                    bells.set_index(snapshot.bells_index);
                    bells.set_index_decay(snapshot.bells_index_decay);
                    reverb.set_handpan_send(snapshot.reverb_handpan);
                    // Drone strip send/pan cover Root+Recent+Second together.
                    reverb.set_ding_send(snapshot.reverb_ding);
                    reverb.set_recent_send(snapshot.reverb_ding);
                    reverb.set_second_send(snapshot.reverb_ding);
                    reverb.set_bass_send(snapshot.reverb_bass);
                    reverb.set_echo_send(snapshot.reverb_echo);
                    reverb.set_decay(snapshot.reverb_decay);
                    reverb.set_tone(snapshot.reverb_tone);
                    groove.set_sensitivity(snapshot.groove_sensitivity);
                    groove.set_phase_pull(snapshot.groove_phase_pull);
                    groove.set_click_level(snapshot.groove_click_level);
                    groove.set_click_enabled(snapshot.groove_click_enabled);
                    groove.set_auto_lock(snapshot.groove_auto_lock);
                    match snapshot.groove_lock_cmd {
                        GROOVE_LOCK_CMD_LOCK => groove.set_tempo_locked(true),
                        GROOVE_LOCK_CMD_UNLOCK => groove.set_tempo_locked(false),
                        _ => {}
                    }
                    support.set_layout(
                        snapshot.handpan_scale,
                        snapshot.handpan_ding,
                        snapshot.handpan_tuning,
                    );
                    fm_bass.set_decay(snapshot.fm_bass_decay);
                    fm_bass.set_ratio(snapshot.fm_bass_ratio);
                    fm_bass.set_index(snapshot.fm_bass_index);
                    fm_bass.set_index_decay(snapshot.fm_bass_index_decay);
                    fm_bass.set_feedback(snapshot.fm_bass_feedback);
                    bass_line.set_amount(snapshot.bass_line_amount);
                    bass_line.set_variation(snapshot.bass_line_variation);
                    bass_line.set_layout(snapshot.handpan_scale, snapshot.handpan_ding);
                    if triggered {
                        for strike in strikes.iter().take(strike_count) {
                            handpan.strike_at(strike.0, strike.1, strike.2);
                            support.hear(strike.0, strike.1);
                            groove.hear(strike.0, strike.1);
                            play_memory.hear(strike.0, strike.1, strike.2);
                            bass_line.hear(strike.0, strike.1);
                        }
                    }
                    let deco_gain = support.echo_gain();
                    let echo_voice = EchoInstrument::from_u8(snapshot.echo_instrument);
                    let any_solo = snapshot.handpan_solo || snapshot.support_solo != 0;
                    let mix_handpan =
                        if snapshot.handpan_muted || (any_solo && !snapshot.handpan_solo) {
                            0.0
                        } else {
                            snapshot
                                .mix_handpan
                                .clamp(MIX_LEVEL_RANGE.0, MIX_LEVEL_RANGE.1)
                        };
                    // Bass strip mute/solo is support part 3 — apply the same gate to
                    // the FM Bass Line (Support's drone already uses part_gain).
                    const BASS_PART_BIT: u8 = 1 << 3;
                    let bass_muted = (snapshot.support_muted & BASS_PART_BIT) != 0;
                    let bass_soloed = (snapshot.support_solo & BASS_PART_BIT) != 0;
                    let fm_line_level = if bass_muted || (any_solo && !bass_soloed) {
                        0.0
                    } else {
                        snapshot
                            .fm_bass_level
                            .clamp(FM_BASS_LEVEL_RANGE.0, FM_BASS_LEVEL_RANGE.1)
                    };
                    let pan_handpan = snapshot.pan_handpan;
                    let pan_ding = snapshot.pan_ding;
                    let pan_bass = snapshot.pan_bass;
                    let pan_echo = snapshot.pan_echo;
                    let mut samples_in_block = 0u32;
                    let peak = write_stereo_frames::<T>(data, channels, &audio_tap, || {
                        play_memory.tick(groove.tempo_locked(), groove.period_s());
                        let click = groove.tick();
                        decorations.tick(&play_memory, &groove, |note, amp, place| {
                            let level = amp * deco_gain;
                            if level <= 0.0 {
                                return;
                            }
                            match echo_voice {
                                EchoInstrument::Bells => {
                                    let transposed = note
                                        .saturating_add(DEFAULT_BELLS_OCTAVE.max(0) as u8)
                                        .min(127);
                                    StrikeVoice::strike_at(&mut bells, transposed, level, place);
                                }
                                EchoInstrument::Handpan => {
                                    handpan.strike_at(note, level, place);
                                }
                            }
                        });
                        bass_line.tick(&groove, |note, amp, place| {
                            fm_bass.strike_at(note, amp, place);
                        });
                        samples_in_block = samples_in_block.wrapping_add(1);
                        if samples_in_block.is_multiple_of(64) {
                            control.publish_groove(&groove.snapshot());
                        }
                        let hand = handpan.next_sample() * mix_handpan;
                        let line = StrikeVoice::next_sample(&mut fm_bass) * fm_line_level;
                        let echo_sig = match echo_voice {
                            EchoInstrument::Bells => StrikeVoice::next_sample(&mut bells),
                            EchoInstrument::Handpan => 0.0,
                        };
                        let parts = support.next_parts().limited();
                        let (hand_l, hand_r) = stereo_pan(pan_handpan);
                        let (drone_l, drone_r) = stereo_pan(pan_ding);
                        let (bass_l, bass_r) = stereo_pan(pan_bass);
                        let (echo_l, echo_r) = stereo_pan(pan_echo);
                        let (wet_l, wet_r) = reverb.next_wet(
                            hand,
                            parts.ding,
                            parts.recent,
                            parts.second,
                            parts.bass,
                            echo_sig,
                        );
                        let drone = parts.ding + parts.recent + parts.second;
                        let dry_l = hand * hand_l
                            + drone * drone_l
                            + parts.bass * bass_l
                            + echo_sig * echo_l
                            + line
                            + click;
                        let dry_r = hand * hand_r
                            + drone * drone_r
                            + parts.bass * bass_r
                            + echo_sig * echo_r
                            + line
                            + click;
                        (dry_l + wet_l, dry_r + wet_r)
                    });
                    control.publish_groove(&groove.snapshot());
                    peak
                } else if snapshot.instrument == Instrument::Bass.as_u8() {
                    fm_bass.set_decay(snapshot.fm_bass_decay);
                    fm_bass.set_ratio(snapshot.fm_bass_ratio);
                    fm_bass.set_index(snapshot.fm_bass_index);
                    fm_bass.set_index_decay(snapshot.fm_bass_index_decay);
                    fm_bass.set_feedback(snapshot.fm_bass_feedback);
                    if triggered {
                        for strike in strikes.iter().take(strike_count) {
                            fm_bass.strike_at(strike.0, strike.1, strike.2);
                        }
                    }
                    let level = snapshot
                        .fm_bass_level
                        .clamp(FM_BASS_LEVEL_RANGE.0, FM_BASS_LEVEL_RANGE.1);
                    write_stereo_frames::<T>(data, channels, &audio_tap, || {
                        let mono = StrikeVoice::next_sample(&mut fm_bass) * level;
                        (mono, mono)
                    })
                } else if snapshot.instrument == Instrument::Bells.as_u8() {
                    bells.set_decay(snapshot.bells_decay);
                    bells.set_ratio(snapshot.bells_ratio);
                    bells.set_index(snapshot.bells_index);
                    bells.set_index_decay(snapshot.bells_index_decay);
                    if triggered {
                        for strike in strikes.iter().take(strike_count) {
                            // Pad pitch as written — echo role still octave-shifts.
                            bells.strike_at(strike.0, strike.1, strike.2);
                        }
                    }
                    let level = snapshot
                        .mix_taps
                        .clamp(MIX_LEVEL_RANGE.0, MIX_LEVEL_RANGE.1);
                    write_stereo_frames::<T>(data, channels, &audio_tap, || {
                        let mono = StrikeVoice::next_sample(&mut bells) * level;
                        (mono, mono)
                    })
                } else if matches!(
                    Instrument::from_u8(snapshot.instrument),
                    Instrument::Resonator | Instrument::Waveshape | Instrument::Tambura
                ) {
                    let voice = match Instrument::from_u8(snapshot.instrument) {
                        Instrument::Waveshape => &mut waveshape,
                        Instrument::Tambura => &mut tambura,
                        _ => &mut resonator,
                    };
                    apply_drone_voice_params(voice, &snapshot);
                    if triggered {
                        for strike in strikes.iter().take(strike_count) {
                            voice.strike_at(strike.0, strike.1, strike.2);
                        }
                    }
                    let level = snapshot
                        .mix_ding
                        .clamp(DRONE_LEVEL_RANGE.0, DRONE_LEVEL_RANGE.1);
                    write_stereo_frames::<T>(data, channels, &audio_tap, || {
                        let mono = StrikeVoice::next_sample(voice) * level;
                        (mono, mono)
                    })
                } else {
                    engine.set_exciter_mode(ExciterMode::from_u8(snapshot.exciter_mode));
                    engine.set_gate(snapshot.gate);
                    engine.set_frequency_hz(snapshot.frequency_hz);
                    engine.set_amplitude(snapshot.amplitude);
                    engine.set_attack_ms(snapshot.attack_ms);
                    engine.set_release_ms(snapshot.release_ms);
                    engine.set_damping_ratio(snapshot.damping_ratio);
                    engine.set_inharmonicity(snapshot.inharmonicity);
                    engine.set_position_coupling_base(snapshot.position_coupling);
                    engine.set_velocity_coupling_base(snapshot.velocity_coupling);
                    engine.set_excitation_gain(snapshot.excitation_gain);
                    engine.set_output_gain(snapshot.output_gain);
                    engine.set_breath_pressure(snapshot.breath_pressure);
                    engine.set_breath_noise_cutoff_hz(snapshot.breath_noise_cutoff_hz);
                    engine.set_breath_feedback(snapshot.breath_feedback);
                    engine.set_breath_auto_level(snapshot.breath_auto_level);
                    if triggered {
                        engine.trigger(snapshot.amplitude);
                    }
                    write_frames::<T>(data, channels, &audio_tap, || engine.next_sample())
                };
                let frames = if channels == 0 {
                    0
                } else {
                    data.len() / channels
                };
                if frames > 0 {
                    let block_duration = frames as f32 / sample_rate_hz.max(1.0);
                    control.update_peak(block_peak, block_duration);
                }
            },
            err_fn,
            None,
        )
    };

    // First try with requested buffer size.
    if let Ok(stream) = try_build(config) {
        log_buffer_acceptance("requested", &config.buffer_size);
        return Ok(stream);
    }

    // Fall back to whatever the backend wants.
    let mut fallback = config.clone();
    fallback.buffer_size = cpal::BufferSize::Default;
    eprintln!(
        "Requested buffer {:?} not supported by device; falling back to default",
        requested_buffer
    );
    let stream = try_build(&fallback).context("Failed to build audio stream")?;
    log_buffer_acceptance("fallback", &fallback.buffer_size);
    Ok(stream)
}

fn log_buffer_acceptance(label: &str, buffer_size: &cpal::BufferSize) {
    match buffer_size {
        cpal::BufferSize::Fixed(n) => println!("Audio buffer ({label}): {n} frames"),
        cpal::BufferSize::Default => println!("Audio buffer ({label}): device default"),
    }
}

fn apply_drone_voice_params(voice: &mut DroneVoice, snapshot: &ControlSnapshot) {
    voice.set_shape(snapshot.ding_shape);
    voice.set_spread(snapshot.ding_spread);
    voice.set_brightness(snapshot.ding_brightness);
    voice.set_strings(snapshot.ding_strings);
    voice.set_jawari(snapshot.tambura_jawari);
    voice.set_formant(0, snapshot.ding_formant_1);
    voice.set_formant(1, snapshot.ding_formant_2);
    voice.set_formant(2, snapshot.ding_formant_3);
    voice.set_formant_q(0, snapshot.ding_q_1);
    voice.set_formant_q(1, snapshot.ding_q_2);
    voice.set_formant_q(2, snapshot.ding_q_3);
    voice.set_formant_rate(snapshot.formant_rate);
    voice.set_formant_depth(snapshot.formant_depth);
    let ding = snapshot.handpan_ding;
    let tuning = snapshot.handpan_tuning;
    let root = handpan_interval_hz(ding, 0, tuning).max(1.0);
    let fifth = handpan_interval_hz(ding, 7, tuning);
    voice.set_fifth_ratio(fifth / root);
}

#[inline]
fn write_stereo_frames<T: FromF32>(
    out: &mut [T],
    channels: usize,
    audio_tap: &AudioTap,
    mut next_frame: impl FnMut() -> (f32, f32),
) -> f32 {
    if channels == 0 {
        return 0.0;
    }

    let mut block_peak = 0.0f32;
    let frames = out.len() / channels;
    for frame_idx in 0..frames {
        let (left, right) = next_frame();
        let mono = 0.5 * (left + right);
        audio_tap.push(mono);
        block_peak = block_peak.max(left.abs()).max(right.abs());
        if channels == 1 {
            out[frame_idx] = FromF32::from_f32(mono);
        } else {
            let base = frame_idx * channels;
            out[base] = FromF32::from_f32(left);
            out[base + 1] = FromF32::from_f32(right);
            for sample in out.iter_mut().take(base + channels).skip(base + 2) {
                *sample = FromF32::from_f32(mono);
            }
        }
    }

    block_peak
}

fn write_frames<T: FromF32>(
    out: &mut [T],
    channels: usize,
    audio_tap: &AudioTap,
    mut next_sample: impl FnMut() -> f32,
) -> f32 {
    if channels == 0 {
        return 0.0;
    }

    let mut block_peak = 0.0f32;
    let frames = out.len() / channels;
    for frame_idx in 0..frames {
        let s = next_sample();
        audio_tap.push(s);
        block_peak = block_peak.max(s.abs());
        for ch in 0..channels {
            let idx = frame_idx * channels + ch;
            out[idx] = FromF32::from_f32(s);
        }
    }

    block_peak
}

trait FromF32: Copy {
    fn from_f32(s: f32) -> Self;
}

impl FromF32 for f32 {
    #[inline]
    fn from_f32(s: f32) -> Self {
        s.clamp(-1.0, 1.0)
    }
}

impl FromF32 for i16 {
    #[inline]
    fn from_f32(s: f32) -> Self {
        (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
    }
}

impl FromF32 for u16 {
    #[inline]
    fn from_f32(s: f32) -> Self {
        let s = s.clamp(-1.0, 1.0);
        (((s * 0.5) + 0.5) * u16::MAX as f32) as u16
    }
}
