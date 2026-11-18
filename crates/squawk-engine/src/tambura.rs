//! Karplus–Strong tambura bank with a nonlinear jawari bridge.
//!
//! Each string is a delay loop: pluck injects noise, a loss filter dulls the
//! return, and jawari folds peaks so the buzz settles as energy dies.

use core::f32::consts::LN_2;

pub const DEFAULT_TAMBURA_JAWARI: f32 = 0.45;
pub const TAMBURA_JAWARI_RANGE: (f32, f32) = (0.0, 1.0);
pub const DEFAULT_TAMBURA_SHAPE: f32 = 0.25;
pub const TAMBURA_SHAPE_RANGE: (f32, f32) = (0.0, 1.0);
pub const DEFAULT_TAMBURA_SPREAD: f32 = 12.0;
pub const TAMBURA_SPREAD_RANGE: (f32, f32) = (0.0, 40.0);
/// 6 = full chord. 1–5 solo one string so each pitch can be checked alone.
pub const DEFAULT_TAMBURA_STRINGS: f32 = 6.0;
pub const TAMBURA_STRINGS_RANGE: (f32, f32) = (1.0, 6.0);

/// 1 low Sa · 2 Sa− · 3 Sa+ · 4 Pa · 5 high Sa · 6 all
const ACTIVE_BY_COUNT: [&[usize]; 6] = [&[0], &[1], &[2], &[3], &[4], &[0, 1, 2, 3, 4]];

const VOICES: usize = 5;
const LEVELS: [f32; VOICES] = [1.0, 0.8, 0.8, 0.55, 0.4];
const CENTS_TO_RATIO: f32 = LN_2 / 1200.0;
const MIN_HZ: f32 = 20.0;
const BODY_GAIN: f32 = 1.05;
const EXCITE_GAIN: f32 = 1.15;
const PLUCK_LEN_CYCLES: f32 = 0.45;
/// Gap between strings in a slow strum (not groove-synced).
const STRUM_GAP_S: f32 = 0.120;
/// Pause after a full strum before the next cycle starts.
const CYCLE_GAP_LO_S: f32 = 2.0;
const CYCLE_GAP_HI_S: f32 = 3.2;
const FLUSH: f32 = 1.0e-15;
/// Only near true silence — earlier values chopped the natural ring.
const ENV_GATE: f32 = 0.0012;
/// Hard-clear the delay once the note is effectively gone.
const ENV_KILL: f32 = 0.00012;

struct StringLine {
    buf: Vec<f32>,
    write: usize,
    delay: f32,
    loss: f32,
    env: f32,
    pluck_left: u32,
    pluck_amp: f32,
    stagger_left: u32,
    pending_amp: f32,
    rng: u32,
}

impl StringLine {
    fn new(capacity: usize, rng: u32) -> Self {
        Self {
            buf: vec![0.0; capacity.max(8)],
            write: 0,
            delay: 100.0,
            loss: 0.0,
            env: 0.0,
            pluck_left: 0,
            pluck_amp: 0.0,
            stagger_left: 0,
            pending_amp: 0.0,
            rng,
        }
    }

    fn set_delay(&mut self, samples: f32) {
        let max = (self.buf.len() - 2) as f32;
        self.delay = samples.clamp(2.0, max);
    }

    fn schedule_pluck(&mut self, amp: f32, stagger_samples: u32) {
        let amp = amp.clamp(0.0, 1.0);
        if amp <= 0.0 {
            return;
        }
        if stagger_samples == 0 {
            self.begin_pluck(amp);
        } else {
            self.stagger_left = stagger_samples;
            self.pending_amp = amp;
        }
    }

    fn begin_pluck(&mut self, amp: f32) {
        self.pluck_amp = amp.clamp(0.0, 1.0);
        let cycles = (self.delay * PLUCK_LEN_CYCLES).round().max(2.0) as u32;
        self.pluck_left = cycles;
        self.env = self.env.max(0.55 * self.pluck_amp);
        // Soften the existing wave so a re-pluck doesn't explode.
        for sample in &mut self.buf {
            *sample *= 0.35;
        }
    }

    fn clear_line(&mut self) {
        self.buf.fill(0.0);
        self.loss = 0.0;
        self.env = 0.0;
        self.pluck_left = 0;
        self.stagger_left = 0;
        self.pending_amp = 0.0;
    }

    fn process(&mut self, shape: f32, jawari: f32) -> (f32, f32) {
        if self.stagger_left > 0 {
            self.stagger_left -= 1;
            if self.stagger_left == 0 {
                self.begin_pluck(self.pending_amp);
                self.pending_amp = 0.0;
            }
        }

        let delayed = self.read_frac(self.delay);
        let mag = delayed.abs();
        // Fast rise / very slow fall so the gate does not race the natural decay.
        let env_coeff = if mag > self.env { 0.06 } else { 0.00035 };
        self.env += env_coeff * (mag - self.env);
        if self.env < FLUSH {
            self.env = 0.0;
        }

        // Once the string is gone, wipe the line so near-unity feedback cannot
        // turn denormals / leftover grit into a glitchy free-run.
        if self.env < ENV_KILL && self.pluck_left == 0 && self.stagger_left == 0 {
            self.clear_line();
            return (0.0, 0.0);
        }

        // Classic KS one-zero: mild HF loss, near-unity sustain.
        // Shape high → less averaging + higher loop gain (brighter, longer).
        let prev = self.loss;
        self.loss = delayed;
        let avg = 0.5 * (delayed + prev);
        let dull = (1.0 - shape).clamp(0.0, 1.0);
        let quiet = (1.0 - (self.env / ENV_GATE).clamp(0.0, 1.0)).powi(2);
        let mix = (0.08 + 0.42 * dull * dull + 0.2 * quiet).min(0.8);
        let filtered = (1.0 - mix) * delayed + mix * avg;
        // Tiny extra loss only at the noise floor — do not chop the audible tail.
        let gain = (0.99915 + 0.00075 * shape) * (1.0 - 0.015 * quiet);
        let into_bridge = flush(filtered * gain.max(0.0));

        // Jawari follows level; keep it alive through most of the decay.
        let jawari_live = jawari * (self.env / (ENV_GATE * 8.0)).clamp(0.0, 1.0);
        let (reflected, buzz) = jawari_bridge(into_bridge, jawari_live);
        let mut pluck = 0.0;
        if self.pluck_left > 0 {
            pluck = noise(&mut self.rng) * self.pluck_amp;
            self.pluck_left -= 1;
        }
        let written = flush((reflected + pluck).clamp(-4.0, 4.0));
        self.write(written);
        // Formants always see the string; jawari adds settling HF as peaks die.
        let excite = reflected + (0.85 + 1.6 * jawari_live) * buzz + 0.2 * pluck;
        (flush(reflected), flush(excite))
    }

    fn read_frac(&self, delay: f32) -> f32 {
        let len = self.buf.len();
        let delay = delay.clamp(1.0, (len - 2) as f32);
        let whole = delay.floor() as usize;
        let frac = delay - whole as f32;
        let i0 = self.wrap_read(whole);
        let i1 = self.wrap_read(whole + 1);
        self.buf[i0] * (1.0 - frac) + self.buf[i1] * frac
    }

    fn wrap_read(&self, back: usize) -> usize {
        let len = self.buf.len();
        (self.write + len - (back % len)) % len
    }

    fn write(&mut self, sample: f32) {
        self.buf[self.write] = sample;
        self.write += 1;
        if self.write >= self.buf.len() {
            self.write = 0;
        }
    }

    fn energy(&self) -> f32 {
        self.buf.iter().map(|s| s * s).sum::<f32>() / self.buf.len() as f32
    }
}

/// Displacement-dependent bridge: loud peaks kiss the plate; quiet waves pass.
fn jawari_bridge(sample: f32, jawari: f32) -> (f32, f32) {
    let jawari = jawari.clamp(TAMBURA_JAWARI_RANGE.0, TAMBURA_JAWARI_RANGE.1);
    if jawari <= 1.0e-4 {
        return (sample, 0.0);
    }
    // Higher jawari → earlier contact. Absolute level still gates as the note dies.
    let threshold = 0.05 + 0.55 * (1.0 - jawari);
    let mag = sample.abs();
    if mag <= threshold {
        return (sample, 0.0);
    }
    let over = (mag - threshold) / threshold.max(1.0e-3);
    let contact = (over * (0.25 + 0.7 * jawari)).clamp(0.0, 3.0);
    // Soft fold: keep most energy in the loop; residual is the mwah sparkle.
    let folded = sample.signum() * (threshold + (mag - threshold) / (1.0 + 0.55 * contact));
    let grit = (sample - folded) * (0.7 + 1.4 * jawari);
    let harsh = sample.signum() * contact * contact * 0.06 * jawari;
    (flush(folded), flush(grit + harsh))
}

/// Four or five waveguide strings around a center pitch.
pub struct TamburaBank {
    sample_rate: f32,
    center_hz: f32,
    shape: f32,
    spread: f32,
    jawari: f32,
    strings: u8,
    fifth_ratio: f32,
    auto_pluck: bool,
    /// Samples until the next strum step or cycle restart.
    strum_wait: u32,
    /// Index into `active_indices()` while a strum is in progress.
    strum_pos: usize,
    strumming: bool,
    strum_rng: u32,
    strings_lines: [StringLine; VOICES],
}

impl TamburaBank {
    pub fn new(sample_rate: f32) -> Self {
        let sample_rate = sample_rate.max(1.0);
        let capacity = ((sample_rate / MIN_HZ).ceil() as usize + 4).max(64);
        let mut bank = Self {
            sample_rate,
            center_hz: 110.0,
            shape: DEFAULT_TAMBURA_SHAPE,
            spread: DEFAULT_TAMBURA_SPREAD,
            jawari: DEFAULT_TAMBURA_JAWARI,
            strings: DEFAULT_TAMBURA_STRINGS as u8,
            fifth_ratio: 1.5,
            auto_pluck: false,
            strum_wait: 0,
            strum_pos: 0,
            strumming: false,
            strum_rng: 0x57A11,
            strings_lines: [
                StringLine::new(capacity, 0xA11CE),
                StringLine::new(capacity, 0xBEEF1),
                StringLine::new(capacity, 0xC0FFEE),
                StringLine::new(capacity, 0xD00D1),
                StringLine::new(capacity, 0xE1A50),
            ],
        };
        bank.retune();
        bank
    }

    pub fn set_auto_pluck(&mut self, enabled: bool) {
        let was = self.auto_pluck;
        self.auto_pluck = enabled;
        if enabled && !was {
            // First strum soon after enabling.
            self.strumming = false;
            self.strum_pos = 0;
            self.strum_wait = ((0.05 * self.sample_rate).round() as u32).max(1);
        }
    }

    pub fn set_pitch_hz(&mut self, hz: f32) {
        let hz = hz.max(MIN_HZ);
        if (hz - self.center_hz).abs() > 1.0e-4 {
            self.center_hz = hz;
            self.retune();
        }
    }

    pub fn set_shape(&mut self, value: f32) {
        self.shape = value.clamp(TAMBURA_SHAPE_RANGE.0, TAMBURA_SHAPE_RANGE.1);
    }

    pub fn set_spread(&mut self, cents: f32) {
        let cents = cents.clamp(TAMBURA_SPREAD_RANGE.0, TAMBURA_SPREAD_RANGE.1);
        if (cents - self.spread).abs() > 1.0e-4 {
            self.spread = cents;
            self.retune();
        }
    }

    pub fn set_jawari(&mut self, value: f32) {
        self.jawari = value.clamp(TAMBURA_JAWARI_RANGE.0, TAMBURA_JAWARI_RANGE.1);
    }

    pub fn set_strings(&mut self, value: f32) {
        let next = value
            .clamp(TAMBURA_STRINGS_RANGE.0, TAMBURA_STRINGS_RANGE.1)
            .round() as u8;
        if next != self.strings {
            self.strings = next;
            self.clear_inactive();
        }
    }

    pub fn string_mode_label(value: f32) -> &'static str {
        match value
            .clamp(TAMBURA_STRINGS_RANGE.0, TAMBURA_STRINGS_RANGE.1)
            .round() as u8
        {
            1 => "1 Low Sa",
            2 => "2 Sa −",
            3 => "3 Sa +",
            4 => "4 Pa",
            5 => "5 High Sa",
            _ => "6 All",
        }
    }

    pub fn set_fifth_ratio(&mut self, ratio: f32) {
        let ratio = ratio.clamp(1.2, 1.9);
        if (ratio - self.fifth_ratio).abs() > 1.0e-5 {
            self.fifth_ratio = ratio;
            self.retune();
        }
    }

    /// Slow strum down the active strings (same spacing as the auto cycle).
    pub fn pluck(&mut self, amp: f32) {
        self.pluck_with_stagger(amp, true);
        // Let the free-running cycle resume after this strum finishes.
        if self.auto_pluck {
            let active_n = self.active_indices().len().max(1);
            let gap = self.strum_gap_samples();
            self.strumming = false;
            self.strum_pos = 0;
            self.strum_wait = gap * active_n as u32 + self.cycle_gap_samples();
        }
    }

    /// Pluck every active string at once (tests / hard re-strike).
    pub fn pluck_together(&mut self, amp: f32) {
        self.pluck_with_stagger(amp, false);
    }

    pub fn active_strings(&self) -> u8 {
        self.voice_count() as u8
    }

    fn pluck_with_stagger(&mut self, amp: f32, stagger: bool) {
        let amp = amp.clamp(0.0, 1.0);
        if amp <= 0.0 {
            return;
        }
        let step = if stagger { self.strum_gap_samples() } else { 0 };
        for (order, &index) in self.active_indices().iter().enumerate() {
            let level = LEVELS[index];
            let string_amp = amp * (0.55 + 0.35 * level);
            self.strings_lines[index].schedule_pluck(string_amp, step * order as u32);
        }
    }

    pub fn next_sample(&mut self) -> (f32, f32) {
        if self.auto_pluck {
            self.tick_strum();
        }
        let active = self.active_indices();
        let mut body = 0.0;
        let mut excite = 0.0;
        let mut weight = 0.0;
        for (index, line) in self.strings_lines.iter_mut().enumerate() {
            if !active.contains(&index) {
                if line.env > 0.0 || line.pluck_left > 0 || line.stagger_left > 0 {
                    line.clear_line();
                }
                continue;
            }
            let (b, e) = line.process(self.shape, self.jawari);
            let level = LEVELS[index];
            body += level * b;
            excite += level * e;
            weight += level;
        }
        let scale = 1.0 / weight.max(1.0e-3);
        (
            flush((body * scale * BODY_GAIN).clamp(-1.5, 1.5)),
            flush((excite * scale * EXCITE_GAIN).clamp(-2.0, 2.0)),
        )
    }

    fn tick_strum(&mut self) {
        if self.strum_wait > 0 {
            self.strum_wait -= 1;
            return;
        }
        let active = self.active_indices();
        if active.is_empty() {
            self.strum_wait = self.cycle_gap_samples();
            return;
        }
        if !self.strumming {
            self.strumming = true;
            self.strum_pos = 0;
        }
        let index = active[self.strum_pos.min(active.len() - 1)];
        let level = LEVELS[index];
        let amp = 0.55 + 0.35 * unit(&mut self.strum_rng);
        self.strings_lines[index].begin_pluck(amp * (0.65 + 0.35 * level));
        self.strum_pos += 1;
        if self.strum_pos >= active.len() {
            self.strumming = false;
            self.strum_pos = 0;
            self.strum_wait = self.cycle_gap_samples();
        } else {
            self.strum_wait = self.strum_gap_samples();
        }
    }

    fn strum_gap_samples(&self) -> u32 {
        (STRUM_GAP_S * self.sample_rate).round().max(1.0) as u32
    }

    fn cycle_gap_samples(&mut self) -> u32 {
        let lo = CYCLE_GAP_LO_S * self.sample_rate;
        let hi = CYCLE_GAP_HI_S * self.sample_rate;
        (lo + (hi - lo) * unit(&mut self.strum_rng))
            .round()
            .max(1.0) as u32
    }

    pub fn energy(&self) -> f32 {
        let active = self.active_indices();
        if active.is_empty() {
            return 0.0;
        }
        active
            .iter()
            .map(|&index| self.strings_lines[index].energy())
            .sum::<f32>()
            / active.len() as f32
    }

    fn voice_count(&self) -> usize {
        self.active_indices().len()
    }

    fn active_indices(&self) -> &'static [usize] {
        ACTIVE_BY_COUNT[(self.strings.clamp(1, 6) - 1) as usize]
    }

    fn clear_inactive(&mut self) {
        let active = self.active_indices();
        for (index, line) in self.strings_lines.iter_mut().enumerate() {
            if !active.contains(&index) {
                line.clear_line();
            }
        }
        // Restart the strum so a mode change does not hang mid-sequence.
        if self.auto_pluck {
            self.strumming = false;
            self.strum_pos = 0;
            self.strum_wait = ((0.05 * self.sample_rate).round() as u32).max(1);
        }
    }

    fn retune(&mut self) {
        let half = self.spread * 0.5;
        let ratios = [
            0.5,
            (-half * CENTS_TO_RATIO).exp(),
            (half * CENTS_TO_RATIO).exp(),
            self.fifth_ratio,
            2.0,
        ];
        for (index, ratio) in ratios.into_iter().enumerate() {
            let hz = (self.center_hz * ratio).max(MIN_HZ);
            // One-pole loss adds ~0.5 sample of delay; compensate lightly.
            let samples = (self.sample_rate / hz) - 0.5;
            self.strings_lines[index].set_delay(samples);
        }
    }
}

fn noise(rng: &mut u32) -> f32 {
    *rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    let unit = ((*rng >> 8) & 0x00FF_FFFF) as f32 / 16_777_216.0;
    unit * 2.0 - 1.0
}

fn unit(rng: &mut u32) -> f32 {
    (noise(rng) + 1.0) * 0.5
}

fn flush(sample: f32) -> f32 {
    if sample.abs() < FLUSH { 0.0 } else { sample }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn collect(bank: &mut TamburaBank, seconds: f32) -> Vec<(f32, f32)> {
        let n = (RATE * seconds) as usize;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(bank.next_sample());
        }
        out
    }

    fn rms_body(samples: &[(f32, f32)]) -> f32 {
        (samples.iter().map(|(b, _)| b * b).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn silent_until_plucked() {
        let mut bank = TamburaBank::new(RATE);
        bank.set_auto_pluck(false);
        for _ in 0..2_000 {
            let (b, e) = bank.next_sample();
            assert_eq!(b, 0.0);
            assert_eq!(e, 0.0);
        }
        assert!(bank.energy() < 1.0e-5);
    }

    #[test]
    fn auto_strum_starts_without_manual_pluck() {
        let mut bank = TamburaBank::new(RATE);
        bank.set_strings(6.0);
        bank.set_pitch_hz(110.0);
        bank.set_auto_pluck(true);
        for _ in 0..(RATE as usize / 2) {
            let _ = bank.next_sample();
        }
        assert!(bank.energy() > 1.0e-4, "auto strum never excited the bank");
    }

    #[test]
    fn strum_spaces_active_strings() {
        let mut bank = TamburaBank::new(RATE);
        bank.set_auto_pluck(false);
        bank.set_strings(6.0);
        bank.set_pitch_hz(110.0);
        bank.pluck(1.0);
        let _ = bank.next_sample();
        assert!(
            bank.strings_lines[0].env > bank.strings_lines[1].env + 0.05,
            "first string should lead the strum"
        );
        for _ in 0..((STRUM_GAP_S * 1.1 * RATE) as usize) {
            let _ = bank.next_sample();
        }
        assert!(
            bank.strings_lines[1].env > 0.02,
            "second string should arrive after the strum gap"
        );
    }

    #[test]
    fn output_stays_finite_and_decays() {
        let mut bank = TamburaBank::new(RATE);
        bank.set_pitch_hz(110.0);
        bank.set_jawari(0.5);
        bank.set_shape(0.4);
        bank.pluck(1.0);
        let early = collect(&mut bank, 0.15);
        assert!(early.iter().all(|(b, e)| b.is_finite() && e.is_finite()));
        let early_rms = rms_body(&early);
        assert!(early_rms > 0.01, "early rms {early_rms}");

        // Drain without re-plucking — sustain is long, so wait it out.
        for _ in 0..(RATE as usize * 12) {
            let _ = bank.next_sample();
        }
        let late = collect(&mut bank, 0.15);
        let late_rms = rms_body(&late);
        assert!(
            late_rms < early_rms * 0.45,
            "did not decay: early {early_rms} late {late_rms}"
        );
    }

    fn residual_rms(samples: &[(f32, f32)]) -> f32 {
        (samples
            .iter()
            .map(|(body, excite)| {
                let r = excite - body;
                r * r
            })
            .sum::<f32>()
            / samples.len() as f32)
            .sqrt()
    }

    #[test]
    fn jawari_raises_early_buzz_then_settles() {
        let mut clean = TamburaBank::new(RATE);
        clean.set_pitch_hz(130.0);
        clean.set_jawari(0.0);
        clean.set_shape(0.55);
        clean.set_strings(6.0);
        clean.set_spread(0.0);
        clean.pluck_together(1.0);
        for _ in 0..200 {
            let _ = clean.next_sample();
        }
        let clean_mid = collect(&mut clean, 0.05);

        let mut buzzed = TamburaBank::new(RATE);
        buzzed.set_pitch_hz(130.0);
        buzzed.set_jawari(0.95);
        buzzed.set_shape(0.55);
        buzzed.set_strings(6.0);
        buzzed.set_spread(0.0);
        buzzed.pluck_together(1.0);
        for _ in 0..200 {
            let _ = buzzed.next_sample();
        }
        let buzz_early = collect(&mut buzzed, 0.05);
        assert!(
            residual_rms(&buzz_early) > residual_rms(&clean_mid) * 1.5,
            "jawari did not raise bridge residual: jawari {} clean {}",
            residual_rms(&buzz_early),
            residual_rms(&clean_mid)
        );

        for _ in 0..(RATE as usize * 10) {
            let _ = buzzed.next_sample();
        }
        let buzz_late = collect(&mut buzzed, 0.08);
        assert!(
            residual_rms(&buzz_late) < residual_rms(&buzz_early) * 0.55,
            "buzz did not settle with the string"
        );
    }

    #[test]
    fn string_mode_solos_or_plays_all() {
        let mut solo = TamburaBank::new(RATE);
        solo.set_strings(1.0);
        assert_eq!(solo.active_strings(), 1);
        solo.set_pitch_hz(100.0);
        solo.pluck_together(1.0);
        let solo_peak = collect(&mut solo, 0.05)
            .into_iter()
            .map(|(b, _)| b.abs())
            .fold(0.0_f32, f32::max);

        let mut all = TamburaBank::new(RATE);
        all.set_strings(6.0);
        assert_eq!(all.active_strings(), 5);
        all.set_pitch_hz(100.0);
        all.pluck_together(1.0);
        let all_peak = collect(&mut all, 0.05)
            .into_iter()
            .map(|(b, _)| b.abs())
            .fold(0.0_f32, f32::max);
        assert!(solo_peak > 0.01 && all_peak > 0.01);
        assert_eq!(TamburaBank::string_mode_label(1.0), "1 Low Sa");
        assert_eq!(TamburaBank::string_mode_label(6.0), "6 All");
    }

    #[test]
    fn pitch_tracks_center() {
        // Lowest Sa is center/2 — autocorrelation should follow that pitch.
        fn period_of(center_hz: f32) -> usize {
            let mut bank = TamburaBank::new(RATE);
            bank.set_strings(6.0);
            bank.set_spread(0.0);
            bank.set_jawari(0.0);
            bank.set_shape(0.8);
            bank.set_pitch_hz(center_hz);
            bank.pluck_together(1.0);
            for _ in 0..3_000 {
                let _ = bank.next_sample();
            }
            let samples: Vec<f32> = collect(&mut bank, 0.35)
                .into_iter()
                .map(|(b, _)| b)
                .collect();
            let expect = (RATE / (center_hz * 0.5)).round() as usize;
            best_period(&samples, expect * 3 / 4, expect * 5 / 4)
        }

        let low_p = period_of(90.0);
        let high_p = period_of(180.0);
        assert!(
            (low_p as f32) > (high_p as f32) * 1.6,
            "pitch did not track: period@90 {low_p} period@180 {high_p}"
        );
    }

    #[test]
    fn fifth_ratio_moves_pa() {
        // Use spaced ratios so DFT bins separate cleanly (12-TET vs just is too close).
        let low_pa = 140.0;
        let high_pa = 165.0;

        let mut low = TamburaBank::new(RATE);
        low.set_strings(6.0);
        low.set_spread(0.0);
        low.set_jawari(0.0);
        low.set_shape(0.75);
        low.set_pitch_hz(100.0);
        low.set_fifth_ratio(low_pa / 100.0);
        low.pluck_together(1.0);
        for _ in 0..2_000 {
            let _ = low.next_sample();
        }
        let low_samples: Vec<f32> = collect(&mut low, 0.5).into_iter().map(|(b, _)| b).collect();

        let mut high = TamburaBank::new(RATE);
        high.set_strings(6.0);
        high.set_spread(0.0);
        high.set_jawari(0.0);
        high.set_shape(0.75);
        high.set_pitch_hz(100.0);
        high.set_fifth_ratio(high_pa / 100.0);
        high.pluck_together(1.0);
        for _ in 0..2_000 {
            let _ = high.next_sample();
        }
        let high_samples: Vec<f32> = collect(&mut high, 0.5)
            .into_iter()
            .map(|(b, _)| b)
            .collect();

        assert!(
            tone_power(&low_samples, low_pa) > tone_power(&high_samples, low_pa) * 1.25,
            "low fifth did not track"
        );
        assert!(
            tone_power(&high_samples, high_pa) > tone_power(&low_samples, high_pa) * 1.25,
            "high fifth did not track"
        );
    }

    fn best_period(samples: &[f32], min_p: usize, max_p: usize) -> usize {
        let mut best_p = min_p;
        let mut best = f32::MIN;
        for period in min_p..=max_p.max(min_p) {
            let mut corr = 0.0;
            let mut n = 0usize;
            for i in 0..samples.len().saturating_sub(period) {
                corr += samples[i] * samples[i + period];
                n += 1;
            }
            if n > 0 {
                corr /= n as f32;
            }
            if corr > best {
                best = corr;
                best_p = period;
            }
        }
        best_p
    }

    fn tone_power(samples: &[f32], hz: f32) -> f32 {
        let mut re = 0.0;
        let mut im = 0.0;
        let w = core::f32::consts::TAU * hz / RATE;
        for (index, sample) in samples.iter().enumerate() {
            let phase = w * index as f32;
            re += sample * phase.cos();
            im += sample * phase.sin();
        }
        (re * re + im * im) / samples.len() as f32
    }
}
