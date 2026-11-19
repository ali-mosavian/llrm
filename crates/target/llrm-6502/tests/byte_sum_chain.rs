//! A chain of byte sums with carry stays `lda abs; adc abs`.

mod common;

use common::pipeline::datalayout;

/// After `SimplifyDemandedBits` on `add` (#7117) the operands of a word sum of
/// bytes are `any_extend`ed loads, not `zext`: the carry-chain combine skipped
/// them and the loads went through a pointer pair and `lda (zp),y`. A
/// three-byte chain was 141 lines, `lda abs; adc` pairs were 78.
#[test]
fn a_byte_sum_chain_with_any_extended_operands_stays_adc_on_absolute_loads() {
    let dir = std::env::temp_dir().join(format!("llrm6502-bytesum-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("f.ll");
    let mut ir = format!("target datalayout = \"{}\"\n", datalayout());
    for name in ["faclo", "arglo", "facmo", "argmo", "facho", "argho"] {
        ir += &format!("@{name} = global i8 0\n");
    }
    ir += "define void @f(i1 %c) {\n  %c0 = zext i1 %c to i16\n";
    let mut carry = "%c0".to_owned();
    for (n, (fac, arg)) in [("faclo", "arglo"), ("facmo", "argmo"), ("facho", "argho")].into_iter().enumerate() {
        ir += &format!(
            "  %f{n} = load i8, ptr @{fac}\n  %a{n} = load i8, ptr @{arg}\n  %fz{n} = zext i8 %f{n} to i16\n  \
             %az{n} = zext i8 %a{n} to i16\n  %s{n} = add nuw nsw i16 {carry}, %fz{n}\n  \
             %t{n} = add nuw nsw i16 %s{n}, %az{n}\n  %r{n} = lshr i16 %t{n}, 8\n  \
             %b{n} = trunc i16 %t{n} to i8\n  store i8 %b{n}, ptr @{fac}\n"
        );
        carry = format!("%r{n}");
    }
    ir += "  ret void\n}\n";
    std::fs::write(&source, ir).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-6502-llc"))
        .args(["--platform=sim65", "-S", "-O2", "-o", dir.join("f.s").to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let asm = std::fs::read_to_string(dir.join("f.s")).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert!(!asm.contains("),y"), "a global was loaded through a pointer pair:\n{asm}");
    assert!(asm.lines().count() < 100, "{} lines:\n{asm}", asm.lines().count());
}
