//! A fixed-length memory of pad hits. Each live strike is written onto two
//! delay lines. When an echo comes due it can be played again, then written
//! back onto the same line a little quieter. The lines are different lengths,
//! so the two copies of a phrase drift apart instead of locking into a bar.

pub const ECHO_DELAYS_S: [f32; 2] = [6.5, 10.1];
pub const DEFAULT_ECHO_FEEDBACK: f32 = 0.75;
pub const ECHO_FEEDBACK_RANGE: (f32, f32) = (0.0, 0.92);

const ECHO_MIN_AMP: f32 = 0.02;
const SUPPRESS_S: f32 = 0.150;
const SPACING_S: f32 = 0.080;
const LINE_CAP: usize = 160;

#[derive(Clone, Copy)]
struct Event {
    due: u64,
    note: u8,
    amp: f32,
    place: f32,
}

struct Line {
    delay: u64,
    spacing: u64,
    events: [Event; LINE_CAP],
    count: usize,
}

impl Line {
    fn new(delay: u64, spacing: u64) -> Self {
        Self {
            delay,
            spacing,
            events: [Event {
                due: 0,
                note: 0,
                amp: 0.0,
                place: 0.0,
            }; LINE_CAP],
            count: 0,
        }
    }

    fn suppress(&mut self, note: u8, now: u64, window: u64) {
        let mut index = 0;
        while index < self.count {
            let event = self.events[index];
            let soon = event.due.saturating_sub(now) <= window;
            if event.note == note && soon {
                self.remove(index);
            } else {
                index += 1;
            }
        }
    }

    fn insert(&mut self, due: u64, note: u8, amp: f32, place: f32) {
        if let Some(index) = self.near(note, due) {
            if amp > self.events[index].amp {
                self.events[index] = Event {
                    due,
                    note,
                    amp,
                    place,
                };
            }
            return;
        }
        if self.count == LINE_CAP {
            if let Some(index) = self.quietest()
                && self.events[index].amp <= amp
            {
                self.events[index] = Event {
                    due,
                    note,
                    amp,
                    place,
                };
            }
            return;
        }
        self.events[self.count] = Event {
            due,
            note,
            amp,
            place,
        };
        self.count += 1;
    }

    fn take_due(&mut self, now: u64) -> Option<Event> {
        let index = self
            .events
            .iter()
            .take(self.count)
            .position(|event| event.due <= now)?;
        let event = self.events[index];
        self.remove(index);
        Some(event)
    }

    fn near(&self, note: u8, due: u64) -> Option<usize> {
        self.events
            .iter()
            .take(self.count)
            .position(|event| event.note == note && due.abs_diff(event.due) < self.spacing)
    }

    fn quietest(&self) -> Option<usize> {
        let mut best = None;
        let mut amp = f32::MAX;
        for (index, event) in self.events.iter().take(self.count).enumerate() {
            if event.amp < amp {
                amp = event.amp;
                best = Some(index);
            }
        }
        best
    }

    fn remove(&mut self, index: usize) {
        self.count -= 1;
        self.events[index] = self.events[self.count];
    }
}

pub struct Echo {
    now: u64,
    feedback: f32,
    suppress: u64,
    lines: [Line; 2],
}

impl Echo {
    pub fn new(sample_rate_hz: f32) -> Self {
        let rate = sample_rate_hz.max(1.0);
        let spacing = seconds(SPACING_S, rate);
        Self {
            now: 0,
            feedback: DEFAULT_ECHO_FEEDBACK,
            suppress: seconds(SUPPRESS_S, rate),
            lines: [
                Line::new(seconds(ECHO_DELAYS_S[0], rate), spacing),
                Line::new(seconds(ECHO_DELAYS_S[1], rate), spacing),
            ],
        }
    }

    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback.clamp(ECHO_FEEDBACK_RANGE.0, ECHO_FEEDBACK_RANGE.1);
    }

    /// Remember a live pad hit. Echoes of this pad that are about to sound are dropped.
    pub fn record(&mut self, note: u8, amp: f32, place: f32) {
        let amp = amp.clamp(0.0, 1.0);
        let place = place.clamp(0.0, 1.0);
        if amp < ECHO_MIN_AMP {
            return;
        }
        for line in &mut self.lines {
            line.suppress(note, self.now, self.suppress);
            line.insert(self.now + line.delay, note, amp, place);
        }
    }

    /// Move one sample forward. Each due echo is handed to `on_echo`, then written
    /// back onto its own line at `amplitude * feedback` when that is still audible.
    pub fn poll(&mut self, mut on_echo: impl FnMut(u8, f32, f32)) {
        self.now = self.now.wrapping_add(1);
        let feedback = self.feedback;
        for line in &mut self.lines {
            let delay = line.delay;
            while let Some(event) = line.take_due(self.now) {
                on_echo(event.note, event.amp, event.place);
                let amp = event.amp * feedback;
                if amp >= ECHO_MIN_AMP {
                    line.insert(event.due + delay, event.note, amp, event.place);
                }
            }
        }
    }
}

fn seconds(duration: f32, sample_rate_hz: f32) -> u64 {
    (duration * sample_rate_hz).round().max(1.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Heard {
        at: u64,
        note: u8,
        amp: f32,
        place: f32,
    }

    fn advance(echo: &mut Echo, samples: u64, out: &mut Vec<Heard>, from: u64) {
        for step in 0..samples {
            let at = from + step + 1;
            echo.poll(|note, amp, place| {
                out.push(Heard {
                    at,
                    note,
                    amp,
                    place,
                });
            });
        }
    }

    #[test]
    fn a_phrase_returns_on_both_lines_and_then_fades() {
        let mut echo = Echo::new(1_000.0);
        echo.set_feedback(0.5);
        echo.record(50, 0.40, 0.25);
        advance(&mut echo, 200, &mut Vec::new(), 0);
        echo.record(57, 0.30, 1.0);
        let short = seconds(ECHO_DELAYS_S[0], 1_000.0);
        let long = seconds(ECHO_DELAYS_S[1], 1_000.0);
        let mut heard = Vec::new();
        let horizon = short * 6 + 50;
        advance(&mut echo, horizon - 200, &mut heard, 200);
        let first = heard
            .iter()
            .find(|hit| hit.note == 50 && (hit.amp - 0.40).abs() < 1.0e-4)
            .expect("the first note should come back");
        assert_eq!(first.at, short);
        assert!((first.place - 0.25).abs() < 1.0e-4);
        let second = heard
            .iter()
            .find(|hit| hit.note == 57 && (hit.amp - 0.30).abs() < 1.0e-4)
            .expect("the second note should keep its place in the phrase");
        assert_eq!(second.at, 200 + short);

        assert!(
            heard
                .iter()
                .any(|hit| hit.note == 50 && hit.at == long && (hit.amp - 0.40).abs() < 1.0e-4),
            "the longer line should repeat the phrase too"
        );
        assert!(
            heard.iter().any(|hit| {
                hit.note == 50 && hit.at == short * 2 && (hit.amp - 0.20).abs() < 1.0e-4
            }),
            "the next pass should be half as loud"
        );
        assert!(
            heard.iter().any(|hit| {
                hit.note == 50 && hit.at == short * 5 && (hit.amp - 0.025).abs() < 1.0e-4
            }),
            "the phrase should still be there on the fifth pass"
        );
        assert!(
            heard
                .iter()
                .all(|hit| !(hit.note == 50 && hit.at == short * 6)),
            "a faded echo should be dropped"
        );
    }

    #[test]
    fn a_live_strike_swallows_the_echo_about_to_sound() {
        let mut echo = Echo::new(1_000.0);
        echo.set_feedback(0.5);
        echo.record(50, 0.80, 0.0);
        let short = seconds(ECHO_DELAYS_S[0], 1_000.0);
        let mut heard = Vec::new();
        advance(&mut echo, short - 150, &mut heard, 0);
        echo.record(50, 0.40, 0.0);
        advance(&mut echo, 400, &mut heard, short - 150);
        assert!(
            heard.iter().all(|hit| hit.note != 50 || hit.amp < 0.79),
            "the echo of the pad just played should stay out of the way"
        );

        let mut echo = Echo::new(1_000.0);
        echo.record(50, 0.80, 0.0);
        let mut heard = Vec::new();
        advance(&mut echo, short - 150, &mut heard, 0);
        echo.record(62, 0.40, 0.0);
        advance(&mut echo, 400, &mut heard, short - 150);
        assert!(
            heard
                .iter()
                .any(|hit| hit.note == 50 && (hit.amp - 0.80).abs() < 1.0e-4),
            "a different pad should leave the echo in place"
        );
    }

    #[test]
    fn the_louder_of_two_close_echoes_stays() {
        let mut echo = Echo::new(1_000.0);
        echo.set_feedback(0.0);
        echo.record(50, 0.30, 0.0);
        echo.record(50, 0.90, 0.0);
        let mut heard = Vec::new();
        let short = seconds(ECHO_DELAYS_S[0], 1_000.0);
        advance(&mut echo, short + 10, &mut heard, 0);
        let shorts: Vec<_> = heard.iter().filter(|hit| hit.at <= short + 1).collect();
        assert_eq!(shorts.len(), 1);
        assert!((shorts[0].amp - 0.90).abs() < 1.0e-4);
    }
}
