//! A `const` global was written in `.data` at m64 and m32 (exec.c
//! `ops`/`calls`: 6,680 B of function pointers; gcc and clang: `.rodata`,
//! `.data.rel.ro` when position independent), so a read-only table was writable
//! and the object's `.data` column read 6,680 B against 0.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "int g0(int); int g1(int); int g2(int); int g3(int);\n\
    static const int t[] = {1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16};\n\
    static int (*const h[])(int) = {g0,g1,g2,g3};\n\
    int f(volatile int i, volatile int j) { return t[i & 15] + h[j & 3](i); }\n";

/// `size -A` of `flags` compiled for `target`: section name -> bytes.
fn sections(
    target: &str,
    flags: &[&str],
) -> Vec<(String, u64)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let stem = format!("constdata-{target}-{}-{}", flags.join(""), std::process::id());
    let (source, object) = (tmp.join(format!("{stem}.c")), tmp.join(format!("{stem}.o")));
    std::fs::write(&source, SOURCE).unwrap();
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", target)
        .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
        .args(["-O2", "-w", "-c", "-isystem", "/usr/include/x86_64-linux-gnu"])
        .args(flags)
        .arg(&source)
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let listed = Command::new("size").arg("-A").arg(&object).output().unwrap();
    String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            Some((words.next()?.to_owned(), words.next()?.parse().ok()?))
        })
        .collect()
}

fn bytes(
    found: &[(String, u64)],
    name: &str,
) -> u64 {
    found.iter().filter(|(one, _)| one == name).map(|(_, size)| size).sum()
}

#[test]
fn a_const_table_is_read_only_at_m64_and_m32_without_pic() {
    // x86-m32's object format defaults to OMF: the ELF target asks for ELF,
    // which the placement must follow.
    for (target, flags) in [("x86_64-linux", ["-m64", "-fno-pic"]), ("x86_32-linux", ["-m32", "-fno-pic"])] {
        let found = sections(target, &flags);
        assert_eq!(bytes(&found, ".data"), 0, "{target}: a const table is in .data: {found:?}");
        assert!(bytes(&found, ".rodata") >= 80, "{target}: no .rodata tables: {found:?}");
    }
}

#[test]
fn a_const_pointer_table_is_relro_and_a_plain_one_read_only_with_pic() {
    let found = sections("x86_64-linux", &["-m64", "-fPIC"]);
    assert_eq!(bytes(&found, ".data"), 0, "a const table is in .data: {found:?}");
    assert!(bytes(&found, ".data.rel.ro") >= 32, "the pointer table is not in .data.rel.ro: {found:?}");
    assert!(bytes(&found, ".rodata") >= 64, "the integer table is not in .rodata: {found:?}");
}

/// The other ELF targets already placed them: held so a later change of the
/// shared placement cannot move them.
#[test]
fn a_const_table_is_read_only_at_arm64_and_m68k() {
    for (target, flags) in [("arm64-linux", vec!["-fno-pic"]), ("m68k-68020", vec!["-fno-pic"])] {
        let found = sections(target, &flags);
        assert_eq!(bytes(&found, ".data"), 0, "{target}: a const table is in .data: {found:?}");
        assert!(bytes(&found, ".rodata") >= 80, "{target}: no .rodata tables: {found:?}");
    }
}
