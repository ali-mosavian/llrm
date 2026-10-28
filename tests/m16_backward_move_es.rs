//! A backward string move sets ES itself.

mod common;

use std::process::Command;

fn ir() -> String {
    "target datalayout = \"@LAYOUT@-ni:2:3\"
declare void @llvm.memmove.p0.p0.i16(ptr, ptr, i16, i1)
define void @f(ptr %d, ptr %s, i16 %n) #0 {
  call void @llvm.memmove.p0.p0.i16(ptr align 1 %d, ptr align 1 %s, i16 %n, i1 false), !llrm.backward !0
  ret void
}
attributes #0 = { nounwind \"no-builtins\" \"target-cpu\"=\"i486\" \"target-features\"=\"+x87\" }
!0 = !{}
"
    .replace("@LAYOUT@", &common::datalayout("x86-m16"))
}

/// The move was `std; rep movsd ...; cld` on whatever ES held: idiom_edges.c
/// `bytes_down(1)` copied nothing when no string move had set ES before it
/// (wrong output at -O2 under both selectors). It now begins `push ds; pop es`.
#[test]
fn test_a_backward_move_loads_es_from_ds() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.ll"), ir()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .current_dir(scratch.path())
        .args(["-m16", "-O2", "-march=i386", "-mabi=cdecl", "--isel=dag", "-S", "-o", "-", "a.ll"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    assert!(asm.contains("db 01eh,007h,0fdh,"), "{asm}");
}
