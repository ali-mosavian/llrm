//! `--target=x86_32-linux`: clang's inline asm with the operands in fixed
//! places (`{st}` in, `=t`/`=u` out, an address register for `=m`, `{eax}`...)
//! is a block of bytes laid down at the site (`llrm.ia32.asm.*`). QuickrBasic's
//! `pfpu.h` (`fistpl`, `fistpll`) and `qbrt_dos.c` (`fsincos`) were refused by
//! llrm-llc ("expected a constant, found `asm`"): double.c, fpmath.c and
//! operands.c did not compile at any level. A constraint that leaves the
//! register to the allocator is refused by name (stage 2, `ISD::INLINEASM`,
//! takes those); it is never miscompiled.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::{Command, Output};

fn compile(
    tag: &str,
    source: &Path,
    level: &str,
) -> (Output, String) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32asm-{tag}-{level}-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args([level, "-w", "-S", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("d.s"))
        .arg(source)
        .output()
        .unwrap();
    let listing = std::fs::read_to_string(dir.join("d.s")).unwrap_or_default();
    (done, listing)
}

fn function<'a>(
    listing: &'a str,
    name: &str,
) -> Vec<&'a str> {
    let start = listing.lines().position(|line| line.trim() == format!("{name} proc near")).expect("the function");
    listing.lines().skip(start).take_while(|line| line.trim() != format!("{name} endp")).map(str::trim).collect()
}

#[test]
fn test_an_x87_asm_block_takes_st0_and_gives_st0_and_st1() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_inline_asm_x87.c");
    for level in ["-O1", "-O2", "-Os"] {
        let (done, listing) = compile("x87", &source, level);
        assert!(done.status.success(), "{level}: {}", String::from_utf8_lossy(&done.stderr));
        // the value is loaded to the stack's top, then the block runs on it
        for (name, bytes) in [("round_int", "db 0dbh,018h"), ("round_long", "db 0dfh,038h")] {
            let body = function(&listing, name);
            let block = body
                .iter()
                .position(|line| *line == bytes)
                .unwrap_or_else(|| panic!("{level} {name}:\n{}", body.join("\n")));
            assert!(body[..block].iter().any(|line| line.starts_with("fld ")), "{level} {name}:\n{}", body.join("\n"));
        }
        // `fsincos` leaves the cosine in st(0) and the sine in st(1): stored in
        // that order
        let body = function(&listing, "sincos_of");
        let block = body.iter().position(|line| *line == "db 0d9h,0fbh").expect("fsincos");
        let stores: Vec<&&str> = body[block..].iter().filter(|line| line.starts_with("fstp ")).collect();
        assert_eq!(stores.len(), 2, "{level}:\n{}", body.join("\n"));
        // general registers, fixed: `cpuid` reads eax and writes eax, ebx, ecx,
        // edx
        let body = function(&listing, "cpuid_a");
        assert!(body.contains(&"db 00fh,0a2h"), "{level}:\n{}", body.join("\n"));
    }
}

#[test]
fn test_a_constraint_that_leaves_the_register_to_the_allocator_is_refused_by_name() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32asm-refused-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("r.c");
    std::fs::write(&source, "int f(int x) { int y; __asm__(\"bswap %0\" : \"=r\"(y) : \"0\"(x)); return y; }\n")
        .unwrap();
    let (done, _) = compile("refused", &source, "-O2");
    assert!(!done.status.success());
    let message = String::from_utf8_lossy(&done.stderr);
    assert!(message.contains("constraint =r is not carried"), "{message}");
}
