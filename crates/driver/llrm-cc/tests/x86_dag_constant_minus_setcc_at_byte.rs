//! Two selects of constants under `--isel=dag` -Os.
//!
//! `c ? 5 : 6` at i8 under `--isel=dag` -Os: `setl cl; mov al, 6; sub al, cl`
//! (the pin's form; its `sub C, zext(setcc)` match needs a `zext`, which an
//! i8 has none of), 4 B and a register more than the old selector's
//! `setge al; add al, 5`. Measured on select_of_constants m64 -Os: 14506 B
//! against 12987.

use std::path::Path;
use std::process::Command;

#[test]
fn test_m64_dag_a_byte_select_of_two_constants_is_an_inverted_setcc_and_add() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("sel-byte-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), "signed char f(int x, int y) { return x < y ? 5 : 6; }\n").unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .args(["-m64", "-Os", "--isel=dag", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let listing = std::fs::read_to_string(dir.join("c.s")).unwrap();
    assert!(
        listing.contains("setge al") && listing.contains("add al, 5") && !listing.contains("sub al"),
        "expected `setge al; add al, 5`:\n{listing}"
    );
}

/// `c ? 4 : 5` at i16 was `xor ecx, ecx; setl cl; mov eax, 5; sub eax, ecx`:
/// the pin's `sub C, X` is `xor X, C` where X cannot borrow (visitSUB L4651)
/// was not translated. It is `xor eax, eax; setl al; xor eax, 5`.
#[test]
fn test_m64_dag_a_sub_that_cannot_borrow_is_an_xor() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("sel-xor-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), "short f(int x, int y) { return x >= y ? 5 : 4; }\n").unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .args(["-m64", "-Os", "--isel=dag", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let listing = std::fs::read_to_string(dir.join("c.s")).unwrap();
    assert!(listing.contains("xor eax, 5") && !listing.contains("sub eax"), "expected `xor eax, 5`:\n{listing}");
}
