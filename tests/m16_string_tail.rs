//! At m16, tuned for size, the bytes past a copy's last dword are string moves.

mod common;

use std::process::Command;

fn copy(bytes: u32) -> String {
    let layout = common::datalayout("x86-m16");
    let ir = format!(
        "target datalayout = \"{layout}-ni:2:3\"\n\
         declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)\n\
         define void @f(ptr %d, ptr %s) #0 {{\n  call void @llvm.memcpy.p0.p0.i16(ptr align 1 %d, ptr align 1 %s, i16 {bytes}, i1 false)\n  ret void\n}}\n\
         attributes #0 = {{ nounwind \"no-builtins\" \"target-cpu\"=\"i486\" \"target-features\"=\"+x87\" }}\n"
    );
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.ll"), ir).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .current_dir(scratch.path())
        .args(["-m16", "-Os", "-march=i386", "-mabi=cdecl", "--isel=dag", "-S", "-o", "-", "a.ll"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// 17 bytes were `rep movsd` and a load and store of the last byte through
/// base registers made again: 36 bytes of code against the old selector's 26.
#[test]
fn test_the_byte_past_the_last_dword_is_a_movsb() {
    let asm = copy(17);
    assert!(asm.contains("rep movsd") && asm.contains("\n    movsb"), "{asm}");
    assert!(!asm.contains("mov al, byte ptr"), "{asm}");
}

/// 15 bytes were four overlapping load and store pairs, 43 bytes against 27.
#[test]
fn test_fifteen_bytes_are_string_moves_not_four_pairs() {
    let asm = copy(15);
    assert!(asm.contains("rep movsd") && asm.contains("\n    movsw") && asm.contains("\n    movsb"), "{asm}");
    assert!(!asm.contains("mov eax, dword ptr"), "{asm}");
}
