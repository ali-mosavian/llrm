//! The selector's output as the shared flow's `LirBody` (gap 1): the round
//! trip MFunc -> LirBody -> MFunc must be the identity.

use llrm_arm64::driver;
use llrm_arm64::lir_body::{checked_operations, forms_seen, set_check};
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

/// Loads, stores in several address forms, `cbz` and a shifted add round-trip.
#[test]
fn memory_and_compare_branch_forms_round_trip() {
    let ir = "define i64 @f(ptr %p, i64 %n, i64 %m) {
entry:
  %c = icmp eq i64 %n, 0
  br i1 %c, label %out, label %body
body:
  %q = getelementptr i32, ptr %p, i64 %n
  %v = load i32, ptr %q
  %w = sext i32 %v to i64
  %s = shl i64 %m, 3
  %t = add i64 %w, %s
  store i64 %t, ptr %p
  br label %out
out:
  %r = phi i64 [ 0, %entry ], [ %t, %body ]
  ret i64 %r
}
";
    lowers_checked(ir).expect("round trip is the identity");
}

/// How the driver took a fixture or program.
enum Outcome {
    RoundTrips,
    /// Refused for a reason that is not the round trip's (a missing
    /// datalayout).
    Refused(String),
    /// The round trip failed, or a form has no mapping.
    Failed(String),
}

fn corpus() -> Vec<(String, Outcome)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut found = Vec::new();
    for dir in ["data", "darwin"] {
        for entry in std::fs::read_dir(root.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|ext| ext != "ll") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let platform = if text.contains("apple") { Platform::Darwin } else { Platform::Linux };
            set_check(false);
            let plain = driver::assembly(&text, platform, true);
            set_check(true);
            let before = checked_operations();
            let said = driver::assembly(&text, platform, true);
            let compared = checked_operations() - before;
            set_check(false);
            let outcome = match (plain, said) {
                (Err(why), _) => Outcome::Refused(why),
                (Ok(_), Err(why)) => Outcome::Failed(why),
                (Ok(plain), Ok(said)) if plain != said => Outcome::Failed("output differs after the round trip".into()),
                // A module whose functions the mid-end removed has nothing to
                // compare.
                (Ok(plain), Ok(_)) if compared == 0 && !plain.trim().is_empty() => {
                    Outcome::Failed("no operation was compared".into())
                }
                (Ok(_), Ok(_)) => Outcome::RoundTrips,
            };
            found.push((format!("{dir}/{}", path.file_name().unwrap().to_string_lossy()), outcome));
        }
    }
    found
}

/// The round trip MFunc -> LirBody -> MFunc was the identity on 7 of 126
/// fixtures and none of the 24 programs: every other one named a form the
/// producer did not map (`Adr` first). Compared operations are counted, so a
/// check that stops running cannot pass.
#[test]
fn every_fixture_and_program_round_trips() {
    let before = checked_operations();
    let failed: Vec<_> = corpus()
        .into_iter()
        .filter_map(|(name, outcome)| match outcome {
            Outcome::Failed(why) => Some(format!("{name}: {why}")),
            _ => None,
        })
        .collect();
    assert!(failed.is_empty(), "{} did not round trip:\n{}", failed.len(), failed.join("\n"));
    println!("{} operations compared", checked_operations() - before);
    assert!(checked_operations() - before > 10_000, "the check compared too little to mean anything");
    println!("{} forms: {}", forms_seen().len(), forms_seen().join(" "));
}

/// Prints how each fixture was taken: `cargo test -p llrm-arm64 --test lir_body
/// -- --ignored --nocapture`.
#[test]
#[ignore = "inventory"]
fn unmapped_inventory() {
    let mut tally = std::collections::BTreeMap::<String, usize>::new();
    for (_, outcome) in corpus() {
        let key = match outcome {
            Outcome::RoundTrips => "round trips".to_owned(),
            Outcome::Refused(why) => format!("refused: {why:.60}"),
            Outcome::Failed(why) => why,
        };
        *tally.entry(key).or_default() += 1;
    }
    for (form, count) in &tally {
        println!("{count:4} {form}");
    }
}
