//! A `Quad` may sit in `v8`-`v15` (the call keeps `d8`-`d15` and clobbers
//! `q8`-`q15`, the pin's regmask) unless it is live across a call. Barred from
//! them, `textfill`'s checksum loop spilled its index constants and rotated
//! its 16 accumulators with moves: 58 instructions a trip to the pin's 28.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const ACCUMULATORS: usize = 28;

/// A loop adding a loaded vector into `ACCUMULATORS` carried vectors; `call`
/// puts a call to an external function in the loop, so each is live across it.
fn assembly(call: bool) -> String {
    let n = ACCUMULATORS;
    let phis: String = (0..n)
        .map(|at| format!("  %a{at} = phi <4 x i32> [ zeroinitializer, %entry ], [ %b{at}, %loop ]\n"))
        .collect();
    let adds: String = (0..n).map(|at| format!("  %b{at} = add <4 x i32> %a{at}, %x\n")).collect();
    let calls = if call { "  call void @g()\n" } else { "" };
    let sum: String = (1..n).fold("  %s0 = add <4 x i32> %b0, %b1\n".to_owned(), |text, at| {
        if at == 1 { text } else { format!("{text}  %s{} = add <4 x i32> %s{}, %b{at}\n", at - 1, at - 2) }
    });
    let ir = format!(
        r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
declare void @g()
define void @f(ptr %p, ptr %o, i64 %n) {{
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]
{phis}  %q = getelementptr <4 x i32>, ptr %p, i64 %i
  %x = load <4 x i32>, ptr %q
{adds}{calls}  %i.next = add i64 %i, 1
  %c = icmp ult i64 %i.next, %n
  br i1 %c, label %loop, label %exit
exit:
{sum}  store <4 x i32> %s{}, ptr %o
  ret void
}}
"#,
        n - 2
    );
    driver::assembly(&ir, Platform::Linux, true).expect("lowers")
}

/// The Q registers (`v8`..`v15` with an arrangement) the text names.
fn upper_vectors(asm: &str) -> Vec<String> {
    let mut found = Vec::new();
    for word in asm.split(|c: char| !c.is_ascii_alphanumeric() && c != '.') {
        if let Some(rest) = word.strip_prefix('v')
            && let Some((number, arrangement)) = rest.split_once('.')
            && arrangement.chars().any(|c| c.is_ascii_digit())
            && (8..=15).contains(&number.parse::<u32>().unwrap_or(0))
        {
            found.push(word.to_owned());
        }
    }
    found
}

/// 29 vectors live round a loop with no call: the 24 caller-saved registers
/// run out, and the rest spilled to the stack each trip (`str q`/`ldr q`).
#[test]
fn a_quad_past_the_caller_saved_registers_takes_v8_to_v15_not_a_spill() {
    let asm = assembly(false);
    let spills = asm
        .lines()
        .filter(|line| (line.contains("str q") || line.contains("ldr q")) && line.contains("[sp"))
        .count();
    assert!(!upper_vectors(&asm).is_empty() && spills == 0, "{spills} vector spills, no v8-v15:\n{asm}");
}

/// The same loop with a call: `q8`-`q15` are clobbered there, so a vector read
/// after the call without being written again since it (a read of one that was
/// live across it) is in none of them. A reload or a fresh result written
/// first is no such read.
#[test]
fn a_quad_live_across_a_call_is_not_in_v8_to_v15() {
    let asm = assembly(true);
    let lines: Vec<&str> = asm.lines().collect();
    let call = lines.iter().position(|line| line.trim_start().starts_with("bl ")).expect("a call");
    let back = call + lines[call..].iter().position(|line| line.trim_start().starts_with("b.")).expect("a back edge");
    let mut written = std::collections::HashSet::new();
    let mut stale = Vec::new();
    for line in &lines[call + 1..=back] {
        let text = line.trim();
        let (mnemonic, rest) = text.split_once(' ').unwrap_or((text, ""));
        let vectors: Vec<(usize, u32)> = rest
            .split(',')
            .enumerate()
            .filter_map(|(at, operand)| {
                let name = operand.trim_matches(|c: char| !c.is_ascii_alphanumeric());
                let number = name.strip_prefix(['v', 'q'])?.split('.').next()?.parse().ok()?;
                (8..=15).contains(&number).then_some((at, number))
            })
            .collect();
        let (stores, loads) = (mnemonic.starts_with("st"), mnemonic.starts_with("ld"));
        for &(at, number) in &vectors {
            if !loads && (stores || at > 0) && !written.contains(&number) {
                stale.push(format!("{number} read by `{text}`"));
            }
        }
        for &(at, number) in &vectors {
            if loads || (!stores && at == 0) {
                written.insert(number);
            }
        }
    }
    assert!(stale.is_empty(), "{stale:?} live across the call:\n{asm}");
}

/// A scalar `double` is `d8`-`d15` the call keeps: two live across a call sit
/// there (saved in the prologue), not spilled. Sized as the 16-byte `v` register
/// it lives in they were lost to the call's `q8`-`q15` clobber and spilled:
/// +3 instructions in x87_arguments -O1, every double across a call.
#[test]
fn a_double_live_across_a_call_sits_in_d8_to_d15() {
    let asm = driver::assembly(
        r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
declare void @g()
define double @f(double %x, ptr %p) {
  %a = load double, ptr %p
  call void @g()
  %r = fadd double %a, %x
  ret double %r
}
"#,
        Platform::Linux,
        true,
    )
    .expect("lowers");
    let spills = asm.lines().filter(|line| line.contains("str d") && line.contains("[sp, #") && !line.contains("!")).count();
    assert!(asm.contains("d8") && asm.contains("d9") && spills == 0, "{spills} double spills:\n{asm}");
}
