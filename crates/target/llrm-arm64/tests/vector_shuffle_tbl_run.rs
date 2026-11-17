//! `tbl` shuffles run under `qemu-aarch64` and match a scalar reference: the
//! index vector (byte order within a lane, the swap of a zero first operand,
//! 255 for a zero or undefined lane, the 64-bit form's doubled table) is what
//! a wrong mask changes.

use std::process::Command;

/// `(llvm type, lanes, element bytes, zero first, source lane of each result
/// lane)`; `-1` is a zero lane.
const CASES: [(&str, usize, usize, bool, [i32; 16]); 4] = [
    ("<16 x i8>", 16, 1, false, [3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3]),
    ("<8 x i16>", 8, 2, true, [7, -1, 5, 1, 3, -1, 1, 4, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("<8 x i8>", 8, 1, false, [7, -1, 5, -1, 3, -1, 1, -1, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("<4 x i32>", 4, 4, false, [3, -1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
];

/// The shuffle mask: a source lane `n` is `n` (or `lanes + n` when the zeros
/// come first); a zero lane reads the zero operand's lane 0.
fn mask_of(
    lanes: usize,
    zero_first: bool,
    source: &[i32],
) -> String {
    source[..lanes]
        .iter()
        .map(|&lane| {
            let index = match (lane, zero_first) {
                (-1, false) => lanes as i32,
                (-1, true) => 0,
                (lane, false) => lane,
                (lane, true) => lanes as i32 + lane,
            };
            format!("i32 {index}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn ir() -> String {
    let mut text = String::from(
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
         target triple = \"aarch64-unknown-linux-gnu\"\n",
    );
    for (at, (ty, lanes, _, zero_first, source)) in CASES.iter().enumerate() {
        let (a, b) = if *zero_first { ("zeroinitializer", "%v") } else { ("%v", "zeroinitializer") };
        let list = mask_of(*lanes, *zero_first, source);
        text += &format!(
            "define void @k{at}(ptr %in, ptr %out) {{\n  %v = load {ty}, ptr %in\n  %s = shufflevector {ty} {a}, {ty} {b}, <{lanes} x i32> <{list}>\n  store {ty} %s, ptr %out\n  ret void\n}}\n"
        );
    }
    text
}

fn driver() -> String {
    let mut text = String::from("#include <string.h>\nint bad;\n");
    for (at, (_, lanes, size, _, source)) in CASES.iter().enumerate() {
        text += &format!("void k{at}(const unsigned char *in, unsigned char *out);\n");
        let picks: Vec<String> = source[..*lanes].iter().map(|lane| lane.to_string()).collect();
        text += &format!(
            "void t{at}(void) {{ unsigned char in[16], out[16], want[16] = {{0}}; int pick[] = {{{}}};\n\
             for (int i = 0; i < 16; i++) in[i] = (unsigned char)(i * 7 + 3);\n\
             for (int l = 0; l < {lanes}; l++) if (pick[l] >= 0) memcpy(want + l * {size}, in + pick[l] * {size}, {size});\n\
             k{at}(in, out); if (memcmp(out, want, {lanes} * {size})) bad |= 1 << {at}; }}\n",
            picks.join(", ")
        );
    }
    text += "int main(void) { t0(); t1(); t2(); t3(); return bad; }\n";
    text
}

#[test]
fn tbl_shuffles_match_the_scalar_reference() {
    if ["aarch64-linux-gnu-gcc", "qemu-aarch64"]
        .iter()
        .any(|tool| Command::new(tool).arg("--version").output().is_err())
    {
        eprintln!("skipped: needs aarch64-linux-gnu-gcc and qemu-aarch64");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (ll, object, main_c, program) =
        (dir.path().join("k.ll"), dir.path().join("k.o"), dir.path().join("main.c"), dir.path().join("k"));
    std::fs::write(&ll, ir()).unwrap();
    std::fs::write(&main_c, driver()).unwrap();
    let run = |command: &mut Command| {
        let said = command.output().expect("the tool starts");
        assert!(said.status.success(), "{command:?}: {}", String::from_utf8_lossy(&said.stderr));
    };
    run(Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc")).arg("-o").arg(&object).arg(&ll));
    run(Command::new("aarch64-linux-gnu-gcc").arg("-static").arg("-o").arg(&program).arg(&main_c).arg(&object));
    let ran = Command::new("qemu-aarch64").arg(&program).output().unwrap();
    assert_eq!(ran.status.code(), Some(0), "bit n set: shuffle n's output differs");
}

/// The byte zero-extend of 8 lanes to 32 (two `tbl`, a half each) puts each
/// source byte at the first byte of its word and zeros elsewhere.
#[test]
fn a_zero_extend_of_eight_bytes_to_words_runs_right() {
    if ["aarch64-linux-gnu-gcc", "qemu-aarch64"]
        .iter()
        .any(|tool| Command::new(tool).arg("--version").output().is_err())
    {
        eprintln!("skipped: needs aarch64-linux-gnu-gcc and qemu-aarch64");
        return;
    }
    let mask: Vec<String> =
        (0..8).flat_map(|at| [format!("i32 {at}"), "i32 8".into(), "i32 8".into(), "i32 8".into()]).collect();
    let ll = format!(
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
         target triple = \"aarch64-unknown-linux-gnu\"\n\
         define void @k(ptr %in, ptr %out) {{\n  %a = load <8 x i8>, ptr %in, align 1\n  \
         %r = shufflevector <8 x i8> %a, <8 x i8> zeroinitializer, <32 x i32> <{}>\n  \
         store <32 x i8> %r, ptr %out, align 1\n  ret void\n}}\n",
        mask.join(", ")
    );
    let main_c = "#include <string.h>\nvoid k(const unsigned char *, unsigned char *);\n\
        int main(void) { unsigned char in[8], out[32], want[32] = {0};\n\
        for (int i = 0; i < 8; i++) { in[i] = (unsigned char)(i * 29 + 7); want[4 * i] = in[i]; }\n\
        memset(out, 0xaa, sizeof out); k(in, out); return memcmp(out, want, 32) ? 1 : 0; }\n";
    let dir = tempfile::tempdir().unwrap();
    let (file, object, source, program) =
        (dir.path().join("k.ll"), dir.path().join("k.o"), dir.path().join("main.c"), dir.path().join("k"));
    std::fs::write(&file, ll).unwrap();
    std::fs::write(&source, main_c).unwrap();
    let run = |command: &mut Command| {
        let said = command.output().expect("the tool starts");
        assert!(said.status.success(), "{command:?}: {}", String::from_utf8_lossy(&said.stderr));
    };
    run(Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc")).arg("-o").arg(&object).arg(&file));
    run(Command::new("aarch64-linux-gnu-gcc").arg("-static").arg("-o").arg(&program).arg(&source).arg(&object));
    let ran = Command::new("qemu-aarch64").arg(&program).output().unwrap();
    assert_eq!(ran.status.code(), Some(0), "the widened words differ from the reference");
}
