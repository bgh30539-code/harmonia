//! Pure-Rust DSP primitives used by the playback pipeline.
//!
//! This module has no dependencies on any audio backend so it can be unit
//! tested anywhere. It implements the RBJ audio EQ cookbook biquad filters:
//! peaking EQs for a fixed ten-band graphic equalizer and a low shelf used
//! for bass boost. The application layer wraps an [`EqChain`] into a
//! `rodio::Source`.

/// Center frequencies (Hz) of the ten EQ bands.
pub const EQ_BANDS_HZ: [f32; 10] = [
    60.0, 170.0, 310.0, 600.0, 1000.0, 3000.0, 6000.0, 12000.0, 14000.0, 16000.0,
];

/// Default Q for the peaking filters (one octave).
pub const EQ_Q: f32 = 1.41;

/// Bass shelf center frequency used for the bass boost.
pub const BASS_SHELF_HZ: f32 = 110.0;

/// Normalized biquad coefficients (a0 folded into b* / a1 / a2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiquadCoeffs {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl Default for BiquadCoeffs {
    /// Identity filter: passes the signal through unchanged.
    fn default() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }
}

impl BiquadCoeffs {
    /// Peaking EQ centered at `f0` (Hz) with quality factor `q` and gain `gain_db`.
    pub fn peaking(sample_rate: f32, f0: f32, q: f32, gain_db: f32) -> Self {
        if gain_db.abs() < 0.05 {
            return Self::default();
        }
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * f0 / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// Low shelf with slope `s` (1.0 = gentle) and gain `gain_db`.
    pub fn low_shelf(sample_rate: f32, f0: f32, s: f32, gain_db: f32) -> Self {
        if gain_db.abs() < 0.05 {
            return Self::default();
        }
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * f0 / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / 2.0 * ((a + 1.0 / a) * (1.0 / s - 1.0) + 2.0).sqrt();
        let two_sqrt_a = 2.0 * a.sqrt();
        let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a * alpha);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a * alpha);
        let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a * alpha;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a * alpha;
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    pub fn is_identity(&self) -> bool {
        *self == Self::default()
    }
}

/// Stateful biquad section (Direct Form II transposed).
#[derive(Debug, Clone)]
pub struct Biquad {
    c: BiquadCoeffs,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    pub fn new(coeffs: BiquadCoeffs) -> Self {
        Self {
            c: coeffs,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.c.b0 * x + self.c.b1 * self.x1 + self.c.b2 * self.x2
            - self.c.a1 * self.y1
            - self.c.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

/// A chain of biquad stages representing the full EQ for a single channel.
///
/// The playback layer keeps one [`EqChain`] per output channel because IIR
/// filter state must not be shared between channels.
pub struct EqChain {
    stages: Vec<Biquad>,
}

impl EqChain {
    /// Builds the filter chain for `sample_rate` from the ten band gains (dB)
    /// and the bass boost gain (dB). Inactive filters are skipped entirely.
    pub fn new(sample_rate: f32, gains: &[f32], bass_boost_db: f32) -> Self {
        let mut stages = Vec::new();
        for (i, f0) in EQ_BANDS_HZ.iter().enumerate() {
            let g = gains.get(i).copied().unwrap_or(0.0);
            if g.abs() >= 0.05 {
                stages.push(Biquad::new(BiquadCoeffs::peaking(
                    sample_rate,
                    *f0,
                    EQ_Q,
                    g.clamp(-24.0, 24.0),
                )));
            }
        }
        if bass_boost_db.abs() >= 0.05 {
            stages.push(Biquad::new(BiquadCoeffs::low_shelf(
                sample_rate,
                BASS_SHELF_HZ,
                1.0,
                bass_boost_db.clamp(-24.0, 24.0),
            )));
        }
        Self { stages }
    }

    pub fn is_active(&self) -> bool {
        !self.stages.is_empty()
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let mut y = x;
        for stage in &mut self.stages {
            y = stage.process(y);
        }
        y
    }

    pub fn reset(&mut self) {
        for stage in &mut self.stages {
            stage.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f32 = 44_100.0;

    fn sine(freq: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let t = i as f32 / FS;
                (2.0 * std::f32::consts::PI * freq * t).sin()
            })
            .collect()
    }

    fn steady_rms(signal: &[f32], skip: usize) -> f32 {
        let tail = &signal[skip..];
        let sum: f32 = tail.iter().map(|x| x * x).sum();
        (sum / tail.len() as f32).sqrt()
    }

    #[test]
    fn peaking_at_zero_gain_is_identity() {
        let c = BiquadCoeffs::peaking(FS, 1000.0, EQ_Q, 0.0);
        assert!(c.is_identity());
    }

    #[test]
    fn low_shelf_boosts_low_frequencies_only() {
        let mut chain = EqChain::new(FS, &[], 12.0); // bass boost +12 dB
        let low = sine(60.0, FS as usize);
        let high = sine(12_000.0, FS as usize);
        let boosted: Vec<f32> = low.iter().map(|&x| chain.process(x)).collect();
        let mut chain = EqChain::new(FS, &[], 12.0);
        let untouched: Vec<f32> = high.iter().map(|&x| chain.process(x)).collect();

        let low_rms = steady_rms(&boosted, 8_000);
        let high_rms = steady_rms(&untouched, 8_000);
        // 12 dB ≈ 3.98x amplitude gain at low end
        assert!(low_rms > 1.5, "low band boosted: {low_rms}");
        assert!(
            (high_rms - 0.707).abs() < 0.15,
            "high band roughly untouched: {high_rms}"
        );
    }

    #[test]
    fn peaking_band_boosts_its_center() {
        let mut chain = EqChain::new(
            FS,
            &[0.0, 0.0, 0.0, 0.0, 12.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            0.0,
        );
        // Band 4 is centered at 1 kHz.
        let boosted: Vec<f32> = sine(1_000.0, FS as usize)
            .iter()
            .map(|&x| chain.process(x))
            .collect();
        let rms = steady_rms(&boosted, 8_000);
        assert!(rms > 1.0, "1 kHz boosted: {rms}");
    }

    #[test]
    fn flat_eq_passes_through_unchanged() {
        let mut chain = EqChain::new(FS, &[0.0; 10], 0.0);
        assert!(!chain.is_active());
        assert_eq!(chain.process(0.5), 0.5);
    }

    #[test]
    fn impulse_response_is_stable() {
        // A single impulse must decay, not grow.
        let mut chain = EqChain::new(FS, &[6.0; 10], 6.0);
        let mut signal = vec![0.0f32; 512];
        signal[0] = 1.0;
        let out: Vec<f32> = signal.iter().map(|&x| chain.process(x)).collect();
        let peak_after = out[128..].iter().cloned().fold(0.0f32, f32::max);
        assert!(peak_after < 0.1, "decayed: {peak_after}");
    }
}
