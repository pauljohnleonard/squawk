//! Shared strike-voice surface for main and accompaniment instruments.
//!
//! New voices implement this so the app can swap them into a role (Echo today,
//! main and drones later) without rewriting the mix callback each time.

/// A pitched strike instrument that can be driven by MIDI-like hits.
pub trait StrikeVoice {
    /// Strike `note` at `place` (0 = center, 1 = shoulder / edge). Amplitude is 0..1.
    fn strike_at(&mut self, note: u8, amplitude: f32, place: f32);

    /// One mono sample of the sounding voice pool.
    fn next_sample(&mut self) -> f32;
}
