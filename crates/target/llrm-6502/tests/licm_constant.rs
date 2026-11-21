//! A constant a loop stores on every trip is made once before it
//! (`MachineLICM`): `fill`'s `lda #255` sat in the loop (14 clocks a trip
//! against 12, `long_double_constant_store`). Needs `llrm-cc` built
//! (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn assembly(level: &str) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("licm-constant");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/fill_counter.c");
    let out = dir.join(format!("fill.{level}.s"));
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", level, "-fno-inline-functions", "-S", "-w", "-o"])
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    std::fs::read_to_string(out).unwrap()
}

#[test]
fn a_constant_stored_on_every_trip_is_loaded_before_the_loop() {
    for level in ["-O1", "-O2", "-Os"] {
        let text = assembly(level);
        let body = &text[text.find("\nfill:").expect("fill")..];
        let (before, rest) = body.split_once("\nfill_b2:").expect("the loop");
        let in_loop = rest.split("\nfill_b3:").next().unwrap();
        assert!(before.contains("lda #255"), "{level}: the constant is not made before the loop:\n{body}");
        assert!(!in_loop.contains("lda #255"), "{level}: the loop makes the constant every trip:\n{body}");
    }
}

/// `total = (total * 3 + p[i]) % 1000003` in a loop: the alias walk of the
/// pressure count followed a cycle of copies for ever, and the compile never
/// ended (copy_runs -O2, found as a stalled A/B). The compile ends in seconds.
#[test]
fn a_loop_with_a_cycle_of_copies_compiles() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("licm-constant");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/modulus_loop.c");
    for level in ["-O1", "-O2", "-Os"] {
        let mut child = Command::new(llrm_cc())
            .current_dir(&dir)
            .args(["--target=6502-sim65", level, "-S", "-w", "-o"])
            .arg(dir.join(format!("modulus_loop.{level}.s")))
            .arg(&src)
            .spawn()
            .unwrap();
        let start = std::time::Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if start.elapsed() > std::time::Duration::from_secs(120) {
                child.kill().unwrap();
                panic!("{level}: the compile did not end in 120 s");
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        };
        assert!(status.success(), "{level}: {status}");
    }
}
