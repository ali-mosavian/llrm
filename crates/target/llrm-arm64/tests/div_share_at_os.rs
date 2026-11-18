//! DEVIATE SD535: at -Os a divide by a constant is expanded to the
//! multiply-high sequence where the share of clocks that saves is at least the
//! share of bytes it adds, and stays a divide otherwise. recsum -Os kept `sdiv`
//! in a loop: 20,188 modelled clocks against clang's 5,253 (the expansion is
//! 40% more bytes).

use std::process::Command;

const HEADER: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\ntarget triple = \"aarch64-unknown-linux-gnu\"\n";

/// A loop that adds `(3 i) % 11`, as recsum does, in a module with code that
/// runs once: the expansion adds a small share of the bytes (frames,
/// nbody_fixed).
const HOT: &str = "define i32 @once(i32 %x) #0 {
  %a0 = add i32 %x, 1
  %a1 = mul i32 %a0, %x
  %a2 = add i32 %a1, 3
  %a3 = mul i32 %a2, %x
  %a4 = add i32 %a3, 5
  %a5 = mul i32 %a4, %x
  %a6 = add i32 %a5, 7
  %a7 = mul i32 %a6, %x
  %a8 = add i32 %a7, 9
  %a9 = mul i32 %a8, %x
  %b0 = add i32 %a9, 11
  %b1 = mul i32 %b0, %x
  %b2 = add i32 %b1, 13
  %b3 = mul i32 %b2, %x
  %b4 = add i32 %b3, 15
  %b5 = mul i32 %b4, %x
  %b6 = add i32 %b5, 17
  %b7 = mul i32 %b6, %x
  %b8 = add i32 %b7, 19
  %b9 = mul i32 %b8, %x
  ret i32 %b9
}
define i32 @f(i32 %n) #0 {
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

/// The loop alone (recsum -Os, recpow): the expansion adds 33% of the bytes for
/// 68% of the clocks, under 4 to 1, so the divide stays.
const LOOP_ALONE: &str = "define i32 @f(i32 %n) #0 {
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
const HOT_POW2: &str = "define i32 @once(i32 %x) #0 {
  %a0 = add i32 %x, 1
  %a1 = mul i32 %a0, %x
  %a2 = add i32 %a1, 3
  %a3 = mul i32 %a2, %x
  %a4 = add i32 %a3, 5
  %a5 = mul i32 %a4, %x
  %a6 = add i32 %a5, 7
  %a7 = mul i32 %a6, %x
  %a8 = add i32 %a7, 9
  %a9 = mul i32 %a8, %x
  %b0 = add i32 %a9, 11
  %b1 = mul i32 %b0, %x
  %b2 = add i32 %b1, 13
  %b3 = mul i32 %b2, %x
  %b4 = add i32 %b3, 15
  %b5 = mul i32 %b4, %x
  %b6 = add i32 %b5, 17
  %b7 = mul i32 %b6, %x
  %b8 = add i32 %b7, 19
  %b9 = mul i32 %b8, %x
  ret i32 %b9
}
define i32 @f(i32 %n) #0 {
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

/// The loop runs its divide each trip and the module has more code besides: the
/// expansion saves most of the clocks for a small share of the bytes.
#[test]
fn a_divide_in_a_loop_is_expanded_at_os() {
    assert_eq!(divides(&assembly(HOT, true)), 0);
}

/// A loop that is most of its module repays the bytes less than 4 to 1: it
/// keeps the divide (recsum, recmany, recpow and switch -Os, where gcc's bytes
/// are the bar).
#[test]
fn a_divide_loop_that_is_most_of_the_module_stays_a_divide_at_os() {
    assert_eq!(divides(&assembly(LOOP_ALONE, true)), 1);
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
