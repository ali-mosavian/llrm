//! Flat x86 (`x86_32-linux`, `x86_64-linux`): clang's inline asm with the
//! operands in fixed places (`{st}` in, `=t`/`=u` out, an address register for
//! `=m`, `{eax}`...) is a block of bytes laid down at the site
//! (`llrm.ia32.asm.*`). QuickrBasic's `pfpu.h` (`fistpl`, `fistpll`) and
//! `qbrt_dos.c` (`fsincos`) were refused by llrm-llc ("expected a constant,
//! found `asm`"): double.c, fpmath.c and operands.c did not compile at any
//! level. A constraint that leaves the register to the allocator (`r`, `q`,
//! `x`) takes a register of the pool at the site; `g` and the rest are refused
//! by name (stage 2, `ISD::INLINEASM`, takes those); none is miscompiled.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::{Command, Output};

fn compile(
    tag: &str,
    source: &Path,
    level: &str,
    target: &str,
) -> (Output, String) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flatasm-{tag}-{level}-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", target)
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args([level, "-w", "-S"])
        .args(if target == "x86_32-linux" { vec!["-march=i486", "-mabi=sysv"] } else { vec![] })
        .arg("-o")
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
        let (done, listing) = compile("x87", &source, level, "x86_32-linux");
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
fn test_a_constraint_that_is_not_a_register_class_is_refused_by_name() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32asm-refused-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("r.c");
    std::fs::write(&source, "int f(int x) { int y; __asm__(\"mov %1, %0\" : \"=a\"(y) : \"g\"(x)); return y; }\n")
        .unwrap();
    let (done, _) = compile("refused", &source, "-O2", "x86_32-linux");
    assert!(!done.status.success());
    let message = String::from_utf8_lossy(&done.stderr);
    assert!(message.contains("constraint imr is not carried"), "{message}");
}

#[test]
fn test_x86_64_blocks_name_their_registers_at_the_operands_width() {
    // `{cx}` holding an `i64` is `rcx` in the block's text: `mulq %cx` did not
    // assemble. The x87 operands were `double`s in xmm registers, which the
    // stack cannot take without a conversion.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m64_inline_asm.c");
    for level in ["-O1", "-O2", "-Os"] {
        let (done, listing) = compile("m64", &source, level, "x86_64-linux");
        assert!(done.status.success(), "{level}: {}", String::from_utf8_lossy(&done.stderr));
        for (name, bytes) in [("round_int", "db 0dbh,018h"), ("round_long", "db 0dfh,038h")] {
            let body = function(&listing, name);
            let block = body
                .iter()
                .position(|line| *line == bytes)
                .unwrap_or_else(|| panic!("{level} {name}:\n{}", body.join("\n")));
            assert!(body[..block].iter().any(|line| line.starts_with("fld ")), "{level} {name}:\n{}", body.join("\n"));
        }
        let body = function(&listing, "sincos_of");
        let block = body.iter().position(|line| *line == "db 0d9h,0fbh").expect("fsincos");
        assert_eq!(
            body[block..].iter().filter(|line| line.starts_with("fstp ")).count(),
            2,
            "{level}:\n{}",
            body.join("\n")
        );
        // `mulq %rcx`: REX.W, f7 /4, rcx
        let body = function(&listing, "mul_hi");
        assert!(body.contains(&"db 048h,0f7h,0e1h"), "{level}:\n{}", body.join("\n"));
    }
}

/// QB `pfpu.h`: `cvtsd2si %1, %0 : "=r"(v) : "x"(x)` and `bswap %0 : "=r" :
/// "0"` were refused at m64 ("constraint =r is not carried"), so double.c,
/// fpmath.c, exec.c and operands.c did not compile at any level. The register
/// classes `r` and `x` take pool registers at the site; the block is the same
/// bytes the named registers give.
#[test]
fn test_x86_64_register_class_constraints_take_registers_of_the_pool() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m64_inline_asm_classes.c");
    for level in ["-O1", "-O2", "-Os"] {
        let (done, listing) = compile("m64-classes", &source, level, "x86_64-linux");
        assert!(done.status.success(), "{level}: {}", String::from_utf8_lossy(&done.stderr));
        // cvtsd2si rax, xmm0 / cvtss2si eax, xmm0
        let body = function(&listing, "rint64");
        assert!(body.iter().any(|line| line.starts_with("db 0f2h,048h,00fh,02dh,0")), "{level}:\n{}", body.join("\n"));
        let body = function(&listing, "rint32");
        assert!(body.iter().any(|line| line.starts_with("db 0f3h,00fh,02dh,0")), "{level}:\n{}", body.join("\n"));
        // bswap, the input tied to the output
        let body = function(&listing, "swap32");
        assert!(body.iter().any(|line| line.starts_with("db 00fh,0c")), "{level}:\n{}", body.join("\n"));
    }
}

/// The pool registers are fixed places, so the allocator keeps what is live
/// out of them: values live across a block, a clobber naming a register the
/// pool would take, two blocks in a function and in a loop, doubles live in
/// the vector registers an `x` operand takes. Compiled at each level, linked
/// and run on the host: the results are the C ones.
#[test]
fn test_x86_64_register_class_blocks_run_with_their_values_live_across() {
    if !cfg!(target_arch = "x86_64") {
        return;
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m64_inline_asm_run.c");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flatasm-run-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for level in ["-O1", "-O2", "-Os"] {
        let object = dir.join(format!("run{level}.o"));
        let built = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "x86_64-linux")
            .env(
                "LLRM_LLC",
                std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
            )
            .args([level, "-w", "-c", "-o"])
            .arg(&object)
            .arg(&source)
            .output()
            .unwrap();
        assert!(built.status.success(), "{level}: {}", String::from_utf8_lossy(&built.stderr));
        let exe = dir.join(format!("a{level}.out"));
        let linked = Command::new("gcc").args(["-static", "-no-pie", "-o"]).arg(&exe).arg(&object).output().unwrap();
        assert!(linked.status.success(), "{level}: {}", String::from_utf8_lossy(&linked.stderr));
        let ran = Command::new(&exe).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), "bad 0", "{level}");
    }
}
