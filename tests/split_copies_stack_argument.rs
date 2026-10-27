//! A split piece of a stack argument is a register copy, not a reload.

use std::process::Command;

/// `place` of queens at m32 -O1 (dag): `n` is a stack argument, the load of an
/// immutable slot. `SplitEditor::defFromParent` rematerializes only what
/// `isAsCheapAsAMove` (`canRematerializeAt(.., true)`), so a split of `n`
/// copies the register. We made the load again at every use, nothing held `n`
/// through the loop and the argument slot was read 4 times; the pin reads it
/// once.
#[test]
fn a_split_stack_argument_is_read_from_its_slot_once() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/queens_place_arg.ll");
    let scratch = tempfile::tempdir().unwrap();
    let out = scratch.path().join("x.s");
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .args(["-m32", "-O1", "-march=i486", "--isel=dag", "-S", "-o"])
        .arg(&out)
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(&out).unwrap();
    let place: Vec<&str> = text.lines().skip_while(|line| !line.starts_with("_place_ proc")).collect();
    let frame: usize = place
        .iter()
        .find_map(|line| line.trim().strip_prefix("sub esp, ").and_then(|size| size.parse().ok()))
        .expect("a frame");
    // Four pushed registers and the return address sit above the frame.
    let slot = format!("dword ptr [esp+{}]", frame + 4 * 4 + 4);
    let reads =
        place.iter().take_while(|line| !line.starts_with("_place_ endp")).filter(|line| line.contains(&slot)).count();
    assert_eq!(reads, 1, "{text}");
}
