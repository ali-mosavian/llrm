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
fn quickr_runs_declared_vbdos_programs() {
    assert_eq!(returned("FUNCTION f&\n  DIM a AS LONG\n  a = 40000\n  f& = a \\ 2\nEND FUNCTION\n"), 20000);
}

/// `FUNCTION f&` whose body is `lines`.
fn long_function(lines: &str) -> i128 {
    returned(&format!("FUNCTION f&\n{lines}\nEND FUNCTION\n"))
}




#[test]
fn unsigned_byte_wraps_at_256_and_widens_without_sign() {
    assert_eq!(long_function("DIM b AS UNSIGNED BYTE\nb = 255\nb = b + 1\nf& = b"), 0);
    assert_eq!(long_function("DIM b AS UNSIGNED BYTE\nb = 200\nf& = b"), 200);
}

#[test]
fn byte_is_signed() {
    assert_eq!(long_function("DIM b AS BYTE\nb = 127\nb = b + 1\nf& = b"), -128);
    let error = compiled("DIM b AS BYTE\nb = 200\n").expect_err("200 is not a signed byte");
    assert!(error.contains("overflow"), "{error}");
}

#[test]
fn byte_arithmetic_promotes_to_integer() {
    // Done in BYTE, 100 + 100 would be -56.
    assert_eq!(long_function("DIM b AS BYTE\nb = 100\nf& = b + b"), 200);
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
    let error = compiled("DIM b AS UNSIGNED BYTE\nb = &HFFFF\n").expect_err("a word is not a byte");
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
    // As SINGLE, the default, b + 1 would be 128.
    assert_eq!(returned("DEFBYTE B\nFUNCTION f&\nDIM b\nb = 127\nb = b + 1\nf& = b\nEND FUNCTION\n"), -128);
}

#[test]
fn undeclared_variables_are_not_defined() {
    for source in ["x = 1\n", "DIM y AS INTEGER\ny = x\n", "DIM i AS INTEGER\nFOR k = 1 TO 2\nNEXT\n"] {
        let error = compiled(source).expect_err(source);
        assert!(error.contains("Variable not defined"), "{source}: {error}");
    }
}

#[test]
fn every_declaring_statement_declares() {
    let source = "DIM SHARED c AS INTEGER, d AS INTEGER\n\
        DIM m AS INTEGER\n\
        CONST k = 3\n\
        REDIM r(5) AS INTEGER\n\
        c = 1: d = 2: m = k: r(1) = m\n\
        SUB s (p AS INTEGER)\n\
        STATIC t AS INTEGER\n\
        SHARED m\n\
        t = p + c + d + m\n\
        END SUB\n";
    compiled(source).expect("declared names compile");
}

#[test]
fn vbdos_still_declares_implicitly() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "vbdos.bas", b"x = 1\n");
    qb_driver::parsed(&path, "vbdos", "vbdos", None, &[], "column-major", false, false, false, false, false)
        .expect("VBDOS declares x");
}

#[test]
fn defu_statements_type_unsuffixed_names_unsigned() {
    // Under the SINGLE default each would read -1, 256 or a wrapped LONG.
    assert_eq!(returned("DEFUINT U\nFUNCTION f&\nDIM u\nu = &HFFFF\nf& = u\nEND FUNCTION\n"), 65535);
    assert_eq!(returned("DEFUBYTE B\nFUNCTION f&\nDIM b\nb = 255\nb = b + 1\nf& = b\nEND FUNCTION\n"), 0);
    let doubled = "DEFULNG U\nFUNCTION f&\nDIM u\nu = 2000000000\nu = u + u\nf& = u \\ 2\nEND FUNCTION\n";
    assert_eq!(returned(doubled), 2_000_000_000);
}

#[test]
fn defuint_types_function_names() {
    let source = "DEFUINT G\nFUNCTION g\ng = &HFFFF\nEND FUNCTION\nFUNCTION f&\nf& = g\nEND FUNCTION\n";
    assert_eq!(returned(source), 65535);
}


#[test]
fn microsoft_profiles_reject_defu_statements() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "vbdos.bas", b"DEFUINT A-Z\n");
    let error = qb_driver::parsed(&path, "vbdos", "vbdos", None, &[], "column-major", false, false, false, false, false)
        .expect_err("VBDOS has no DEFUINT");
    assert!(error.to_string().contains("quickr"), "{error}");
}

#[test]
fn conversions_give_their_sized_type() {
    // Each result is observed through a wider variable, so a conversion that
    // kept its source type would return the unconverted value.
    let converted = |call: &str| long_function(&format!("DIM l AS LONG\nl = 65535\nf& = {call}"));
    assert_eq!(converted("CBYTE(l AND 255)"), -1);
    assert_eq!(converted("CUBYTE(l AND 255)"), 255);
    assert_eq!(converted("CUINT(l)"), 65535);
    assert_eq!(converted("CINT(l)"), -1);
    // As LONG, -1 \ 2 would be 0.
    assert_eq!(converted("CULNG(l - 65536) \\ 2"), 2_147_483_647);
}

#[test]
fn conversion_to_unsigned_selects_unsigned_division() {
    let code = operation("INTEGER", "r = CUINT(a) \\ CUINT(b)");
    assert!(code.contains(&"div".into()) && !code.contains(&"idiv".into()), "{code:?}");
}

#[test]
fn conversion_constants_keep_the_overflow_rules() {
    assert_eq!(long_function("f& = CUINT(&HFFFF)"), 65535);
    for call in ["CBYTE(200)", "CUBYTE(256)", "CUINT(70000)"] {
        let error = compiled(&format!("DIM b AS LONG\nb = {call}\n")).expect_err(call);
        assert!(error.contains("overflow"), "{call}: {error}");
    }
}

#[test]
fn microsoft_profiles_have_no_sized_conversions() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "vbdos.bas", b"FUNCTION cuint% (x AS INTEGER)\ncuint% = x + 1\nEND FUNCTION\n");
    qb_driver::parsed(&path, "vbdos", "vbdos", None, &[], "column-major", false, false, false, false, false)
        .expect("CUINT is an ordinary name in VBDOS");
}

#[test]
fn print_and_str_show_sized_integers_by_value() {
    // Printed as their Microsoft bit patterns these would be -25536, -56 and -294967296.
    let source = "DIM u AS UNSIGNED INTEGER, b AS UNSIGNED BYTE, l AS UNSIGNED LONG, s AS BYTE\n\
        u = 40000: b = 200: l = 2000000000: l = l + l: s = -5\n\
        PRINT u; b; l; s\n\
        PRINT STR$(u); STR$(l)\n";
    let printed = super::test_runtime_model::printed_on(source, "quickr", "vbdos");
    assert_eq!(printed, " 40000  200  4000000000 -5 \n 40000 4000000000\n");
}

fn printed(source: &str) -> String {
    super::test_runtime_model::printed_on(source, "quickr", "vbdos")
}

#[test]
fn f_strings_interpolate_strings_and_numbers() {
    let source = "DIM who AS STRING, n AS INTEGER, d AS DOUBLE, u AS UNSIGNED INTEGER\n\
        who = \"Ada\": n = -3: d = 0.25: u = 40000\n\
        PRINT f\"{who} has {n} and {d}, {u}{{}}\"\n\
        PRINT F\"{n + 5}\"; f\"\"; f\"plain\"\n";
    assert_eq!(printed(source), "Ada has -3 and 0.25, 40000{}\n2plain\n");
}

#[test]
fn f_strings_are_string_values() {
    let source = "DIM s AS STRING, i AS INTEGER\ni = 7\ns = f\"<{i}>\" + \"!\"\nPRINT s; LEN(s)\n";
    assert_eq!(printed(source), "<7>! 4 \n");
}


#[test]
fn f_string_errors() {
    for (source, message) in [
        ("PRINT f\"{\"\n", "unterminated f-string field"),
        ("PRINT f\"}\"\n", "single '}'"),
        ("PRINT f\"{}\"\n", "empty f-string field"),
        ("SUB quickr_x\nEND SUB\nPRINT f\"a\"\n", "reserved"),
    ] {
        let error = compiled(source).expect_err(source);
        assert!(error.contains(message), "{source}: {error}");
    }
}

/// Each field's expected text is what Python's own `format()` gives.
const FORMAT_CASES: &[(&str, &str, &str, &str)] = &[
        ("LONG", "42", "", "42"),
        ("LONG", "-42", "5", "  -42"),
        ("LONG", "42", "<5", "42   "),
        ("LONG", "42", "^6", "  42  "),
        ("LONG", "42", "*^7", "**42***"),
        ("LONG", "-42", "=6", "-   42"),
        ("LONG", "-42", "06", "-00042"),
        ("LONG", "42", "+", "+42"),
        ("LONG", "42", " ", " 42"),
        ("LONG", "1234567", ",", "1,234,567"),
        ("LONG", "1234567", "_", "1_234_567"),
        ("LONG", "1234", "010,", "00,001,234"),
        ("LONG", "255", "x", "ff"),
        ("LONG", "255", "#X", "0XFF"),
        ("LONG", "255", "#010b", "0b11111111"),
        ("LONG", "255", "o", "377"),
        ("LONG", "65535", "#_x", "0xffff"),
        ("LONG", "65", "c", "A"),
        ("LONG", "-7", "n", "-7"),
        ("LONG", "3", ".2f", "3.00"),
        ("LONG", "3", "e", "3.000000e+00"),
        ("LONG", "-255", "#x", "-0xff"),
        ("UNSIGNED LONG", "4000000000#", ",", "4,000,000,000"),
        ("UNSIGNED LONG", "4000000000#", "x", "ee6b2800"),
        ("DOUBLE", "3.14159#", ".2f", "3.14"),
        ("DOUBLE", "-3.14159#", "+.3f", "-3.142"),
        ("DOUBLE", "2.5#", ".0f", "2"),
        ("DOUBLE", "3.5#", ".0f", "4"),
        ("DOUBLE", "0.125#", ".2f", "0.12"),
        ("DOUBLE", "2.675#", ".2f", "2.67"),
        ("DOUBLE", "1234567.891#", ",.2f", "1,234,567.89"),
        ("DOUBLE", "0.5#", "%", "50.000000%"),
        ("DOUBLE", "0.1234#", ".1%", "12.3%"),
        ("DOUBLE", "12345.678#", "e", "1.234568e+04"),
        ("DOUBLE", "12345.678#", ".2E", "1.23E+04"),
        ("DOUBLE", "0.000123#", ".3e", "1.230e-04"),
        ("DOUBLE", "12345.678#", "g", "12345.7"),
        ("DOUBLE", "0.0001#", "g", "0.0001"),
        ("DOUBLE", "0.00001#", "g", "1e-05"),
        ("DOUBLE", "123456789#", "g", "1.23457e+08"),
        ("DOUBLE", "100#", "g", "100"),
        ("DOUBLE", "100#", "#g", "100.000"),
        ("DOUBLE", "1.5#", ".3", "1.5"),
        ("DOUBLE", "1234.5#", ".3", "1.23e+03"),
        ("DOUBLE", "1#", ".3", "1.0"),
        ("DOUBLE", "-0.0001#", "z.2f", "0.00"),
        ("DOUBLE", "-2.5#", "10.1f", "      -2.5"),
        ("DOUBLE", "-2.5#", "<10.1f", "-2.5      "),
        ("DOUBLE", "-2.5#", "010.1f", "-0000002.5"),
        ("DOUBLE", "0.25#", "", "0.25"),
        ("DOUBLE", "0.25#", "8", "    0.25"),
        ("DOUBLE", "-0.25#", "+", "-0.25"),
        ("DOUBLE", "1234.5#", ",", "1,234.5"),
        ("DOUBLE", "3#", "#.0f", "3."),
        ("DOUBLE", "3#", "#.0e", "3.e+00"),
        ("DOUBLE", "0#", "e", "0.000000e+00"),
        ("DOUBLE", "1D+100", ".2e", "1.00e+100"),
        ("DOUBLE", "9.9999#", ".2f", "10.00"),
        ("DOUBLE", "9.9999#", ".2e", "1.00e+01"),
        ("STRING", "\"hi\"", "", "hi"),
        ("STRING", "\"hi\"", "5", "hi   "),
        ("STRING", "\"hi\"", ">5", "   hi"),
        ("STRING", "\"hi\"", "^6", "  hi  "),
        ("STRING", "\"hello\"", ".3", "hel"),
        ("STRING", "\"hi\"", "-<6", "hi----"),
        ("STRING", "\"hi\"", "05", "hi000"),
        // Found by a 400-case comparison with Python.
        ("DOUBLE", "6.02D+23", "+_.2%", "+60_200_000_000_000_001_459_617_792.00%"),
        // Found by a 400-case comparison with Python.
        ("DOUBLE", "6.02D+23", "#,f", "601,999,999,999,999,995,805,696.000000"),
        // Found by a 400-case comparison with Python.
        ("DOUBLE", "6.02D+23", " 7,.5F", " 601,999,999,999,999,995,805,696.00000"),
        // Found by a 400-case comparison with Python.
        ("DOUBLE", "1000000000000000.0#", "0^#", "1000000000000000.0"),
        // Found by a 400-case comparison with Python.
        ("LONG", "255", "=-#3.0g", "3.e+02"),
        // Found by a 400-case comparison with Python.
        ("DOUBLE", "-5687.276487884854#", "-=_", "-5_687.276487884854"),
        // Found by a 400-case comparison with Python.
        ("DOUBLE", "0.0#", " 10", "       0.0"),
        // Found by a 400-case comparison with Python.
        ("DOUBLE", "6.02D+23", ".0%", "60200000000000001459617792%"),
        // Found by a 400-case comparison with Python.
        ("DOUBLE", "0.0#", "0<1", "0.0"),
        // Found by a 400-case comparison with Python.
        ("LONG", "958510", "#1g", "958510."),
        // Found by a 400-case comparison with Python.
        ("DOUBLE", "0.1D0+0.2D0", "", "0.30000000000000004"),
        // Found by a 400-case comparison with Python.
        ("DOUBLE", "1D+16", "", "1e+16"),
        // Found by a 400-case comparison with Python.
        ("DOUBLE", "1D-05", "", "1e-05"),
];

#[test]
fn f_string_specs_format_as_python_does() {
    let mut source = String::new();
    let mut expected = String::new();
    for (index, (type_name, value, spec, text)) in FORMAT_CASES.iter().enumerate() {
        source += &format!("DIM v{index} AS {type_name}\nv{index} = {value}\nPRINT f\"[{{v{index}:{spec}}}]\"\n");
        expected += &format!("[{text}]\n");
    }
    let printed = printed(&source);
    for ((case, want), got) in FORMAT_CASES.iter().zip(expected.lines()).zip(printed.lines()) {
        assert_eq!(got, want, "{case:?}");
    }
    assert_eq!(printed.lines().count(), FORMAT_CASES.len());
}




#[test]
fn plain_floats_print_as_python_repr() {
    // STR$ gave " .1" and 15 digits; repr is the shortest text that reads back.
    let source = "DIM s AS SINGLE, d AS DOUBLE\ns = 0.1\nd = 0.1#\nd = d + 0.2#\nPRINT f\"{s} {d} {-s}\"\n";
    assert_eq!(printed(source), "0.1 0.30000000000000004 -0.1\n");
}
