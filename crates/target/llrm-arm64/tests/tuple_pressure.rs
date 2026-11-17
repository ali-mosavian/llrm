//! Tuple registers (`ld4`/`st4` lists) under pressure and across a join:
//! more tuples live than the file holds, and a tuple that reaches a `phi`
//! through two paths. Each list must stay consecutive (`v31` wraps to `v0`);
//! a tuple moved piecewise or placed apart is no `ldN`/`stN` operand.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const HEAD: &str = r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
declare {<4 x i32>, <4 x i32>, <4 x i32>, <4 x i32>} @llvm.aarch64.neon.ld4.v4i32.p0(ptr)
declare void @llvm.aarch64.neon.st4.v4i32.p0(<4 x i32>, <4 x i32>, <4 x i32>, <4 x i32>, ptr)
"#;

const AGG: &str = "{<4 x i32>, <4 x i32>, <4 x i32>, <4 x i32>}";

fn store(
    from: &str,
    to: &str,
) -> String {
    let members: String = (0..4).map(|at| format!("  {from}.{at} = extractvalue {AGG} {from}, {at}\n")).collect();
    format!(
        "{members}  call void @llvm.aarch64.neon.st4.v4i32.p0(<4 x i32> {from}.0, <4 x i32> {from}.1, <4 x i32> {from}.2, <4 x i32> {from}.3, ptr {to})\n"
    )
}

/// The vector numbers each `mnemonic` line lists.
fn lists(
    asm: &str,
    mnemonic: &str,
) -> Vec<Vec<u32>> {
    asm.lines()
        .filter(|line| line.trim_start().starts_with(mnemonic))
        .map(|line| {
            let inside = &line[line.find('{').expect("a list")..line.find('}').expect("a list")];
            inside
                .split(',')
                .map(|one| one.trim_start_matches(['{', ' ', 'v']).split('.').next().unwrap().parse().unwrap())
                .collect()
        })
        .collect()
}

fn consecutive(asm: &str) {
    for mnemonic in ["ld4", "st4"] {
        for regs in lists(asm, mnemonic) {
            assert!(regs.windows(2).all(|pair| pair[1] == (pair[0] + 1) % 32), "{mnemonic} {regs:?}\n{asm}");
        }
    }
}

/// Nine `ld4` results (36 registers) live together: the file has 32, so some
/// are spilled; all nine still go through `ld4`/`st4` on consecutive lists.
#[test]
fn more_live_tuples_than_the_file_holds_are_spilled_and_stay_consecutive() {
    let loads: String = (0..9)
        .map(|at| format!("  %r{at} = call {AGG} @llvm.aarch64.neon.ld4.v4i32.p0(ptr %p{at})\n"))
        .collect();
    let stores: String = (0..9).map(|at| store(&format!("%r{at}"), &format!("%o{at}"))).collect();
    let params: Vec<String> = (0..9).map(|at| format!("ptr %p{at}")).chain((0..9).map(|at| format!("ptr %o{at}"))).collect();
    let ir = format!("{HEAD}define void @f({}) {{\n{loads}{stores}  ret void\n}}\n", params.join(", "));
    let asm = driver::assembly(&ir, Platform::Linux, true).expect("lowers");
    assert_eq!((lists(&asm, "ld4").len(), lists(&asm, "st4").len()), (9, 9), "{asm}");
    consecutive(&asm);
    let spilled = asm.lines().filter(|line| line.contains("[sp")).count();
    assert!(spilled > 0, "36 live registers in 32: something must go to the frame\n{asm}");
}

/// A tuple that reaches a `phi` from two paths: the copies between tuple
/// registers move whole tuples, and the `st4` still lists a consecutive run.
#[test]
fn a_tuple_joined_from_two_paths_is_stored_from_a_consecutive_run() {
    let ir = format!(
        "{HEAD}define void @f(ptr %p, ptr %q, ptr %o, i1 %c) {{
  %r1 = call {AGG} @llvm.aarch64.neon.ld4.v4i32.p0(ptr %p)
  br i1 %c, label %a, label %join
a:
  %r2 = call {AGG} @llvm.aarch64.neon.ld4.v4i32.p0(ptr %q)
  br label %join
join:
  %m = phi {AGG} [ %r1, %0 ], [ %r2, %a ]
{}  ret void
}}
",
        store("%m", "%o")
    );
    let asm = driver::assembly(&ir, Platform::Linux, true).expect("lowers");
    assert_eq!((lists(&asm, "ld4").len(), lists(&asm, "st4").len()), (2, 1), "{asm}");
    consecutive(&asm);
}
