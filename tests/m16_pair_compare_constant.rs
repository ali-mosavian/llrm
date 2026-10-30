//! A long a far call returns in dx:ax is compared by halves.

use std::process::Command;

const IR: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"\n\
target triple = \"i386-unknown-unknown-code16\"\n\
define i16 @is104(ptr %a) addrspace(1) {\n  %n = tail call addrspace(1) i32 @ftell(ptr %a)\n  %c = icmp eq i32 %n, 104\n  %r = zext i1 %c to i16\n  ret i16 %r\n}\n\
declare i32 @ftell(ptr) addrspace(1)\n";

/// `ftell(a) == 104` joined the halves (`shl edx,16; movzx eax,ax; or eax,edx;
/// cmp eax,104`, 56 sites in QCport savegame.c, +357 B at -Os) where old
/// compared them (`xor ax,104; or ax,dx`): only a compare with zero was split.
#[test]
fn test_a_pair_equal_to_a_constant_is_not_joined() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.ll"), IR).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .current_dir(scratch.path())
        .args(["-Os", "-mabi=cdecl", "--isel=dag", "-S", "-o", "-", "a.ll"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    assert!(!asm.contains("shl edx, 16"), "{asm}");
    assert!(asm.contains("xor ax, 104"), "{asm}");
}
