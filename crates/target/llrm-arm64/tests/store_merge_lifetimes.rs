//! A walk up a store chain passes the lifetime markers of other objects.

use std::fmt::Write;
use std::process::Command;

/// Four arrays set up element by element (two zeroed), each with a lifetime
/// start: the pin's `GatherAllAliases` forwards past another object's
/// `LIFETIME_START`, and only the stores 19 or more deep keep their serial
/// chain, so the zeroed array's stores merge as pairs from the first element
/// (`stp xzr, xzr` and `str xzr`). Lifetime markers stopped every walk, the
/// merge windows differed and each zeroed array was a 4 + 8 + 8 + 4 byte
/// split (`str wzr`, `stur xzr`, `stur xzr`, `str wzr`): 12 bytes of fpbench.
#[test]
fn zeroed_arrays_with_lifetimes_merge_in_pairs_from_the_first_element() {
    let mut ir = String::from(
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
         target triple = \"aarch64-unknown-linux-gnu\"\n\
         declare void @llvm.lifetime.start.p0(ptr)\ndeclare void @use(ptr, ptr, ptr, ptr)\ndefine void @f() {\n",
    );
    for name in ["a", "b", "c", "d"] {
        writeln!(ir, "  %{name} = alloca [6 x float], align 4").unwrap();
    }
    for name in ["a", "b", "c", "d"] {
        writeln!(ir, "  call void @llvm.lifetime.start.p0(ptr %{name})").unwrap();
    }
    for element in 0..6 {
        for (name, value) in [("a", "1.0"), ("b", "2.0"), ("c", "0.0"), ("d", "0.0")] {
            writeln!(ir, "  %{name}{element} = getelementptr inbounds nuw i8, ptr %{name}, i64 {}", element * 4)
                .unwrap();
            writeln!(ir, "  store float {value}, ptr %{name}{element}, align 4").unwrap();
        }
    }
    ir.push_str("  call void @use(ptr %a, ptr %b, ptr %c, ptr %d)\n  ret void\n}\n");
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let (source, out) = (dir.join("f.ll"), dir.join("f.s"));
    std::fs::write(&source, ir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-O2", "-S", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(out).unwrap();
    let zero_stores = asm.lines().filter(|line| line.contains("xzr, [sp") || line.contains("wzr, [sp")).count();
    assert_eq!(zero_stores, 4, "two zeroed arrays are an stp and an str each:\n{asm}");
}
