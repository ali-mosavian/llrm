//! `(x << 1) + C` at i8 stayed an i8 `add al, al; add al, 2`: the add clobbered
//! EFLAGS, so a three-way relation compared its operands twice (9 instructions,
//! the same select costs gcc `setne; lea`). The pair is done in i32 as one
//! `lea`. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_three_way_relation_compares_once_and_doubles_with_a_lea() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("i8-select-lea-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O3", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/relation_select.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn"]).arg(&object).output().unwrap();
    let text = String::from_utf8_lossy(&dump.stdout).into_owned();
    let body: Vec<_> = text
        .lines()
        .skip_while(|line| !line.contains("<relation>:"))
        .skip(1)
        .take_while(|line| !line.is_empty())
        .collect();
    let compares = body.iter().filter(|line| line.contains("cmp")).count();
    assert_eq!(compares, 1, "the operands are compared once:\n{text}");
    assert!(body.iter().any(|line| line.contains("lea")), "the doubling and the add are one lea:\n{text}");
}
