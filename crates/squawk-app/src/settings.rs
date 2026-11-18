use std::collections::HashMap;
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::ParameterId;

const SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredFmBassPreset {
    #[serde(default = "default_fm_bass_decay")]
    decay: f32,
    #[serde(default = "default_fm_bass_ratio")]
    ratio: f32,
    #[serde(default = "default_fm_bass_index")]
    index: f32,
    #[serde(default = "default_fm_bass_index_decay")]
    index_decay: f32,
    #[serde(default = "default_fm_bass_feedback")]
    feedback: f32,
    #[serde(default = "default_fm_bass_level")]
    level: f32,
}

impl StoredFmBassPreset {
    fn from_params(params: squawk_engine::FmBassParams) -> Self {
        let p = params.clamp();
        Self {
            decay: p.decay,
            ratio: p.ratio,
            index: p.index,
            index_decay: p.index_decay,
            feedback: p.feedback,
            level: p.level,
        }
    }

    fn into_params(self) -> squawk_engine::FmBassParams {
        squawk_engine::FmBassParams {
            decay: self.decay,
            ratio: self.ratio,
            index: self.index,
            index_decay: self.index_decay,
            feedback: self.feedback,
            level: self.level,
        }
        .clamp()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSettings {
    #[serde(default = "default_exciter_mode")]
    exciter_mode: u8,
    attack_ms: f32,
    release_ms: f32,
    damping_ratio: f32,
    #[serde(default = "default_inharmonicity")]
    inharmonicity: f32,
    #[serde(default = "default_position_coupling")]
    position_coupling: f32,
    #[serde(default = "default_velocity_coupling")]
    velocity_coupling: f32,
    #[serde(default = "default_excitation_gain")]
    excitation_gain: f32,
    #[serde(default = "default_output_gain")]
    output_gain: f32,
    #[serde(default = "default_breath_pressure")]
    breath_pressure: f32,
    #[serde(default = "default_breath_noise_cutoff_hz")]
    breath_noise_cutoff_hz: f32,
    #[serde(default = "default_breath_feedback")]
    breath_feedback: f32,
    #[serde(default = "default_breath_auto_level")]
    breath_auto_level: f32,
    #[serde(default = "default_instrument")]
    instrument: u8,
    #[serde(default = "default_handpan_scale")]
    handpan_scale: u8,
    #[serde(default = "default_handpan_ding")]
    handpan_ding: u8,
    #[serde(default = "default_handpan_tuning")]
    handpan_tuning: u8,
    #[serde(default = "default_handpan_decay")]
    handpan_decay: f32,
    #[serde(default = "default_handpan_detune_cents")]
    handpan_detune_cents: f32,
    #[serde(default = "default_handpan_coupling")]
    handpan_coupling: f32,
    #[serde(default = "default_handpan_hit")]
    handpan_hit: f32,
    #[serde(default = "default_handpan_glide")]
    handpan_glide: f32,
    #[serde(default = "default_handpan_fundamental")]
    handpan_fundamental: f32,
    #[serde(default = "default_handpan_octave")]
    handpan_octave: f32,
    #[serde(default = "default_handpan_fifth")]
    handpan_fifth: f32,
    #[serde(default = "default_handpan_higher_decay")]
    handpan_higher_decay: f32,
    #[serde(default = "default_support_level")]
    support_level: f32,
    #[serde(default = "default_support_pulse")]
    support_pulse: f32,
    #[serde(default = "default_support_width")]
    support_width: f32,
    #[serde(default = "default_support_wander")]
    support_wander: f32,
    #[serde(default = "default_support_drift")]
    support_drift: f32,
    #[serde(default = "default_support_motion")]
    support_motion: f32,
    #[serde(default = "default_support_fade")]
    support_fade: f32,
    #[serde(default = "default_reverb_handpan")]
    reverb_handpan: f32,
    #[serde(default = "default_reverb_support")]
    reverb_support: f32,
    /// Absent in older settings files; filled from `reverb_support` on load.
    #[serde(default)]
    reverb_ding: Option<f32>,
    #[serde(default)]
    reverb_recent: Option<f32>,
    #[serde(default)]
    reverb_second: Option<f32>,
    #[serde(default)]
    reverb_bass: Option<f32>,
    #[serde(default = "default_reverb_echo")]
    reverb_echo: f32,
    #[serde(default = "default_reverb_decay")]
    reverb_decay: f32,
    #[serde(default = "default_reverb_tone")]
    reverb_tone: f32,
    #[serde(default = "default_echo_instrument")]
    echo_instrument: u8,
    #[serde(default = "default_ding_wash")]
    ding_wash: u8,
    #[serde(default = "default_ding_shape")]
    ding_shape: f32,
    #[serde(default = "default_ding_spread")]
    ding_spread: f32,
    #[serde(default = "default_ding_brightness")]
    ding_brightness: f32,
    #[serde(default = "default_ding_strings")]
    ding_strings: f32,
    #[serde(default = "default_tambura_jawari")]
    tambura_jawari: f32,
    #[serde(default = "default_ding_formant_1")]
    ding_formant_1: f32,
    #[serde(default = "default_ding_formant_2")]
    ding_formant_2: f32,
    #[serde(default = "default_ding_formant_3")]
    ding_formant_3: f32,
    #[serde(default = "default_ding_resonance")]
    ding_resonance: f32,
    #[serde(default = "default_ding_resonance")]
    ding_q_1: f32,
    #[serde(default = "default_ding_resonance")]
    ding_q_2: f32,
    #[serde(default = "default_ding_resonance")]
    ding_q_3: f32,
    #[serde(default = "default_mix_level")]
    mix_handpan: f32,
    #[serde(default = "default_mix_level")]
    mix_ding: f32,
    #[serde(default = "default_mix_level")]
    mix_recent: f32,
    #[serde(default = "default_mix_level")]
    mix_second: f32,
    #[serde(default = "default_mix_level")]
    mix_bass: f32,
    #[serde(default = "default_mix_level")]
    mix_taps: f32,
    #[serde(default = "default_deco_density")]
    deco_density: f32,
    #[serde(default = "default_deco_lag_beats")]
    deco_lag_beats: f32,
    /// Legacy Echo feedback; folded into deco_density on load.
    #[serde(default, skip_serializing)]
    echo_feedback: Option<f32>,
    #[serde(default = "default_pan")]
    pan_handpan: f32,
    #[serde(default = "default_pan")]
    pan_ding: f32,
    #[serde(default = "default_pan")]
    pan_recent: f32,
    #[serde(default = "default_pan")]
    pan_second: f32,
    #[serde(default = "default_pan")]
    pan_bass: f32,
    #[serde(default = "default_pan")]
    pan_echo: f32,
    #[serde(default = "default_formant_rate")]
    formant_rate: f32,
    #[serde(default = "default_formant_depth")]
    formant_depth: f32,
    #[serde(default = "default_groove_sensitivity")]
    groove_sensitivity: f32,
    #[serde(default = "default_groove_phase_pull")]
    groove_phase_pull: f32,
    #[serde(default = "default_groove_click_level")]
    groove_click_level: f32,
    #[serde(default = "default_fm_bass_decay")]
    fm_bass_decay: f32,
    #[serde(default = "default_fm_bass_ratio")]
    fm_bass_ratio: f32,
    #[serde(default = "default_fm_bass_index")]
    fm_bass_index: f32,
    #[serde(default = "default_fm_bass_index_decay")]
    fm_bass_index_decay: f32,
    #[serde(default = "default_fm_bass_feedback")]
    fm_bass_feedback: f32,
    #[serde(default = "default_fm_bass_level")]
    fm_bass_level: f32,
    #[serde(default = "default_bass_line_amount")]
    bass_line_amount: f32,
    #[serde(default = "default_bass_line_variation")]
    bass_line_variation: f32,
    #[serde(default = "default_bells_decay")]
    bells_decay: f32,
    #[serde(default = "default_bells_ratio")]
    bells_ratio: f32,
    #[serde(default = "default_bells_index")]
    bells_index: f32,
    #[serde(default = "default_bells_index_decay")]
    bells_index_decay: f32,
    #[serde(default)]
    fm_bass_presets: HashMap<String, StoredFmBassPreset>,
    #[serde(default = "default_groove_click_enabled")]
    groove_click_enabled: bool,
    #[serde(default = "default_groove_auto_lock")]
    groove_auto_lock: bool,
    #[serde(default = "default_support_part_flags")]
    support_muted: [bool; 5],
    #[serde(default = "default_support_part_flags")]
    support_solo: [bool; 5],
    #[serde(default)]
    handpan_muted: bool,
    #[serde(default)]
    handpan_solo: bool,
    #[serde(default = "default_keyboard_rows_set_place")]
    keyboard_rows_set_place: bool,
    midi_cc: HashMap<String, u8>,
}

impl Default for StoredSettings {
    fn default() -> Self {
        Self {
            exciter_mode: default_exciter_mode(),
            attack_ms: crate::DEFAULT_ATTACK_MS,
            release_ms: crate::DEFAULT_RELEASE_MS,
            damping_ratio: crate::DEFAULT_DAMPING_RATIO,
            inharmonicity: crate::DEFAULT_INHARMONICITY,
            position_coupling: crate::DEFAULT_POSITION_COUPLING,
            velocity_coupling: crate::DEFAULT_VELOCITY_COUPLING,
            excitation_gain: crate::DEFAULT_EXCITATION_GAIN,
            output_gain: crate::DEFAULT_OUTPUT_GAIN,
            breath_pressure: crate::DEFAULT_BREATH_PRESSURE,
            breath_noise_cutoff_hz: crate::DEFAULT_BREATH_NOISE_CUTOFF_HZ,
            breath_feedback: crate::DEFAULT_BREATH_FEEDBACK,
            breath_auto_level: crate::DEFAULT_BREATH_AUTO_LEVEL,
            instrument: default_instrument(),
            handpan_scale: default_handpan_scale(),
            handpan_ding: default_handpan_ding(),
            handpan_tuning: default_handpan_tuning(),
            handpan_decay: crate::DEFAULT_HANDPAN_DECAY,
            handpan_detune_cents: crate::DEFAULT_HANDPAN_DETUNE_CENTS,
            handpan_coupling: crate::DEFAULT_HANDPAN_COUPLING,
            handpan_hit: crate::DEFAULT_HANDPAN_HIT,
            handpan_glide: crate::DEFAULT_HANDPAN_GLIDE,
            handpan_fundamental: crate::DEFAULT_HANDPAN_FUNDAMENTAL,
            handpan_octave: crate::DEFAULT_HANDPAN_OCTAVE,
            handpan_fifth: crate::DEFAULT_HANDPAN_FIFTH,
            handpan_higher_decay: crate::DEFAULT_HANDPAN_HIGHER_DECAY,
            support_level: crate::DEFAULT_SUPPORT_LEVEL,
            support_pulse: crate::DEFAULT_SUPPORT_PULSE,
            support_width: crate::DEFAULT_SUPPORT_WIDTH,
            support_wander: crate::DEFAULT_SUPPORT_WANDER,
            support_drift: crate::DEFAULT_SUPPORT_DRIFT,
            support_motion: crate::DEFAULT_SUPPORT_MOTION,
            support_fade: crate::DEFAULT_SUPPORT_FADE,
            reverb_handpan: crate::DEFAULT_REVERB_HANDPAN,
            reverb_support: crate::DEFAULT_REVERB_SUPPORT,
            reverb_ding: Some(crate::DEFAULT_REVERB_DING),
            reverb_recent: Some(crate::DEFAULT_REVERB_RECENT),
            reverb_second: Some(crate::DEFAULT_REVERB_SECOND),
            reverb_bass: Some(crate::DEFAULT_REVERB_BASS),
            reverb_echo: crate::DEFAULT_REVERB_ECHO,
            reverb_decay: crate::DEFAULT_REVERB_DECAY,
            reverb_tone: crate::DEFAULT_REVERB_TONE,
            echo_instrument: crate::DEFAULT_ECHO_INSTRUMENT,
            ding_wash: default_ding_wash(),
            ding_shape: crate::DEFAULT_DING_SHAPE,
            ding_spread: crate::DEFAULT_DING_SPREAD,
            ding_brightness: crate::DEFAULT_DING_BRIGHTNESS,
            ding_strings: crate::DEFAULT_DING_STRINGS,
            tambura_jawari: crate::DEFAULT_TAMBURA_JAWARI,
            ding_formant_1: crate::DEFAULT_DING_FORMANT_1,
            ding_formant_2: crate::DEFAULT_DING_FORMANT_2,
            ding_formant_3: crate::DEFAULT_DING_FORMANT_3,
            ding_resonance: crate::DEFAULT_DING_RESONANCE,
            ding_q_1: crate::DEFAULT_DING_RESONANCE,
            ding_q_2: crate::DEFAULT_DING_RESONANCE,
            ding_q_3: crate::DEFAULT_DING_RESONANCE,
            mix_handpan: crate::DEFAULT_MIX_LEVEL,
            mix_ding: crate::DEFAULT_MIX_LEVEL,
            mix_recent: crate::DEFAULT_MIX_LEVEL,
            mix_second: crate::DEFAULT_MIX_LEVEL,
            mix_bass: crate::DEFAULT_MIX_LEVEL,
            mix_taps: crate::DEFAULT_MIX_LEVEL,
            deco_density: crate::DEFAULT_DECO_DENSITY,
            deco_lag_beats: crate::DEFAULT_DECO_LAG_BEATS,
            echo_feedback: None,
            pan_handpan: crate::DEFAULT_PAN,
            pan_ding: crate::DEFAULT_PAN,
            pan_recent: crate::DEFAULT_PAN,
            pan_second: crate::DEFAULT_PAN,
            pan_bass: crate::DEFAULT_PAN,
            pan_echo: crate::DEFAULT_PAN,
            formant_rate: crate::DEFAULT_FORMANT_RATE,
            formant_depth: crate::DEFAULT_FORMANT_DEPTH,
            groove_sensitivity: crate::DEFAULT_GROOVE_SENSITIVITY,
            groove_phase_pull: crate::DEFAULT_GROOVE_PHASE_PULL,
            groove_click_level: crate::DEFAULT_GROOVE_CLICK_LEVEL,
            fm_bass_decay: crate::DEFAULT_FM_BASS_DECAY,
            fm_bass_ratio: crate::DEFAULT_FM_BASS_RATIO,
            fm_bass_index: crate::DEFAULT_FM_BASS_INDEX,
            fm_bass_index_decay: crate::DEFAULT_FM_BASS_INDEX_DECAY,
            fm_bass_feedback: crate::DEFAULT_FM_BASS_FEEDBACK,
            fm_bass_level: crate::DEFAULT_FM_BASS_LEVEL,
            bass_line_amount: crate::DEFAULT_BASS_LINE_AMOUNT,
            bass_line_variation: crate::DEFAULT_BASS_LINE_VARIATION,
            bells_decay: crate::DEFAULT_BELLS_DECAY,
            bells_ratio: crate::DEFAULT_BELLS_RATIO,
            bells_index: crate::DEFAULT_BELLS_INDEX,
            bells_index_decay: crate::DEFAULT_BELLS_INDEX_DECAY,
            fm_bass_presets: HashMap::new(),
            groove_click_enabled: crate::DEFAULT_GROOVE_CLICK_ENABLED,
            groove_auto_lock: crate::DEFAULT_GROOVE_AUTO_LOCK,
            support_muted: default_support_part_flags(),
            support_solo: default_support_part_flags(),
            handpan_muted: false,
            handpan_solo: false,
            keyboard_rows_set_place: default_keyboard_rows_set_place(),
            midi_cc: HashMap::new(),
        }
    }
}

pub(crate) struct SettingsSnapshot {
    pub exciter_mode: u8,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub damping_ratio: f32,
    pub inharmonicity: f32,
    pub position_coupling: f32,
    pub velocity_coupling: f32,
    pub excitation_gain: f32,
    pub output_gain: f32,
    pub breath_pressure: f32,
    pub breath_noise_cutoff_hz: f32,
    pub breath_feedback: f32,
    pub breath_auto_level: f32,
    pub instrument: u8,
    pub handpan_scale: u8,
    pub handpan_ding: u8,
    pub handpan_tuning: u8,
    pub handpan_decay: f32,
    pub handpan_detune_cents: f32,
    pub handpan_coupling: f32,
    pub handpan_hit: f32,
    pub handpan_glide: f32,
    pub handpan_fundamental: f32,
    pub handpan_octave: f32,
    pub handpan_fifth: f32,
    pub handpan_higher_decay: f32,
    pub support_level: f32,
    pub support_pulse: f32,
    pub support_width: f32,
    pub support_wander: f32,
    pub support_drift: f32,
    pub support_motion: f32,
    pub support_fade: f32,
    pub reverb_handpan: f32,
    pub reverb_support: f32,
    pub reverb_ding: f32,
    pub reverb_recent: f32,
    pub reverb_second: f32,
    pub reverb_bass: f32,
    pub reverb_echo: f32,
    pub reverb_decay: f32,
    pub reverb_tone: f32,
    pub echo_instrument: u8,
    pub ding_wash: u8,
    pub ding_shape: f32,
    pub ding_spread: f32,
    pub ding_brightness: f32,
    pub ding_strings: f32,
    pub tambura_jawari: f32,
    pub ding_formant_1: f32,
    pub ding_formant_2: f32,
    pub ding_formant_3: f32,
    pub ding_resonance: f32,
    pub ding_q_1: f32,
    pub ding_q_2: f32,
    pub ding_q_3: f32,
    pub mix_handpan: f32,
    pub mix_ding: f32,
    pub mix_recent: f32,
    pub mix_second: f32,
    pub mix_bass: f32,
    pub mix_taps: f32,
    pub deco_density: f32,
    pub deco_lag_beats: f32,
    pub pan_handpan: f32,
    pub pan_ding: f32,
    pub pan_recent: f32,
    pub pan_second: f32,
    pub pan_bass: f32,
    pub pan_echo: f32,
    pub formant_rate: f32,
    pub formant_depth: f32,
    pub groove_sensitivity: f32,
    pub groove_phase_pull: f32,
    pub groove_click_level: f32,
    pub fm_bass_decay: f32,
    pub fm_bass_ratio: f32,
    pub fm_bass_index: f32,
    pub fm_bass_index_decay: f32,
    pub fm_bass_feedback: f32,
    pub fm_bass_level: f32,
    pub bass_line_amount: f32,
    pub bass_line_variation: f32,
    pub bells_decay: f32,
    pub bells_ratio: f32,
    pub bells_index: f32,
    pub bells_index_decay: f32,
    pub groove_click_enabled: bool,
    pub groove_auto_lock: bool,
    pub support_muted: u8,
    pub support_solo: u8,
    pub handpan_muted: bool,
    pub handpan_solo: bool,
    pub keyboard_rows_set_place: bool,
    pub midi_assignments: Vec<(ParameterId, u8)>,
}

impl From<StoredSettings> for SettingsSnapshot {
    fn from(value: StoredSettings) -> Self {
        let mut midi_assignments = Vec::new();
        for (key, cc) in value.midi_cc.iter() {
            if let Some(param) = ParameterId::from_storage_key(key) {
                midi_assignments.push((param, *cc));
            }
        }

        Self {
            exciter_mode: value.exciter_mode.min(1),
            attack_ms: ParameterId::AttackMs.clamp(value.attack_ms),
            release_ms: ParameterId::ReleaseMs.clamp(value.release_ms),
            damping_ratio: ParameterId::DampingRatio.clamp(value.damping_ratio),
            inharmonicity: ParameterId::Inharmonicity.clamp(value.inharmonicity),
            position_coupling: ParameterId::PositionCoupling.clamp(value.position_coupling),
            velocity_coupling: ParameterId::VelocityCoupling.clamp(value.velocity_coupling),
            excitation_gain: ParameterId::ExcitationGain.clamp(value.excitation_gain),
            output_gain: ParameterId::OutputGain.clamp(value.output_gain),
            breath_pressure: ParameterId::BreathPressure.clamp(value.breath_pressure),
            breath_noise_cutoff_hz: ParameterId::BreathNoiseCutoffHz
                .clamp(value.breath_noise_cutoff_hz),
            breath_feedback: ParameterId::BreathFeedback.clamp(value.breath_feedback),
            breath_auto_level: ParameterId::BreathAutoLevel.clamp(value.breath_auto_level),
            instrument: crate::Instrument::from_u8(value.instrument).as_u8(),
            handpan_scale: value.handpan_scale.min(crate::HANDPAN_SCALE_COUNT - 1),
            handpan_ding: value
                .handpan_ding
                .clamp(crate::HANDPAN_DING_MIN, crate::HANDPAN_DING_MAX),
            handpan_tuning: u8::from(value.handpan_tuning != 0),
            handpan_decay: ParameterId::HandpanDecay.clamp(value.handpan_decay),
            handpan_detune_cents: ParameterId::HandpanDetuneCents.clamp(value.handpan_detune_cents),
            handpan_coupling: ParameterId::HandpanCoupling.clamp(value.handpan_coupling),
            handpan_hit: ParameterId::HandpanHit.clamp(value.handpan_hit),
            handpan_glide: ParameterId::HandpanGlide.clamp(value.handpan_glide),
            handpan_fundamental: ParameterId::HandpanFundamental.clamp(value.handpan_fundamental),
            handpan_octave: ParameterId::HandpanOctave.clamp(value.handpan_octave),
            handpan_fifth: ParameterId::HandpanFifth.clamp(value.handpan_fifth),
            handpan_higher_decay: ParameterId::HandpanHigherDecay.clamp(value.handpan_higher_decay),
            support_level: ParameterId::SupportLevel.clamp(value.support_level),
            support_pulse: ParameterId::SupportPulse.clamp(value.support_pulse),
            support_width: ParameterId::SupportWidth.clamp(value.support_width),
            support_wander: ParameterId::SupportWander.clamp(value.support_wander),
            support_drift: ParameterId::SupportDrift.clamp(value.support_drift),
            support_motion: ParameterId::SupportMotion.clamp(value.support_motion),
            support_fade: ParameterId::SupportFade.clamp(value.support_fade),
            reverb_handpan: ParameterId::ReverbHandpan.clamp(value.reverb_handpan),
            reverb_support: ParameterId::ReverbSupport.clamp(value.reverb_support),
            reverb_ding: ParameterId::ReverbDing
                .clamp(value.reverb_ding.unwrap_or(value.reverb_support)),
            reverb_recent: ParameterId::ReverbRecent
                .clamp(value.reverb_recent.unwrap_or(value.reverb_support)),
            reverb_second: ParameterId::ReverbSecond
                .clamp(value.reverb_second.unwrap_or(value.reverb_support)),
            reverb_bass: ParameterId::ReverbBass
                .clamp(value.reverb_bass.unwrap_or(value.reverb_support)),
            reverb_echo: ParameterId::ReverbEcho.clamp(value.reverb_echo),
            reverb_decay: ParameterId::ReverbDecay.clamp(value.reverb_decay),
            reverb_tone: ParameterId::ReverbTone.clamp(value.reverb_tone),
            echo_instrument: crate::EchoInstrument::from_u8(value.echo_instrument).as_u8(),
            ding_wash: value.ding_wash.min(2),
            ding_shape: ParameterId::DingShape.clamp(value.ding_shape),
            ding_spread: ParameterId::DingSpread.clamp(value.ding_spread),
            ding_brightness: ParameterId::DingBrightness.clamp(value.ding_brightness),
            ding_strings: ParameterId::DingStrings.clamp(value.ding_strings),
            tambura_jawari: ParameterId::TamburaJawari.clamp(value.tambura_jawari),
            ding_formant_1: ParameterId::DingFormant1.clamp(value.ding_formant_1),
            ding_formant_2: ParameterId::DingFormant2.clamp(value.ding_formant_2),
            ding_formant_3: ParameterId::DingFormant3.clamp(value.ding_formant_3),
            ding_resonance: ParameterId::DingResonance.clamp(value.ding_resonance),
            ding_q_1: ParameterId::DingQ1.clamp(value.ding_q_1),
            ding_q_2: ParameterId::DingQ2.clamp(value.ding_q_2),
            ding_q_3: ParameterId::DingQ3.clamp(value.ding_q_3),
            mix_handpan: ParameterId::MixHandpan.clamp(value.mix_handpan),
            mix_ding: ParameterId::MixDing.clamp(value.mix_ding),
            mix_recent: ParameterId::MixRecent.clamp(value.mix_recent),
            mix_second: ParameterId::MixSecond.clamp(value.mix_second),
            mix_bass: ParameterId::MixBass.clamp(value.mix_bass),
            mix_taps: ParameterId::MixTaps.clamp(value.mix_taps),
            deco_density: ParameterId::DecoDensity
                .clamp(value.echo_feedback.unwrap_or(value.deco_density)),
            deco_lag_beats: ParameterId::DecoLagBeats.clamp(value.deco_lag_beats),
            pan_handpan: ParameterId::PanHandpan.clamp(value.pan_handpan),
            pan_ding: ParameterId::PanDing.clamp(value.pan_ding),
            pan_recent: ParameterId::PanRecent.clamp(value.pan_recent),
            pan_second: ParameterId::PanSecond.clamp(value.pan_second),
            pan_bass: ParameterId::PanBass.clamp(value.pan_bass),
            pan_echo: ParameterId::PanEcho.clamp(value.pan_echo),
            formant_rate: ParameterId::FormantRate.clamp(value.formant_rate),
            formant_depth: ParameterId::FormantDepth.clamp(value.formant_depth),
            groove_sensitivity: ParameterId::GrooveSensitivity.clamp(value.groove_sensitivity),
            groove_phase_pull: ParameterId::GroovePhasePull.clamp(value.groove_phase_pull),
            groove_click_level: ParameterId::GrooveClickLevel.clamp(value.groove_click_level),
            fm_bass_decay: ParameterId::FmBassDecay.clamp(value.fm_bass_decay),
            fm_bass_ratio: ParameterId::FmBassRatio.clamp(value.fm_bass_ratio),
            fm_bass_index: ParameterId::FmBassIndex.clamp(value.fm_bass_index),
            fm_bass_index_decay: ParameterId::FmBassIndexDecay.clamp(value.fm_bass_index_decay),
            fm_bass_feedback: ParameterId::FmBassFeedback.clamp(value.fm_bass_feedback),
            fm_bass_level: ParameterId::FmBassLevel.clamp(value.fm_bass_level),
            bass_line_amount: ParameterId::BassLineAmount.clamp(value.bass_line_amount),
            bass_line_variation: ParameterId::BassLineVariation.clamp(value.bass_line_variation),
            bells_decay: ParameterId::BellsDecay.clamp(value.bells_decay),
            bells_ratio: ParameterId::BellsRatio.clamp(value.bells_ratio),
            bells_index: ParameterId::BellsIndex.clamp(value.bells_index),
            bells_index_decay: ParameterId::BellsIndexDecay.clamp(value.bells_index_decay),
            groove_click_enabled: value.groove_click_enabled,
            groove_auto_lock: value.groove_auto_lock,
            support_muted: squawk_engine::support_part_mask(value.support_muted),
            support_solo: squawk_engine::support_part_mask(value.support_solo),
            handpan_muted: value.handpan_muted,
            handpan_solo: value.handpan_solo,
            keyboard_rows_set_place: value.keyboard_rows_set_place,
            midi_assignments,
        }
    }
}

pub(crate) struct SettingsStore {
    path: PathBuf,
    state: Mutex<StoredSettings>,
}

impl SettingsStore {
    pub(crate) fn load() -> Result<Self> {
        let path = resolve_settings_path()?;
        let stored = read_settings(&path).unwrap_or_default();
        Ok(Self {
            path,
            state: Mutex::new(stored),
        })
    }

    pub(crate) fn snapshot(&self) -> SettingsSnapshot {
        let data = self.state.lock().clone();
        SettingsSnapshot::from(data)
    }

    pub(crate) fn update_parameter(&self, param: ParameterId, value: f32) -> Result<()> {
        let clamped = param.clamp(value);
        {
            let mut data = self.state.lock();
            match param {
                ParameterId::AttackMs => data.attack_ms = clamped,
                ParameterId::ReleaseMs => data.release_ms = clamped,
                ParameterId::DampingRatio => data.damping_ratio = clamped,
                ParameterId::Inharmonicity => data.inharmonicity = clamped,
                ParameterId::PositionCoupling => data.position_coupling = clamped,
                ParameterId::VelocityCoupling => data.velocity_coupling = clamped,
                ParameterId::ExcitationGain => data.excitation_gain = clamped,
                ParameterId::OutputGain => data.output_gain = clamped,
                ParameterId::BreathPressure => data.breath_pressure = clamped,
                ParameterId::BreathNoiseCutoffHz => data.breath_noise_cutoff_hz = clamped,
                ParameterId::BreathFeedback => data.breath_feedback = clamped,
                ParameterId::BreathAutoLevel => data.breath_auto_level = clamped,
                ParameterId::HandpanDecay => data.handpan_decay = clamped,
                ParameterId::HandpanDetuneCents => data.handpan_detune_cents = clamped,
                ParameterId::HandpanCoupling => data.handpan_coupling = clamped,
                ParameterId::HandpanHit => data.handpan_hit = clamped,
                ParameterId::HandpanGlide => data.handpan_glide = clamped,
                ParameterId::HandpanFundamental => data.handpan_fundamental = clamped,
                ParameterId::HandpanOctave => data.handpan_octave = clamped,
                ParameterId::HandpanFifth => data.handpan_fifth = clamped,
                ParameterId::HandpanHigherDecay => data.handpan_higher_decay = clamped,
                ParameterId::SupportLevel => data.support_level = clamped,
                ParameterId::SupportPulse => data.support_pulse = clamped,
                ParameterId::SupportWidth => data.support_width = clamped,
                ParameterId::SupportWander => data.support_wander = clamped,
                ParameterId::SupportDrift => data.support_drift = clamped,
                ParameterId::SupportMotion => data.support_motion = clamped,
                ParameterId::SupportFade => data.support_fade = clamped,
                ParameterId::ReverbHandpan => data.reverb_handpan = clamped,
                ParameterId::ReverbSupport => {
                    data.reverb_support = clamped;
                    data.reverb_ding = Some(clamped);
                    data.reverb_recent = Some(clamped);
                    data.reverb_second = Some(clamped);
                    data.reverb_bass = Some(clamped);
                }
                ParameterId::ReverbDing => data.reverb_ding = Some(clamped),
                ParameterId::ReverbRecent => data.reverb_recent = Some(clamped),
                ParameterId::ReverbSecond => data.reverb_second = Some(clamped),
                ParameterId::ReverbBass => data.reverb_bass = Some(clamped),
                ParameterId::ReverbEcho => data.reverb_echo = clamped,
                ParameterId::ReverbDecay => data.reverb_decay = clamped,
                ParameterId::ReverbTone => data.reverb_tone = clamped,
                ParameterId::DingShape => data.ding_shape = clamped,
                ParameterId::DingSpread => data.ding_spread = clamped,
                ParameterId::DingBrightness => data.ding_brightness = clamped,
                ParameterId::DingStrings => data.ding_strings = clamped,
                ParameterId::TamburaJawari => data.tambura_jawari = clamped,
                ParameterId::DingFormant1 => data.ding_formant_1 = clamped,
                ParameterId::DingFormant2 => data.ding_formant_2 = clamped,
                ParameterId::DingFormant3 => data.ding_formant_3 = clamped,
                ParameterId::DingResonance => data.ding_resonance = clamped,
                ParameterId::DingQ1 => data.ding_q_1 = clamped,
                ParameterId::DingQ2 => data.ding_q_2 = clamped,
                ParameterId::DingQ3 => data.ding_q_3 = clamped,
                ParameterId::MixHandpan => data.mix_handpan = clamped,
                ParameterId::MixDing => data.mix_ding = clamped,
                ParameterId::MixRecent => data.mix_recent = clamped,
                ParameterId::MixSecond => data.mix_second = clamped,
                ParameterId::MixBass => data.mix_bass = clamped,
                ParameterId::MixTaps => data.mix_taps = clamped,
                ParameterId::DecoDensity => data.deco_density = clamped,
                ParameterId::DecoLagBeats => data.deco_lag_beats = clamped,
                ParameterId::PanHandpan => data.pan_handpan = clamped,
                ParameterId::PanDing => data.pan_ding = clamped,
                ParameterId::PanRecent => data.pan_recent = clamped,
                ParameterId::PanSecond => data.pan_second = clamped,
                ParameterId::PanBass => data.pan_bass = clamped,
                ParameterId::PanEcho => data.pan_echo = clamped,
                ParameterId::FormantRate => data.formant_rate = clamped,
                ParameterId::FormantDepth => data.formant_depth = clamped,
                ParameterId::GrooveSensitivity => data.groove_sensitivity = clamped,
                ParameterId::GroovePhasePull => data.groove_phase_pull = clamped,
                ParameterId::GrooveClickLevel => data.groove_click_level = clamped,
                ParameterId::FmBassDecay => data.fm_bass_decay = clamped,
                ParameterId::FmBassRatio => data.fm_bass_ratio = clamped,
                ParameterId::FmBassIndex => data.fm_bass_index = clamped,
                ParameterId::FmBassIndexDecay => data.fm_bass_index_decay = clamped,
                ParameterId::FmBassFeedback => data.fm_bass_feedback = clamped,
                ParameterId::FmBassLevel => data.fm_bass_level = clamped,
                ParameterId::BassLineAmount => data.bass_line_amount = clamped,
                ParameterId::BassLineVariation => data.bass_line_variation = clamped,
                ParameterId::BellsDecay => data.bells_decay = clamped,
                ParameterId::BellsRatio => data.bells_ratio = clamped,
                ParameterId::BellsIndex => data.bells_index = clamped,
                ParameterId::BellsIndexDecay => data.bells_index_decay = clamped,
            }
        }
        self.persist_current_state()
    }

    pub(crate) fn update_groove_click_enabled(&self, enabled: bool) -> Result<()> {
        {
            let mut data = self.state.lock();
            data.groove_click_enabled = enabled;
        }
        self.persist_current_state()
    }

    pub(crate) fn update_groove_auto_lock(&self, enabled: bool) -> Result<()> {
        {
            let mut data = self.state.lock();
            data.groove_auto_lock = enabled;
        }
        self.persist_current_state()
    }

    pub(crate) fn update_handpan_layout(&self, scale: u8, ding: u8, tuning: u8) -> Result<()> {
        {
            let mut data = self.state.lock();
            data.handpan_scale = scale.min(crate::HANDPAN_SCALE_COUNT - 1);
            data.handpan_ding = ding.clamp(crate::HANDPAN_DING_MIN, crate::HANDPAN_DING_MAX);
            data.handpan_tuning = u8::from(tuning != 0);
        }
        self.persist_current_state()
    }

    pub(crate) fn update_ding_wash(&self, wash: u8) -> Result<()> {
        {
            let mut data = self.state.lock();
            data.ding_wash = wash.min(2);
        }
        self.persist_current_state()
    }

    pub(crate) fn update_echo_instrument(&self, instrument: u8) -> Result<()> {
        {
            let mut data = self.state.lock();
            data.echo_instrument = crate::EchoInstrument::from_u8(instrument).as_u8();
        }
        self.persist_current_state()
    }

    pub(crate) fn update_support_parts(&self, muted: [bool; 5], solo: [bool; 5]) -> Result<()> {
        {
            let mut data = self.state.lock();
            data.support_muted = muted;
            data.support_solo = solo;
        }
        self.persist_current_state()
    }

    pub(crate) fn update_handpan_mute_solo(&self, muted: bool, solo: bool) -> Result<()> {
        {
            let mut data = self.state.lock();
            data.handpan_muted = muted;
            data.handpan_solo = solo;
        }
        self.persist_current_state()
    }

    pub(crate) fn update_keyboard_rows_set_place(&self, enabled: bool) -> Result<()> {
        {
            let mut data = self.state.lock();
            data.keyboard_rows_set_place = enabled;
        }
        self.persist_current_state()
    }

    pub(crate) fn update_instrument(&self, instrument: u8) -> Result<()> {
        {
            let mut data = self.state.lock();
            let instrument = crate::Instrument::from_u8(instrument);
            data.instrument = instrument.as_u8();
            if let Some(wash) = instrument.drone_wash() {
                data.ding_wash = wash.min(2);
            }
        }
        self.persist_current_state()
    }

    pub(crate) fn update_fm_bass_params(&self, params: squawk_engine::FmBassParams) -> Result<()> {
        let p = params.clamp();
        {
            let mut data = self.state.lock();
            data.fm_bass_decay = ParameterId::FmBassDecay.clamp(p.decay);
            data.fm_bass_ratio = ParameterId::FmBassRatio.clamp(p.ratio);
            data.fm_bass_index = ParameterId::FmBassIndex.clamp(p.index);
            data.fm_bass_index_decay = ParameterId::FmBassIndexDecay.clamp(p.index_decay);
            data.fm_bass_feedback = ParameterId::FmBassFeedback.clamp(p.feedback);
            data.fm_bass_level = ParameterId::FmBassLevel.clamp(p.level);
        }
        self.persist_current_state()
    }

    pub(crate) fn save_fm_bass_preset(
        &self,
        name: &str,
        params: squawk_engine::FmBassParams,
    ) -> Result<()> {
        let name = name.trim();
        anyhow::ensure!(!name.is_empty(), "Preset name is empty");
        {
            let mut data = self.state.lock();
            data.fm_bass_presets
                .insert(name.to_string(), StoredFmBassPreset::from_params(params));
        }
        self.persist_current_state()
    }

    pub(crate) fn delete_fm_bass_preset(&self, name: &str) -> Result<()> {
        {
            let mut data = self.state.lock();
            data.fm_bass_presets.remove(name);
        }
        self.persist_current_state()
    }

    pub(crate) fn fm_bass_presets(&self) -> Vec<(String, squawk_engine::FmBassParams)> {
        let data = self.state.lock();
        let mut presets: Vec<_> = data
            .fm_bass_presets
            .iter()
            .map(|(name, preset)| (name.clone(), preset.clone().into_params()))
            .collect();
        presets.sort_by(|a, b| a.0.cmp(&b.0));
        presets
    }

    pub(crate) fn update_exciter_mode(&self, mode: u8) -> Result<()> {
        {
            let mut data = self.state.lock();
            data.exciter_mode = mode.min(1);
        }
        self.persist_current_state()
    }

    pub(crate) fn update_mapping(&self, param: ParameterId, cc: u8) -> Result<()> {
        {
            let mut data = self.state.lock();
            data.midi_cc.insert(param.storage_key().to_string(), cc);
        }
        self.persist_current_state()
    }

    pub(crate) fn remove_mapping(&self, param: ParameterId) -> Result<()> {
        {
            let mut data = self.state.lock();
            data.midi_cc.remove(param.storage_key());
        }
        self.persist_current_state()
    }

    pub(crate) fn reset_parameters(&self) -> Result<()> {
        {
            let mut data = self.state.lock();
            data.exciter_mode = default_exciter_mode();
            data.attack_ms = crate::DEFAULT_ATTACK_MS;
            data.release_ms = crate::DEFAULT_RELEASE_MS;
            data.damping_ratio = crate::DEFAULT_DAMPING_RATIO;
            data.inharmonicity = crate::DEFAULT_INHARMONICITY;
            data.position_coupling = crate::DEFAULT_POSITION_COUPLING;
            data.velocity_coupling = crate::DEFAULT_VELOCITY_COUPLING;
            data.excitation_gain = crate::DEFAULT_EXCITATION_GAIN;
            data.output_gain = crate::DEFAULT_OUTPUT_GAIN;
            data.breath_pressure = crate::DEFAULT_BREATH_PRESSURE;
            data.breath_noise_cutoff_hz = crate::DEFAULT_BREATH_NOISE_CUTOFF_HZ;
            data.breath_feedback = crate::DEFAULT_BREATH_FEEDBACK;
            data.breath_auto_level = crate::DEFAULT_BREATH_AUTO_LEVEL;
            data.handpan_scale = default_handpan_scale();
            data.handpan_ding = default_handpan_ding();
            data.handpan_tuning = default_handpan_tuning();
            data.handpan_decay = crate::DEFAULT_HANDPAN_DECAY;
            data.handpan_detune_cents = crate::DEFAULT_HANDPAN_DETUNE_CENTS;
            data.handpan_coupling = crate::DEFAULT_HANDPAN_COUPLING;
            data.handpan_hit = crate::DEFAULT_HANDPAN_HIT;
            data.handpan_glide = crate::DEFAULT_HANDPAN_GLIDE;
            data.handpan_fundamental = crate::DEFAULT_HANDPAN_FUNDAMENTAL;
            data.handpan_octave = crate::DEFAULT_HANDPAN_OCTAVE;
            data.handpan_fifth = crate::DEFAULT_HANDPAN_FIFTH;
            data.handpan_higher_decay = crate::DEFAULT_HANDPAN_HIGHER_DECAY;
            data.support_level = crate::DEFAULT_SUPPORT_LEVEL;
            data.support_pulse = crate::DEFAULT_SUPPORT_PULSE;
            data.support_width = crate::DEFAULT_SUPPORT_WIDTH;
            data.support_wander = crate::DEFAULT_SUPPORT_WANDER;
            data.support_drift = crate::DEFAULT_SUPPORT_DRIFT;
            data.support_motion = crate::DEFAULT_SUPPORT_MOTION;
            data.support_fade = crate::DEFAULT_SUPPORT_FADE;
            data.reverb_handpan = crate::DEFAULT_REVERB_HANDPAN;
            data.reverb_support = crate::DEFAULT_REVERB_SUPPORT;
            data.reverb_ding = Some(crate::DEFAULT_REVERB_DING);
            data.reverb_recent = Some(crate::DEFAULT_REVERB_RECENT);
            data.reverb_second = Some(crate::DEFAULT_REVERB_SECOND);
            data.reverb_bass = Some(crate::DEFAULT_REVERB_BASS);
            data.reverb_echo = crate::DEFAULT_REVERB_ECHO;
            data.reverb_decay = crate::DEFAULT_REVERB_DECAY;
            data.reverb_tone = crate::DEFAULT_REVERB_TONE;
            data.echo_instrument = crate::DEFAULT_ECHO_INSTRUMENT;
            data.ding_wash = default_ding_wash();
            data.ding_shape = crate::DEFAULT_DING_SHAPE;
            data.ding_spread = crate::DEFAULT_DING_SPREAD;
            data.ding_brightness = crate::DEFAULT_DING_BRIGHTNESS;
            data.ding_strings = crate::DEFAULT_DING_STRINGS;
            data.tambura_jawari = crate::DEFAULT_TAMBURA_JAWARI;
            data.ding_formant_1 = crate::DEFAULT_DING_FORMANT_1;
            data.ding_formant_2 = crate::DEFAULT_DING_FORMANT_2;
            data.ding_formant_3 = crate::DEFAULT_DING_FORMANT_3;
            data.ding_resonance = crate::DEFAULT_DING_RESONANCE;
            data.ding_q_1 = crate::DEFAULT_DING_RESONANCE;
            data.ding_q_2 = crate::DEFAULT_DING_RESONANCE;
            data.ding_q_3 = crate::DEFAULT_DING_RESONANCE;
            data.mix_handpan = crate::DEFAULT_MIX_LEVEL;
            data.mix_ding = crate::DEFAULT_MIX_LEVEL;
            data.mix_recent = crate::DEFAULT_MIX_LEVEL;
            data.mix_second = crate::DEFAULT_MIX_LEVEL;
            data.mix_bass = crate::DEFAULT_MIX_LEVEL;
            data.mix_taps = crate::DEFAULT_MIX_LEVEL;
            data.deco_density = crate::DEFAULT_DECO_DENSITY;
            data.deco_lag_beats = crate::DEFAULT_DECO_LAG_BEATS;
            data.echo_feedback = None;
            data.pan_handpan = crate::DEFAULT_PAN;
            data.pan_ding = crate::DEFAULT_PAN;
            data.pan_recent = crate::DEFAULT_PAN;
            data.pan_second = crate::DEFAULT_PAN;
            data.pan_bass = crate::DEFAULT_PAN;
            data.pan_echo = crate::DEFAULT_PAN;
            data.formant_rate = crate::DEFAULT_FORMANT_RATE;
            data.formant_depth = crate::DEFAULT_FORMANT_DEPTH;
            data.groove_sensitivity = crate::DEFAULT_GROOVE_SENSITIVITY;
            data.groove_phase_pull = crate::DEFAULT_GROOVE_PHASE_PULL;
            data.groove_click_level = crate::DEFAULT_GROOVE_CLICK_LEVEL;
            data.fm_bass_decay = crate::DEFAULT_FM_BASS_DECAY;
            data.fm_bass_ratio = crate::DEFAULT_FM_BASS_RATIO;
            data.fm_bass_index = crate::DEFAULT_FM_BASS_INDEX;
            data.fm_bass_index_decay = crate::DEFAULT_FM_BASS_INDEX_DECAY;
            data.fm_bass_feedback = crate::DEFAULT_FM_BASS_FEEDBACK;
            data.fm_bass_level = crate::DEFAULT_FM_BASS_LEVEL;
            data.bass_line_amount = crate::DEFAULT_BASS_LINE_AMOUNT;
            data.bass_line_variation = crate::DEFAULT_BASS_LINE_VARIATION;
            data.bells_decay = crate::DEFAULT_BELLS_DECAY;
            data.bells_ratio = crate::DEFAULT_BELLS_RATIO;
            data.bells_index = crate::DEFAULT_BELLS_INDEX;
            data.bells_index_decay = crate::DEFAULT_BELLS_INDEX_DECAY;
            data.groove_click_enabled = crate::DEFAULT_GROOVE_CLICK_ENABLED;
            data.groove_auto_lock = crate::DEFAULT_GROOVE_AUTO_LOCK;
            data.support_muted = default_support_part_flags();
            data.support_solo = default_support_part_flags();
            data.handpan_muted = false;
            data.handpan_solo = false;
            data.keyboard_rows_set_place = default_keyboard_rows_set_place();
        }
        self.persist_current_state()
    }

    fn persist_current_state(&self) -> Result<()> {
        let data = self.state.lock().clone();
        let serialized = serde_json::to_vec_pretty(&data)?;
        let tmp_path = self.path.with_extension("tmp");
        fs::write(&tmp_path, &serialized).with_context(|| {
            format!(
                "Failed to write temporary settings file: {}",
                tmp_path.display()
            )
        })?;
        fs::rename(&tmp_path, &self.path).with_context(|| {
            format!("Failed to replace settings file at {}", self.path.display())
        })?;
        Ok(())
    }
}

fn read_settings(path: &PathBuf) -> Option<StoredSettings> {
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<StoredSettings>(&bytes) {
            Ok(parsed) => Some(parsed),
            Err(err) => {
                eprintln!(
                    "Warning: Failed to parse settings at {}: {err}",
                    path.display()
                );
                None
            }
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => None,
        Err(err) => {
            eprintln!(
                "Warning: Failed to read settings at {}: {err}",
                path.display()
            );
            None
        }
    }
}

fn resolve_settings_path() -> Result<PathBuf> {
    if let Some(proj_dirs) = ProjectDirs::from("com", "OrgBrain", "squawk") {
        let dir = proj_dirs.config_dir().to_path_buf();
        fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create config directory {}", dir.display()))?;
        return Ok(dir.join(SETTINGS_FILE_NAME));
    }

    let fallback = env::current_dir()
        .context("Failed to resolve current directory for fallback settings path")?
        .join(".squawk");
    fs::create_dir_all(&fallback).with_context(|| {
        format!(
            "Failed to create fallback settings directory {}",
            fallback.display()
        )
    })?;
    Ok(fallback.join(SETTINGS_FILE_NAME))
}

fn default_inharmonicity() -> f32 {
    crate::DEFAULT_INHARMONICITY
}

fn default_position_coupling() -> f32 {
    crate::DEFAULT_POSITION_COUPLING
}

fn default_velocity_coupling() -> f32 {
    crate::DEFAULT_VELOCITY_COUPLING
}

fn default_excitation_gain() -> f32 {
    crate::DEFAULT_EXCITATION_GAIN
}

fn default_output_gain() -> f32 {
    crate::DEFAULT_OUTPUT_GAIN
}

fn default_breath_pressure() -> f32 {
    crate::DEFAULT_BREATH_PRESSURE
}

fn default_breath_noise_cutoff_hz() -> f32 {
    crate::DEFAULT_BREATH_NOISE_CUTOFF_HZ
}

fn default_breath_feedback() -> f32 {
    crate::DEFAULT_BREATH_FEEDBACK
}

fn default_breath_auto_level() -> f32 {
    crate::DEFAULT_BREATH_AUTO_LEVEL
}

fn default_exciter_mode() -> u8 {
    0
}

fn default_handpan_scale() -> u8 {
    crate::DEFAULT_HANDPAN_SCALE
}

fn default_handpan_ding() -> u8 {
    crate::DEFAULT_HANDPAN_DING
}

fn default_handpan_tuning() -> u8 {
    crate::DEFAULT_HANDPAN_TUNING
}

fn default_instrument() -> u8 {
    0
}

fn default_handpan_decay() -> f32 {
    crate::DEFAULT_HANDPAN_DECAY
}

fn default_handpan_detune_cents() -> f32 {
    crate::DEFAULT_HANDPAN_DETUNE_CENTS
}

fn default_handpan_coupling() -> f32 {
    crate::DEFAULT_HANDPAN_COUPLING
}

fn default_handpan_hit() -> f32 {
    crate::DEFAULT_HANDPAN_HIT
}

fn default_handpan_glide() -> f32 {
    crate::DEFAULT_HANDPAN_GLIDE
}

fn default_handpan_fundamental() -> f32 {
    crate::DEFAULT_HANDPAN_FUNDAMENTAL
}

fn default_handpan_octave() -> f32 {
    crate::DEFAULT_HANDPAN_OCTAVE
}

fn default_handpan_fifth() -> f32 {
    crate::DEFAULT_HANDPAN_FIFTH
}

fn default_handpan_higher_decay() -> f32 {
    crate::DEFAULT_HANDPAN_HIGHER_DECAY
}

fn default_support_level() -> f32 {
    crate::DEFAULT_SUPPORT_LEVEL
}

fn default_support_pulse() -> f32 {
    crate::DEFAULT_SUPPORT_PULSE
}

fn default_support_width() -> f32 {
    crate::DEFAULT_SUPPORT_WIDTH
}

fn default_support_wander() -> f32 {
    crate::DEFAULT_SUPPORT_WANDER
}

fn default_support_drift() -> f32 {
    crate::DEFAULT_SUPPORT_DRIFT
}

fn default_support_motion() -> f32 {
    crate::DEFAULT_SUPPORT_MOTION
}

fn default_support_fade() -> f32 {
    crate::DEFAULT_SUPPORT_FADE
}

fn default_reverb_handpan() -> f32 {
    crate::DEFAULT_REVERB_HANDPAN
}

fn default_reverb_support() -> f32 {
    crate::DEFAULT_REVERB_SUPPORT
}

fn default_reverb_echo() -> f32 {
    crate::DEFAULT_REVERB_ECHO
}

fn default_echo_instrument() -> u8 {
    crate::DEFAULT_ECHO_INSTRUMENT
}

fn default_reverb_decay() -> f32 {
    crate::DEFAULT_REVERB_DECAY
}

fn default_reverb_tone() -> f32 {
    crate::DEFAULT_REVERB_TONE
}

fn default_ding_wash() -> u8 {
    crate::DEFAULT_DING_WASH
}

fn default_ding_shape() -> f32 {
    crate::DEFAULT_DING_SHAPE
}

fn default_ding_spread() -> f32 {
    crate::DEFAULT_DING_SPREAD
}

fn default_ding_brightness() -> f32 {
    crate::DEFAULT_DING_BRIGHTNESS
}

fn default_ding_strings() -> f32 {
    crate::DEFAULT_DING_STRINGS
}

fn default_tambura_jawari() -> f32 {
    crate::DEFAULT_TAMBURA_JAWARI
}

fn default_ding_formant_1() -> f32 {
    crate::DEFAULT_DING_FORMANT_1
}

fn default_ding_formant_2() -> f32 {
    crate::DEFAULT_DING_FORMANT_2
}

fn default_ding_formant_3() -> f32 {
    crate::DEFAULT_DING_FORMANT_3
}

fn default_ding_resonance() -> f32 {
    crate::DEFAULT_DING_RESONANCE
}

fn default_mix_level() -> f32 {
    crate::DEFAULT_MIX_LEVEL
}

fn default_deco_density() -> f32 {
    crate::DEFAULT_DECO_DENSITY
}

fn default_deco_lag_beats() -> f32 {
    crate::DEFAULT_DECO_LAG_BEATS
}

fn default_pan() -> f32 {
    crate::DEFAULT_PAN
}

fn default_formant_rate() -> f32 {
    crate::DEFAULT_FORMANT_RATE
}

fn default_formant_depth() -> f32 {
    crate::DEFAULT_FORMANT_DEPTH
}

fn default_groove_sensitivity() -> f32 {
    crate::DEFAULT_GROOVE_SENSITIVITY
}

fn default_groove_phase_pull() -> f32 {
    crate::DEFAULT_GROOVE_PHASE_PULL
}

fn default_groove_click_level() -> f32 {
    crate::DEFAULT_GROOVE_CLICK_LEVEL
}

fn default_fm_bass_decay() -> f32 {
    crate::DEFAULT_FM_BASS_DECAY
}

fn default_fm_bass_ratio() -> f32 {
    crate::DEFAULT_FM_BASS_RATIO
}

fn default_fm_bass_index() -> f32 {
    crate::DEFAULT_FM_BASS_INDEX
}

fn default_fm_bass_index_decay() -> f32 {
    crate::DEFAULT_FM_BASS_INDEX_DECAY
}

fn default_fm_bass_feedback() -> f32 {
    crate::DEFAULT_FM_BASS_FEEDBACK
}

fn default_fm_bass_level() -> f32 {
    crate::DEFAULT_FM_BASS_LEVEL
}

fn default_bass_line_amount() -> f32 {
    crate::DEFAULT_BASS_LINE_AMOUNT
}

fn default_bass_line_variation() -> f32 {
    crate::DEFAULT_BASS_LINE_VARIATION
}

fn default_bells_decay() -> f32 {
    crate::DEFAULT_BELLS_DECAY
}

fn default_bells_ratio() -> f32 {
    crate::DEFAULT_BELLS_RATIO
}

fn default_bells_index() -> f32 {
    crate::DEFAULT_BELLS_INDEX
}

fn default_bells_index_decay() -> f32 {
    crate::DEFAULT_BELLS_INDEX_DECAY
}

fn default_groove_click_enabled() -> bool {
    crate::DEFAULT_GROOVE_CLICK_ENABLED
}

fn default_groove_auto_lock() -> bool {
    crate::DEFAULT_GROOVE_AUTO_LOCK
}

fn default_support_part_flags() -> [bool; 5] {
    [false; 5]
}

fn default_keyboard_rows_set_place() -> bool {
    false
}
