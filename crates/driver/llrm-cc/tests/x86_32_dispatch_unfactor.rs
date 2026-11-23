//! A computed-goto interpreter with 20 handlers on m32. At -O2 its dispatch was
//! one block with an indirect jump (2 in the object); the pin's post-allocation
//! tail duplication copies it into every handler (21). QuickrBasic's
//! `exec_loop` ran 20-24% more instructions per op on float programs (sng_math:
//! 282M against 222M) for the dispatch's copies. The same at -Os, where one
//! shared dispatch kept the float and loop programs 19-30% over clang's
//! instructions; its copies are +2.6 KB in `exec.c`, so -Os keeps it one block
//! (TL4). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

/// Indirect jumps in the object of `m32_dispatch_unfactor.c` at `level`.
fn indirect_jumps(level: &str) -> usize {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-dispatch-unfactor-{}-{}", std::process::id(), level));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-march=i486", level, "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_dispatch_unfactor.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    let listing = String::from_utf8_lossy(&dump.stdout);
    listing.lines().filter(|line| line.contains("jmp    DWORD PTR [")).count()
}

#[test]
fn a_computed_goto_dispatch_is_copied_into_each_handler() {
    let jumps = indirect_jumps("-O2");
    assert!(jumps >= 20, "{jumps} indirect jumps: the dispatch is still one shared block (the pin: 21)");
}

/// DEVIATE TL4 (`skip_computed_goto_copies_at_size`): at -Os the dispatch stays
/// one block, so `exec.c` is not over the best of gcc and clang in bytes.
#[test]
fn an_os_dispatch_stays_one_block() {
    let jumps = indirect_jumps("-Os");
    assert!(jumps <= 4, "{jumps} indirect jumps at -Os: the dispatch was copied into the handlers (bytes first)");
}
