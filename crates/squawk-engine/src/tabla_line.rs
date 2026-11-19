//! Groove-locked tabla theka scheduler.
//!
//! Fires catalog bols on steps from a shared [`PhraseClock`](crate::phrase_clock::PhraseClock)
//! so tabla stays aligned with bass and other accompaniment.

use crate::handpan::handpan_notes;
use crate::tabla::TablaStroke;
use crate::tabla_grooves::{DEFAULT_TABLA_GROOVE_ID, GROOVES, TablaGroove};

pub const DEFAULT_TABLA_LINE_AMOUNT: f32 = 0.65;
pub const TABLA_LINE_AMOUNT_RANGE: (f32, f32) = (0.0, 1.0);

const PAD_COUNT: usize = 10;
const MIN_AMP: f32 = 0.04;

/// One expanded strike for the voice callback.
#[derive(Debug, Clone, Copy)]
pub struct TablaLineHit {
    pub stroke: TablaStroke,
    pub note: u8,
    pub amp: f32,
    pub place: f32,
}

/// Maps phrase steps onto factory thekas for a [`Tabla`](crate::tabla::Tabla) voice.
pub struct TablaLine {
    amount: f32,
    groove_index: usize,
    notes: [u8; PAD_COUNT],
}

impl TablaLine {
    pub fn new(_sample_rate: f32) -> Self {
        Self {
            amount: DEFAULT_TABLA_LINE_AMOUNT,
            groove_index: groove_index_for_id(DEFAULT_TABLA_GROOVE_ID).unwrap_or(0),
            notes: handpan_notes(0, 50),
        }
    }

    pub fn set_amount(&mut self, value: f32) {
        self.amount = value.clamp(TABLA_LINE_AMOUNT_RANGE.0, TABLA_LINE_AMOUNT_RANGE.1);
    }

    pub fn set_groove_index(&mut self, index: usize) {
        if index < GROOVES.len() {
            self.groove_index = index;
        }
    }

    pub fn set_groove_id(&mut self, id: &str) {
        if let Some(index) = groove_index_for_id(id) {
            self.set_groove_index(index);
        }
    }

    pub fn groove_index(&self) -> usize {
        self.groove_index
    }

    pub fn groove(&self) -> &'static TablaGroove {
        &GROOVES[self.groove_index.min(GROOVES.len() - 1)]
    }

    /// Phrase length for the shared clock (matras × steps_per_matra).
    pub fn phrase_len(&self) -> u16 {
        self.groove().steps().max(1)
    }

    pub fn set_layout(&mut self, scale: u8, ding: u8) {
        self.notes = handpan_notes(scale, ding);
    }

    pub fn hear(&mut self, _note: u8, _amp: f32) {}

    /// Fire bols for a phrase step from the shared clock.
    pub fn on_phrase_step(&self, step: u16, mut on_hit: impl FnMut(TablaLineHit)) {
        if self.amount <= 0.0 {
            return;
        }
        self.fire_step(step, &mut on_hit);
    }

    fn fire_step(&self, step: u16, on_hit: &mut impl FnMut(TablaLineHit)) {
        let ding = self.notes[0];
        let dayan = if self.notes.len() > 2 {
            self.notes[2]
        } else {
            ding.saturating_add(7)
        };
        for hit in self.groove().hits_at(step) {
            let amp = (hit.amp * self.amount).clamp(MIN_AMP, 1.0);
            for &(stroke, scale) in hit.bol.expand() {
                let note = match stroke {
                    TablaStroke::Ga | TablaStroke::Ge | TablaStroke::Ka => ding,
                    _ => dayan,
                };
                let place = match stroke {
                    TablaStroke::Ga | TablaStroke::Ge | TablaStroke::Tu => 0.0,
                    TablaStroke::Ka | TablaStroke::Ta => 0.5,
                    TablaStroke::Na => 1.0,
                };
                on_hit(TablaLineHit {
                    stroke,
                    note,
                    amp: (amp * scale).clamp(MIN_AMP, 1.0),
                    place,
                });
            }
        }
    }
}

fn groove_index_for_id(id: &str) -> Option<usize> {
    GROOVES.iter().position(|g| g.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phrase_clock::PhraseClock;
    use crate::tabla_grooves::groove_by_id;
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
        assert!(groove.tempo_locked());
    }

    #[test]
    fn amount_zero_silent() {
        let line = {
            let mut line = TablaLine::new(RATE);
            line.set_amount(0.0);
            line
        };
        let mut hits = 0u32;
        line.on_phrase_step(0, |_| hits += 1);
        assert_eq!(hits, 0);
    }

    #[test]
    fn keherwa_fires_dha_on_sam() {
        let mut line = TablaLine::new(RATE);
        line.set_layout(0, 50);
        line.set_groove_id("keherwa");
        line.set_amount(1.0);
        let mut strokes = Vec::new();
        line.on_phrase_step(0, |hit| strokes.push(hit.stroke));
        assert!(
            strokes.contains(&TablaStroke::Ge) && strokes.contains(&TablaStroke::Na),
            "sam Dha should expand to Ge+Na, got {strokes:?}"
        );
    }

    #[test]
    fn groove_switch_updates_phrase_len() {
        let mut line = TablaLine::new(RATE);
        line.set_groove_id("keherwa");
        assert_eq!(line.phrase_len(), 8);
        line.set_groove_id("dadra");
        assert_eq!(line.groove().id, "dadra");
        assert_eq!(line.phrase_len(), 6);
        assert_eq!(groove_by_id("dadra").unwrap().matras, 6);
    }

    #[test]
    fn shared_clock_walks_distinct_bols() {
        let mut line = TablaLine::new(RATE);
        line.set_layout(0, 50);
        line.set_groove_id("keherwa");
        line.set_amount(1.0);
        let mut clock = PhraseClock::new();
        clock.set_phrase_len(line.phrase_len());
        let mut groove = GrooveTracker::new(RATE);
        let period_s = 60.0 / 100.0;
        lock_four_taps(&mut groove, period_s);

        let mut first_stroke_per_beat = Vec::new();
        let samples_per_beat = (period_s * RATE).round() as usize;
        for _ in 0..(samples_per_beat * 10) {
            clock.set_phrase_len(line.phrase_len());
            if let Some(step) = clock.tick(&groove) {
                let before = first_stroke_per_beat.len();
                line.on_phrase_step(step, |hit| {
                    if first_stroke_per_beat.len() == before {
                        first_stroke_per_beat.push(hit.stroke);
                    }
                });
            }
            let _ = groove.tick();
        }

        assert!(
            first_stroke_per_beat.len() >= 8,
            "expected a full cycle of hits, got {}",
            first_stroke_per_beat.len()
        );
        let first = first_stroke_per_beat[0];
        assert!(
            first_stroke_per_beat.iter().any(|&s| s != first),
            "keherwa should vary strokes across the cycle, got {first_stroke_per_beat:?}"
        );
    }
}
