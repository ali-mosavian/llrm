//! The overflow operations the 6502 does not select: they are refused by name,
//! not expanded.

mod common;

use common::pipeline::datalayout;

/// `UMulO`, `SMulO`, `SAddO` and `SSubO` on a word had no pattern, and the
/// legalizer refused them ("convert node to libcall"). The shared expansion of
/// the overflow operations (`expand_overflow`) would expand them to a multiply
/// and a compare, and the scheduler then ran away on the dependence cycle of
/// the `i1` select/setcc that makes (#6429). The 6502 sets them to `LibCall`
/// so the refusal stays, until that cycle is fixed.
#[test]
fn a_word_overflow_operation_is_refused_by_name_not_expanded_into_a_cycle() {
    for (intrinsic, op) in [
        ("llvm.umul.with.overflow.i16", "UMulO"),
        ("llvm.smul.with.overflow.i16", "SMulO"),
        ("llvm.sadd.with.overflow.i16", "SAddO"),
        ("llvm.ssub.with.overflow.i16", "SSubO"),
    ] {
        let dir = std::env::temp_dir().join(format!("llrm6502-ovf-{}-{op}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("f.ll");
        std::fs::write(
            &source,
            format!(
                "target datalayout = \"{}\"
define i16 @f(i16 %a, i16 %b) {{
  %r = call {{ i16, i1 }} @{intrinsic}(i16 %a, i16 %b)
  %v = extractvalue {{ i16, i1 }} %r, 0
  %o = extractvalue {{ i16, i1 }} %r, 1
  %z = zext i1 %o to i16
  %s = add i16 %v, %z
  ret i16 %s
}}
declare {{ i16, i1 }} @{intrinsic}(i16, i16)
",
                datalayout()
            ),
        )
        .unwrap();
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-6502-llc"))
            .args(["--platform=sim65", "-S", "-O2", "-o", dir.join("f.s").to_str().unwrap(), source.to_str().unwrap()])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        std::fs::remove_dir_all(&dir).ok();
        assert!(!out.status.success(), "{op} compiled: it should still be refused");
        assert!(stderr.contains(&format!("op: {op}")) && stderr.contains("convert node to libcall"), "{op}: {stderr}");
        assert!(!stderr.contains("panicked"), "{op} ran into the scheduler: {stderr}");
    }
}
