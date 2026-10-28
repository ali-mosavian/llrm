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

/// clang keeps a library builtin (`memcpy`) in C's convention whatever
/// `-fdefault-calling-conv` says, and refused its address in a pointer of the
/// default convention: memcpy_runtime.c and memfuncs_runtime.c (6 rows) did
/// not compile at x86_32-wat.
#[test]
fn the_address_of_a_library_builtin_fits_a_default_convention_pointer_at_m32() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("llrm-cc-wat-bi-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("a.c"),
        "extern void *memcpy(void *, const void *, unsigned long);\nstatic void *(*volatile copy)(void *, const void *, unsigned long) = memcpy;\nint f(char *d, char *s) { copy(d, s, 4); return 0; }\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_LLC", std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()))
        .args(["-O1", "-w", "-S", "-o"])
        .arg(dir.join("a.s"))
        .arg(dir.join("a.c"))
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
}

/// The shared start-up called `_main` whatever the frontend: llrm-cc's Watcom
/// `main_` was left unresolved. `LL_MAIN` names the entry, `_main` by default
/// (llrm-nib, llrm-qb), `main_` for a program from llrm-cc.
#[test]
fn the_start_up_calls_the_entry_the_frontend_names() {
    let Some(jwasm) = ["JWASM", "HOME"].iter().find_map(|_| {
        let path = Path::new(&std::env::var_os("HOME")?).join(".local/bin/jwasm");
        path.exists().then_some(path)
    }) else {
        eprintln!("skipped: no jwasm");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("llrm-cc-start-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (model, extra) in [
        ("m32", &["-DSTACK_BYTES=4096", "-DSTACK_RESERVE=512"][..]),
        ("m16", &["-DNEAR_CODE", "-DSTACK_BYTES=4096", "-DSTACK_RESERVE=512"][..]),
    ] {
        for (define, name, absent) in [(None, "_main", "main_"), (Some("-DLL_MAIN=main_"), "main_", "_main")] {
            let obj = dir.join(format!("{model}.obj"));
            let mut assemble = Command::new(&jwasm);
            assemble
                .args(["-q", "-c", "-Cp", "-Zg"])
                .args(if model == "m16" { ["-0"] } else { ["-3"] })
                .args(extra)
                .args(define);
            let done = assemble
                .arg(format!("-Fo{}", obj.display()))
                .arg(root.join(format!("runtime/shared/dos/{model}/start.asm")))
                .output()
                .unwrap();
            assert!(
                done.status.success(),
                "{model}: {}{}",
                String::from_utf8_lossy(&done.stdout),
                String::from_utf8_lossy(&done.stderr)
            );
            let bytes = std::fs::read(&obj).unwrap();
            let has = |text: &str| bytes.windows(text.len()).any(|w| w == text.as_bytes());
            assert!(has(name) && !has(absent), "{model} {define:?}: expected {name} and not {absent}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// `#pragma aux NAME "LINK"` names the symbol a call links to. The pinned
/// clang ignores it, so at m32 `cb` stayed `call cb_` and a runtime that
/// provides `B$COPY` did not link; the x86_32-wat and x86_32-dos rows now take
/// the i8086 fork's clang (`LLRM_CLANG_M16`), which has it (the dos row's
/// `llrm_os.h` names the OS layer's `LL$CLOSE` so: 11 tests/run/c programs
/// failed to link `_llrm_os_close`).
#[test]
fn pragma_aux_link_name_is_the_symbol_at_m32() {
    let Some(fork) = std::env::var_os("LLRM_CLANG_M16") else {
        eprintln!("skipped: LLRM_CLANG_M16 is not set");
        return;
    };
    if std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for row in ["x86_32-wat", "x86_32-dos"] {
        let dir = std::env::temp_dir().join(format!("llrm-cc-wat-aux-{row}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("a.c"),
            "#pragma aux cb \"B$COPY\"\nextern int cb(int a, int b);\nint f(int x) { return cb(x, 2); }\n",
        )
        .unwrap();
        let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", row)
            .env("LLRM_CLANG_M16", &fork)
            .env(
                "LLRM_LLC",
                std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()),
            )
            .args(["-O2", "-w", "-S", "-o"])
            .arg(dir.join("a.s"))
            .arg(dir.join("a.c"))
            .output()
            .unwrap();
        let listing = std::fs::read_to_string(dir.join("a.s")).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
        assert!(listing.contains("B$COPY") && !listing.contains("cb_") && !listing.contains("_cb"), "{row}: {listing}");
    }
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
    std::fs::write(
        dir.join("m.c"),
        "volatile float v = 4.0f;\nint main(void) { return (int)__builtin_sqrtf(v) - 2; }\n",
    )
    .unwrap();
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
            .env(
                "LLRM_LLC",
                std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()),
            )
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

/// m16 `main` was `_main`, which Open Watcom's `cstrt086` does not call: every
/// m16 program failed to link with `undefined symbol main_`.
#[test]
fn an_m16_program_links_against_watcoms_start_up() {
    let ow = std::env::var_os("LLRM_OW").map(std::path::PathBuf::from);
    if std::env::var_os("LLRM_PIN_CLANG").is_none()
        || std::env::var_os("LLRM_CLANG_M16").is_none()
        || !ow.as_ref().is_some_and(|ow| ow.join("bld/clib/library/msdos.086/mm/clibm.lib").exists())
    {
        eprintln!("skipped: needs LLRM_PIN_CLANG, LLRM_CLANG_M16 and a built Open Watcom C library");
        return;
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("llrm-cc-m16-link-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("m.c"), "int main(void) { return 3; }\n").unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_16-wat")
        .env("LLRM_CC_BIN", env!("CARGO_BIN_EXE_llrm-cc"))
        .args(["-O1", "-w", "-o"])
        .arg(dir.join("m.exe"))
        .arg(dir.join("m.c"))
        .output()
        .unwrap();
    let linked = dir.join("m.exe").exists();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        run.status.success() && linked,
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}
