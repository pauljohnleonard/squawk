//! Groove-locked bass skeleton into a StrikeVoice.
//!
//! Simple v1: once the groove is locked, play the root on every beat — strong
//! on 1, softer on 2/3/4. Variation / cell patterns stay as knobs for later.

use crate::groove::GrooveTracker;
use crate::handpan::handpan_notes;

pub const DEFAULT_BASS_LINE_AMOUNT: f32 = 0.7;
pub const BASS_LINE_AMOUNT_RANGE: (f32, f32) = (0.0, 1.0);
pub const DEFAULT_BASS_LINE_VARIATION: f32 = 0.35;
pub const BASS_LINE_VARIATION_RANGE: (f32, f32) = (0.0, 1.0);

const PAD_COUNT: usize = 10;
const MIN_AMP: f32 = 0.05;
const BEAT1_AMP: f32 = 0.9;
const OTHER_BEAT_AMP: f32 = 0.38;

/// Schedules sparse bass strikes when the groove is locked.
pub struct BassLine {
    amount: f32,
    variation: f32,
    notes: [u8; PAD_COUNT],
    prev_groove_phase: f32,
    beat_in_bar: u8,
    armed: bool,
}

impl BassLine {
    pub fn new(_sample_rate: f32) -> Self {
        Self {
            amount: DEFAULT_BASS_LINE_AMOUNT,
            variation: DEFAULT_BASS_LINE_VARIATION,
            notes: handpan_notes(0, 50),
            prev_groove_phase: 0.0,
            beat_in_bar: 0,
            armed: false,
        }
    }

    pub fn set_amount(&mut self, value: f32) {
        self.amount = value.clamp(BASS_LINE_AMOUNT_RANGE.0, BASS_LINE_AMOUNT_RANGE.1);
    }

    pub fn set_variation(&mut self, value: f32) {
        self.variation = value.clamp(BASS_LINE_VARIATION_RANGE.0, BASS_LINE_VARIATION_RANGE.1);
    }

    pub fn set_layout(&mut self, scale: u8, ding: u8) {
        self.notes = handpan_notes(scale, ding);
    }

    /// Live pad strike — reserved for future harmony following.
    pub fn hear(&mut self, _note: u8, _amp: f32) {}

    /// Advance one sample with the groove. Emits due skeleton hits through `on_hit`.
    pub fn tick(&mut self, groove: &GrooveTracker, mut on_hit: impl FnMut(u8, f32, f32)) {
        if !groove.tempo_locked() || self.amount <= 0.0 {
            self.armed = false;
            self.beat_in_bar = 0;
            self.prev_groove_phase = groove.phase();
            return;
        }

        let groove_phase = groove.phase();
        let wrapped = self.armed && groove_phase + 0.25 < self.prev_groove_phase;
        self.prev_groove_phase = groove_phase;

        if !self.armed {
            // Align bar to the locked downbeat: next wrap starts beat 1.
            self.armed = true;
            self.beat_in_bar = 0;
            // If we just locked (phase near 0), fire beat 1 immediately.
            if groove_phase < 0.08 {
                self.fire_beat(0, &mut on_hit);
            }
            return;
        }

        if !wrapped {
            return;
        }

        self.beat_in_bar = (self.beat_in_bar + 1) % 4;
        self.fire_beat(self.beat_in_bar, &mut on_hit);
    }

    fn fire_beat(&self, beat: u8, on_hit: &mut impl FnMut(u8, f32, f32)) {
        let note = self.notes[0]; // root / ding
        let base = if beat == 0 { BEAT1_AMP } else { OTHER_BEAT_AMP };
        let amp = (base * self.amount).clamp(MIN_AMP, 1.0);
        on_hit(note, amp, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::groove::GrooveTracker;

    const RATE: f32 = 48_000.0;

    fn lock_four_taps(groove: &mut GrooveTracker, period_s: f32) {
        groove.set_auto_lock(true);
        let step = (period_s * RATE).round() as u64;
        for _ in 0..4 {
            for _ in 0..step {
                let _ = groove.tick();
            }
            groove.hear(50, 0.7);
        }
        assert!(groove.tempo_locked(), "expected lock after four taps");
    }

    #[test]
    fn silent_until_locked() {
        let mut line = BassLine::new(RATE);
        let mut groove = GrooveTracker::new(RATE);
        groove.set_auto_lock(false);
        line.set_amount(1.0);
        let mut hits = 0u32;
        for _ in 0..(RATE as usize) {
            line.tick(&groove, |_, _, _| hits += 1);
            let _ = groove.tick();
        }
        assert_eq!(hits, 0);
    }

    #[test]
    fn amount_zero_stays_silent_when_locked() {
        let mut line = BassLine::new(RATE);
        let mut groove = GrooveTracker::new(RATE);
        lock_four_taps(&mut groove, 60.0 / 80.0);
        line.set_amount(0.0);
        let mut hits = 0u32;
        for _ in 0..(RATE as usize * 2) {
            line.tick(&groove, |_, _, _| hits += 1);
            let _ = groove.tick();
        }
        assert_eq!(hits, 0);
    }

    #[test]
    fn four_on_floor_when_locked() {
        let mut line = BassLine::new(RATE);
        let mut groove = GrooveTracker::new(RATE);
        lock_four_taps(&mut groove, 60.0 / 80.0);
        line.set_amount(1.0);
        let mut hits = 0u32;
        let mut strong = 0u32;
        let seconds = 4.0;
        for _ in 0..((RATE * seconds) as usize) {
            line.tick(&groove, |_, amp, _| {
                hits += 1;
                if amp > 0.7 {
                    strong += 1;
                }
            });
            let _ = groove.tick();
        }
        // ~80 BPM × 4s ≈ 5–6 beats; one hit per beat.
        assert!(
            (5..=8).contains(&hits),
            "expected roughly one hit per beat over {seconds}s, got {hits}"
        );
        assert!(
            strong >= 1 && strong <= hits,
            "expected some strong beat-1 hits, strong {strong} of {hits}"
        );
    }

    #[test]
    fn plays_root_note() {
        let mut line = BassLine::new(RATE);
        line.set_layout(0, 50);
        let root = handpan_notes(0, 50)[0];
        let mut groove = GrooveTracker::new(RATE);
        lock_four_taps(&mut groove, 60.0 / 90.0);
        line.set_amount(1.0);
        let mut note = 0u8;
        for _ in 0..(RATE as usize) {
            line.tick(&groove, |n, _, _| note = n);
            let _ = groove.tick();
            if note != 0 {
                break;
            }
        }
        assert_eq!(note, root);
    }
}
