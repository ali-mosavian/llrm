//! QB's `pfpu_rint32` is `__asm__("fistpl %0" : "=m"(v) : ...)`: inline bytes
//! with a memory operand, `fistpl (%eax)`. Any such code kept the frame
//! register (`push ebp; mov ebp, esp`), a general register less than the pin
//! has, so `exec_loop` reloaded its state pointer from the frame at every
//! dispatch. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing(level: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-inline-x87-frame-{}-{level}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-march=i486", level, "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_inline_x87_frame.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout).into_owned()
}

#[test]
fn inline_x87_code_through_a_register_does_not_keep_the_frame_register() {
    for level in ["-O2", "-O3", "-Os"] {
        let text = listing(level);
        assert!(text.contains("fistp"), "{level}: the inline code is there\n{text}");
        assert!(
            !text.contains("mov    ebp,esp"),
            "{level}: a function with only this inline code keeps its frame register\n{text}"
        );
    }
}
