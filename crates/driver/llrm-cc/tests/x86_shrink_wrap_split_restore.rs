//! Paths that leave a shrink-wrapped frame into the shared early return took
//! the frame and registers back each at its own end (`add rsp; pop; pop;
//! jmp ret`, once per path): QB `decl_sametype` m64 -Os had seven copies and
//! was 105 B over clang, which has one. The pin's `tryToSplitRestore` makes
//! one block they all go through. The fixture is the function reduced from it.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

#[test]
fn test_late_exits_into_the_shared_return_share_one_restore() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("splitrestore-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let llc = std::env::var_os("LLRM_LLC")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc"));
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ll/x86_shrink_wrap_split_restore.ll");
    let done = Command::new(llc)
        .args(["-Os", "-relocation-model=pic", "-S", "-o"])
        .arg(dir.join("d.s"))
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(dir.join("d.s")).unwrap();

    // Premise: the frame is wrapped, with at least three exits to take back.
    let exits = asm.lines().filter(|l| l.trim() == "jmp L0_2").count();
    let copies = asm.lines().filter(|l| l.trim() == "pop r14").count();
    assert!(asm.contains("push r14") && copies >= 1, "the fixture is no longer wrapped:\n{asm}");
    assert!(copies <= 2, "{copies} copies of the restore, {exits} exits jumping to the return:\n{asm}");
}
