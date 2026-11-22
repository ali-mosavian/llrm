//! -Os is bytes first: the hot rotation (ROTATION-HOT) is kept only where the
//! object does not grow. It added 4 B a loop for less work, and on QB
//! `suggest.c` that was +64 B for -12% modelled work (12,956 -> 13,020 B),
//! kept ahead of keep-negative-calls (-52 B), which was then put back.

use llrm_arm64::trial::{Price, keeps_rotated};
use llrm_mir::tti::Tti;

fn price(
    bytes: i64,
    work: f64,
) -> Price {
    Price { bytes, work, laid_out: work, divides: 0.0 }
}

#[test]
fn the_target_lets_a_hot_rotation_add_no_bytes() {
    let tti = llrm_arm64::tti::AArch64::of_cpu("generic").expect("premise: a generic arm64 cpu");
    assert_eq!(tti.rotation_hot_bytes_per_loop(), 0);
}

#[test]
fn a_hot_rotation_that_grows_the_object_is_put_back_however_much_work_it_saves() {
    assert!(!keeps_rotated(price(12956, 70470.0), price(13020, 61984.0), 16, 0), "suggest.c: +64 B for -12% work");
    assert!(!keeps_rotated(price(192, 250302.0), price(196, 225694.0), 1, 0), "shellsort: +4 B for -10% work");
}

#[test]
fn a_hot_rotation_that_does_not_grow_the_object_is_kept_where_it_runs_less() {
    assert!(keeps_rotated(price(124, 648.0), price(116, 560.0), 1, 0));
    assert!(keeps_rotated(price(228, 4445.0), price(228, 2375.0), 1, 0));
}

/// The trials that save bytes are decided before the hot rotation, which is
/// priced on the smallest object they made: on `suggest.c` rotation-hot went
/// first, kept +64 B, and keep-negative-calls (-52 B) was then put back.
#[test]
fn the_hot_rotation_is_decided_after_the_trials_that_save_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/jpeg_fixed_os.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .env("LLRM_DEBUG", "trial")
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let log = String::from_utf8_lossy(&said.stderr);
    let at = |name: &str| log.lines().position(|line| line.starts_with(&format!("trial {name}:")));
    let (negative, rotation) = (at("keep-negative-calls"), at("rotation-hot"));
    assert!(negative.is_some() && rotation.is_some(), "premise: both trials ran:\n{log}");
    assert!(negative < rotation, "{log}");
}
