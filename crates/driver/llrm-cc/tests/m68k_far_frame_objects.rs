//! A frame of several large objects put one 120 KB below `%a6`: the encoder
//! stopped with "MOV8pi: operand 0 = -120012 does not fit its 16-bit field" (QB
//! unit/synth_test.c, both CPUs). A displacement past 16 bits from the frame
//! pointer moves `%a6` to the object and back.

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "int f(int i) { volatile char a[40000], b[40000], c[40000];\n\
    a[0] = 1; a[39999] = 2; b[0] = 3; b[39999] = 4; c[0] = 5; c[39999] = 6; a[i] = 1; b[i] = 2; c[i] = 3;\n\
    return a[0] + a[39999] + b[0] + b[39999] + c[0] + c[39999] + a[i] + b[i] + c[i]; }\n";

#[test]
fn objects_far_below_a6_are_reached_by_moving_a6() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m68kfarframe-{}", std::process::id()));
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
