//! Groove-locked bass skeleton into a StrikeVoice.
//!
//! Shares a [`PhraseClock`](crate::phrase_clock::PhraseClock) with tabla: by
//! default the root lands only on phrase sam (step 0). Variation is reserved
//! for denser phrase placements later.

use crate::handpan::handpan_notes;

pub const DEFAULT_BASS_LINE_AMOUNT: f32 = 0.7;
pub const BASS_LINE_AMOUNT_RANGE: (f32, f32) = (0.0, 1.0);
pub const DEFAULT_BASS_LINE_VARIATION: f32 = 0.35;
pub const BASS_LINE_VARIATION_RANGE: (f32, f32) = (0.0, 1.0);

/// Phrase step where the bass root lands (sam / downbeat of the shared theka).
pub const DEFAULT_BASS_PHRASE_HIT: u16 = 0;

const PAD_COUNT: usize = 10;
const MIN_AMP: f32 = 0.05;
const SAM_AMP: f32 = 0.9;

/// Schedules sparse bass strikes on a shared accompaniment phrase.
pub struct BassLine {
    amount: f32,
    variation: f32,
    /// Phrase step that triggers the root hit (0 = sam).
    phrase_hit: u16,
    notes: [u8; PAD_COUNT],
}

impl BassLine {
    pub fn new(_sample_rate: f32) -> Self {
        Self {
            amount: DEFAULT_BASS_LINE_AMOUNT,
            variation: DEFAULT_BASS_LINE_VARIATION,
            phrase_hit: DEFAULT_BASS_PHRASE_HIT,
            notes: handpan_notes(0, 50),
        }
    }

    pub fn set_amount(&mut self, value: f32) {
        self.amount = value.clamp(BASS_LINE_AMOUNT_RANGE.0, BASS_LINE_AMOUNT_RANGE.1);
    }

    pub fn set_variation(&mut self, value: f32) {
        self.variation = value.clamp(BASS_LINE_VARIATION_RANGE.0, BASS_LINE_VARIATION_RANGE.1);
    }

    pub fn set_phrase_hit(&mut self, step: u16) {
        self.phrase_hit = step;
    }

    pub fn phrase_hit(&self) -> u16 {
        self.phrase_hit
    }

    pub fn set_layout(&mut self, scale: u8, ding: u8) {
        self.notes = handpan_notes(scale, ding);
    }

    /// Live pad strike — reserved for future harmony following.
    pub fn hear(&mut self, _note: u8, _amp: f32) {}

    /// Fire when the shared phrase clock lands on this line's hit step.
    pub fn on_phrase_step(&self, step: u16, mut on_hit: impl FnMut(u8, f32, f32)) {
        if self.amount <= 0.0 || step != self.phrase_hit {
            return;
        }
        let note = self.notes[0]; // root / ding
        let amp = (SAM_AMP * self.amount).clamp(MIN_AMP, 1.0);
        on_hit(note, amp, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::groove::GrooveTracker;
    use crate::phrase_clock::PhraseClock;

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
    fn amount_zero_stays_silent() {
        let mut line = BassLine::new(RATE);
        line.set_amount(0.0);
        let mut hits = 0u32;
        line.on_phrase_step(0, |_, _, _| hits += 1);
        assert_eq!(hits, 0);
    }

    #[test]
    fn only_fires_on_phrase_hit_step() {
        let mut line = BassLine::new(RATE);
        line.set_amount(1.0);
        line.set_phrase_hit(0);
        let mut hits = 0u32;
        for step in 0..8u16 {
            line.on_phrase_step(step, |_, _, _| hits += 1);
        }
        assert_eq!(hits, 1);
    }

    #[test]
    fn once_per_phrase_with_shared_clock() {
        let mut line = BassLine::new(RATE);
        line.set_amount(1.0);
        let mut clock = PhraseClock::new();
        clock.set_phrase_len(8);
        let mut groove = GrooveTracker::new(RATE);
        let period_s = 60.0 / 80.0;
        lock_four_taps(&mut groove, period_s);

        let mut hits = 0u32;
        let samples_per_beat = (period_s * RATE).round() as usize;
        // Two full 8-beat phrases.
        for _ in 0..(samples_per_beat * 16) {
            if let Some(step) = clock.tick(&groove) {
                line.on_phrase_step(step, |_, _, _| hits += 1);
            }
            let _ = groove.tick();
        }
        assert!(
            (2..=3).contains(&hits),
            "expected roughly one bass hit per 8-beat phrase, got {hits}"
        );
    }

    #[test]
    fn plays_root_note() {
        let mut line = BassLine::new(RATE);
        line.set_layout(0, 50);
        let root = handpan_notes(0, 50)[0];
        line.set_amount(1.0);
        let mut note = 0u8;
        line.on_phrase_step(0, |n, _, _| note = n);
        assert_eq!(note, root);
    }
}
