//! A frame of a 32,767-byte array and a few slots put an incoming argument at
//! `(32792,%sp)`: the encoder stopped with "MOV32rp: operand 1 = 32792 does not
//! fit its 16-bit field" (QB unit/link_test.c at every level, both CPUs). The
//! frame takes `%a6` as its pointer when the laid-out frame leaves an argument
//! past the stack pointer's reach.

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "void use(char *p, int *q);\n\
    int f(int n) { char buf[32767]; int x = n; use(buf, &x); return buf[n] + x; }\n";

#[test]
fn a_frame_just_past_the_reach_of_sp_addresses_its_arguments_from_a6() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m68kbigframe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.c"), SOURCE).unwrap();
    for target in ["m68k-68000", "m68k-68020"] {
        for level in ["-O1", "-Os"] {
            let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
                .env("LLRM_CC_TARGET", target)
                .args([level, "-w", "-c", "-o"])
                .arg(dir.join("f.o"))
                .arg(dir.join("f.c"))
                .output()
                .unwrap();
            assert!(run.status.success(), "{target} {level}: {}", String::from_utf8_lossy(&run.stderr));
        }
    }
}
