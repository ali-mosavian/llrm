//! A store through an undefined far pointer panicked the allocator at m16
//! ("a unique value", `LiveRangeCalc::extend`): `ProcessImplicitDefs` flagged
//! the address undef, then the class peel copied the undefined value as a real
//! read. queens (bq16) took the descriptor load of `!invariant.load` to `undef`
//! and no longer compiled.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const IR: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"\n\
define cc 1000 void @f(i16 %v) addrspace(1) {\n  store i16 %v, ptr addrspace(1) undef\n  ret void\n}\n";

#[test]
fn a_store_through_an_undef_far_pointer_compiles_at_m16() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let source = dir.join(format!("undef-{}.ll", std::process::id()));
    std::fs::write(&source, IR).unwrap();
    let out = Command::new(target_dir::of(&root).join("release/llrm-llc"))
        .args(["-march=i386", "-o"])
        .arg(dir.join(format!("undef-{}.obj", std::process::id())))
        .arg(&source)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}
