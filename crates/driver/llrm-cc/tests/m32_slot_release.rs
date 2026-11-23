//! m32 -Os: a one-slot frame is released where `emitEpilogue` stands, before
//! the first callee-saved `pop`, where `findDeadCallerSavedReg` answers 0: an
//! `add esp, 4`. A `pop ecx` there cost 3 clocks on a 486 against clang
//! (sfm16_float_{add,sub,mul,div} -Os, 14,426 against 14,423 clk). With nothing
//! to restore the `ret` is next: `pop`.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn function(name: &str) -> Vec<String> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32slot-{name}-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        // The one-slot epilogue shapes pin the 4-aligned frame; the 16-aligned
        // default has its own (x86_32_sysv_aligned_calls).
        .args(["-Os", "-w", "-S", "-march=i486", "-mabi=sysv", "-mstack-alignment=4", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_slot_release.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(dir.join("d.s")).unwrap();
    let start = asm.lines().position(|line| line.trim() == format!("{name} proc near")).expect("the function");
    asm.lines()
        .skip(start)
        .take_while(|line| line.trim() != format!("{name} endp"))
        .map(|line| line.trim().to_owned())
        .collect()
}

#[test]
fn test_a_slot_with_registers_to_restore_is_an_add() {
    let body = function("slot_with_saved");
    let at = body.iter().position(|line| line == "pop esi").expect("esi is restored");
    assert_eq!(body[at - 1], "add esp, 4", "the slot is released before the pop:\n{}", body.join("\n"));
}

#[test]
fn test_a_slot_alone_is_popped_into_a_dead_register_at_the_ret() {
    let body = function("slot_alone");
    let at = body.iter().position(|line| line == "ret").expect("a return");
    assert_eq!(body[at - 1], "pop ecx", "the slot is popped at the ret:\n{}", body.join("\n"));
}
