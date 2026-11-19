//! Shared accompaniment phrase clock.
//!
//! Advances on GrooveTracker beat wraps and walks an N-step cycle so bass,
//! tabla, and later parts stay locked to the same theka/phrase grid.

use crate::groove::GrooveTracker;

/// Beat-wrap phrase counter shared by accompaniment lines.
pub struct PhraseClock {
    len: u16,
    step: u16,
    prev_groove_phase: f32,
    armed: bool,
}

impl PhraseClock {
    pub fn new() -> Self {
        Self {
            len: 8,
            step: 0,
            prev_groove_phase: 0.0,
            armed: false,
        }
    }

    pub fn phrase_len(&self) -> u16 {
        self.len
    }

    pub fn step(&self) -> u16 {
        self.step
    }

    /// Set phrase length in beats/matras. Only resets the step when length changes.
    pub fn set_phrase_len(&mut self, len: u16) {
        let len = len.max(1);
        if len == self.len {
            return;
        }
        self.len = len;
        self.step = 0;
    }

    /// Advance one sample. Returns the step that just began, if any.
    pub fn tick(&mut self, groove: &GrooveTracker) -> Option<u16> {
        if !groove.tempo_locked() {
            self.armed = false;
            self.step = 0;
            self.prev_groove_phase = groove.phase();
            return None;
        }

        let groove_phase = groove.phase();
        let wrapped = self.armed && groove_phase + 0.25 < self.prev_groove_phase;
        self.prev_groove_phase = groove_phase;

        if !self.armed {
            self.armed = true;
            self.step = 0;
            if groove_phase < 0.08 {
                return Some(0);
            }
            return None;
        }

        if !wrapped {
            return None;
        }

        self.step = (self.step + 1) % self.len;
        Some(self.step)
    }
}

impl Default for PhraseClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(groove.tempo_locked());
    }

    #[test]
    fn silent_until_locked() {
        let mut clock = PhraseClock::new();
        let mut groove = GrooveTracker::new(RATE);
        groove.set_auto_lock(false);
        let mut hits = 0u32;
        for _ in 0..(RATE as usize / 2) {
            if clock.tick(&groove).is_some() {
                hits += 1;
            }
            let _ = groove.tick();
        }
        assert_eq!(hits, 0);
    }

    #[test]
    fn walks_cycle_when_locked() {
        let mut clock = PhraseClock::new();
        clock.set_phrase_len(8);
        let mut groove = GrooveTracker::new(RATE);
        let period_s = 60.0 / 100.0;
        lock_four_taps(&mut groove, period_s);

        let mut steps = Vec::new();
        let samples_per_beat = (period_s * RATE).round() as usize;
        for _ in 0..(samples_per_beat * 10) {
            if let Some(step) = clock.tick(&groove) {
                steps.push(step);
            }
            let _ = groove.tick();
        }
        assert!(steps.len() >= 8, "got {steps:?}");
        assert_eq!(steps[0], 0);
        assert!(steps.windows(2).any(|w| w[0] == 7 && w[1] == 0));
    }

    #[test]
    fn same_len_preserves_step() {
        let mut clock = PhraseClock::new();
        clock.set_phrase_len(8);
        clock.step = 3;
        clock.set_phrase_len(8);
        assert_eq!(clock.step(), 3);
        clock.set_phrase_len(6);
        assert_eq!(clock.step(), 0);
        assert_eq!(clock.phrase_len(), 6);
    }
}
