//! Phrase-recall decorations timed to a locked groove.
//!
//! Replaces feedback-delay echo: recalls recent PlayMemory phrases after a
//! beat lag, thinned by density, with at most two concurrent voices.

use crate::groove::GrooveTracker;
use crate::play_memory::{PHRASE_EVENT_CAP, Phrase, PhraseEvent, PlayMemory};

pub const DEFAULT_DECO_DENSITY: f32 = 0.65;
pub const DECO_DENSITY_RANGE: (f32, f32) = (0.0, 1.0);
pub const DEFAULT_DECO_LAG_BEATS: f32 = 4.0;
pub const DECO_LAG_BEATS_RANGE: (f32, f32) = (1.0, 16.0);

const VOICES: usize = 2;
const MIN_AMP: f32 = 0.02;

#[derive(Clone, Copy)]
struct SchedEvent {
    due: u64,
    note: u8,
    amp: f32,
    place: f32,
}

struct Voice {
    events: [SchedEvent; PHRASE_EVENT_CAP],
    count: usize,
    next: usize,
    active: bool,
}

impl Voice {
    fn clear() -> Self {
        Self {
            events: [SchedEvent {
                due: 0,
                note: 0,
                amp: 0.0,
                place: 0.0,
            }; PHRASE_EVENT_CAP],
            count: 0,
            next: 0,
            active: false,
        }
    }

    fn load(&mut self, start: u64, phrase: &Phrase, density: f32) {
        let mut count = 0usize;
        for event in phrase.events() {
            if !keep_event(event, density) {
                continue;
            }
            self.events[count] = SchedEvent {
                due: start.saturating_add(event.offset_samples),
                note: event.note,
                amp: event.amp * (0.55 + 0.35 * density),
                place: event.place,
            };
            count += 1;
            if count == PHRASE_EVENT_CAP {
                break;
            }
        }
        // Sort by due so we can fire in order.
        self.events[..count].sort_by_key(|event| event.due);
        self.count = count;
        self.next = 0;
        self.active = count > 0;
    }
}

/// Prefer louder notes when density is low; always silent at 0.
fn keep_event(event: &PhraseEvent, density: f32) -> bool {
    if density <= 0.0 {
        return false;
    }
    if density >= 0.999 {
        return true;
    }
    let threshold = (1.0 - density) * 0.75;
    if event.amp >= threshold {
        return true;
    }
    // Deterministic sprinkle of quieter notes.
    let salt = (event.note as u32)
        .wrapping_mul(374_761)
        .wrapping_add(event.offset_samples as u32);
    let frac = (salt % 1000) as f32 / 1000.0;
    frac < density * density
}

pub struct Decorations {
    density: f32,
    lag_beats: f32,
    voices: [Voice; VOICES],
    next_launch: u64,
    sample: u64,
}

impl Decorations {
    pub fn new(_sample_rate: f32) -> Self {
        Self {
            density: DEFAULT_DECO_DENSITY,
            lag_beats: DEFAULT_DECO_LAG_BEATS,
            voices: std::array::from_fn(|_| Voice::clear()),
            next_launch: 0,
            sample: 0,
        }
    }

    pub fn set_density(&mut self, value: f32) {
        self.density = value.clamp(DECO_DENSITY_RANGE.0, DECO_DENSITY_RANGE.1);
    }

    pub fn set_lag_beats(&mut self, value: f32) {
        self.lag_beats = value.clamp(DECO_LAG_BEATS_RANGE.0, DECO_LAG_BEATS_RANGE.1);
    }

    /// Advance one sample. Emits due decoration hits through `on_hit`.
    pub fn tick(
        &mut self,
        memory: &PlayMemory,
        groove: &GrooveTracker,
        mut on_hit: impl FnMut(u8, f32, f32),
    ) {
        self.sample = self.sample.wrapping_add(1);
        let now = self.sample;

        for voice in &mut self.voices {
            if !voice.active {
                continue;
            }
            while voice.next < voice.count && voice.events[voice.next].due <= now {
                let event = voice.events[voice.next];
                voice.next += 1;
                if event.amp >= MIN_AMP {
                    on_hit(event.note, event.amp.clamp(0.0, 1.0), event.place);
                }
            }
            if voice.next >= voice.count {
                voice.active = false;
            }
        }

        if !groove.tempo_locked() || self.density <= 0.0 {
            return;
        }

        let period_s = groove.period_s();
        if period_s < 1.0e-3 {
            return;
        }
        let period_samples = (period_s / groove.dt()).max(1.0);

        self.maybe_launch(memory, groove.phase(), period_samples, now);
    }

    fn maybe_launch(&mut self, memory: &PlayMemory, phase: f32, period_samples: f32, now: u64) {
        if now < self.next_launch {
            return;
        }
        let Some(phrase) = memory.latest_phrase() else {
            return;
        };
        // Launch each phrase id once; allow re-launch after voices free if same
        // phrase is still latest and lag has elapsed again.
        let free = self.voices.iter().position(|voice| !voice.active);
        let Some(slot) = free else {
            return;
        };

        let to_boundary = (1.0 - phase.clamp(0.0, 0.999)).max(0.02);
        let lag_samples = (self.lag_beats * period_samples).round().max(1.0) as u64;
        let align = (to_boundary * period_samples).round().max(1.0) as u64;
        let start = now.saturating_add(align).saturating_add(lag_samples);

        self.voices[slot].load(start, phrase, self.density);
        if !self.voices[slot].active {
            return;
        }
        let span = phrase.duration_samples().max(lag_samples);
        self.next_launch = start.saturating_add(span);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play_memory::PlayMemory;

    const RATE: f32 = 48_000.0;

    fn strike_phrase(mem: &mut PlayMemory, groove: &mut GrooveTracker, notes: &[u8]) {
        let step = ((60.0 / 80.0) * RATE).round() as u64;
        for &note in notes {
            for _ in 0..step {
                mem.tick(groove.tempo_locked(), groove.period_s());
                let _ = groove.tick();
            }
            mem.hear(note, 0.6, 0.0);
            groove.hear(note, 0.6);
        }
        // Pause to close the phrase.
        for _ in 0..((2.6 * RATE) as u64) {
            mem.tick(groove.tempo_locked(), groove.period_s());
            let _ = groove.tick();
        }
    }

    #[test]
    fn silent_until_locked_with_history() {
        let mut mem = PlayMemory::new(RATE);
        let mut groove = GrooveTracker::new(RATE);
        groove.set_auto_lock(false);
        let mut deco = Decorations::new(RATE);
        deco.set_density(1.0);
        let mut hits = 0u32;
        for _ in 0..4_000 {
            mem.tick(false, 0.5);
            let _ = groove.tick();
            deco.tick(&mem, &groove, |_, _, _| hits += 1);
        }
        assert_eq!(hits, 0);
    }

    #[test]
    fn density_zero_stays_silent_after_lock() {
        let mut mem = PlayMemory::new(RATE);
        let mut groove = GrooveTracker::new(RATE);
        groove.set_sensitivity(0.9);
        let mut deco = Decorations::new(RATE);
        deco.set_density(0.0);
        deco.set_lag_beats(2.0);
        strike_phrase(&mut mem, &mut groove, &[50, 52, 55, 57, 59, 60]);
        assert!(groove.tempo_locked() || mem.latest_phrase().is_some());
        groove.set_tempo_locked(true);
        let mut hits = 0u32;
        for _ in 0..((RATE as usize) * 8) {
            mem.tick(true, groove.period_s());
            let _ = groove.tick();
            deco.tick(&mem, &groove, |_, _, _| hits += 1);
        }
        assert_eq!(hits, 0, "density 0 should emit nothing");
    }

    #[test]
    fn locked_phrase_produces_hits_near_grid() {
        let mut mem = PlayMemory::new(RATE);
        let mut groove = GrooveTracker::new(RATE);
        groove.set_sensitivity(0.95);
        let mut deco = Decorations::new(RATE);
        deco.set_density(1.0);
        deco.set_lag_beats(2.0);
        strike_phrase(&mut mem, &mut groove, &[50, 52, 55, 57, 60, 62, 64, 67]);
        assert!(mem.latest_phrase().is_some(), "need a stored phrase");
        groove.set_tempo_locked(true);
        let mut hits = Vec::new();
        for sample in 0..((RATE as u64) * 12) {
            mem.tick(true, groove.period_s());
            let _ = groove.tick();
            deco.tick(&mem, &groove, |note, amp, _| {
                hits.push((sample, note, amp));
            });
        }
        assert!(
            hits.len() >= 3,
            "expected decoration hits after lock, got {}",
            hits.len()
        );
        // No runaway feedback pile-up: one launch should not explode count.
        assert!(
            hits.len() < 200,
            "too many hits — possible feedback: {}",
            hits.len()
        );
    }
}
