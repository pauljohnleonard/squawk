//! A stereo tail shared by the handpan and the accompaniment.
//!
//! Eight delay lines share one decay. A lowpass lets the top die first, and a
//! highpass keeps the low end from sitting. The delays stay put: moving them
//! is what turns a tail into chorus.

use core::f32::consts::TAU;

pub const DEFAULT_REVERB_HANDPAN: f32 = 0.0;
pub const DEFAULT_REVERB_SUPPORT: f32 = 0.0;
pub const DEFAULT_REVERB_DING: f32 = DEFAULT_REVERB_SUPPORT;
pub const DEFAULT_REVERB_RECENT: f32 = DEFAULT_REVERB_SUPPORT;
pub const DEFAULT_REVERB_SECOND: f32 = DEFAULT_REVERB_SUPPORT;
pub const DEFAULT_REVERB_BASS: f32 = DEFAULT_REVERB_SUPPORT;
pub const DEFAULT_REVERB_ECHO: f32 = DEFAULT_REVERB_SUPPORT;
pub const DEFAULT_REVERB_DECAY: f32 = 2.2;
pub const DEFAULT_REVERB_TONE: f32 = 0.35;
pub const REVERB_HANDPAN_RANGE: (f32, f32) = (0.0, 1.0);
pub const REVERB_SUPPORT_RANGE: (f32, f32) = (0.0, 1.0);
pub const REVERB_DING_RANGE: (f32, f32) = REVERB_SUPPORT_RANGE;
pub const REVERB_RECENT_RANGE: (f32, f32) = REVERB_SUPPORT_RANGE;
pub const REVERB_SECOND_RANGE: (f32, f32) = REVERB_SUPPORT_RANGE;
pub const REVERB_BASS_RANGE: (f32, f32) = REVERB_SUPPORT_RANGE;
pub const REVERB_ECHO_RANGE: (f32, f32) = REVERB_SUPPORT_RANGE;
pub const REVERB_DECAY_RANGE: (f32, f32) = (0.5, 6.0);
pub const REVERB_TONE_RANGE: (f32, f32) = (0.0, 1.0);

const LINES: usize = 8;
const LINE_MS: [f32; LINES] = [25.0, 31.7, 39.4, 47.3, 55.1, 63.8, 71.6, 80.0];
const DIFFUSE_MS: [f32; 4] = [1.7, 2.9, 4.1, 5.3];
const ALLPASS_G: f32 = 0.5;
const SMOOTH_S: f32 = 0.030;
const HIGHPASS_HZ: f32 = 80.0;
const TONE_HZ_LO: f32 = 1_200.0;
const TONE_HZ_HI: f32 = 9_000.0;
/// Keeps a full-level send under the dry strike.
const WET_TRIM: f32 = 0.50;
const FLUSH: f32 = 1.0e-15;
/// 1/sqrt(8). The Hadamard mix and the eight-way inject both use it.
const MIX: f32 = 0.353_553_38;

/// Rows of a Hadamard matrix. Inject, left, and right are different rows, so
/// the first return does not pile up on one side and the two ears differ.
const INJECT: [f32; LINES] = [1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
const LEFT: [f32; LINES] = [1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0];
const RIGHT: [f32; LINES] = [1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0];

struct Line {
    buf: Vec<f32>,
    pos: usize,
    seconds: f32,
    low: f32,
    hp_x: f32,
    hp_y: f32,
}

impl Line {
    fn new(samples: usize, sample_rate: f32) -> Self {
        Self {
            buf: vec![0.0; samples.max(2)],
            pos: 0,
            seconds: samples.max(2) as f32 / sample_rate,
            low: 0.0,
            hp_x: 0.0,
            hp_y: 0.0,
        }
    }

    fn read(&self) -> f32 {
        self.buf[self.pos]
    }

    fn write(&mut self, sample: f32) {
        self.buf[self.pos] = flush(sample);
        self.pos += 1;
        if self.pos == self.buf.len() {
            self.pos = 0;
        }
    }

    fn shape(&mut self, sample: f32, lowpass: f32, highpass: f32) -> f32 {
        self.low += (1.0 - lowpass) * (sample - self.low);
        self.low = flush(self.low);
        let y = (1.0 + highpass) * 0.5 * (self.low - self.hp_x) + highpass * self.hp_y;
        self.hp_x = self.low;
        self.hp_y = flush(y);
        self.hp_y
    }
}

struct Allpass {
    buf: Vec<f32>,
    pos: usize,
}

impl Allpass {
    fn new(samples: usize) -> Self {
        Self {
            buf: vec![0.0; samples.max(2)],
            pos: 0,
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        let delayed = self.buf[self.pos];
        let y = ALLPASS_G * sample + delayed;
        self.buf[self.pos] = flush(sample - ALLPASS_G * y);
        self.pos += 1;
        if self.pos == self.buf.len() {
            self.pos = 0;
        }
        flush(y)
    }
}

pub struct Reverb {
    lines: [Line; LINES],
    diffuse: [Allpass; 4],
    gain: [f32; LINES],
    dt: f32,
    smooth: f32,
    hp: f32,
    lowpass: f32,
    handpan_send: f32,
    handpan_target: f32,
    ding_send: f32,
    ding_target: f32,
    recent_send: f32,
    recent_target: f32,
    second_send: f32,
    second_target: f32,
    bass_send: f32,
    bass_target: f32,
    echo_send: f32,
    echo_target: f32,
    decay: f32,
    decay_target: f32,
    tone: f32,
    tone_target: f32,
}

impl Reverb {
    pub fn new(sample_rate: f32) -> Self {
        let sample_rate = sample_rate.max(1.0);
        let dt = 1.0 / sample_rate;
        let mut used = Vec::with_capacity(LINES + DIFFUSE_MS.len());
        let lines = std::array::from_fn(|index| {
            let target = (LINE_MS[index] * 0.001 * sample_rate).round().max(2.0) as usize;
            let samples = unused_prime(target, &used);
            used.push(samples);
            Line::new(samples, sample_rate)
        });
        let diffuse = std::array::from_fn(|index| {
            let target = (DIFFUSE_MS[index] * 0.001 * sample_rate).round().max(2.0) as usize;
            let samples = unused_prime(target, &used);
            used.push(samples);
            Allpass::new(samples)
        });
        let mut reverb = Self {
            lines,
            diffuse,
            gain: [0.0; LINES],
            dt,
            smooth: 1.0 - (-dt / SMOOTH_S).exp(),
            hp: (-TAU * HIGHPASS_HZ * dt).exp(),
            lowpass: 0.0,
            handpan_send: DEFAULT_REVERB_HANDPAN,
            handpan_target: DEFAULT_REVERB_HANDPAN,
            ding_send: DEFAULT_REVERB_DING,
            ding_target: DEFAULT_REVERB_DING,
            recent_send: DEFAULT_REVERB_RECENT,
            recent_target: DEFAULT_REVERB_RECENT,
            second_send: DEFAULT_REVERB_SECOND,
            second_target: DEFAULT_REVERB_SECOND,
            bass_send: DEFAULT_REVERB_BASS,
            bass_target: DEFAULT_REVERB_BASS,
            echo_send: DEFAULT_REVERB_ECHO,
            echo_target: DEFAULT_REVERB_ECHO,
            decay: DEFAULT_REVERB_DECAY,
            decay_target: DEFAULT_REVERB_DECAY,
            tone: DEFAULT_REVERB_TONE,
            tone_target: DEFAULT_REVERB_TONE,
        };
        reverb.update_tone_and_gain();
        reverb
    }

    pub fn set_handpan_send(&mut self, send: f32) {
        self.handpan_target = send.clamp(REVERB_HANDPAN_RANGE.0, REVERB_HANDPAN_RANGE.1);
    }

    pub fn set_ding_send(&mut self, send: f32) {
        self.ding_target = send.clamp(REVERB_DING_RANGE.0, REVERB_DING_RANGE.1);
    }

    pub fn set_recent_send(&mut self, send: f32) {
        self.recent_target = send.clamp(REVERB_RECENT_RANGE.0, REVERB_RECENT_RANGE.1);
    }

    pub fn set_second_send(&mut self, send: f32) {
        self.second_target = send.clamp(REVERB_SECOND_RANGE.0, REVERB_SECOND_RANGE.1);
    }

    pub fn set_bass_send(&mut self, send: f32) {
        self.bass_target = send.clamp(REVERB_BASS_RANGE.0, REVERB_BASS_RANGE.1);
    }

    pub fn set_echo_send(&mut self, send: f32) {
        self.echo_target = send.clamp(REVERB_ECHO_RANGE.0, REVERB_ECHO_RANGE.1);
    }

    pub fn set_decay(&mut self, seconds: f32) {
        self.decay_target = seconds.clamp(REVERB_DECAY_RANGE.0, REVERB_DECAY_RANGE.1);
    }

    pub fn set_tone(&mut self, tone: f32) {
        self.tone_target = tone.clamp(REVERB_TONE_RANGE.0, REVERB_TONE_RANGE.1);
    }

    /// Wet tail for this frame. The dry mix stays outside.
    pub fn next_wet(
        &mut self,
        handpan: f32,
        ding: f32,
        recent: f32,
        second: f32,
        bass: f32,
        echo: f32,
    ) -> (f32, f32) {
        ease(&mut self.handpan_send, self.handpan_target, self.smooth);
        ease(&mut self.ding_send, self.ding_target, self.smooth);
        ease(&mut self.recent_send, self.recent_target, self.smooth);
        ease(&mut self.second_send, self.second_target, self.smooth);
        ease(&mut self.bass_send, self.bass_target, self.smooth);
        ease(&mut self.echo_send, self.echo_target, self.smooth);
        ease(&mut self.decay, self.decay_target, self.smooth);
        ease(&mut self.tone, self.tone_target, self.smooth);
        self.update_tone_and_gain();

        let input = flush(
            handpan * self.handpan_send
                + ding * self.ding_send
                + recent * self.recent_send
                + second * self.second_send
                + bass * self.bass_send
                + echo * self.echo_send,
        );
        let mut diffused = input;
        for allpass in &mut self.diffuse {
            diffused = allpass.process(diffused);
        }

        let mut tank = [0.0; LINES];
        let mut left = 0.0;
        let mut right = 0.0;
        for index in 0..LINES {
            let shaped = self.lines[index].shape(self.lines[index].read(), self.lowpass, self.hp);
            tank[index] = shaped;
            left += shaped * LEFT[index];
            right += shaped * RIGHT[index];
        }
        hadamard(&mut tank);
        let inject = diffused * MIX;
        for index in 0..LINES {
            let sample = tank[index] * self.gain[index] + inject * INJECT[index];
            self.lines[index].write(sample);
        }
        let trim = WET_TRIM * MIX;
        (flush(left * trim), flush(right * trim))
    }

    fn update_tone_and_gain(&mut self) {
        let cutoff = TONE_HZ_LO * (TONE_HZ_HI / TONE_HZ_LO).powf(self.tone);
        self.lowpass = (-TAU * cutoff * self.dt).exp();
        let decay = self.decay.max(REVERB_DECAY_RANGE.0);
        for index in 0..LINES {
            // The same decay for every line. A line that rings longer than the
            // others is the metallic part of a small reverb.
            let g = 10.0_f32.powf(-3.0 * self.lines[index].seconds / decay);
            self.gain[index] = g.min(0.99);
        }
    }
}

fn ease(current: &mut f32, target: f32, coeff: f32) {
    *current += (target - *current) * coeff;
    if (*current - target).abs() < 1.0e-5 {
        *current = target;
    }
}

fn flush(sample: f32) -> f32 {
    if sample.abs() < FLUSH { 0.0 } else { sample }
}

fn hadamard(values: &mut [f32; LINES]) {
    let mut span = 1;
    while span < LINES {
        let mut start = 0;
        while start < LINES {
            for offset in 0..span {
                let left = values[start + offset];
                let right = values[start + span + offset];
                values[start + offset] = left + right;
                values[start + span + offset] = left - right;
            }
            start += span * 2;
        }
        span *= 2;
    }
    for value in values {
        *value *= MIX;
    }
}

fn unused_prime(target: usize, used: &[usize]) -> usize {
    let mut radius = 0;
    loop {
        let above = target + radius;
        if above > 1 && is_prime(above) && !used.contains(&above) {
            return above;
        }
        if radius > 0 && target > radius {
            let below = target - radius;
            if below > 1 && is_prime(below) && !used.contains(&below) {
                return below;
            }
        }
        radius += 1;
    }
}

fn is_prime(value: usize) -> bool {
    if value < 2 {
        return false;
    }
    if value.is_multiple_of(2) {
        return value == 2;
    }
    let mut divisor = 3;
    while divisor * divisor <= value {
        if value.is_multiple_of(divisor) {
            return false;
        }
        divisor += 2;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn settle(reverb: &mut Reverb) {
        let samples = (RATE * 0.25) as usize;
        for _ in 0..samples {
            reverb.next_wet(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        }
    }

    fn impulse(reverb: &mut Reverb, inputs: [f32; 6], seconds: f32) -> Vec<(f32, f32)> {
        let mut frames = Vec::with_capacity((RATE * seconds) as usize + 1);
        frames.push(reverb.next_wet(
            inputs[0], inputs[1], inputs[2], inputs[3], inputs[4], inputs[5],
        ));
        for _ in 1..(RATE * seconds) as usize {
            frames.push(reverb.next_wet(0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
        }
        frames
    }

    fn energy(frames: &[(f32, f32)]) -> f32 {
        frames
            .iter()
            .map(|(left, right)| left * left + right * right)
            .sum()
    }

    #[test]
    fn silent_sends_leave_no_wet() {
        let mut reverb = Reverb::new(RATE);
        for _ in 0..(RATE as usize / 10) {
            let (left, right) = reverb.next_wet(0.8, -0.6, 0.4, -0.3, 0.5, 0.7);
            assert_eq!(left, 0.0);
            assert_eq!(right, 0.0);
        }
    }

    #[test]
    fn each_send_reaches_the_tail_alone() {
        let mut handpan = Reverb::new(RATE);
        handpan.set_handpan_send(1.0);
        settle(&mut handpan);
        let ignored = energy(&impulse(&mut handpan, [0.0, 1.0, 1.0, 1.0, 1.0, 1.0], 0.4));
        assert!(
            ignored < 1.0e-8,
            "accompaniment leaked into the handpan send: {ignored}"
        );

        let mut handpan = Reverb::new(RATE);
        handpan.set_handpan_send(1.0);
        settle(&mut handpan);
        let heard = impulse(&mut handpan, [1.0, 0.0, 0.0, 0.0, 0.0, 0.0], 0.5);
        let heard_energy = energy(&heard);
        assert!(heard_energy > 0.008, "handpan send produced {heard_energy}");
        let peak = heard.iter().fold(0.0_f32, |peak, (left, right)| {
            peak.max(left.abs()).max(right.abs())
        });
        assert!(
            peak < 0.45,
            "handpan tail peak {peak} sits on the dry strike"
        );
        let difference: f32 = heard.iter().map(|(left, right)| (left - right).abs()).sum();
        assert!(difference > 0.05, "the tail did not open into stereo");

        for (name, which) in [
            ("ding", 0usize),
            ("recent", 1),
            ("second", 2),
            ("bass", 3),
            ("echo", 4),
        ] {
            let mut part = Reverb::new(RATE);
            match which {
                0 => part.set_ding_send(1.0),
                1 => part.set_recent_send(1.0),
                2 => part.set_second_send(1.0),
                3 => part.set_bass_send(1.0),
                _ => part.set_echo_send(1.0),
            }
            settle(&mut part);
            let ignored = energy(&impulse(&mut part, [1.0, 0.0, 0.0, 0.0, 0.0, 0.0], 0.4));
            assert!(
                ignored < 1.0e-8,
                "handpan leaked into the {name} send: {ignored}"
            );

            let mut part = Reverb::new(RATE);
            match which {
                0 => part.set_ding_send(1.0),
                1 => part.set_recent_send(1.0),
                2 => part.set_second_send(1.0),
                3 => part.set_bass_send(1.0),
                _ => part.set_echo_send(1.0),
            }
            settle(&mut part);
            let mut inputs = [0.0; 6];
            inputs[which + 1] = 1.0;
            let heard = energy(&impulse(&mut part, inputs, 0.5));
            assert!(heard > 0.008, "{name} send produced {heard}");
        }
    }

    #[test]
    fn the_tail_decays_and_stays_finite() {
        let mut reverb = Reverb::new(RATE);
        reverb.set_handpan_send(1.0);
        reverb.set_decay(REVERB_DECAY_RANGE.1);
        reverb.set_tone(REVERB_TONE_RANGE.1);
        settle(&mut reverb);
        let frames = impulse(&mut reverb, [1.0, 0.0, 0.0, 0.0, 0.0, 0.0], 3.0);
        assert!(
            frames
                .iter()
                .all(|(left, right)| left.is_finite() && right.is_finite())
        );
        let early_start = (RATE * 0.05) as usize;
        let early_end = (RATE * 0.25) as usize;
        let late_start = (RATE * 2.2) as usize;
        let late_end = (RATE * 2.4) as usize;
        let early = energy(&frames[early_start..early_end]);
        let late = energy(&frames[late_start..late_end]);
        assert!(early > late * 4.0, "early {early} late {late}");
        assert!(late > 1.0e-6, "the long decay died early: {late}");
    }

    #[test]
    fn the_tail_has_no_single_echo() {
        for (decay, tone) in [
            (DEFAULT_REVERB_DECAY, DEFAULT_REVERB_TONE),
            (REVERB_DECAY_RANGE.1, REVERB_TONE_RANGE.1),
        ] {
            let mut reverb = Reverb::new(RATE);
            reverb.set_handpan_send(1.0);
            reverb.set_decay(decay);
            reverb.set_tone(tone);
            settle(&mut reverb);
            let frames = impulse(&mut reverb, [1.0, 0.0, 0.0, 0.0, 0.0, 0.0], 1.0);
            let start = (RATE * 0.04) as usize;
            let tail: Vec<f32> = frames[start..]
                .iter()
                .map(|(left, right)| 0.5 * (left + right))
                .collect();
            let peak = max_abs_correlation(&tail, (RATE * 0.008) as usize, (RATE * 0.12) as usize);
            assert!(peak < 0.45, "decay {decay} tone {tone} correlation {peak}");
        }
    }

    fn max_abs_correlation(tail: &[f32], min_lag: usize, max_lag: usize) -> f32 {
        let power: f32 = tail.iter().map(|sample| sample * sample).sum();
        if power < 1.0e-12 {
            return 0.0;
        }
        let mut peak = 0.0_f32;
        let mut lag = min_lag;
        while lag < max_lag.min(tail.len()) {
            let mut dot = 0.0;
            for index in 0..tail.len() - lag {
                dot += tail[index] * tail[index + lag];
            }
            peak = peak.max((dot / power).abs());
            lag += 4;
        }
        peak
    }
}
