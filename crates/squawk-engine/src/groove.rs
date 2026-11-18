//! Event-based tempo and phase lock from pad strikes.
//!
//! Simple path: the first four accepted taps set the period (mean of the three
//! gaps), snap beat 0 to the fourth tap, and lock. The click then free-runs.
//! Unlock clears the count so four fresh taps can lock again.

use core::f32::consts::TAU;

pub const GROOVE_BPM_MIN: f32 = 30.0;
pub const GROOVE_BPM_MAX: f32 = 150.0;
/// When half/double is ambiguous, prefer a period near this BPM.
pub const GROOVE_BPM_PREF: f32 = 75.0;
pub const DEFAULT_GROOVE_SENSITIVITY: f32 = 0.55;
pub const DEFAULT_GROOVE_PHASE_PULL: f32 = 0.35;
pub const DEFAULT_GROOVE_CLICK_LEVEL: f32 = 0.45;
pub const DEFAULT_GROOVE_AUTO_LOCK: bool = true;
pub const GROOVE_SENSITIVITY_RANGE: (f32, f32) = (0.1, 1.0);
pub const GROOVE_PHASE_PULL_RANGE: (f32, f32) = (0.0, 1.0);
pub const GROOVE_CLICK_LEVEL_RANGE: (f32, f32) = (0.0, 1.0);

const FLAM_S: f32 = 0.080;
const PAUSE_S: f32 = 2.5;
const IOI_CAP: usize = 12;
/// Four taps → three gaps → lock. Kept for confidence-based status while learning.
const MIN_IOIS_FOR_TEMPO: u8 = 3;
const TAPS_TO_LOCK: u8 = 4;
/// After a manual unlock, auto-lock waits until confidence falls below this.
const AUTO_REARM_CONFIDENCE: f32 = 0.35;
const CLICK_HZ: f32 = 1_850.0;
const CLICK_DECAY_S: f32 = 0.028;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrooveStatus {
    Listening,
    Locking,
    Locked,
    Lost,
}

#[derive(Debug, Clone, Copy)]
pub struct GrooveSnapshot {
    pub bpm: f32,
    pub confidence: f32,
    pub phase: f32,
    pub locked: bool,
    pub status: GrooveStatus,
    /// Recent inter-onset intervals in milliseconds, oldest first.
    pub iois_ms: [f32; IOI_CAP],
    pub ioi_count: usize,
}

pub struct GrooveTracker {
    dt: f32,
    sample: u64,
    last_onset: Option<u64>,
    iois_s: [f32; IOI_CAP],
    ioi_n: usize,
    ioi_i: usize,
    iois_seen: u8,
    period_s: f32,
    phase: f32,
    /// Latched after the first strike that defines beat 0 for this lock.
    phase_acquired: bool,
    /// When true, period stops updating from new IOIs (phase still steers).
    tempo_locked: bool,
    /// Auto-freeze period once confidence crosses the lock threshold.
    auto_lock: bool,
    /// Cleared on manual unlock so auto-lock cannot fire again immediately.
    auto_arm: bool,
    confidence: f32,
    sensitivity: f32,
    phase_pull: f32,
    click_level: f32,
    click_enabled: bool,
    click_amp: f32,
    click_phase: f32,
    click_inc: f32,
    click_decay: f32,
    silence_s: f32,
    /// Accepted taps since last unlock (flam-filtered). Lock fires at [`TAPS_TO_LOCK`].
    taps_since_unlock: u8,
}

impl GrooveTracker {
    pub fn new(sample_rate: f32) -> Self {
        let sample_rate = sample_rate.max(1.0);
        let dt = 1.0 / sample_rate;
        Self {
            dt,
            sample: 0,
            last_onset: None,
            iois_s: [0.0; IOI_CAP],
            ioi_n: 0,
            ioi_i: 0,
            iois_seen: 0,
            period_s: 60.0 / GROOVE_BPM_PREF,
            phase: 0.0,
            phase_acquired: false,
            tempo_locked: false,
            auto_lock: DEFAULT_GROOVE_AUTO_LOCK,
            auto_arm: true,
            confidence: 0.0,
            sensitivity: DEFAULT_GROOVE_SENSITIVITY,
            phase_pull: DEFAULT_GROOVE_PHASE_PULL,
            click_level: DEFAULT_GROOVE_CLICK_LEVEL,
            click_enabled: true,
            click_amp: 0.0,
            click_phase: 0.0,
            click_inc: TAU * CLICK_HZ * dt,
            click_decay: (-dt / CLICK_DECAY_S).exp(),
            silence_s: 0.0,
            taps_since_unlock: 0,
        }
    }

    pub fn set_sensitivity(&mut self, value: f32) {
        self.sensitivity = value.clamp(GROOVE_SENSITIVITY_RANGE.0, GROOVE_SENSITIVITY_RANGE.1);
    }

    pub fn set_phase_pull(&mut self, value: f32) {
        self.phase_pull = value.clamp(GROOVE_PHASE_PULL_RANGE.0, GROOVE_PHASE_PULL_RANGE.1);
    }

    pub fn set_click_level(&mut self, value: f32) {
        self.click_level = value.clamp(GROOVE_CLICK_LEVEL_RANGE.0, GROOVE_CLICK_LEVEL_RANGE.1);
    }

    pub fn set_click_enabled(&mut self, enabled: bool) {
        self.click_enabled = enabled;
    }

    pub fn set_auto_lock(&mut self, enabled: bool) {
        self.auto_lock = enabled;
    }

    /// Freeze or release the tempo estimate. Unlocking clears the tap count so
    /// four fresh taps can lock again.
    pub fn set_tempo_locked(&mut self, locked: bool) {
        self.tempo_locked = locked;
        if !locked {
            self.taps_since_unlock = 0;
            self.auto_arm = true;
        }
    }

    pub fn tempo_locked(&self) -> bool {
        self.tempo_locked
    }

    pub fn period_s(&self) -> f32 {
        self.period_s
    }

    pub fn phase(&self) -> f32 {
        self.phase.clamp(0.0, 1.0)
    }

    pub fn dt(&self) -> f32 {
        self.dt
    }

    /// A live pad strike. Flam and pause rules match Support.
    pub fn hear(&mut self, _note: u8, amp: f32) {
        let now = self.sample;
        let amp = amp.clamp(0.0, 1.0);
        if amp < 0.01 {
            return;
        }

        if let Some(previous) = self.last_onset {
            let gap_s = now.saturating_sub(previous) as f32 * self.dt;
            if gap_s < FLAM_S {
                return;
            }
            if gap_s < PAUSE_S {
                self.push_ioi(gap_s);
            } else if !self.tempo_locked {
                // Long pause before lock: start the four-tap count fresh.
                self.taps_since_unlock = 0;
                self.ioi_n = 0;
                self.ioi_i = 0;
                self.iois_seen = 0;
            }
        }
        self.last_onset = Some(now);
        self.silence_s = 0.0;
        if !self.tempo_locked {
            self.taps_since_unlock = self.taps_since_unlock.saturating_add(1);
        }
        self.estimate_period();
        self.pull_phase();
        self.update_confidence();
        self.maybe_four_tap_lock();
    }

    /// Advance one sample. Returns the soft metronome click (may be 0).
    pub fn tick(&mut self) -> f32 {
        self.sample = self.sample.wrapping_add(1);
        self.silence_s += self.dt;
        // While tempo is held, keep confidence (and the click) steady through pauses.
        // Fade only when unlocked and learning again.
        if !self.tempo_locked && self.silence_s > PAUSE_S {
            let fall = 1.0 - (-self.dt / 0.6).exp();
            self.confidence += (0.0 - self.confidence) * fall * (0.4 + 0.6 * self.sensitivity);
            if self.confidence < 0.15 {
                self.phase_acquired = false;
            }
        }

        let period = self.period_s.max(60.0 / GROOVE_BPM_MAX);
        self.phase += self.dt / period;
        let mut wrapped = false;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            wrapped = true;
        }

        let click_gate = if self.tempo_locked {
            1.0
        } else {
            self.confidence
        };
        if wrapped && click_gate > 0.05 && self.click_enabled {
            self.click_amp = self.click_level * click_gate * 0.55;
            self.click_phase = 0.0;
        }

        self.click_amp *= self.click_decay;
        if self.click_amp < 1.0e-5 {
            self.click_amp = 0.0;
            return 0.0;
        }
        let sample = self.click_phase.sin() * self.click_amp;
        self.click_phase += self.click_inc;
        if self.click_phase >= TAU {
            self.click_phase -= TAU;
        }
        sample
    }

    pub fn snapshot(&self) -> GrooveSnapshot {
        let mut iois_ms = [0.0; IOI_CAP];
        let count = self.ioi_n;
        for (index, out) in iois_ms.iter_mut().enumerate().take(count) {
            let slot = if self.ioi_n < IOI_CAP {
                index
            } else {
                (self.ioi_i + index) % IOI_CAP
            };
            *out = self.iois_s[slot] * 1_000.0;
        }
        GrooveSnapshot {
            bpm: 60.0 / self.period_s.max(1.0e-3),
            confidence: self.confidence,
            phase: self.phase.clamp(0.0, 1.0),
            locked: self.tempo_locked,
            status: self.status(),
            iois_ms,
            ioi_count: count,
        }
    }

    fn status(&self) -> GrooveStatus {
        if self.iois_seen == 0 {
            GrooveStatus::Listening
        } else if self.tempo_locked {
            GrooveStatus::Locked
        } else if self.silence_s > PAUSE_S && self.confidence < 0.2 {
            GrooveStatus::Lost
        } else {
            GrooveStatus::Locking
        }
    }

    /// Lock after four taps: period = mean of the three gaps, phase = 0 on this tap.
    fn maybe_four_tap_lock(&mut self) {
        if self.confidence < AUTO_REARM_CONFIDENCE {
            self.auto_arm = true;
        }
        if !self.auto_lock || !self.auto_arm || self.tempo_locked {
            return;
        }
        if self.taps_since_unlock < TAPS_TO_LOCK || self.ioi_n < (TAPS_TO_LOCK as usize - 1) {
            return;
        }
        let mean = self.mean_recent_iois(TAPS_TO_LOCK as usize - 1);
        if mean <= 0.0 {
            return;
        }
        // Use the tapped gaps directly — no half/double guess — so the click
        // matches what the player just played.
        self.period_s = mean.clamp(60.0 / GROOVE_BPM_MAX, 60.0 / GROOVE_BPM_MIN);
        self.phase = 0.0;
        self.phase_acquired = true;
        self.tempo_locked = true;
        self.confidence = 1.0;
        if self.click_enabled && self.click_level > 0.05 {
            self.click_amp = self.click_level * 0.55;
            self.click_phase = 0.0;
        }
    }

    fn mean_recent_iois(&self, count: usize) -> f32 {
        let n = self.ioi_n.min(count).max(1);
        let mut sum = 0.0_f32;
        for age in 0..n {
            let slot = if self.ioi_n < IOI_CAP {
                self.ioi_n - 1 - age
            } else {
                (self.ioi_i + IOI_CAP - 1 - age) % IOI_CAP
            };
            sum += self.iois_s[slot];
        }
        sum / n as f32
    }

    fn push_ioi(&mut self, gap_s: f32) {
        if self.ioi_n < IOI_CAP {
            self.iois_s[self.ioi_n] = gap_s;
            self.ioi_n += 1;
        } else {
            self.iois_s[self.ioi_i] = gap_s;
            self.ioi_i = (self.ioi_i + 1) % IOI_CAP;
        }
        self.iois_seen = self.iois_seen.saturating_add(1);
    }

    fn estimate_period(&mut self) {
        if self.tempo_locked {
            return;
        }
        // Period is set firmly at four-tap lock; before that just track the mean
        // so the UI BPM readout stays sensible while learning.
        if self.ioi_n == 0 {
            return;
        }
        let mean = self.mean_recent_iois(self.ioi_n.min(3));
        if mean <= 0.0 {
            return;
        }
        self.period_s = mean.clamp(60.0 / GROOVE_BPM_MAX, 60.0 / GROOVE_BPM_MIN);
    }

    fn pull_phase(&mut self) {
        if self.iois_seen < MIN_IOIS_FOR_TEMPO {
            return;
        }
        // Hard lock: once tempo is held, the click free-runs — no phase chase,
        // so playing a little off does not drag the grid into a feedback loop.
        if self.tempo_locked {
            return;
        }
        // First eligible strike defines beat 0. Later off-beats (phase ~0.5 when
        // the chosen period is 2× the IOI) must not yank the oscillator.
        if !self.phase_acquired {
            self.phase = 0.0;
            self.phase_acquired = true;
            return;
        }
        // Distance of current phase from beat 0, wrapped to [-0.5, 0.5].
        let mut err = self.phase;
        if err > 0.5 {
            err -= 1.0;
        }
        const NEAR_BEAT: f32 = 0.22;
        if err.abs() > NEAR_BEAT {
            return;
        }
        let pull = self.phase_pull * (0.45 + 0.55 * self.confidence);
        self.phase -= err * pull;
        self.wrap_phase();
    }

    fn wrap_phase(&mut self) {
        while self.phase < 0.0 {
            self.phase += 1.0;
        }
        while self.phase >= 1.0 {
            self.phase -= 1.0;
        }
    }

    fn update_confidence(&mut self) {
        if self.iois_seen < MIN_IOIS_FOR_TEMPO {
            self.confidence = 0.0;
            return;
        }
        let target = self.ioi_agreement().clamp(0.0, 1.0);
        let rise = 0.12 + 0.35 * self.sensitivity;
        self.confidence += (target - self.confidence) * rise;
    }

    /// How tightly recent IOIs match the chosen period (or half / double).
    fn ioi_agreement(&self) -> f32 {
        let n = self.ioi_n;
        if n < MIN_IOIS_FOR_TEMPO as usize {
            return 0.0;
        }
        let period = self.period_s.max(1.0e-3);
        let targets = [period * 0.5, period, period * 2.0];
        let mut score = 0.0;
        let mut count = 0.0;
        for index in 0..n {
            let slot = if self.ioi_n < IOI_CAP {
                index
            } else {
                (self.ioi_i + index) % IOI_CAP
            };
            let ioi = self.iois_s[slot];
            let best_err = targets
                .iter()
                .map(|target| ((ioi - target) / target).abs())
                .fold(f32::MAX, f32::min);
            // Within ~12% of period, half, or double counts as agreement.
            let close = (1.0 - best_err / 0.12).clamp(0.0, 1.0);
            score += close;
            count += 1.0;
        }
        if count <= 0.0 { 0.0 } else { score / count }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn strike_every(groove: &mut GrooveTracker, period_s: f32, count: usize) {
        let step = (period_s * RATE).round() as u64;
        for _ in 0..count {
            for _ in 0..step {
                let _ = groove.tick();
            }
            groove.hear(60, 0.5);
        }
    }

    #[test]
    fn silent_until_struck() {
        let mut groove = GrooveTracker::new(RATE);
        for _ in 0..2_000 {
            assert_eq!(groove.tick(), 0.0);
        }
        assert_eq!(groove.snapshot().status, GrooveStatus::Listening);
    }

    #[test]
    fn four_taps_lock_at_played_tempo() {
        let mut groove = GrooveTracker::new(RATE);
        groove.set_click_level(1.0);
        strike_every(&mut groove, 60.0 / 80.0, 4);
        let snap = groove.snapshot();
        assert!(snap.locked, "expected lock on the fourth tap");
        assert!(
            (snap.bpm - 80.0).abs() < 4.0,
            "expected ~80 bpm from tapped gaps, got {}",
            snap.bpm
        );
        assert!((snap.confidence - 1.0).abs() < 0.01);
    }

    #[test]
    fn three_taps_do_not_lock() {
        let mut groove = GrooveTracker::new(RATE);
        strike_every(&mut groove, 60.0 / 90.0, 3);
        assert!(!groove.snapshot().locked);
    }

    #[test]
    fn locked_click_follows_tapped_period() {
        let mut groove = GrooveTracker::new(RATE);
        groove.set_click_level(1.0);
        let ioi = 0.5_f32; // 120 BPM
        strike_every(&mut groove, ioi, 4);
        let snap = groove.snapshot();
        assert!(snap.locked);
        assert!(
            (snap.bpm - 120.0).abs() < 4.0,
            "expected ~120 bpm (no half-time guess), got {}",
            snap.bpm
        );

        let period_samples = (ioi * RATE).round() as u64;
        let mut clicks = Vec::new();
        let mut prev = 0.0_f32;
        let mute_after = period_samples / 4;
        let mut armed_at = 0u64;
        for sample in 0..(period_samples * 6) {
            if sample > 0 && sample.is_multiple_of(period_samples / 2) {
                groove.hear(60, 0.8);
            }
            let x = groove.tick().abs();
            if sample >= armed_at && x > 0.02 && prev <= 0.02 {
                clicks.push(sample);
                armed_at = sample + mute_after;
            }
            prev = x;
        }
        assert!(
            clicks.len() >= 4,
            "expected several beat clicks, got {}",
            clicks.len()
        );
        let limit = (0.08 * RATE) as u64;
        for window in clicks.windows(2).skip(1) {
            let gap = window[1] - window[0];
            assert!(
                gap.abs_diff(period_samples) < limit,
                "click spacing {gap} should stay near locked period {period_samples}"
            );
        }
    }

    #[test]
    fn pause_before_lock_resets_tap_count() {
        let mut groove = GrooveTracker::new(RATE);
        strike_every(&mut groove, 60.0 / 100.0, 2);
        assert!(!groove.snapshot().locked);
        for _ in 0..(RATE as usize * 3) {
            let _ = groove.tick();
        }
        strike_every(&mut groove, 60.0 / 100.0, 3);
        assert!(
            !groove.snapshot().locked,
            "pause should have cleared the early taps"
        );
        strike_every(&mut groove, 60.0 / 100.0, 1);
        assert!(groove.snapshot().locked);
    }

    #[test]
    fn auto_lock_freezes_tempo_against_new_iois() {
        let mut groove = GrooveTracker::new(RATE);
        strike_every(&mut groove, 60.0 / 80.0, 4);
        let snap = groove.snapshot();
        assert!(snap.locked, "expected auto-lock after four taps");
        let locked_bpm = snap.bpm;
        strike_every(&mut groove, 60.0 / 120.0, 10);
        let after = groove.snapshot();
        assert!(after.locked);
        assert!(
            (after.bpm - locked_bpm).abs() < 0.5,
            "tempo moved while locked: {locked_bpm} → {}",
            after.bpm
        );
    }

    #[test]
    fn locked_click_stays_up_through_a_pause() {
        let mut groove = GrooveTracker::new(RATE);
        groove.set_click_level(1.0);
        strike_every(&mut groove, 60.0 / 90.0, 4);
        assert!(groove.snapshot().locked);
        let conf_before = groove.snapshot().confidence;
        for _ in 0..(RATE as usize * 4) {
            let _ = groove.tick();
        }
        assert!(groove.snapshot().locked);
        assert!(
            (groove.snapshot().confidence - conf_before).abs() < 0.05,
            "confidence should hold while locked through a pause"
        );
        let mut peak = 0.0_f32;
        for _ in 0..(RATE as usize) {
            peak = peak.max(groove.tick().abs());
        }
        assert!(peak > 0.05, "expected full click while locked, peak {peak}");
    }

    #[test]
    fn manual_unlock_allows_four_fresh_taps() {
        let mut groove = GrooveTracker::new(RATE);
        strike_every(&mut groove, 60.0 / 70.0, 4);
        assert!(groove.snapshot().locked);
        let locked_bpm = groove.snapshot().bpm;
        groove.set_tempo_locked(false);
        assert!(!groove.snapshot().locked);
        strike_every(&mut groove, 60.0 / 110.0, 4);
        let after = groove.snapshot();
        assert!(after.locked);
        assert!(
            (after.bpm - locked_bpm).abs() > 8.0,
            "tempo should move after unlock: was {locked_bpm}, now {}",
            after.bpm
        );
    }
}
