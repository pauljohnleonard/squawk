//! Shared onset log and phrase segmentation from live pad strikes.
//!
//! Consumed by Decorations (and later pads / bass / percussion). Echo delay
//! feedback does not write here — only the player's hits.

const FLAM_S: f32 = 0.080;
const PAUSE_S: f32 = 2.5;
const ONSET_CAP: usize = 512;
pub(crate) const PHRASE_EVENT_CAP: usize = 64;
const PHRASE_CAP: usize = 4;
const MIN_PHRASE_EVENTS: usize = 2;
/// When tempo is locked, close the open phrase after this many beats.
const MAX_PHRASE_BEATS: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Onset {
    pub sample: u64,
    pub note: u8,
    pub amp: f32,
    pub place: f32,
}

/// One completed (or snapshot of open) phrase: times relative to the first onset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhraseEvent {
    pub offset_samples: u64,
    pub note: u8,
    pub amp: f32,
    pub place: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Phrase {
    pub events: [PhraseEvent; PHRASE_EVENT_CAP],
    pub count: usize,
    pub id: u64,
}

impl Phrase {
    fn empty(id: u64) -> Self {
        Self {
            events: [PhraseEvent {
                offset_samples: 0,
                note: 0,
                amp: 0.0,
                place: 0.0,
            }; PHRASE_EVENT_CAP],
            count: 0,
            id,
        }
    }

    pub fn events(&self) -> &[PhraseEvent] {
        &self.events[..self.count]
    }

    pub fn duration_samples(&self) -> u64 {
        self.events()
            .last()
            .map(|event| event.offset_samples)
            .unwrap_or(0)
    }
}

pub struct PlayMemory {
    dt: f32,
    sample: u64,
    last_onset: Option<u64>,
    silence_s: f32,
    onsets: [Onset; ONSET_CAP],
    onset_n: usize,
    onset_i: usize,
    /// Indices into the onset ring for the open phrase (oldest → newest order via walk).
    open_start_sample: Option<u64>,
    open_count: usize,
    phrases: [Phrase; PHRASE_CAP],
    phrase_n: usize,
    phrase_i: usize,
    next_phrase_id: u64,
}

impl PlayMemory {
    pub fn new(sample_rate: f32) -> Self {
        let sample_rate = sample_rate.max(1.0);
        Self {
            dt: 1.0 / sample_rate,
            sample: 0,
            last_onset: None,
            silence_s: 0.0,
            onsets: [Onset {
                sample: 0,
                note: 0,
                amp: 0.0,
                place: 0.0,
            }; ONSET_CAP],
            onset_n: 0,
            onset_i: 0,
            open_start_sample: None,
            open_count: 0,
            phrases: std::array::from_fn(|index| Phrase::empty(index as u64)),
            phrase_n: 0,
            phrase_i: 0,
            next_phrase_id: 1,
        }
    }

    pub fn sample(&self) -> u64 {
        self.sample
    }

    pub fn phrase_count(&self) -> usize {
        self.phrase_n
    }

    /// Most recently completed phrase, if any.
    pub fn latest_phrase(&self) -> Option<&Phrase> {
        if self.phrase_n == 0 {
            return None;
        }
        let index = if self.phrase_n < PHRASE_CAP {
            self.phrase_n - 1
        } else {
            (self.phrase_i + PHRASE_CAP - 1) % PHRASE_CAP
        };
        let phrase = &self.phrases[index];
        if phrase.count >= MIN_PHRASE_EVENTS {
            Some(phrase)
        } else {
            None
        }
    }

    /// Advance one sample. Closes the open phrase on pause or beat-length cap.
    pub fn tick(&mut self, tempo_locked: bool, period_s: f32) {
        self.sample = self.sample.wrapping_add(1);
        self.silence_s += self.dt;
        if self.silence_s > PAUSE_S {
            self.close_open_phrase();
        } else if tempo_locked
            && period_s > 1.0e-3
            && let Some(start) = self.open_start_sample
        {
            let span_s = self.sample.saturating_sub(start) as f32 * self.dt;
            if span_s >= MAX_PHRASE_BEATS * period_s {
                self.close_open_phrase();
            }
        }
    }

    /// Record a live strike.
    pub fn hear(&mut self, note: u8, amp: f32, place: f32) {
        let amp = amp.clamp(0.0, 1.0);
        let place = place.clamp(0.0, 1.0);
        if amp < 0.01 {
            return;
        }
        let now = self.sample;
        if let Some(previous) = self.last_onset {
            let gap_s = now.saturating_sub(previous) as f32 * self.dt;
            if gap_s < FLAM_S {
                return;
            }
        }
        self.last_onset = Some(now);
        self.silence_s = 0.0;
        self.push_onset(Onset {
            sample: now,
            note,
            amp,
            place,
        });
        if self.open_start_sample.is_none() {
            self.open_start_sample = Some(now);
            self.open_count = 0;
        }
        self.open_count = self.open_count.saturating_add(1).min(PHRASE_EVENT_CAP);
    }

    fn push_onset(&mut self, onset: Onset) {
        if self.onset_n < ONSET_CAP {
            self.onsets[self.onset_n] = onset;
            self.onset_n += 1;
        } else {
            self.onsets[self.onset_i] = onset;
            self.onset_i = (self.onset_i + 1) % ONSET_CAP;
        }
    }

    fn close_open_phrase(&mut self) {
        let Some(start) = self.open_start_sample else {
            return;
        };
        if self.open_count < MIN_PHRASE_EVENTS {
            self.open_start_sample = None;
            self.open_count = 0;
            return;
        }
        let mut phrase = Phrase::empty(self.next_phrase_id);
        self.next_phrase_id = self.next_phrase_id.wrapping_add(1);
        let n = self.onset_n.min(ONSET_CAP);
        // Collect onsets belonging to this phrase (sample >= start), oldest first.
        let mut collected = 0usize;
        for index in 0..n {
            let slot = if self.onset_n < ONSET_CAP {
                index
            } else {
                (self.onset_i + index) % ONSET_CAP
            };
            let onset = self.onsets[slot];
            if onset.sample < start {
                continue;
            }
            if collected >= PHRASE_EVENT_CAP {
                break;
            }
            phrase.events[collected] = PhraseEvent {
                offset_samples: onset.sample.saturating_sub(start),
                note: onset.note,
                amp: onset.amp,
                place: onset.place,
            };
            collected += 1;
        }
        phrase.count = collected;
        self.open_start_sample = None;
        self.open_count = 0;
        if phrase.count < MIN_PHRASE_EVENTS {
            return;
        }
        if self.phrase_n < PHRASE_CAP {
            self.phrases[self.phrase_n] = phrase;
            self.phrase_n += 1;
        } else {
            self.phrases[self.phrase_i] = phrase;
            self.phrase_i = (self.phrase_i + 1) % PHRASE_CAP;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn advance(mem: &mut PlayMemory, samples: u64, locked: bool, period_s: f32) {
        for _ in 0..samples {
            mem.tick(locked, period_s);
        }
    }

    #[test]
    fn silent_until_struck() {
        let mut mem = PlayMemory::new(RATE);
        advance(&mut mem, 1_000, false, 0.5);
        assert_eq!(mem.phrase_count(), 0);
        assert!(mem.latest_phrase().is_none());
    }

    #[test]
    fn grows_onsets_and_closes_phrase_on_pause() {
        let mut mem = PlayMemory::new(RATE);
        let step = (0.4 * RATE) as u64;
        for note in [50u8, 52, 55, 57] {
            mem.hear(note, 0.5, 0.0);
            advance(&mut mem, step, false, 0.5);
        }
        assert_eq!(mem.phrase_count(), 0);
        advance(&mut mem, (PAUSE_S * RATE) as u64 + 10, false, 0.5);
        let phrase = mem.latest_phrase().expect("phrase after pause");
        assert_eq!(phrase.count, 4);
        assert_eq!(phrase.events()[0].note, 50);
        assert_eq!(phrase.events()[3].note, 57);
        assert!(phrase.duration_samples() > 0);
    }

    #[test]
    fn beat_cap_closes_phrase_when_locked() {
        let mut mem = PlayMemory::new(RATE);
        let period = 0.5_f32;
        let step = (0.45 * RATE) as u64;
        for note in 0..12u8 {
            mem.hear(60 + note, 0.4, 0.5);
            advance(&mut mem, step, true, period);
        }
        assert!(
            mem.phrase_count() >= 1,
            "expected a phrase from the beat cap"
        );
    }
}
