//! A C `bool` is loaded as `i8 !range [0,2)` and tested through `trunc to i1`.
//! The DAG ignored the range (`computeKnownBits` of a `LOAD` reads
//! `MachineMemOperand::getRanges`), so the test was `move.b (%a0), %d0;
//! and.b #1, %d0; beq`: 4 B more than `tst.b (%a0); beq` (QuickrBasic m68k Os:
//! 579 `and.b #1`).

use std::process::Command;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
declare void @a()
define void @f(ptr %p) nounwind optsize {
  %v = load i8, ptr %p, align 1, !range !0, !noundef !1
  %t = trunc i8 %v to i1
  br i1 %t, label %y, label %n
y:
  call void @a()
  ret void
n:
  ret void
}
!0 = !{i8 0, i8 2}
!1 = !{}
";

fn assembly(level: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, IR).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-m68k-llc"))
        .args(["-S", level, "-mcpu=68020", "-o", "-"])
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(said.stdout).unwrap()
}

#[test]
fn a_bool_load_is_tested_without_masking_its_low_bit() {
    for level in ["-Os", "-O2"] {
        let text = assembly(level);
        assert!(!text.contains("and.b\t#1"), "{level}: {text}");
        assert!(text.contains("tst.b"), "{level}: {text}");
    }
}
