//! The speaker in virtual time (tests/sound_unit.c): BEEP, SOUND and PLAY run
//! on the headless device, whose clock is a number, so eight seconds of sound
//! take a fraction of one. Their tone log is rendered by the sdl platform's own
//! renderer, and a short-time Fourier transform of the samples says what a
//! listener would hear: which pitch, for how long, and a full-swing square wave
//! rather than the unipolar triangle a too-slow fade once made of it.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[path = "../../../tests/qbhost/lists.rs"]
mod lists;
use lists::{DEVICE, RUNTIME};

const RATE: f64 = 44100.0;
const WINDOW: usize = 2048;
const HOP: usize = 256;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Builds tests/sound_unit.c, runs it, and returns its scenario lines and the
/// samples it wrote.
fn render() -> (Vec<(String, f64, f64)>, Vec<f64>, f64) {
    let root = root();
    let qb = root.join("runtime/qb");
    let out = std::env::temp_dir().join(format!("sound_unit_{}", std::process::id()));
    let raw = std::env::temp_dir().join(format!("sound_unit_{}.raw", std::process::id()));
    let mut build = Command::new("cc");
    build.args(["-std=gnu11", "-w", "-g", "-pthread", "-D__cdecl="]);
    for include in ["posix", ".", "dev", "dev/headless", "dev/host", "dev/sdl", "../shared/posix"] {
        build.arg("-I").arg(qb.join(include));
    }
    build.arg(qb.join("tests/sound_unit.c")).arg(qb.join("dev/sdl/tone.c")).arg(root.join("tests/qbhost/hostmodule.c"));
    for source in DEVICE.iter().chain(RUNTIME.iter()) {
        build.arg(root.join(source));
    }
    build.arg("-lm").arg("-o").arg(&out);
    let built = build.output().expect("cc runs");
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let started = Instant::now();
    let done = Command::new(&out).arg(&raw).output().expect("the program runs");
    let wall = started.elapsed().as_secs_f64();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let scenarios = String::from_utf8_lossy(&done.stdout)
        .lines()
        .map(|line| {
            let mut part = line.split(' ');
            let name = part.next().unwrap().to_owned();
            let first: f64 = part.next().unwrap().parse().unwrap();
            let last: f64 = part.next().unwrap().parse().unwrap();
            (name, first / 100.0, last / 100.0)
        })
        .collect();
    let bytes = std::fs::read(&raw).unwrap();
    let samples = bytes.chunks_exact(2).map(|pair| f64::from(i16::from_le_bytes([pair[0], pair[1]]))).collect();
    std::fs::remove_file(&out).ok();
    std::fs::remove_file(&raw).ok();
    (scenarios, samples, wall)
}

/// In-place radix-2 FFT.
fn fft(
    re: &mut [f64],
    im: &mut [f64],
) {
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
    let mut size = 2;
    while size <= n {
        let angle = -2.0 * std::f64::consts::PI / size as f64;
        for start in (0..n).step_by(size) {
            for k in 0..size / 2 {
                let (sin, cos) = (angle * k as f64).sin_cos();
                let (a, b) = (start + k, start + k + size / 2);
                let (tr, ti) = (re[b] * cos - im[b] * sin, re[b] * sin + im[b] * cos);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        size <<= 1;
    }
}

struct Frame {
    centre: f64,
    /// Hann-weighted rms of the window.
    rms: f64,
    /// Magnitudes of bins 0..WINDOW/2.
    magnitude: Vec<f64>,
}

fn stft(samples: &[f64]) -> Vec<Frame> {
    let hann: Vec<f64> =
        (0..WINDOW).map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / WINDOW as f64).cos()).collect();
    let weight: f64 = hann.iter().map(|w| w * w).sum();
    (0..samples.len().saturating_sub(WINDOW))
        .step_by(HOP)
        .map(|start| {
            let mut re: Vec<f64> = (0..WINDOW).map(|i| samples[start + i] * hann[i]).collect();
            let mut im = vec![0.0; WINDOW];
            let rms = (re.iter().map(|x| x * x).sum::<f64>() / weight).sqrt();
            fft(&mut re, &mut im);
            let magnitude = (0..WINDOW / 2).map(|k| re[k].hypot(im[k])).collect();
            Frame { centre: (start + WINDOW / 2) as f64 / RATE, rms, magnitude }
        })
        .collect()
}

/// The frequency of the strongest bin, refined by a parabola through it and its
/// neighbours.
fn peak_hertz(magnitude: &[f64]) -> f64 {
    let at = (1..magnitude.len() - 1).max_by(|&a, &b| magnitude[a].total_cmp(&magnitude[b])).unwrap();
    let (a, b, c) = (magnitude[at - 1].ln(), magnitude[at].ln(), magnitude[at + 1].ln());
    let shift = 0.5 * (a - c) / (a - 2.0 * b + c);
    (at as f64 + shift) * RATE / WINDOW as f64
}

/// The level of the strongest bin within three of harmonic `n` of `hertz`, in
/// dB.
fn level(
    magnitude: &[f64],
    hertz: f64,
    n: f64,
) -> f64 {
    let bin = (hertz * n * WINDOW as f64 / RATE).round() as usize;
    let top = (bin.saturating_sub(3)..=(bin + 3).min(magnitude.len() - 1)).map(|k| magnitude[k]).fold(0.0, f64::max);
    20.0 * top.max(1e-9).log10()
}

struct Note {
    hertz: f64,
    start: f64,
    end: f64,
    centre: usize,
}

/// The notes in frames `[from, to)`: runs of frames whose energy is over half
/// of a full-swing square's.
fn notes(
    frames: &[Frame],
    from: f64,
    to: f64,
) -> Vec<Note> {
    let active =
        |frame: &Frame| frame.rms > 3000.0 * 0.5_f64.sqrt().max(0.5) && frame.centre >= from && frame.centre < to;
    let mut found = Vec::new();
    let mut at = 0;
    while at < frames.len() {
        if !active(&frames[at]) {
            at += 1;
            continue;
        }
        let first = at;
        while at < frames.len() && active(&frames[at]) {
            at += 1;
        }
        let middle = (first + at - 1) / 2;
        found.push(Note {
            hertz: peak_hertz(&frames[middle].magnitude),
            start: frames[first].centre,
            end: frames[at - 1].centre,
            centre: middle,
        });
    }
    found
}

fn semitones(from_a4: f64) -> f64 {
    440.0 * 2f64.powf(from_a4 / 12.0)
}

fn assert_note(
    name: &str,
    note: &Note,
    hertz: f64,
    length: f64,
    begin: f64,
    late: f64,
) {
    assert!((note.hertz / hertz - 1.0).abs() < 0.01, "{name}: {:.1} Hz, wanted {hertz:.1}", note.hertz);
    let heard = note.end - note.start;
    assert!((heard - length).abs() < 0.06, "{name}: {heard:.3} s long, wanted {length:.3}");
    assert!(
        note.start >= begin - 0.03 && note.start - begin < late,
        "{name}: starts {:.3} s into it",
        note.start - begin
    );
}

#[test]
fn beep_sound_and_play_sound_as_a_listener_would_hear_them() {
    let (scenarios, samples, wall) = render();
    let frames = stft(&samples);
    let begin = |name: &str| scenarios.iter().find(|s| s.0 == name).unwrap_or_else(|| panic!("no scenario {name}")).1;
    let end = |name: &str| scenarios.iter().find(|s| s.0 == name).unwrap().2 + 0.4;

    assert!(
        wall < samples.len() as f64 / RATE / 4.0,
        "{wall:.2} s of wall clock for {:.1} s of sound: not virtual time",
        samples.len() as f64 / RATE
    );

    let mut heard: Vec<(&str, Vec<Note>)> = Vec::new();
    for (name, from, _) in &scenarios {
        heard.push((name, notes(&frames, *from, end(name))));
    }
    let of = |name: &str| &heard.iter().find(|h| h.0 == name).unwrap().1;

    // BEEP: 800 Hz for a quarter of a second, at once.
    let beep = of("beep");
    assert_eq!(beep.len(), 1, "beep: {} notes", beep.len());
    assert_note("beep", &beep[0], 800.0, 0.25, begin("beep"), 0.05);

    // SOUND hertz, ticks: 18.2 ticks a second, starting at the next tick.
    for (name, hertz, ticks) in
        [("sound_440_18", 440.0, 18.0), ("sound_880_9", 880.0, 9.0), ("sound_2000_10", 2000.0, 10.0)]
    {
        let found = of(name);
        assert_eq!(found.len(), 1, "{name}: {} notes", found.len());
        assert_note(name, &found[0], hertz, ticks / 18.2, begin(name), 0.12);
    }

    // PLAY "MB T120 O3 CDEFG": five notes of seven eighths of a half-second
    // beat, each its own sound.
    let play = of("play_cdefg");
    assert_eq!(play.len(), 5, "play: {} notes", play.len());
    for (note, (name, offset)) in play.iter().zip([("C", -9.0), ("D", -7.0), ("E", -5.0), ("F", -4.0), ("G", -2.0)]) {
        assert_note(name, note, semitones(offset), 0.4375, begin("play_cdefg"), 3.0);
    }
    for pair in play.windows(2) {
        assert!(pair[1].start - pair[0].end > 0.005, "the notes run into each other");
    }

    // The wave, by the sample and by the spectrum: full swing, no offset, odd
    // harmonics only. A fade too slow for the pitch made a unipolar
    // triangle: swing 0 to 6000, offset 3000, third harmonic 19 dB down.
    for (name, found) in &heard {
        for note in found {
            let (from, to) = ((note.start * RATE) as usize + 400, (note.end * RATE) as usize - 400);
            let body = &samples[from..to];
            let peak = body.iter().fold(0.0f64, |m, x| m.max(x.abs()));
            let mean = body.iter().sum::<f64>() / body.len() as f64;
            assert!(peak > 5900.0 && peak <= 6000.0, "{name} {:.0} Hz: peak {peak}", note.hertz);
            assert!(mean.abs() < 300.0, "{name} {:.0} Hz: offset {mean:.0}", note.hertz);
            if note.hertz <= 1000.0 {
                let magnitude = &frames[note.centre].magnitude;
                let first = level(magnitude, note.hertz, 1.0);
                let second = level(magnitude, note.hertz, 2.0) - first;
                let third = level(magnitude, note.hertz, 3.0) - first;
                assert!(second < -30.0, "{name} {:.0} Hz: second harmonic {second:.1} dB", note.hertz);
                assert!(
                    (-12.0..-7.0).contains(&third),
                    "{name} {:.0} Hz: third harmonic {third:.1} dB, a square's is -9.5",
                    note.hertz
                );
            }
        }
    }
}
