//! The selector's output as the shared flow's `LirBody` (gap 1): the round
//! trip MFunc -> LirBody -> MFunc must be the identity.

use llrm_arm64::driver;
use llrm_arm64::lir_body::set_check;
use llrm_arm64::machine::Platform;

const LAYOUT: &str =
    "target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn lowers_checked(ir: &str) -> Result<String, String> {
    set_check(true);
    let asm = driver::assembly(&format!("{LAYOUT}{ir}"), Platform::Darwin, true);
    set_check(false);
    asm
}

/// A function of forms the producer maps (`mov`, `ret`) round-trips.
#[test]
fn a_constant_return_round_trips() {
    lowers_checked("define i32 @f() {\n  ret i32 42\n}\n").expect("round trip is the identity");
}

/// A form the producer does not map yet is named by its variant, so the first
/// missing form of a program is the next one to write (here a load).
#[test]
fn the_first_unmapped_form_is_named() {
    let ir = "define i32 @f(ptr %p) {\n  %v = load i32, ptr %p\n  ret i32 %v\n}\n";
    let why = lowers_checked(ir).expect_err("a load has no mapping yet");
    assert!(why.contains("unmapped form Load"), "{why}");
}

/// Prints the first unmapped form of each fixture: `cargo test -p llrm-arm64
/// --test lir_body -- --ignored --nocapture`.
#[test]
#[ignore = "inventory"]
fn unmapped_inventory() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let mut tally = std::collections::BTreeMap::<String, usize>::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "ll") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let platform = if text.contains("apple") { Platform::Darwin } else { Platform::Linux };
        set_check(true);
        let said = driver::assembly(&text, platform, true);
        set_check(false);
        let form = match said {
            Ok(_) => "mapped".to_owned(),
            Err(why) => why.split("unmapped form ").nth(1).map_or(format!("other: {why:.60}"), str::to_owned),
        };
        *tally.entry(form).or_default() += 1;
    }
    for (form, count) in &tally {
        println!("{count:4} {form}");
    }
}
