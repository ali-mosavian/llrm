//! DEVIATE SD535: at -Os a divide by a constant is expanded to the
//! multiply-high sequence where the share of clocks that saves is at least the
//! share of bytes it adds, and stays a divide otherwise. recsum -Os kept `sdiv`
//! in a loop: 20,188 modelled clocks against clang's 5,253 (the expansion is
//! 40% more bytes).

use std::process::Command;

const HEADER: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\ntarget triple = \"aarch64-unknown-linux-gnu\"\n";

/// A loop that adds `(3 i) % 11`, as recsum does.
const HOT: &str = "define i32 @f(i32 %n) #0 {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s1, %loop ]
  %m = mul i32 %i, 3
  %r = srem i32 %m, 11
  %s1 = add i32 %s, %r
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %s1
}
attributes #0 = { nounwind optsize }
";

/// The same loop dividing by a power of two (nbody_fixed -Os).
const HOT_POW2: &str = "define i32 @f(i32 %n) #0 {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s1, %loop ]
  %m = mul i32 %i, 3
  %r = sdiv i32 %m, 512
  %s1 = add i32 %s, %r
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %s1
}
attributes #0 = { nounwind optsize }
";

/// One divide, run once.
const COLD: &str = "define i32 @f(i32 %n) #0 {
  %q = sdiv i32 %n, 11
  ret i32 %q
}
attributes #0 = { nounwind optsize }
";

fn assembly(
    body: &str,
    trial: bool,
) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, format!("{HEADER}{body}")).unwrap();
    let mut llc = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
    llc.args(["-S", "-Os", "-o"]).arg(&output).arg(&input);
    if !trial {
        llc.env("LLRM_ARM64_NO_TRIAL", "1");
    }
    let said = llc.output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    std::fs::read_to_string(&output).unwrap()
}

fn divides(asm: &str) -> usize {
    asm.lines().filter(|line| line.trim_start().starts_with("sdiv")).count()
}

/// The loop runs its divide each trip: the expansion saves most of the clocks.
#[test]
fn a_divide_in_a_loop_is_expanded_at_os() {
    assert_eq!(divides(&assembly(HOT, true)), 0);
}

/// A signed divide by a power of two follows the same policy: shifts in a loop.
#[test]
fn a_power_of_two_divide_in_a_loop_is_shifts_at_os() {
    assert_eq!(divides(&assembly(HOT_POW2, true)), 0);
}

/// One divide run once: the expansion's bytes are not repaid, so it stays one.
#[test]
fn a_divide_run_once_stays_a_divide_at_os() {
    assert_eq!(divides(&assembly(COLD, true)), 1);
}

/// Without the trial the pin's rule holds: expanded at -Os, whatever it costs.
#[test]
fn the_pin_alone_expands_every_divide_at_os() {
    assert_eq!(divides(&assembly(COLD, false)), 0);
}
