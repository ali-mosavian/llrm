//! `ld3` results live across a call, each block stored `st3` permuted: the
//! members are spilled around the call and the tuples built again for the
//! stores. Built by this compiler, linked with the cross gcc and run under
//! `qemu-aarch64`, a wrong register order or a lost member changes a byte.

use std::process::Command;

const BLOCKS: usize = 4;

fn ir() -> String {
    let ty = "<16 x i8>";
    let agg = format!("{{{ty}, {ty}, {ty}}}");
    let mut body = String::new();
    for b in 0..BLOCKS {
        body += &format!("  %i{b} = getelementptr i8, ptr %in, i64 {}\n", 48 * b);
        body += &format!("  %r{b} = call {agg} @llvm.aarch64.neon.ld3.v16i8.p0(ptr %i{b})\n");
        for m in 0..3 {
            body += &format!("  %m{b}_{m} = extractvalue {agg} %r{b}, {m}\n");
        }
    }
    body += "  call void @touch()\n";
    for b in 0..BLOCKS {
        body += &format!("  %o{b} = getelementptr i8, ptr %out, i64 {}\n", 48 * b);
        body += &format!(
            "  call void @llvm.aarch64.neon.st3.v16i8.p0({ty} %m{b}_1, {ty} %m{b}_2, {ty} %m{b}_0, ptr %o{b})\n"
        );
    }
    format!(
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
         target triple = \"aarch64-unknown-linux-gnu\"\n\
         declare {agg} @llvm.aarch64.neon.ld3.v16i8.p0(ptr)\n\
         declare void @llvm.aarch64.neon.st3.v16i8.p0({ty}, {ty}, {ty}, ptr)\n\
         declare void @touch()\n\
         define void @kern(ptr %in, ptr %out) {{\n{body}  ret void\n}}\n"
    )
}

const DRIVER: &str = r#"
#include <string.h>
volatile int sink;
void touch(void) { sink++; }
void kern(const unsigned char *in, unsigned char *out);
int main(void) {
    unsigned char in[192], out[192], want[192];
    for (int i = 0; i < 192; i++) in[i] = (unsigned char)(i * 7 + 3);
    for (int b = 0; b < 4; b++)
        for (int j = 0; j < 16; j++) {
            want[b * 48 + 3 * j + 0] = in[b * 48 + 3 * j + 1];
            want[b * 48 + 3 * j + 1] = in[b * 48 + 3 * j + 2];
            want[b * 48 + 3 * j + 2] = in[b * 48 + 3 * j + 0];
        }
    kern(in, out);
    return memcmp(out, want, sizeof want) != 0;
}
"#;

#[test]
fn ld3_results_live_across_a_call_are_stored_permuted() {
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
    std::fs::write(&main_c, DRIVER).unwrap();
    let run = |command: &mut Command| {
        let said = command.output().expect("the tool starts");
        assert!(said.status.success(), "{command:?}: {}", String::from_utf8_lossy(&said.stderr));
    };
    run(Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc")).arg("-o").arg(&object).arg(&ll));
    run(Command::new("aarch64-linux-gnu-gcc").arg("-static").arg("-o").arg(&program).arg(&main_c).arg(&object));
    let ran = Command::new("qemu-aarch64").arg(&program).output().unwrap();
    assert_eq!(ran.status.code(), Some(0), "the stored bytes differ from the permuted input");
}
