//! A dense switch at `-O0 --isel=dag` loads the table entry into a register and
//! `jmp eax`. That was `Unmapped("JMP32r")`; once mapped, the table was written
//! as offsets from itself (64-bit PIC form) and the jump went to garbage.

use std::process::Command;

fn listing(target: &str) -> String {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("jt-{target}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let c = dir.join("c.c");
    std::fs::write(
        &c,
        "extern void report(long);\nint f(int x){switch(x){case 0:report(1);return 7;case 1:report(2);return 9;case 2:report(3);return 4;case 3:report(4);return 5;case 4:report(5);return 6;case 5:report(6);return 8;}return 0;}\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", target)
        .args(["-O0", "--isel=dag", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(&c)
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(dir.join("c.s")).unwrap()
}

#[test]
fn a_register_jump_through_a_32_bit_table_reads_pointer_wide_entries() {
    let text = listing("x86_32-linux");
    assert!(text.contains("jmp eax"), "expected a jump through a register:\n{text}");
    assert!(!text.contains(" - $J"), "table entries are offsets, the jump uses them as addresses:\n{text}");
}
