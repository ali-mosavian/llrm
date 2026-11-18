//! `zext <16 x i1>` of a compare to `<16 x i16>` stopped the compile
//! (`Unsupported ZeroExtend v16i16 "split extension"`, then `ExtractSubvector
//! v8i1 "promote result"`, then `ExtractElement i8 "promote result"`), and
//! once through, was sixteen lane reads and inserts. The pin makes it
//! `cmeq; ushll; ushll2; and` (`visitZERO_EXTEND`'s vector compare arm).

use std::process::Command;

const HEAD: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn assembly(body: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, format!("{HEAD}define void @f(ptr %a, ptr %d) {{\n{body}\n  ret void\n}}\n")).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    std::fs::read_to_string(&output).unwrap()
}

/// A compare's mask counted as `i16`s: one `cmeq`, a widening a half, no lane
/// moved by hand.
#[test]
fn a_zext_of_a_byte_compare_is_a_compare_and_two_widenings() {
    let text = assembly(
        "  %x = load <16 x i8>, ptr %a, align 16\n  %m = icmp eq <16 x i8> %x, zeroinitializer\n  \
         %z = zext <16 x i1> %m to <16 x i16>\n  store <16 x i16> %z, ptr %d, align 16",
    );
    assert!(text.contains("cmeq") && text.contains("ushll2") && text.contains("ushll "), "{text}");
    assert!(!text.contains("umov") && !text.contains("ins "), "lanes moved one by one:\n{text}");
}

/// The same through `sext`: the mask itself widened, no `and`.
#[test]
fn a_sext_of_a_byte_compare_compiles() {
    let text = assembly(
        "  %x = load <16 x i8>, ptr %a, align 16\n  %m = icmp eq <16 x i8> %x, zeroinitializer\n  \
         %z = sext <16 x i1> %m to <16 x i16>\n  store <16 x i16> %z, ptr %d, align 16",
    );
    assert!(text.contains("cmeq"), "{text}");
}
