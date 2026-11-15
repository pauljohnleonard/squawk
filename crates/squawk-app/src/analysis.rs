use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

#[derive(Debug)]
pub struct AudioTap {
    enabled: AtomicBool,
    sample_rate_hz: f32,
    ring: Vec<AtomicU32>,
    write_idx: AtomicUsize,
}

impl AudioTap {
    pub fn new(sample_rate_hz: f32, ring_len_samples: usize) -> Self {
        let ring_len_samples = ring_len_samples.max(1);
        let mut ring = Vec::with_capacity(ring_len_samples);
        ring.resize_with(ring_len_samples, || AtomicU32::new(0.0f32.to_bits()));

        Self {
            enabled: AtomicBool::new(false),
            sample_rate_hz,
            ring,
            write_idx: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub fn sample_rate_hz(&self) -> f32 {
        self.sample_rate_hz
    }

    #[inline]
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    #[inline]
    pub fn push(&self, sample: f32) {
        if !self.enabled() {
            return;
        }
        if !sample.is_finite() {
            return;
        }

        let idx = self.write_idx.fetch_add(1, Ordering::Relaxed);
        let slot = idx % self.ring.len();
        self.ring[slot].store(sample.to_bits(), Ordering::Relaxed);
    }

    pub fn snapshot_latest(&self, n_samples: usize) -> Vec<f32> {
        let len = self.ring.len();
        let n = n_samples.min(len);
        if n == 0 {
            return Vec::new();
        }

        // Read a contiguous window ending at the current write head.
        // Races are tolerated: this is for debug/visualization.
        let write = self.write_idx.load(Ordering::Relaxed);
        let start = write.saturating_sub(n);

        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let slot = (start + i) % len;
            let bits = self.ring[slot].load(Ordering::Relaxed);
            out.push(f32::from_bits(bits));
        }
        out
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PitchEstimate {
    pub f0_hz: f32,
    pub confidence: f32,
    pub rms: f32,
}

pub fn estimate_pitch_nccf(
    samples: &[f32],
    sample_rate_hz: f32,
    expected_hz: Option<f32>,
) -> Option<PitchEstimate> {
    if samples.len() < 64 {
        return None;
    }
    if !(sample_rate_hz.is_finite() && sample_rate_hz > 1.0) {
        return None;
    }

    // Remove DC and compute RMS.
    let mut mean = 0.0f32;
    for &x in samples {
        mean += x;
    }
    mean /= samples.len() as f32;

    let mut x: Vec<f32> = Vec::with_capacity(samples.len());
    let mut e = 0.0f32;
    for &s in samples {
        let v = s - mean;
        e += v * v;
        x.push(v);
    }
    let rms = (e / samples.len() as f32).sqrt();
    if !rms.is_finite() || rms < 0.000_5 {
        return None;
    }

    // Frequency bounds.
    let min_hz = 50.0f32;
    let max_hz = 2000.0f32;

    let (min_hz, max_hz) = match expected_hz {
        Some(f) if f.is_finite() && f > 1.0 => {
            // Tight tracking window around expected for low latency.
            // (Wide enough to follow overblow/instability, but still fast.)
            let lo = (f * 0.70).clamp(min_hz, max_hz);
            let hi = (f * 1.30).clamp(min_hz, max_hz);
            if lo < hi { (lo, hi) } else { (min_hz, max_hz) }
        }
        _ => (min_hz, max_hz),
    };

    let mut min_lag = (sample_rate_hz / max_hz).floor() as isize;
    let mut max_lag = (sample_rate_hz / min_hz).ceil() as isize;
    min_lag = min_lag.max(2);
    max_lag = max_lag.max(min_lag + 1);
    if max_lag as usize >= x.len() - 1 {
        max_lag = (x.len() as isize - 2).max(min_lag + 1);
    }

    // If we have an expected Hz, narrow the lag range further.
    if let Some(f) = expected_hz
        && f.is_finite()
        && f > 1.0
        && (min_hz..=max_hz).contains(&f)
    {
        let expected_lag = sample_rate_hz / f;
        let lag_lo = (sample_rate_hz / max_hz).floor();
        let lag_hi = (sample_rate_hz / min_hz).ceil();
        // Search roughly within the same +-30% window.
        let lo = (expected_lag / 1.30).floor().clamp(lag_lo, lag_hi);
        let hi = (expected_lag / 0.70).ceil().clamp(lag_lo, lag_hi);
        min_lag = (lo as isize).max(min_lag);
        max_lag = (hi as isize).min(max_lag);
    }

    let min_lag = min_lag as usize;
    let max_lag = max_lag as usize;

    // NCCF: corr(lag) / sqrt(E0 * E1)
    // For debug use, an O(N*L) loop is fine at small windows.
    let mut best_lag = 0usize;
    let mut best_corr = -1.0f32;

    for lag in min_lag..=max_lag {
        let n = x.len() - lag;
        if n < 16 {
            break;
        }

        let mut corr = 0.0f32;
        let mut e0 = 0.0f32;
        let mut e1 = 0.0f32;
        for i in 0..n {
            let a = x[i];
            let b = x[i + lag];
            corr += a * b;
            e0 += a * a;
            e1 += b * b;
        }

        let denom = (e0 * e1).sqrt();
        if denom <= 1.0e-12 {
            continue;
        }
        let nccf = (corr / denom).clamp(-1.0, 1.0);
        if nccf > best_corr {
            best_corr = nccf;
            best_lag = lag;
        }
    }

    if best_lag == 0 {
        return None;
    }

    // Parabolic interpolation around the best lag for a fractional refinement.
    let refined_lag = if best_lag > min_lag && best_lag < max_lag {
        let c0 = nccf_at_lag(&x, best_lag - 1);
        let c1 = nccf_at_lag(&x, best_lag);
        let c2 = nccf_at_lag(&x, best_lag + 1);
        let denom = c0 - 2.0 * c1 + c2;
        if denom.abs() > 1.0e-6 {
            let delta = 0.5 * (c0 - c2) / denom;
            (best_lag as f32 + delta.clamp(-0.5, 0.5)).max(1.0)
        } else {
            best_lag as f32
        }
    } else {
        best_lag as f32
    };

    let f0_hz = (sample_rate_hz / refined_lag).clamp(min_hz, max_hz);
    if !f0_hz.is_finite() {
        return None;
    }

    Some(PitchEstimate {
        f0_hz,
        confidence: best_corr.clamp(0.0, 1.0),
        rms,
    })
}

fn nccf_at_lag(x: &[f32], lag: usize) -> f32 {
    if lag == 0 || lag >= x.len() - 1 {
        return 0.0;
    }
    let n = x.len() - lag;
    if n < 16 {
        return 0.0;
    }

    let mut corr = 0.0f32;
    let mut e0 = 0.0f32;
    let mut e1 = 0.0f32;
    for i in 0..n {
        let a = x[i];
        let b = x[i + lag];
        corr += a * b;
        e0 += a * a;
        e1 += b * b;
    }
    let denom = (e0 * e1).sqrt();
    if denom <= 1.0e-12 {
        return 0.0;
    }
    (corr / denom).clamp(-1.0, 1.0)
}
