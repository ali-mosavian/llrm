//! The C types the bench kernels are compiled with: clang's msp430 front end and cc65 must give the
//! sizes the 6502 target description has (16-bit pointers, 16-bit int), or the bench compares
//! different programs. Needs clang and cc65.

use std::process::Command;

use llrm_6502::LAYOUT;

/// (name, C expression for the size in bytes, the size the 6502 target has).
fn expected() -> Vec<(&'static str, &'static str, u64)> {
    // pointer width from the target description's datalayout: `p:16:8`
    let layout = LAYOUT.lines().find_map(|l| l.strip_prefix("datalayout = ")).expect("a datalayout").trim_matches('"');
    let pointer_bits: u64 = layout.split('-').find_map(|f| f.strip_prefix("p:")).and_then(|f| f.split(':').next()).unwrap().parse().unwrap();
    vec![
        ("char", "sizeof(char)", 1),
        ("short", "sizeof(short)", 2),
        ("int", "sizeof(int)", 2),
        ("long", "sizeof(long)", 4),
        ("longlong", "sizeof(long long)", 8),
        ("pointer", "sizeof(void *)", pointer_bits / 8),
        ("size_t", "sizeof(sizeof(int))", pointer_bits / 8),
        ("ptrdiff", "sizeof((char *)0 - (char *)0)", pointer_bits / 8),
    ]
}

fn source(skip_long_long: bool) -> String {
    expected()
        .iter()
        .filter(|(name, ..)| !(skip_long_long && *name == "longlong"))
        .map(|(name, expression, _)| format!("unsigned size_{name} = {expression};\n"))
        .collect()
}

fn write(
    name: &str,
    text: &str,
) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("c-sizes");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name), text).unwrap();
    dir
}

/// clang `--target=msp430`, the front end of the bench, against the 6502 description.
#[test]
#[ignore = "needs clang"]
fn clangs_msp430_sizes_are_the_6502s() {
    let dir = write("probe.c", &source(false));
    let out = Command::new("clang").args(["--target=msp430", "-S", "-emit-llvm", "-o", "-", "probe.c"]).current_dir(&dir).output().unwrap();
    let ir = String::from_utf8_lossy(&out.stdout).into_owned();
    for (name, _, want) in expected() {
        let line = ir.lines().find(|l| l.starts_with(&format!("@size_{name} "))).unwrap_or_else(|| panic!("no size_{name} in {ir}"));
        let got: u64 = line.split("global i16 ").nth(1).unwrap().split(',').next().unwrap().parse().unwrap();
        assert_eq!(got, want, "sizeof {name}: msp430 {got}, the 6502 target {want}");
    }
}

/// cc65 has no `long long`; every other size matches too.
#[test]
#[ignore = "needs cc65"]
fn cc65s_sizes_are_the_6502s() {
    let dir = write("probe-cc65.c", &source(true));
    let status = Command::new("cc65").args(["-t", "sim6502", "-o", "probe-cc65.s", "probe-cc65.c"]).current_dir(&dir).status().unwrap();
    assert!(status.success());
    let asm = std::fs::read_to_string(dir.join("probe-cc65.s")).unwrap();
    for (name, _, want) in expected().into_iter().filter(|(name, ..)| *name != "longlong") {
        let at = asm.find(&format!("_size_{name}:")).unwrap_or_else(|| panic!("no _size_{name}"));
        let word = asm[at..].lines().nth(1).unwrap().trim().strip_prefix(".word").unwrap().trim().trim_start_matches('$');
        let got = u64::from_str_radix(word, 16).unwrap();
        assert_eq!(got, want, "sizeof {name}: cc65 {got}, the 6502 target {want}");
    }
}
