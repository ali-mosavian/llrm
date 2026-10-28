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

/// A far pointer initialised from an integer (`(Video far *)0xA0000000`, a
/// video segment) and written elsewhere: isel refused it, "an address of no
/// global", and the QB runtime's gfxdev.c did not build at x86_16-wat.
#[test]
fn a_far_pointer_initialised_from_an_integer_is_data() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::temp_dir().join(format!("llrm-cc-wat-farint-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("v.c"),
        "typedef volatile unsigned char far Video;\nstatic Video *screen = (Video far *)0xA0000000UL;\nvoid page(Video *p) { screen = p; }\nVideo *get(void) { return screen; }\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_16-wat")
        .env("LLRM_LLC", std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()))
        .args(["-Os", "-w", "-c", "-o"])
        .arg(dir.join("v.o"))
        .arg(dir.join("v.c"))
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
}

/// The same on the flat target, a near pointer (`(void *)(0xA000 << 4)`).
#[test]
fn a_near_pointer_initialised_from_an_integer_is_data() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::temp_dir().join(format!("llrm-cc-wat-nearint-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("v.c"),
        "typedef volatile unsigned char Video;\nstatic Video *screen = (Video *)(0xA000UL << 4);\nvoid page(Video *p) { screen = p; }\nVideo *get(void) { return screen; }\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_LLC", std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()))
        .args(["-Os", "-w", "-c", "-o"])
        .arg(dir.join("v.o"))
        .arg(dir.join("v.c"))
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
}

/// The m16 `main_` pushed and popped bx, cx and dx (6 bytes in 56 programs,
/// +336 B over llrm-c) for a start-up that discards every register after it
/// returns. Nothing in the module calls it, so it keeps nothing; one that
/// something calls keeps Watcom's contract.
#[test]
fn an_uncalled_main_keeps_no_register_and_a_called_one_keeps_watcoms() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_CLANG_M16 or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::temp_dir().join(format!("llrm-cc-main-clobbers-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let body = "extern long f(long);\nextern void report(long);\nint main(void)\n{\n    long a = f(1), b = f(2), c = f(3);\n    report(a);\n    report(b);\n    report(c);\n    return 0;\n}\n";
    let again = format!("{body}int again(void) {{ return main(); }}\n");
    let compile = |name: &str, source: &str| {
        std::fs::write(dir.join(format!("{name}.c")), source).unwrap();
        let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", "x86_16-wat")
            .env(
                "LLRM_LLC",
                std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()),
            )
            .args(["-Os", "-w", "-S", "-o"])
            .arg(dir.join(format!("{name}.s")))
            .arg(dir.join(format!("{name}.c")))
            .output()
            .unwrap();
        assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
        let listing = std::fs::read_to_string(dir.join(format!("{name}.s"))).unwrap();
        let from = listing.find("main_ proc").unwrap();
        listing[from..from + listing[from..].find("main_ endp").unwrap()].to_owned()
    };
    let uncalled = compile("uncalled", body);
    let called = compile("called", &again);
    let _ = std::fs::remove_dir_all(&dir);
    let pushes =
        |text: &str| text.lines().filter(|line| ["push bx", "push cx", "push dx"].contains(&line.trim())).count();
    assert!(pushes(&called) > 0, "premise: the called main keeps registers\n{called}");
    assert_eq!(pushes(&uncalled), 0, "{uncalled}");
}

/// The all-llrm DOS configuration: `x86_16-dos-rp3` is regparm3 (`name@3`,
/// `main_`) where `x86_16-dos` is cdecl (`_name`), and a program in it links
/// with the shared start-up. Without the row, QCport-sized comparisons against
/// llrm-c had cdecl's push/pop cleanup in them (+8.6 KB of +9.8 KB over 65
/// modules).
#[test]
fn the_rp3_dos_row_is_regparm3_and_links() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_CLANG_M16 or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::temp_dir().join(format!("llrm-cc-rp3-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("g.c"), "int g(int a) { return a + 1; }\nint main(void) { return g(1) - 2; }\n").unwrap();
    let llc = std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into());
    let cc = |target: &str, args: &[&str], output: &str| {
        let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", target)
            .env("LLRM_LLC", &llc)
            .args(args)
            .arg(dir.join("g.c"))
            .arg("-o")
            .arg(dir.join(output))
            .output()
            .unwrap();
        assert!(run.status.success(), "{target}: {}", String::from_utf8_lossy(&run.stderr));
    };
    cc("x86_16-dos-rp3", &["-O0", "-S"], "rp3.s");
    cc("x86_16-dos", &["-O0", "-S"], "cdecl.s");
    cc("x86_16-dos-rp3", &["-O2"], "g.exe");
    let rp3 = std::fs::read_to_string(dir.join("rp3.s")).unwrap();
    let cdecl = std::fs::read_to_string(dir.join("cdecl.s")).unwrap();
    let linked = dir.join("g.exe").exists();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(rp3.contains("_g@3 proc") && rp3.contains("main_ proc"), "{rp3}");
    assert!(cdecl.contains("_g proc") && cdecl.contains("_main proc"), "{cdecl}");
    assert!(linked, "the rp3 program did not link");
}

/// The m32 row packed structs to a byte (`-fpack-struct=1`, copied from the m16
/// rows): `struct { char id; void *next; }` was 5 bytes where llrm-c
/// and the assembly sharing the QB runtime's structs (compvect: ids at +4,
/// vectors at +8) have 8, so every m32 QB program died at start-up
/// ("Out of memory", "String formula too complex") once the runtime was built
/// by llrm-cc.
#[test]
fn a_struct_has_its_natural_layout_at_m32_and_is_byte_packed_at_m16() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_CLANG_M16 or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::temp_dir().join(format!("llrm-cc-wat-pack-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("s.c"),
        "struct S { char id; void *next; };\nint size(void) { return sizeof(struct S); }\n",
    )
    .unwrap();
    let size = |target: &str| {
        let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", target)
            .env(
                "LLRM_LLC",
                std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()),
            )
            .args(["-O1", "-w", "-S", "-o"])
            .arg(dir.join("s.s"))
            .arg(dir.join("s.c"))
            .output()
            .unwrap();
        assert!(run.status.success(), "{target}: {}", String::from_utf8_lossy(&run.stderr));
        std::fs::read_to_string(dir.join("s.s")).unwrap()
    };
    let (flat, real) = (size("x86_32-wat"), size("x86_16-wat"));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(flat.contains("mov eax, 8"), "{flat}");
    assert!(real.contains("mov ax, 3"), "{real}");
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

/// The OS layer's routines take the stack convention at m16: the row's
/// default is the register one, and borland.h erases `__cdecl`, so
/// `llrm_os_close(h)` passed the handle in AX where os.asm reads [bp+6]
/// (DOS error -6 for 6 in os_files, os_find, bench_io).
#[test]
fn an_os_layer_call_pushes_its_arguments_at_x86_16_wat() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::temp_dir().join(format!("llrm-cc-wat-oscall-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("o.c"), "#include \"llrm_os.h\"\nint f(short h) { return llrm_os_close(h) + 1; }\n")
        .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_16-wat")
        .env("LLRM_LLC", std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()))
        .args(["-O2", "-w", "-S", "-o"])
        .arg(dir.join("o.s"))
        .arg(dir.join("o.c"))
        .output()
        .unwrap();
    let listing = std::fs::read_to_string(dir.join("o.s")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let before = listing.split("call far ptr LL$CLOSE").next().unwrap_or_default();
    let last: Vec<&str> = before.trim_end().lines().rev().take(2).collect();
    assert!(last.iter().any(|line| line.trim_start().starts_with("push")), "{listing}");
    assert!(!last.iter().any(|line| line.trim_start().starts_with("mov ax")), "{listing}");
}

/// The rp3 row links Borland's library: `cos` and `fabs` are `_cos` and
/// `_fabs`, not the Watcom `cos_` (llc named every library function Watcom's
/// under regparm3: QCport's rp3 link failed on undefined `cos_`, `sin_`,
/// `sqrt_`, `fabs_`), while the program's own functions stay `_name@3`.
#[test]
fn the_rp3_row_calls_the_library_by_its_c_names() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::temp_dir().join(format!("llrm-cc-rp3-lib-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("l.c"),
        "double cos(double);\ndouble fabs(double);\ndouble f(double x) { return cos(x) + fabs(x); }\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_16-dos-rp3")
        .env("LLRM_LLC", std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()))
        .args(["-O2", "-w", "-S", "-o"])
        .arg(dir.join("l.s"))
        .arg(dir.join("l.c"))
        .output()
        .unwrap();
    let listing = std::fs::read_to_string(dir.join("l.s")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    assert!(listing.contains("extern _cos:") && listing.contains("extern _fabs:"), "{listing}");
    assert!(listing.contains("_f@3 proc"), "{listing}");
}

/// Open Watcom's front end predefines the CPU macro (`__386__` flat, `__I86__`
/// real mode) that code written for it tests: bench/textfill took its
/// 16-bit branch at x86_32-wat, and the torture limits.h gave `int` 32767.
#[test]
fn the_wat_rows_predefine_the_watcom_cpu_macro() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return;
    }
    let dir = std::env::temp_dir().join(format!("llrm-cc-wat-cpu-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (row, want, other) in [("x86_32-wat", "__386__", "__I86__"), ("x86_16-wat", "__I86__", "__386__")] {
        std::fs::write(dir.join("m.c"), format!("#ifndef {want}\n#error {want} is not defined\n#endif\n#ifdef {other}\n#error {other} is defined\n#endif\nint v;\n")).unwrap();
        let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", row)
            .args(["-O2", "-w", "-S", "-o"])
            .arg(dir.join("m.s"))
            .arg(dir.join("m.c"))
            .output()
            .unwrap();
        assert!(run.status.success(), "{row}: {}", String::from_utf8_lossy(&run.stderr));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Code written for Open Watcom and GCC assigns function pointers of other
/// types (torture 930608-1); clang 16+ makes that an error by default.
#[test]
fn the_wat_rows_take_incompatible_function_pointer_assignments_as_warnings() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return;
    }
    let dir = std::env::temp_dir().join(format!("llrm-cc-wat-fnptr-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("m.c"), "void g(long);\nvoid (*p)(int);\nvoid f(void) { p = g; }\n").unwrap();
    for row in ["x86_32-wat", "x86_16-wat"] {
        let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", row)
            .args(["-O2", "-S", "-o"])
            .arg(dir.join("m.s"))
            .arg(dir.join("m.c"))
            .output()
            .unwrap();
        assert!(run.status.success(), "{row}: {}", String::from_utf8_lossy(&run.stderr));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
