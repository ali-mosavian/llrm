//! m32 -O1: no byte rate is stated, so an inline that stays must not grow the
//! code (INL-O1). `sum` is a loop called from ten sites: the pin's threshold
//! (225) copies it into each, which grew x_idioms -O1 from 1418 B to 1896 B
//! (gcc -O1 1600 B). Needs `LLRM_OW` and the m16 clang (`LLRM_CLANG_M16`).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "short a[160];
long sum(short *p, short n)
{
    long s = 0;
    short i;
    for (i = 0; i < n; ++i) s += (long)(i + 1) * p[i];
    return s;
}
long f(short n)
{
    long t = 0;
    t = t * 31 + sum(a, n); t = t * 31 + sum(a + 1, n); t = t * 31 + sum(a + 2, n);
    t = t * 31 + sum(a + 3, n); t = t * 31 + sum(a + 4, n); t = t * 31 + sum(a + 5, n);
    t = t * 31 + sum(a + 6, n); t = t * 31 + sum(a + 7, n);
    return t;
}
";

fn compile(
    name: &str,
    source: &str,
    flag: &str,
) -> (String, u64) {
    for var in ["LLRM_OW", "LLRM_CLANG_M16"] {
        assert!(std::env::var_os(var).is_some(), "{var} is not set: this test needs it and does not skip");
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("inlo1-{name}-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), source).unwrap();
    let run = |mode: &str, out: &str| {
        let result = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "x86_32-linux")
            .env(
                "LLRM_LLC",
                std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
            )
            .args([flag, "-w", mode, "-march=i486", "-mabi=sysv", "-o"])
            .arg(dir.join(out))
            .arg(dir.join("d.c"))
            .output()
            .unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    };
    run("-S", "d.s");
    run("-c", "d.o");
    (
        std::fs::read_to_string(dir.join("d.s")).unwrap(),
        std::fs::metadata(dir.join("d.o")).unwrap().len(),
    )
}

#[test]
fn test_m32_o1_a_loop_called_from_many_sites_is_not_copied_into_each() {
    let (listing, _) = compile("many", SOURCE, "-O1");
    let calls = listing.matches("call sum").count();
    assert!(calls >= 8, "sum was inlined into {} of 8 sites:\n{listing}", 8 - calls);
}
