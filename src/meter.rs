//! Signal analysis for the visualizers: a log-spaced spectrum for the bar analyzer and VU needle
//! deflection for the level meters. Pure functions over decoded samples; smoothing and ballistics
//! belong to the interface.

use std::f32::consts::PI;

/// The analyzer covers the musically useful range, one bar per third of an octave or so.
const LOW_HZ: f32 = 45.0;
const HIGH_HZ: f32 = 16_000.0;
/// Bars span this many decibels, topping out just under full scale.
const FLOOR_DB: f32 = -66.0;
const CEILING_DB: f32 = -8.0;
/// Music carries less energy per band toward the top; lift it so the treble bars take part.
const TILT_DB_PER_OCTAVE: f32 = 3.0;
/// A steady tone at this RMS level (dBFS) reads 0 VU, which suits mastered music.
const VU_REFERENCE_DB: f32 = -10.0;
const LARGEST_WINDOW: usize = 2048;

/// In-place radix-2 FFT; `re.len()` must be a power of two.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -2.0 * PI / len as f32;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (sin, cos) = (angle * k as f32).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * cos - im[b] * sin, re[b] * sin + im[b] * cos);
                (re[b], im[b]) = (re[a] - tr, im[a] - ti);
                (re[a], im[a]) = (re[a] + tr, im[a] + ti);
            }
        }
        len <<= 1;
    }
}

/// `bands` bar heights in 0..=1 from the most recent samples, spaced logarithmically in frequency.
pub fn spectrum(samples: &[f32], rate: u32, bands: usize) -> Vec<f32> {
    let mut out = vec![0.0; bands];
    if samples.len() < 64 || rate == 0 || bands == 0 {
        return out;
    }
    let n = (1 << samples.len().ilog2()).min(LARGEST_WINDOW);
    let recent = &samples[samples.len() - n..];
    let mut re: Vec<f32> =
        recent.iter().enumerate().map(|(i, s)| s * (0.5 - 0.5 * (2.0 * PI * i as f32 / n as f32).cos())).collect();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im);
    let hz_per_bin = rate as f32 / n as f32;
    let top = HIGH_HZ.min(rate as f32 / 2.0);
    let edge = |band: usize| LOW_HZ * (top / LOW_HZ).powf(band as f32 / bands as f32);
    for (band, level) in out.iter_mut().enumerate() {
        let (low, high) = (edge(band), edge(band + 1));
        let first = ((low / hz_per_bin).round() as usize).clamp(1, n / 2 - 1);
        let last = ((high / hz_per_bin).round() as usize).clamp(first + 1, n / 2);
        let peak = (first..last).map(|bin| re[bin].hypot(im[bin])).fold(0.0, f32::max);
        // A Hann window halves a sine's peak, hence the factor of four over n.
        let db = 20.0 * (peak * 4.0 / n as f32).max(1e-9).log10();
        let tilt = TILT_DB_PER_OCTAVE * ((low * high).sqrt() / 1000.0).log2();
        *level = ((db + tilt - FLOOR_DB) / (CEILING_DB - FLOOR_DB)).clamp(0.0, 1.0);
    }
    out
}

/// Where a VU needle wants to rest for these samples: 0 at the left stop, about 0.71 at 0 VU,
/// 1 at +3 VU. Like the real instrument, the scale is linear in voltage.
pub fn vu(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    let reference = 10f32.powf(VU_REFERENCE_DB / 20.0);
    (rms / reference / 10f32.powf(3.0 / 20.0)).clamp(0.0, 1.08)
}
