//! `scc` writes `0xFF`, so a bool made by a compare was masked with `and.b #1`
//! to be read (QuickrBasic m68k Os: 422 `and.b #1`, 112 of them after `scc`).
//! A value use is `scc; neg.b`, a branch tests the byte; a byte no `scc` made
//! keeps its mask.

use std::process::Command;

const HEAD: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
declare void @g()
";

fn assembly(
    body: &str,
    level: &str,
) -> String {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, format!("{HEAD}{body}")).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-m68k-llc"))
        .args(["-S", level, "-mcpu=68020", "-o", "-"])
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(said.stdout).unwrap()
}

#[test]
fn a_compare_returned_as_a_bool_is_scc_then_neg() {
    let text = assembly(
        "define zeroext i1 @f(i32 %x, i32 %k) nounwind optsize {
  %c = icmp eq i32 %x, %k
  ret i1 %c
}
",
        "-Os",
    );
    assert!(text.contains("seq") && text.contains("neg.b"), "{text}");
    assert!(!text.contains("and.b\t#1"), "{text}");
}

#[test]
fn a_branch_on_compares_made_in_an_earlier_block_does_not_mask_the_byte() {
    let text = assembly(
        "define void @f(i32 %n, i32 %x, i32 %k, ptr %p) nounwind optsize {
entry:
  %a = icmp eq i32 %n, 0
  %b = icmp eq i32 %x, %k
  call void @g()
  %lt = icmp ult i32 %n, 16
  br i1 %lt, label %m, label %e
m:
  %ok = or i1 %a, %b
  br i1 %ok, label %y, label %e
y:
  store i16 1, ptr %p
  br label %e
e:
  ret void
}
",
        "-Os",
    );
    assert!(!text.contains("and.b\t#1"), "{text}");
}

#[test]
fn a_byte_no_scc_made_keeps_its_mask() {
    let text = assembly(
        "define zeroext i1 @f(i32 %x, i32 %k, i8 %b) nounwind optsize {
  %c = icmp eq i32 %x, %k
  %d = trunc i8 %b to i1
  %e = and i1 %c, %d
  ret i1 %e
}
",
        "-Os",
    );
    assert!(text.contains("and.b\t#1"), "{text}");
}

/// -O2 and -O3 ran no pass on the byte (`optsize` was the only gate): `and.b
/// #1` is 8 clk on a 68000 where `neg.b` is 4 (QuickrBasic m68k -O2 -310 B).
#[test]
fn a_compare_returned_as_a_bool_is_scc_then_neg_at_speed_levels_too() {
    for level in ["-O2", "-O3"] {
        let text = assembly(
            "define zeroext i1 @f(i32 %x, i32 %k) nounwind {
  %c = icmp eq i32 %x, %k
  ret i1 %c
}
",
            level,
        );
        assert!(text.contains("neg.b"), "{level}: {text}");
        assert!(!text.contains("and.b\t#1"), "{level}: {text}");
    }
}
