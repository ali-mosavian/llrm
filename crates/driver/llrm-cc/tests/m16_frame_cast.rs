//! Frame arrays reached through `addrspacecast ptr to ptr addrspace(5)` were
//! addressed by a `lea` of each array and `ss:[bx+si]`: the mid-end hoists the
//! cast out of the loop and `cgpaddr` did not fold it back (the pin's
//! `matchOperationAddr` does, CodeGenPrepare.cpp#L5244). Nib particle -O2:
//! 259 B against 166, 6 `lea` a trip; with the index in si, `[bp+si-N]`.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const IR: &str = r#"target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define i16 @f() {
entry:
  %a = alloca [101 x i16], align 2
  %b = alloca [101 x i16], align 2
  call void @llvm.memset.p0.i16(ptr %a, i8 0, i16 202, i1 false)
  call void @llvm.memset.p0.i16(ptr %b, i8 0, i16 202, i1 false)
  %a5 = addrspacecast ptr %a to ptr addrspace(5)
  %b5 = addrspacecast ptr %b to ptr addrspace(5)
  br label %inner
inner:
  %j = phi i16 [0, %entry], [%j1, %inner]
  %pa = getelementptr i16, ptr addrspace(5) %a5, i16 %j
  %pb = getelementptr i16, ptr addrspace(5) %b5, i16 %j
  %x = load i16, ptr addrspace(5) %pb
  %y = load i16, ptr addrspace(5) %pa
  %z = add i16 %x, %y
  store i16 %z, ptr addrspace(5) %pa
  %j1 = add i16 %j, 1
  %e = icmp ne i16 %j1, 101
  br i1 %e, label %inner, label %exit
exit:
  ret i16 0
}
"#;

#[test]
fn a_frame_array_through_a_stack_space_cast_is_bp_plus_index() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let source = dir.join(format!("fcast-{}.ll", std::process::id()));
    let asm = dir.join(format!("fcast-{}.s", std::process::id()));
    std::fs::write(&source, IR).unwrap();
    let out = Command::new(target_dir::of(&root).join("release/llrm-llc"))
        .args(["-march=i486", "-O2", "-S", "-o"])
        .arg(&asm)
        .arg(&source)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    let body = text.rsplit("rep stosw").next().unwrap().split("jne").next().unwrap();
    assert!(!body.contains("lea ") && !body.contains("ss:"), "an address built in the loop:\n{body}");
    assert!(body.contains("[bp+si"), "no bp+index form:\n{body}");
}
