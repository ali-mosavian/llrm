//! x86 merges stores after type legalization too (`X86TargetLowering::
//! mergeStoresAfterLegalization` is true for scalars). The hook was false, so
//! the stores of `matmul`'s two constant arrays past the `GatherAllAliases`
//! depth stayed 16-bit: 52 `mov word ptr [rsp+..], imm` where the pin leaves
//! 30 and five dword immediates; matmul was 64 bytes larger at x86-64 -O2.

use std::process::Command;

#[test]
fn constant_array_stores_past_the_chain_depth_still_merge() {
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .args(["-m64", "-O2", "-S", "-o", "-", "tests/llc/m64_matmul_stack_stores.ll"])
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = String::from_utf8_lossy(&done.stdout);
    let stores = |size: &str| {
        asm.lines().filter(|line| line.trim_start().starts_with(&format!("mov {size} ptr [rsp"))).filter(|line| {
            line.rsplit(", ").next().is_some_and(|value| value.trim().parse::<i64>().is_ok())
        }).count()
    };
    let (words, dwords) = (stores("word"), stores("dword"));
    assert!(words <= 32 && dwords >= 5, "{words} word and {dwords} dword immediate stores:\n{asm}");
}
