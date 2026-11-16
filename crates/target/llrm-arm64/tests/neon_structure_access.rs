//! `llvm.aarch64.neon.ld2/ld3/ld4/st2/st3/st4` are `ldN`/`stN`
//! (`AArch64DAGToDAGISel::SelectLoad`, `SelectStore`) on consecutive vector
//! registers: the interleaved accesses of `InterleavedAccess` need them, and
//! without them `textfill -O2` runs 32009 instructions to the pin's 4120.
//! Before the tuple register, a member read twice was rewritten out of the
//! run and the compile died ("a unique value").

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const SHAPES: [(&str, &str); 5] =
    [("<16 x i8>", "v16i8"), ("<4 x i32>", "v4i32"), ("<2 x i64>", "v2i64"), ("<8 x i8>", "v8i8"), ("<2 x i32>", "v2i32")];

/// `r = ldN [p]`, the members combined in a way that needs each, `stN [o]` of
/// results: the registers `ldN`/`stN` list are consecutive (`v31` wraps to
/// `v0`).
fn assembly(
    n: usize,
    vec: &str,
    name: &str,
) -> String {
    let aggregate = format!("{{{}}}", vec![vec; n].join(", "));
    let members: String = (0..n).map(|at| format!("  %m{at} = extractvalue {aggregate} %r, {at}\n")).collect();
    let mixed: String = (0..n).map(|at| format!("  %s{at} = add {vec} %m{at}, %m{}\n", (at + 1) % n)).collect();
    let args: Vec<String> = (0..n).map(|at| format!("{vec} %s{at}")).collect();
    let ir = format!(
        r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
declare {aggregate} @llvm.aarch64.neon.ld{n}.{name}.p0(ptr)
declare void @llvm.aarch64.neon.st{n}.{name}.p0({}, ptr)
define void @f(ptr %p, ptr %o) {{
  %r = call {aggregate} @llvm.aarch64.neon.ld{n}.{name}.p0(ptr %p)
{members}{mixed}  call void @llvm.aarch64.neon.st{n}.{name}.p0({}, ptr %o)
  ret void
}}
"#,
        vec![vec; n].join(", "),
        args.join(", ")
    );
    driver::assembly(&ir, Platform::Linux, true).expect("lowers")
}

/// The vector numbers `{ v16.4s, v17.4s }` lists on the line of `mnemonic`.
fn list(
    asm: &str,
    mnemonic: &str,
) -> Vec<u32> {
    let line = asm.lines().find(|line| line.trim_start().starts_with(mnemonic)).expect(mnemonic);
    let inside = &line[line.find('{').expect("a list")..line.find('}').expect("a list")];
    inside.split(',').map(|one| one.trim_start_matches(['{', ' ', 'v']).split('.').next().unwrap().parse().unwrap()).collect()
}

#[test]
fn ldn_and_stn_of_each_count_and_shape_list_consecutive_registers() {
    for n in 2..=4 {
        for (vec, name) in SHAPES {
            let asm = assembly(n, vec, name);
            for mnemonic in [format!("ld{n}"), format!("st{n}")] {
                let regs = list(&asm, &mnemonic);
                assert_eq!(regs.len(), n, "{mnemonic} {vec}: {asm}");
                assert!(regs.windows(2).all(|pair| pair[1] == (pair[0] + 1) % 32), "{mnemonic} {vec}: {regs:?}\n{asm}");
            }
        }
    }
}
