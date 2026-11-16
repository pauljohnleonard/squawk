//! Times one second of the handpan audio callback and the glide power it calls.
//!
//! ```text
//! cargo run --release -p squawk-engine --example handpan_cpu
//! ```
//!
//! The printed fraction is CPU time divided by one second of audio. 0.05 means
//! the callback used 5% of one core to stay in real time. This does not open
//! the window.

use std::hint::black_box;
use std::time::Instant;

use squawk_engine::{HANDPAN_GLIDE_RANGE, Handpan, handpan_notes};

const SAMPLE_RATE: f32 = 48_000.0;
const ONE_SECOND: usize = 48_000;
/// 10 pads x 8 modes, one power per mode per sample.
const POW_CALLS: usize = 10 * 8 * ONE_SECOND;

fn main() {
    let notes = handpan_notes(0, 50);
    let max_glide = HANDPAN_GLIDE_RANGE.1;

    println!("handpan next_sample, 48 kHz, one second of audio");
    report("4 pads ringing, glide off", time_handpan(0.0, &notes[..4]));
    report(
        "4 pads ringing, glide 0.40",
        time_handpan(max_glide, &notes[..4]),
    );
    report(
        "10 pads ringing, glide 0.40",
        time_handpan(max_glide, &notes),
    );
    report("glide power, 80 calls x 48 kHz", time_glide_powers());
}

fn report(label: &str, runs: [f64; 3]) {
    let mut ordered = runs;
    ordered.sort_by(f64::total_cmp);
    let best = ordered[0];
    println!(
        "  {label}: {best:.4} of real time (best of {:.4}, {:.4}, {:.4})",
        runs[0], runs[1], runs[2]
    );
}

/// Strike the notes, then time the following second while they ring.
fn time_handpan(glide: f32, notes: &[u8]) -> [f64; 3] {
    let mut runs = [0.0; 3];
    for run in &mut runs {
        let mut handpan = Handpan::new(SAMPLE_RATE);
        handpan.set_glide(glide);
        for _ in 0..64 {
            black_box(handpan.next_sample());
        }
        for &note in notes {
            handpan.strike_midi(note, 0.25);
        }
        let started = Instant::now();
        let mut energy = 0.0f32;
        for _ in 0..ONE_SECOND {
            let sample = handpan.next_sample();
            energy += sample * sample;
        }
        black_box(energy);
        *run = started.elapsed().as_secs_f64();
    }
    runs
}

/// The same power the mode loop uses: `(2^(cents/1200))^2`.
fn time_glide_powers() -> [f64; 3] {
    let mut runs = [0.0; 3];
    for run in &mut runs {
        let started = Instant::now();
        let mut acc = 0.0f32;
        for index in 0..POW_CALLS {
            let cents = (index % 64) as f32 * 0.9;
            let ratio = 2.0_f32.powf(cents / 1200.0);
            acc += ratio * ratio;
        }
        black_box(acc);
        *run = started.elapsed().as_secs_f64();
    }
    runs
}
