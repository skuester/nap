use nap::{meter, preference};
use std::f32::consts::PI;

fn sine(hz: f32, amplitude: f32, rate: u32, frames: usize) -> Vec<f32> {
    (0..frames).map(|i| amplitude * (2.0 * PI * hz * i as f32 / rate as f32).sin()).collect()
}

#[test]
fn spectrum_finds_a_tone_and_rests_in_silence() {
    let bars = meter::spectrum(&sine(1000.0, 0.5, 48000, 2048), 48000, 24);
    let loudest = bars.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0;
    // 24 log bands from 45 Hz to 16 kHz put 1 kHz a little past halfway.
    assert!((11..=13).contains(&loudest), "{loudest} {bars:?}");
    assert!(bars[loudest] > 0.9 && bars[2] < 0.2 && bars[22] < 0.2, "{bars:?}");
    let bass = meter::spectrum(&sine(60.0, 0.5, 44100, 1500), 44100, 24);
    assert!(bass[..3].iter().any(|b| *b > 0.6) && bass[12] < 0.2, "{bass:?}");
    assert!(meter::spectrum(&vec![0.0; 2048], 48000, 24).iter().all(|b| *b == 0.0));
    assert_eq!(meter::spectrum(&[0.5; 10], 48000, 24), vec![0.0; 24]);
    assert!(meter::spectrum(&sine(1000.0, 1.0, 48000, 2048), 0, 24).iter().all(|b| *b == 0.0));
}

#[test]
fn vu_reads_zero_at_the_reference_level() {
    // A sine's RMS is its amplitude over root two; -10 dBFS RMS is 0 VU, about 71% deflection.
    let reference = 10f32.powf(-10.0 / 20.0) * 2f32.sqrt();
    assert!((meter::vu(&sine(440.0, reference, 48000, 4800)) - 0.708).abs() < 0.01);
    assert_eq!(meter::vu(&[]), 0.0);
    assert_eq!(meter::vu(&[0.0; 512]), 0.0);
    assert_eq!(meter::vu(&[1.0; 512]), 1.08);
}

#[test]
fn visualizer_choice_cycles_and_sticks() {
    let temp = tempfile::tempdir().unwrap();
    let file = preference::state_file(Some(temp.path())).unwrap();
    assert_eq!(preference::read(Some(&file)), "scope");
    assert_eq!(preference::read(None), "scope");
    assert_eq!(preference::after("scope"), "bars");
    assert_eq!(preference::after("bars"), "vu");
    assert_eq!(preference::after("vu"), "scope");
    assert_eq!(preference::after("nonsense"), "bars");
    preference::write(&file, "vu").unwrap();
    assert_eq!(preference::read(Some(&file)), "vu");
    std::fs::write(&file, "milkdrop").unwrap();
    assert_eq!(preference::read(Some(&file)), "scope");
}
