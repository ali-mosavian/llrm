//! C semantics the Watcom targets keep. Needs the pinned clang
//! (`LLRM_PIN_CLANG`) and the Open Watcom headers (`LLRM_OW`).

use std::path::Path;
use std::process::Command;

/// `main` that falls off its end returns 0 (C99): `-ffreestanding` made clang
/// leave the return value unset, so the program exited with a stack load.
#[test]
fn main_that_falls_off_its_end_returns_zero() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::temp_dir().join(format!("llrm-cc-wat-main-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("m.c"), "int v;\nint main(void) { v = 2; }\n").unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_LLC", std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()))
        .args(["-O2", "-w", "-S", "-o"])
        .arg(dir.join("m.s"))
        .arg(dir.join("m.c"))
        .output()
        .unwrap();
    let listing = std::fs::read_to_string(dir.join("m.s")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let body = listing.split("main_ proc").nth(1).unwrap_or_default();
    assert!(body.contains("xor eax, eax") || body.contains("mov eax, 0"), "{listing}");
}

/// `__cdecl` on a flat Watcom target is the stack convention, `_name`; an
/// unmarked function stays the register one, `name_`.
#[test]
fn cdecl_is_the_stack_convention_on_x86_32_wat() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::temp_dir().join(format!("llrm-cc-wat-cdecl-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("c.c"),
        "int __cdecl a(int x, int y) { return x + y; }\nint b(int x, int y) { return x - y; }\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_LLC", std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()))
        .args(["-O2", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    let listing = std::fs::read_to_string(dir.join("c.s")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    assert!(listing.contains("_a proc") && listing.contains("b_ proc"), "{listing}");
    assert!(listing.split("_a proc").nth(1).unwrap().contains("[esp+4]"), "{listing}");
}

/// A float spilled to the cell of its type, as the pin's RFP32 does and
/// llrm-c ran it: without `rounds_float_spills` llrm-cc spilled each of the
/// ten live floats here as a qword (12 `fstp qword`, no `fstp dword`).
#[test]
fn a_spilled_float_is_stored_in_a_dword_cell() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::temp_dir().join(format!("llrm-cc-wat-fspill-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = "float f(float *p, int n) {\n\
        float a=p[0],b=p[1],c=p[2],d=p[3],e=p[4],f=p[5],h=p[6],i=p[7],j=p[8],k=p[9];\n\
        int t;\n\
        for (t=0;t<n;t++) {\n\
            a=a*b+c; b=b*c+d; c=c*d+e; d=d*e+f; e=e*f+h; f=f*h+i; h=h*i+j; i=i*j+k; j=j*k+a; k=k*a+b;\n\
        }\n\
        return a+b+c+d+e+f+h+i+j+k;\n}\n";
    std::fs::write(dir.join("s.c"), source).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_LLC", std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()))
        .args(["-O2", "-w", "-march=i486", "-S", "-o"])
        .arg(dir.join("s.s"))
        .arg(dir.join("s.c"))
        .output()
        .unwrap();
    let listing = std::fs::read_to_string(dir.join("s.s")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    assert!(listing.matches("fstp dword").count() >= 10, "{listing}");
    assert_eq!(listing.matches("fstp qword").count(), 0, "{listing}");
}

/// `wat_link.py` built the float support with `target/release/llrm-cc`: from
/// any other target directory a link failed with "No such file" (18 of the
/// debug-info tests, built in a private target dir). The program calls
/// `sqrtf`, which the float support (floatm.c) defines: it was `_sqrtf`
/// against a call of `sqrtf_`.
#[test]
fn a_program_links_whichever_target_directory_the_driver_is_in() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::temp_dir().join(format!("llrm-cc-wat-link-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("m.c"), "volatile float v = 4.0f;\nint main(void) { return (int)__builtin_sqrtf(v) - 2; }\n").unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env_remove("LLRM_CC_BIN")
        .env("LLRM_LLC", std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()))
        .args(["-O2", "-w", "-o"])
        .arg(dir.join("m.exe"))
        .arg(dir.join("m.c"))
        .output()
        .unwrap();
    let linked = dir.join("m.exe").exists();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(run.status.success() && linked, "{}", String::from_utf8_lossy(&run.stderr));
}

/// The Watcom library is the register convention: `strlen_`, `atan2_`,
/// `printf_`. clang left the functions it knows in C's convention, so a
/// program linked against `_strlen` (m32) or `_atan2@3` (m16), and none of the
/// 29 debug-info tests could link (the float support calls `atan2`).
#[test]
fn library_calls_are_the_watcom_names_at_both_models() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::temp_dir().join(format!("llrm-cc-wat-lib-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("l.c"),
        "#include <string.h>\n#include <math.h>\n#include <stdio.h>\ndouble g(const char *s, double x) { printf(\"%d\", 1); return strlen(s) + atan2(x, x); }\n",
    )
    .unwrap();
    for target in ["x86_16-wat", "x86_32-wat"] {
        let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", target)
            .env("LLRM_LLC", std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()))
            .args(["-O1", "-w", "-S", "-o"])
            .arg(dir.join("l.s"))
            .arg(dir.join("l.c"))
            .output()
            .unwrap();
        assert!(run.status.success(), "{target}: {}", String::from_utf8_lossy(&run.stderr));
        let listing = std::fs::read_to_string(dir.join("l.s")).unwrap();
        for name in ["strlen_", "atan2_", "printf_"] {
            assert!(listing.contains(&format!("extern {name}:")), "{target} {name}: {listing}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// `int (far *fp)(int) = callee;` with `int far callee(int)` was refused by the
/// fork's clang ("incompatible function pointer types"): `far` qualified the
/// result of `callee` instead of its code, and the pointer's `far` was dropped
/// or made a far pointer to a near function. Every m16 function is far.
#[test]
fn a_far_function_pointer_takes_a_far_function() {
    let Some(fork) = std::env::var_os("LLRM_CLANG_M16") else {
        eprintln!("skipped: LLRM_CLANG_M16 is not set");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::temp_dir().join(format!("llrm-cc-wat-fp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("p.c"),
        "int far callee(int x) { return x + 1; }\nint (far *fp)(int) = callee;\nint (*np)(int) = callee;\nint (* far fq)(int) = callee;\nint apply(int (far *f)(int), int v) { return f(v); }\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_16-wat")
        .env("LLRM_CLANG_M16", fork)
        .env("LLRM_LLC", std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()))
        .args(["-O0", "-w", "-c", "-o"])
        .arg(dir.join("p.o"))
        .arg(dir.join("p.c"))
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
}
