//! An `ld2` tuple and 24 more vectors live across a call run under
//! `qemu-aarch64` and come back unchanged. The call destroys the upper half of
//! `v8`-`v15` and all of `v0`-`v7`, `v16`-`v31` (its stand-in writes them): a
//! tuple or a vector kept in them, or a tuple spilled without all its members,
//! comes back wrong.

use std::process::Command;

const OTHERS: usize = 24;

fn ir() -> String {
    let mut text = String::from(
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
         target triple = \"aarch64-unknown-linux-gnu\"\n\
         declare { <16 x i8>, <16 x i8> } @llvm.aarch64.neon.ld2.v16i8.p0(ptr)\n\
         declare void @llvm.aarch64.neon.st2.v16i8.p0(<16 x i8>, <16 x i8>, ptr)\n\
         declare void @clobber()\n\
         define void @k(ptr %in, ptr %out) {\n",
    );
    text += "  %t = call { <16 x i8>, <16 x i8> } @llvm.aarch64.neon.ld2.v16i8.p0(ptr %in)\n";
    for at in 0..OTHERS {
        text += &format!(
            "  %p{at} = getelementptr <16 x i8>, ptr %in, i64 {}\n  %x{at} = load <16 x i8>, ptr %p{at}\n",
            at + 2
        );
    }
    text += "  call void @clobber()\n  %m0 = extractvalue { <16 x i8>, <16 x i8> } %t, 0\n  %m1 = extractvalue { <16 x i8>, <16 x i8> } %t, 1\n";
    text += "  call void @llvm.aarch64.neon.st2.v16i8.p0(<16 x i8> %m0, <16 x i8> %m1, ptr %out)\n";
    text += "  %s0 = add <16 x i8> %x0, %x1\n";
    for at in 2..OTHERS {
        text += &format!("  %s{} = add <16 x i8> %s{}, %x{at}\n", at - 1, at - 2);
    }
    text += &format!(
        "  %po = getelementptr <16 x i8>, ptr %out, i64 2\n  store <16 x i8> %s{}, ptr %po\n  ret void\n}}\n",
        OTHERS - 2
    );
    text
}

fn driver() -> String {
    let mut text = String::from("#include <string.h>\n#include <stdio.h>\n");
    // Every vector register written, the upper half of v8-v15 only: what AAPCS
    // lets a call do.
    text += "__asm__(\".globl clobber\\nclobber:\\n";
    for n in (0..8).chain(16..32) {
        text += &format!("movi v{n}.2d, #0xffffffffffffffff\\n");
    }
    for n in 8..16 {
        text += &format!("mov x9, #-1\\nins v{n}.d[1], x9\\n");
    }
    text += "ret\\n\");\nvoid k(const unsigned char *in, unsigned char *out);\n";
    text += &format!(
        "int main(void) {{ unsigned char in[16 * {n}], out[16 * 3], want[16 * 3]; \n\
         for (int i = 0; i < 16 * {n}; i++) in[i] = (unsigned char)(i * 5 + 1);\n\
         memcpy(want, in, 32);\n\
         for (int l = 0; l < 16; l++) {{ unsigned char s = 0; for (int v = 0; v < {others}; v++) s += in[16 * (v + 2) + l]; want[32 + l] = s; }}\n\
         memset(out, 0, sizeof out); k(in, out);\n\
         return memcmp(out, want, sizeof want) ? 1 : 0; }}\n",
        n = OTHERS + 2,
        others = OTHERS
    );
    text
}

#[test]
fn a_tuple_and_vectors_live_across_a_call_come_back_unchanged() {
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
    assert_eq!(ran.status.code(), Some(0), "the stored tuple or the sum differs from the reference");
}
