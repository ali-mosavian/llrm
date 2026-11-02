//! `ctlz` and `cttz` of a word at flat 32-bit x86 are `bsr` and `bsf`.

use std::process::Command;

/// `__builtin_clz` and `__builtin_ctz` went through the shift-and-popcount
/// expansion (`expand = [.. "ctlz", "cttz" ..]` of the m32 description, 25 or
/// more instructions each, `and eax, 1431655765` among them). The pin's llc
/// at i486 writes `bsr; xor 31` and `bsf`.
#[test]
fn test_a_word_count_of_leading_or_trailing_zeros_is_bsr_or_bsf() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("a.c");
    std::fs::write(
        &source,
        "unsigned clz(unsigned a) { return __builtin_clz(a); }\nunsigned ctz(unsigned a) { return __builtin_ctz(a); }\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["--target=x86_32-linux", "-march=i486", "-O2", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(text.contains("bsr "), "{text}");
    assert!(text.contains("bsf "), "{text}");
    assert!(!text.contains("1431655765"), "the popcount expansion is back:\n{text}");
}

/// `bsr; xor 31` and `rep bsf` give the right counts on the built object, not
/// only the right text: a few inputs including the top bit, all ones and, where
/// the source guards it, zero (`ctlz`/`cttz` of 0 is 32 on the guarded forms).
/// Linked with `cc -m32` and run natively; a host that cannot is skipped.
#[test]
fn test_the_counts_of_a_built_object_are_right() {
    let probe = Command::new("cc")
        .args(["-m32", "-x", "c", "-", "-o", "/dev/null"])
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            std::io::Write::write_all(child.stdin.as_mut().unwrap(), b"#include <stdio.h>\nint main(void) { return 0; }\n")?;
            child.wait()
        });
    if !probe.is_ok_and(|status| status.success()) {
        eprintln!("SKIPPED: needs 32-bit libc development files (`cc -m32`)");
        assert!(std::env::var_os("LLRM_REQUIRE_LIBC32").is_none(), "LLRM_REQUIRE_LIBC32 is set");
        return;
    }
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("f.c"),
        "unsigned clz(unsigned a) { return __builtin_clz(a); }\nunsigned ctz(unsigned a) { return __builtin_ctz(a); }\n\
         unsigned clz0(unsigned a) { return a ? __builtin_clz(a) : 32; }\nunsigned ctz0(unsigned a) { return a ? __builtin_ctz(a) : 32; }\n",
    )
    .unwrap();
    std::fs::write(
        scratch.path().join("m.c"),
        "#include <stdio.h>\nunsigned clz(unsigned), ctz(unsigned), clz0(unsigned), ctz0(unsigned);\n\
         int main(void) {\n    unsigned v[] = {1, 2, 3, 0x80000000u, 0xFFFFFFFFu, 0x00010000u, 0x00ff0000u, 0x40u};\n    int bad = 0;\n\
         for (unsigned i = 0; i < sizeof v / sizeof *v; i++) {\n\
         bad += clz(v[i]) != (unsigned)__builtin_clz(v[i]) || ctz(v[i]) != (unsigned)__builtin_ctz(v[i]);\n\
         bad += clz0(v[i]) != (unsigned)__builtin_clz(v[i]) || ctz0(v[i]) != (unsigned)__builtin_ctz(v[i]);\n    }\n\
         bad += clz0(0) != 32 || ctz0(0) != 32;\n    printf(\"bad=%d\\n\", bad);\n    return bad != 0;\n}\n",
    )
    .unwrap();
    let compiled = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["--target=x86_32-linux", "-march=i486", "-O2", "-c", "-fobject-format=elf", "-o", "f.o", "f.c"])
        .output()
        .unwrap();
    assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
    let linked = Command::new("cc")
        .current_dir(scratch.path())
        .args(["-m32", "-no-pie", "m.c", "f.o", "-o", "m"])
        .output()
        .unwrap();
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    let ran = Command::new(scratch.path().join("m")).output().unwrap();
    assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stdout));
}
