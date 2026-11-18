//! x86-64 -O2: the bits of a `double` kept across a call stopped the compile with
//! "Semantics(op=EXTEND, name='movq')": the spiller folded the reload of the xmm
//! source of `movq r64, xmm` into `movq r64, [slot]`, which has no encoding (the
//! pin's fold tables hold only the store form of that move). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn the_bits_of_a_double_kept_across_a_call_compile_at_x86_64() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("movq-reload-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for level in ["-O1", "-O2", "-Os"] {
        let object = dir.join("m.o");
        let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "x86_64-linux")
            .env("LLRM_LLC", &llc)
            .args([level, "-w", "-c", "-o"])
            .arg(&object)
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/movq_reload.c"))
            .output()
            .unwrap();
        assert!(run.status.success(), "{level}: {}", String::from_utf8_lossy(&run.stderr));
        assert!(object.metadata().unwrap().len() > 0, "{level}: empty object");
    }
}
