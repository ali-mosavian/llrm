//! The C data model of the 6502 is cc65's. The bench compiles the same C with
//! cc65 and, for our backend, with clang's msp430 front end: this measures cc65
//! and checks that clang with the flags the bench uses gives the same sizes,
//! signedness and struct layout, and that both agree with the 6502 description'
//! s pointer width. Needs clang and cc65.

use std::process::Command;

use llrm_6502::LAYOUT;

mod common;

/// The flags that make clang's msp430 front end match cc65.
pub const CLANG_FLAGS: [&str; 3] = ["--target=msp430", "-funsigned-char", "-fpack-struct=1"];

/// One probe for both compilers. Each value is an `unsigned` global.
const PROBE: &str = "\
enum E { A, B, C };
enum F { BIG = 300 };
struct B1 { unsigned a : 3; unsigned b : 5; };
struct S1 { char c; int i; };
struct S2 { char c; long l; };
struct S3 { int i; char c; };
unsigned s_char = sizeof(char), s_short = sizeof(short), s_int = sizeof(int), s_long = sizeof(long);
unsigned s_float = sizeof(float), s_ptr = sizeof(void *), s_fptr = sizeof(void (*)(void));
unsigned s_size_t = sizeof(sizeof(int)), s_ptrdiff = sizeof((char *)0 - (char *)0);
unsigned s_enum_small = sizeof(enum E), s_enum_big = sizeof(enum F);
unsigned s_b1 = sizeof(struct B1);
unsigned s_s1 = sizeof(struct S1), s_s2 = sizeof(struct S2), s_s3 = sizeof(struct S3);
unsigned s_char_const = sizeof('a'), s_int_const = sizeof(40000), s_long_const = sizeof(100000);
unsigned s_char_unsigned = !(((char)-1) < 0);
";

/// Measured on cc65 and not reproducible with clang's msp430: `double` is 32
/// bits in cc65 and 64 in clang; a bit-field that crosses a byte is laid out
/// differently (cc65: `{unsigned a:9, b:9}` is 4 bytes, clang packed: 3); cc65
/// has no `long long`. Kernels using them are not comparable.
pub const NOT_COMPARABLE: [&str; 3] = ["double", "bit-fields wider than a byte", "long long"];

fn dir() -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("c-sizes");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `name = value` for every `s_*` global, as cc65 lays them out.
fn cc65() -> Vec<(String, u64)> {
    let dir = dir();
    std::fs::write(dir.join("probe.c"), PROBE).unwrap();
    let out = Command::new("cc65")
        .args(["-t", "sim6502", "-o", "probe-cc65.s", "probe.c"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let asm = std::fs::read_to_string(dir.join("probe-cc65.s")).unwrap();
    let mut out = Vec::new();
    let mut lines = asm.lines();
    while let Some(line) = lines.next() {
        if let Some(name) = line.strip_prefix("_s_").and_then(|l| l.strip_suffix(':')) {
            let value = lines.next().unwrap().trim();
            let number = value.split_whitespace().nth(1).unwrap().trim_start_matches('$');
            out.push((name.to_owned(), u64::from_str_radix(number, 16).unwrap()));
        }
    }
    out
}

/// The same from clang with `flags`.
fn clang(flags: &[&str]) -> Vec<(String, u64)> {
    sizes("clang".as_ref(), flags)
}

/// The same from the front end as llrm-cc runs it for the 6502 row.
fn llrm_cc() -> Vec<(String, u64)> {
    sizes(&common::pipeline::llrm_cc(), &["--target=6502-sim65"])
}

fn sizes(program: &std::path::Path, flags: &[&str]) -> Vec<(String, u64)> {
    let dir = dir();
    std::fs::write(dir.join("probe.c"), PROBE).unwrap();
    let out = Command::new(program)
        .args(flags)
        .args(["-S", "-emit-llvm", "-o", "-", "probe.c"])
        .current_dir(&dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("@s_")?;
            let (name, value) = rest.split_once(' ')?;
            Some((name.to_owned(), value.split("global i16 ").nth(1)?.split(',').next()?.parse().ok()?))
        })
        .collect()
}

fn pointer_bytes() -> u64 {
    let layout = LAYOUT.lines().find_map(|l| l.strip_prefix("datalayout = ")).expect("a datalayout").trim_matches('"');
    let bits: u64 = layout
        .split('-')
        .find_map(|f| f.strip_prefix("p:"))
        .and_then(|f| f.split(':').next())
        .unwrap()
        .parse()
        .unwrap();
    bits / 8
}

/// cc65's table: the 6502 data model. Pointers, `size_t` and `ptrdiff_t` have
/// the description's width; `char` is unsigned; structs are packed (alignment
/// 1).
#[test]
#[ignore = "needs cc65"]
fn cc65_is_the_6502_data_model() {
    let table = cc65();
    let get = |name: &str| table.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("no {name}")).1;
    for name in ["ptr", "fptr", "size_t", "ptrdiff"] {
        assert_eq!(get(name), pointer_bytes(), "{name}");
    }
    let want = [
        ("char", 1),
        ("short", 2),
        ("int", 2),
        ("long", 4),
        ("float", 4),
        ("enum_small", 2),
        ("enum_big", 2),
        ("b1", 1),
        ("s1", 3),
        ("s2", 5),
        ("s3", 3),
        ("char_const", 2),
        ("int_const", 4),
        ("long_const", 4),
        ("char_unsigned", 1),
    ];
    for (name, value) in want {
        assert_eq!(get(name), value, "cc65 {name}");
    }
}

/// The front end of llrm-cc's 6502 rows gives cc65's table.
#[test]
#[ignore = "needs llrm-cc, the pinned clang and cc65"]
fn the_llrm_cc_rows_front_end_matches_cc65() {
    assert_eq!(llrm_cc(), cc65());
}

/// The same flags on bare clang: the control that the probe sees them.
#[test]
#[ignore = "needs clang and cc65"]
fn clang_with_the_row_flags_matches_cc65() {
    assert_eq!(clang(&CLANG_FLAGS), cc65());
}

/// The probe sees each flag: without it clang differs from cc65 (a dropped flag
/// must fail the match).
#[test]
#[ignore = "needs clang and cc65"]
fn dropping_a_flag_breaks_the_match() {
    let cc65 = cc65();
    assert_ne!(clang(&["--target=msp430", "-funsigned-char"]), cc65, "-fpack-struct=1 changes nothing");
    assert_ne!(
        clang(&["--target=msp430", "-fsigned-char", "-fpack-struct=1"]),
        cc65,
        "-funsigned-char changes nothing"
    );
}
