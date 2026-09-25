//! QuickrBASIC (`quickr`): sized integers and mandatory declarations.

use crate::hir::execute;
use crate::hir::model::Number;

use super::driver as qb_driver;
use super::test_hir::written;

fn compiled(source: &str) -> Result<crate::hir::model::Program, String> {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "quickr.bas", source.as_bytes());
    qb_driver::parsed(&path, "quickr", "vbdos", None, &[], "column-major", false, false, false, false, false)
        .map_err(|error| error.to_string())
}

/// What FUNCTION `f` returns when the HIR interpreter runs it.
fn returned(source: &str) -> i128 {
    let program = compiled(source).unwrap_or_else(|error| panic!("{error}"));
    match execute::run(&program, "F&", &[]).expect("runs").value {
        Some(Number::Int(value)) => value.into(),
        other => panic!("F returned {other:?}"),
    }
}

#[test]
fn quickr_runs_vbdos_programs() {
    assert_eq!(returned("FUNCTION f&\n  DIM a AS LONG\n  a = 40000\n  f& = a \\ 2\nEND FUNCTION\n"), 20000);
}

/// `FUNCTION f&` whose body is `lines`.
fn long_function(lines: &str) -> i128 {
    returned(&format!("FUNCTION f&\n{lines}\nEND FUNCTION\n"))
}




#[test]
fn byte_wraps_at_256_and_widens_without_sign() {
    assert_eq!(long_function("DIM b AS BYTE\nb = 255\nb = b + 1\nf& = b"), 0);
    assert_eq!(long_function("DIM b AS BYTE\nb = 200\nf& = b"), 200);
}

#[test]
fn byte_arithmetic_promotes_to_integer() {
    // Done in BYTE, 200 + 200 would be 144.
    assert_eq!(long_function("DIM b AS BYTE\nb = 200\nf& = b + b"), 400);
}

#[test]
fn signed_byte_widens_with_sign() {
    assert_eq!(long_function("DIM s AS SIGNED BYTE\ns = -128\nf& = s"), -128);
}

#[test]
fn unsigned_long_holds_values_above_long() {
    let doubled = "DIM u AS UNSIGNED LONG\nu = 2000000000\nu = u + u\n";
    assert_eq!(long_function(&format!("{doubled}f& = u \\ 2")), 2_000_000_000);
    assert_eq!(long_function(&format!("{doubled}f& = u > 1")), -1);
}

#[test]
fn unsigned_integer_plus_long_is_long() {
    assert_eq!(long_function("DIM u AS UNSIGNED INTEGER\nDIM l AS LONG\nu = 65535\nl = -1\nf& = u + l"), 65534);
}

#[test]
fn signed_and_unsigned_spell_the_plain_types() {
    assert_eq!(long_function("DIM i AS SIGNED INTEGER\ni = -1\nf& = i"), -1);
    assert_eq!(long_function("DIM b AS UNSIGNED BYTE\nb = 255\nf& = b"), 255);
}

#[test]
fn byte_constant_out_of_range_is_an_overflow() {
    let error = compiled("DIM b AS BYTE\nb = 256\n").expect_err("256 is not a BYTE");
    assert!(error.contains("overflow"), "{error}");
}

#[test]
fn microsoft_profiles_reject_sized_integers() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "vbdos.bas", b"DIM u AS UNSIGNED INTEGER\nu = 1\n");
    let error = qb_driver::parsed(&path, "vbdos", "vbdos", None, &[], "column-major", false, false, false, false, false)
        .expect_err("VBDOS has no UNSIGNED");
    assert!(error.to_string().contains("quickr"), "{error}");
}


/// The machine code of procedure `name`, one instruction mnemonic per entry.
fn mnemonics(source: &str, name: &str) -> Vec<String> {
    let program = compiled(source).unwrap_or_else(|error| panic!("{error}"));
    let listing = super::test_hir::listing(&program);
    super::test_hir::between(&listing, &format!("{name} proc far"), &format!("{name} endp"))
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

fn operation(type_name: &str, statement: &str) -> Vec<String> {
    mnemonics(&format!("SUB s(a AS {type_name}, b AS {type_name}, r AS LONG)\n{statement}\nEND SUB\n"), "S")
}

#[test]
fn unsigned_integer_divides_with_div() {
    // Signed INTEGER is the control: the same source must pick IDIV.
    assert!(operation("INTEGER", "r = a \\ b").contains(&"idiv".into()));
    let unsigned = operation("UNSIGNED INTEGER", "r = a \\ b");
    assert!(unsigned.contains(&"div".into()) && !unsigned.contains(&"idiv".into()), "{unsigned:?}");
    let unsigned = operation("UNSIGNED LONG", "r = a MOD b");
    assert!(unsigned.contains(&"div".into()) && !unsigned.contains(&"idiv".into()), "{unsigned:?}");
}

#[test]
fn unsigned_integer_compares_with_unsigned_jumps() {
    let signed = ["jl", "jle", "jg", "jge"];
    let unsigned = ["jb", "jbe", "ja", "jae"];
    let jumps = |type_name| operation(type_name, "IF a < b THEN r = 1");
    let control = jumps("INTEGER");
    assert!(control.iter().any(|one| signed.contains(&one.as_str())), "{control:?}");
    let code = jumps("UNSIGNED INTEGER");
    assert!(code.iter().any(|one| unsigned.contains(&one.as_str())), "{code:?}");
    assert!(!code.iter().any(|one| signed.contains(&one.as_str())), "{code:?}");
}

#[test]
fn unsigned_counter_counts_down_with_a_negative_step() {
    // STEP -1 used to convert to the counter's type: a compile-time overflow.
    let body = "DIM u AS UNSIGNED INTEGER\nFOR u = 40002 TO 40000 STEP -1\nf& = f& + u\nNEXT";
    assert_eq!(long_function(body), 120_003);
}

#[test]
fn unsigned_counter_tests_its_limit_unsigned() {
    let code = operation("UNSIGNED INTEGER", "FOR a = a TO b\nr = r + 1\nNEXT");
    assert!(!code.iter().any(|one| ["jl", "jle", "jg", "jge"].contains(&one.as_str())), "{code:?}");
}

#[test]
fn hex_constants_fill_unsigned_types_of_their_width() {
    // &HFFFF is INTEGER -1; it used to be an overflow for UNSIGNED INTEGER.
    assert_eq!(long_function("DIM u AS UNSIGNED INTEGER\nu = &HFFFF\nf& = u"), 65535);
    assert_eq!(long_function("DIM u AS UNSIGNED LONG\nu = &HFFFFFFFF\nf& = u \\ 65536"), 65535);
    let error = compiled("DIM b AS BYTE\nb = &HFFFF\n").expect_err("a word is not a byte");
    assert!(error.contains("overflow"), "{error}");
}

#[test]
fn input_and_read_accept_sized_integers() {
    // Each used to be "destination has an unsupported type".
    compiled("DIM b AS BYTE, u AS UNSIGNED LONG\nINPUT b, u\n").expect("INPUT compiles");
    compiled("DIM b AS SIGNED BYTE\nREAD b\nDATA -5\n").expect("READ compiles");
}

#[test]
fn defbyte_types_unsuffixed_names() {
    // As SINGLE, the default, b + 1 would be 256.
    assert_eq!(returned("DEFBYTE B\nFUNCTION f&\nDIM b\nb = 255\nb = b + 1\nf& = b\nEND FUNCTION\n"), 0);
}
