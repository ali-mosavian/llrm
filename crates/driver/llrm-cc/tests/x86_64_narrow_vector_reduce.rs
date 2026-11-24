//! A reduction of a vector narrower than a register (`<8 x i8>`) stopped llc
//! with "Unsupported VecReduce*, V8I8, widen operand" (QB synth_test.c at
//! -O2/-O3, vectorizer output). `WidenVecOp_VECREDUCE` pads the widened lanes
//! with the operation's neutral element.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "typedef unsigned char u8 __attribute__((vector_size(8)));\n\
    typedef signed char s8 __attribute__((vector_size(8)));\n\
    unsigned char umin(const u8 *p) { return __builtin_reduce_min(*p); }\n\
    unsigned char umax(const u8 *p) { return __builtin_reduce_max(*p); }\n\
    signed char smin(const s8 *p) { return __builtin_reduce_min(*p); }\n\
    signed char smax(const s8 *p) { return __builtin_reduce_max(*p); }\n\
    unsigned char add(const u8 *p) { return __builtin_reduce_add(*p); }\n\
    unsigned char mul(const u8 *p) { return __builtin_reduce_mul(*p); }\n\
    unsigned char and_(const u8 *p) { return __builtin_reduce_and(*p); }\n\
    unsigned char or_(const u8 *p) { return __builtin_reduce_or(*p); }\n\
    unsigned char xor_(const u8 *p) { return __builtin_reduce_xor(*p); }\n";

#[test]
fn test_a_reduction_of_a_vector_narrower_than_a_register_compiles() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("narrow-reduce-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), SOURCE).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for level in ["-O1", "-O2", "-Os"] {
        let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "x86_64-linux")
            .env(
                "LLRM_LLC",
                std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
            )
            .args([level, "-w", "-S", "-o"])
            .arg(dir.join("c.s"))
            .arg(dir.join("c.c"))
            .output()
            .unwrap();
        assert!(run.status.success(), "{level}: {}", String::from_utf8_lossy(&run.stderr));
    }
}
