//! QuickrBASIC (`quickr`): sized integers and mandatory declarations.

use llrm_core::hir::execute;
use llrm_core::hir::model::Number;

use super::driver as qb_driver;
use super::test_hir::written;

fn compiled(source: &str) -> Result<llrm_core::hir::model::Program, String> {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "quickr.bas", source.as_bytes());
    qb_driver::parsed(&path, &qb_driver::Frontend::new("quickr", "vbdos"), None)
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
    let error = qb_driver::parsed(&path, &qb_driver::Frontend::new("vbdos", "vbdos"), None)
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
    qb_driver::parsed(&path, &qb_driver::Frontend::new("vbdos", "vbdos"), None)
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
    let error = qb_driver::parsed(&path, &qb_driver::Frontend::new("vbdos", "vbdos"), None)
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
    qb_driver::parsed(&path, &qb_driver::Frontend::new("vbdos", "vbdos"), None)
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

/// The listing of procedure `name` in `source` compiled as `dialect`.
fn procedure_listing(source: &str, dialect: &str, name: &str) -> String {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "frame.bas", source.as_bytes());
    let program = qb_driver::parsed(&path, &qb_driver::Frontend::new(dialect, "vbdos"), None)
        .unwrap_or_else(|error| panic!("{error}"));
    let listing = super::test_hir::listing(&program);
    super::test_hir::between(&listing, &format!("{name} proc far"), &format!("{name} endp")).to_owned()
}

#[test]
fn quickr_procedures_frame_and_zero_fill_themselves() {
    let source = "SUB s (x AS INTEGER)\nDIM a AS LONG, b AS DOUBLE\na = x\nb = a\nx = b\nEND SUB\n";
    let own = procedure_listing(source, "quickr", "S");
    assert!(!own.contains("B$ENRA") && !own.contains("B$EXSA"), "{own}");
    // The HIR's own stores zero the locals; no inline rep stosw.
    assert!(!own.contains("0f3h,0abh"), "{own}");
    // VBDOS keeps the runtime's frame.
    assert!(procedure_listing(source, "vbdos", "S").contains("B$ENRA"));
}

#[test]
fn quickr_keeps_the_runtime_frame_where_the_runtime_needs_it() {
    let strings = "SUB s\nDIM t AS STRING\nt = \"x\"\nEND SUB\n";
    let handler = "SUB s\nDIM i AS INTEGER\nON LOCAL ERROR GOTO h\ni = 1\nEXIT SUB\nh:\nRESUME NEXT\nEND SUB\n";
    for source in [strings, handler] {
        assert!(procedure_listing(source, "quickr", "S").contains("B$ENRA"), "{source}");
    }
}

#[test]
fn locals_read_before_assignment_start_at_zero() {
    // Frames hold garbage under quickr; entry stores must zero these locals.
    let source = "TYPE P\nx AS INTEGER\ny AS DOUBLE\nEND TYPE\n\
        s\ns\n\
        SUB s\nDIM a AS LONG, d AS DOUBLE, u AS UNSIGNED BYTE, p AS P, i AS INTEGER\n\
        FOR i = 1 TO 2\na = a + 1\nNEXT\n\
        PRINT a; d; u; p.x; p.y\nd = 5: p.x = 7\nEND SUB\n";
    assert_eq!(printed(source), " 2  0  0  0  0 \n 2  0  0  0  0 \n");
}

fn quickr_program(source: &str) -> llrm_core::hir::model::Program {
    compiled(source).unwrap_or_else(|error| panic!("{error}"))
}

/// Each local of procedure `name` after the backend's layout: (name, low, high).
fn frame_after_layout(source: &str, name: &str) -> Vec<(String, i64, i64)> {
    let program = quickr_program(source);
    let laid_out = super::zero_fill::laid_out(&program, |module, function| {
        !super::compile::_inline_frame(&program, module, function)
    });
    let module = &laid_out.modules[0];
    let widths: std::collections::HashMap<i64, i64> = module.types.iter().map(|one| (one.id, one.width)).collect();
    let function = module.functions.iter().find(|one| one.name == name).expect("the procedure");
    let mut frame: Vec<_> = function
        .places
        .iter()
        .filter(|one| one.storage == llrm_core::hir::model::Storage::Local)
        .map(|one| (one.name.clone(), one.offset, one.offset + one.extent.unwrap_or(widths[&one.r#type])))
        .collect();
    frame.sort_by_key(|one| -one.2);
    frame
}

#[test]
fn zeroed_locals_are_one_block_below_bp() {
    // Declared interleaved, zeroed and written-first locals used to alternate.
    let source = "SUB s\nDIM a AS LONG, b AS LONG, c AS INTEGER, d AS DOUBLE, e(2) AS INTEGER\n\
        b = 1\nd = 2\nPRINT a; b; c; d; e(1)\nEND SUB\n";
    let frame = frame_after_layout(source, "S");
    let names: Vec<&str> = frame.iter().map(|one| one.0.as_str()).collect();
    let zeroed = ["A", "C", "E$descriptor"];
    let split = names.iter().position(|name| !zeroed.contains(name)).expect("written-first locals");
    assert!(names[..split].iter().all(|name| zeroed.contains(name)), "{frame:?}");
    assert!(names[split..].iter().all(|name| !zeroed.contains(name)), "{frame:?}");
    // Contiguous from BP down, and no two locals share bytes.
    assert_eq!(frame[0].2, 0, "{frame:?}");
    for pair in frame.windows(2) {
        assert!(pair[1].2 <= pair[0].1, "{frame:?}");
    }
    assert!(frame[..split].windows(2).all(|pair| pair[0].1 - pair[1].2 <= 1), "{frame:?}");
}

#[test]
fn a_runtime_framed_procedure_keeps_no_zero_stores() {
    // B$ENRA zero-fills this frame already: the stores were redundant.
    let source = "SUB s\nDIM t AS STRING, a AS LONG\nPRINT a; t\nEND SUB\n";
    let program = quickr_program(source);
    let laid_out = super::zero_fill::laid_out(&program, |module, function| {
        !super::compile::_inline_frame(&program, module, function)
    });
    let entry = |program: &llrm_core::hir::model::Program| {
        let function = program.modules[0].functions.iter().find(|one| one.name == "S").expect("S");
        function.blocks.iter().find(|block| block.id == function.entry).expect("entry").instructions.len()
    };
    assert!(entry(&laid_out) < entry(&program));
}

#[test]
fn a_large_zeroed_block_is_one_fill() {
    let large = "SUB s\nDIM a AS DOUBLE, b AS DOUBLE, c AS DOUBLE\nPRINT a; b; c\nEND SUB\n";
    let code = procedure_listing(large, "quickr", "S");
    assert!(code.contains("rep stosd") && code.contains("mov cx, 6"), "{code}");
    // Below FILL_BYTES the stores stay: the fill's setup would be larger.
    let small = procedure_listing("SUB s\nDIM a AS INTEGER\nPRINT a\nEND SUB\n", "quickr", "S");
    assert!(!small.contains("stos"), "{small}");
}

/// The whole listing of `source`, compiled as `dialect`.
fn module_listing(source: &str, dialect: &str) -> String {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "module.bas", source.as_bytes());
    let program = qb_driver::parsed(&path, &qb_driver::Frontend::new(dialect, "vbdos"), None)
        .unwrap_or_else(|error| panic!("{error}"));
    super::test_hir::listing(&program)
}

#[test]
fn private_procedures_are_near_and_not_public() {
    let source = "PRINT add&(40, 2); pub&(1)\n\
        PRIVATE FUNCTION add& (a AS LONG, b AS LONG)\nadd& = a + b\nEND FUNCTION\n\
        FUNCTION pub& (x AS LONG)\npub& = x + 1\nEND FUNCTION\n";
    let listing = module_listing(source, "quickr");
    assert!(listing.contains("ADD proc near") && listing.contains("PUB proc far"), "{listing}");
    assert!(!listing.contains("public ADD") && listing.contains("public PUB"), "{listing}");
    assert!(listing.contains("call ADD") && listing.contains("call far ptr PUB"), "{listing}");
    // A near return address puts the first of two parameters at bp+6, not bp+8.
    let add = super::test_hir::between(&listing, "ADD proc near", "ADD endp");
    assert!(add.contains("[bp+6]") && add.contains("[bp+4]") && add.contains("ret 4"), "{add}");
}

#[test]
fn a_private_procedure_on_the_runtime_frame_stays_far() {
    let source = "s\nPRIVATE SUB s\nDIM t AS STRING\nt = \"x\"\nEND SUB\n";
    let listing = module_listing(source, "quickr");
    assert!(listing.contains("S proc far") && !listing.contains("public S"), "{listing}");
}

#[test]
fn microsoft_profiles_reject_private() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "vbdos.bas", b"PRIVATE SUB s\nEND SUB\n");
    let error = qb_driver::parsed(&path, &qb_driver::Frontend::new("vbdos", "vbdos"), None)
        .expect_err("VBDOS has no PRIVATE");
    assert!(error.to_string().contains("quickr"), "{error}");
}

#[test]
fn the_prelude_is_private_to_each_module() {
    // Public, two modules using f-strings both defined QUICKR_REPR$ and so on.
    let listing = module_listing("DIM d AS DOUBLE\nd = 1\nPRINT f\"{d:.2f}\"\n", "quickr");
    assert!(!listing.contains("public QUICKR"), "{listing}");
}

#[test]
fn augmented_assignment_applies_each_operator() {
    let source = "DIM n AS LONG, d AS DOUBLE, f AS INTEGER\n\
        n = 7: n += 5: n -= 2: n *= 3: n \\= 4: n MOD= 5: n ^= 2: PRINT n\n\
        d = 1: d /= 4: PRINT d\n\
        f = 12: f AND= 10: f OR= 1: f XOR= 3: PRINT f\n";
    // (7+5-2)*3 = 30; 30\4 = 7; 7 MOD 5 = 2; 2^2 = 4. 12 AND 10 = 8, OR 1 = 9, XOR 3 = 10.
    assert_eq!(printed(source), " 4 \n .25 \n 10 \n");
}

#[test]
fn augmented_assignment_appends_to_strings_and_fields() {
    let source = "TYPE P\nx AS INTEGER\nEND TYPE\nDIM s AS STRING, p AS P\n\
        s = \"ab\": s += \"cd\": p.x = 1: p.x += 41\nPRINT s; p.x\n";
    assert_eq!(printed(source), "abcd 42 \n");
}

#[test]
fn augmented_assignment_evaluates_its_target_once() {
    // As `a(tick) = a(tick) + 5` the index function would run twice.
    let source = "DIM SHARED hits AS INTEGER\nDIM a(3) AS INTEGER\nhits = 0\n\
        a(tick%) += 5\nPRINT hits; a(1)\n\
        FUNCTION tick%\nhits += 1\ntick% = 1\nEND FUNCTION\n";
    assert_eq!(printed(source), " 1  5 \n");
}

#[test]
fn augmented_assignment_works_in_a_one_line_if() {
    let source = "DIM t AS INTEGER\nt = 10\nIF t > 5 THEN t += 1 ELSE t -= 1\nIF t < 5 THEN t += 100 ELSE t -= 3\nPRINT t\n";
    assert_eq!(printed(source), " 8 \n");
}

#[test]
fn augmented_assignment_takes_a_conditional_value() {
    // The rewrite ended the value at the conditional's ELSE: a syntax error.
    let source = "DIM t AS INTEGER\nt = 1\nt += 10 IF t > 0 ELSE 20\nIF t > 5 THEN t -= 1 IF t > 9 ELSE 2 ELSE t = 0\nPRINT t\n";
    assert_eq!(printed(source), " 10 \n");
}

#[test]
fn microsoft_profiles_reject_augmented_assignment() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "vbdos.bas", b"x = 1\nx += 1\n");
    let error = qb_driver::parsed(&path, &qb_driver::Frontend::new("vbdos", "vbdos"), None)
        .expect_err("VBDOS has no +=");
    assert!(error.to_string().contains("quickr"), "{error}");
}

#[test]
fn break_and_continue_act_on_the_innermost_loop() {
    // CONTINUE in a FOR must still step the counter, or the loop never ends.
    let source = "DIM i AS INTEGER, j AS INTEGER, n AS INTEGER\n\
        FOR i = 1 TO 6\nIF i MOD 2 = 0 THEN CONTINUE\nIF i = 5 THEN BREAK\n\
        FOR j = 1 TO 9\nIF j = 2 THEN BREAK\nn += 1\nNEXT\nPRINT i;\nNEXT\nPRINT n; i\n";
    assert_eq!(printed(source), " 1  3  2  5 \n");
}

#[test]
fn continue_reaches_each_kind_of_loop_test() {
    let source = "DIM i AS INTEGER, s AS INTEGER\n\
        WHILE i < 5\ni += 1\nIF i = 2 THEN CONTINUE\ns += i\nWEND\n\
        i = 0\nDO\ni += 1\nIF i = 4 THEN CONTINUE\ns += 10\nLOOP UNTIL i >= 4\n\
        i = 0\nDO WHILE i < 3\ni += 1\nIF i = 1 THEN CONTINUE\ns += 100\nLOOP\n\
        i = 0\nDO\ni += 1\nIF i < 3 THEN CONTINUE\nIF i = 4 THEN BREAK\ns += 1000\nLOOP\n\
        PRINT s\n";
    assert_eq!(printed(source), " 1243 \n");
}

#[test]
fn break_outside_a_loop_is_an_error() {
    let error = compiled("BREAK\n").expect_err("no loop");
    assert!(error.to_string().contains("outside a loop"), "{error}");
}

#[test]
fn quickr_reserves_break_and_continue() {
    for source in ["DIM break AS INTEGER\n", "SUB continue\nEND SUB\n"] {
        let error = compiled(source).expect_err(source);
        assert!(error.to_string().contains("reserved"), "{error}");
    }
}

#[test]
fn for_each_counts_through_range() {
    // Assigning the variable must not steer the loop: it is a copy.
    let source = "FOR EACH i AS INTEGER IN RANGE(3)\nPRINT i;\ni = 10\nNEXT\nPRINT\n\
        FOR EACH i AS INTEGER IN RANGE(10, 0, -3)\nPRINT i;\nNEXT\nPRINT\n\
        FOR EACH i IN RANGE(2, 4)\nPRINT i;\nNEXT\nPRINT i\n\
        FOR EACH i IN RANGE(0)\nPRINT \"never\"\nNEXT\n";
    assert_eq!(printed(source), " 0  1  2 \n 10  7  4  1 \n 2  3  3 \n");
}

#[test]
fn for_each_evaluates_range_bounds_once() {
    let source = "DIM SHARED hits AS INTEGER\n\
        FOR EACH i AS INTEGER IN RANGE(1, 6, by%)\nNEXT\nPRINT hits; i\n\
        FUNCTION by%\nhits += 1\nby% = 2\nEND FUNCTION\n";
    assert_eq!(printed(source), " 1  5 \n");
}

#[test]
fn for_each_copies_array_elements() {
    let source = "TYPE P\nx AS INTEGER\nEND TYPE\n\
        DIM a(2) AS INTEGER, ps(2) AS P\na(0) = 5: a(1) = 6: a(2) = 7\n\
        FOR EACH v AS INTEGER IN a()\nv = v * 10\nPRINT v;\nNEXT\nPRINT a(0)\n\
        ps(1).x = 4\nFOR EACH p AS P IN ps\nPRINT p.x;\nNEXT\nPRINT\n";
    assert_eq!(printed(source), " 50  60  70  5 \n 0  4  0 \n");
}

#[test]
fn for_each_walks_a_copy_of_a_string() {
    let source = "DIM s AS STRING, n AS INTEGER\ns = \"abc\"\n\
        FOR EACH c AS STRING IN s + \"d\"\nIF c = \"b\" THEN CONTINUE\nPRINT c;\nNEXT\nPRINT\n\
        FOR EACH c IN s\ns = \"\"\nn += 1\nNEXT\nPRINT n\n";
    assert_eq!(printed(source), "acd\n 3 \n");
}

#[test]
fn for_each_errors() {
    for (source, message) in [
        ("FOR EACH i AS INTEGER IN RANGE(2)\nNEXT\nFOR EACH i AS LONG IN RANGE(2)\nNEXT\n", "another type"),
        ("FOR EACH d AS DOUBLE IN RANGE(2)\nNEXT\n", "integer variable"),
        ("FOR EACH i IN RANGE(2)\nNEXT\n", "not defined"),
    ] {
        let error = compiled(source).expect_err(source);
        assert!(error.contains(message), "{source}: {error}");
    }
}

#[test]
fn microsoft_profiles_keep_each_a_name() {
    let source = "FOR each = 1 TO 2\nPRINT each;\nNEXT\n";
    assert_eq!(super::test_runtime_model::printed_on(source, "vbdos", "vbdos"), " 1  2 ");
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "vbdos.bas", b"DIM a(2)\nFOR EACH v IN a()\nNEXT\n");
    let error = qb_driver::parsed(&path, &qb_driver::Frontend::new("vbdos", "vbdos"), None)
        .expect_err("VBDOS has no FOR EACH");
    assert!(error.to_string().contains("quickr"), "{error}");
}

#[test]
fn quickr_arrays_start_at_zero() {
    let source = "DIM a(3) AS INTEGER\nPRINT LBOUND(a); UBOUND(a)\n";
    assert_eq!(printed(source), " 0  3 \n");
    for source in [
        "DIM a(1 TO 3) AS INTEGER\n",
        "OPTION BASE 1\n",
        "SUB s\nIF 1 THEN\nREDIM b(2 TO 4) AS INTEGER\nEND IF\nEND SUB\n",
    ] {
        let error = compiled(source).expect_err(source);
        assert!(error.contains("start at 0"), "{source}: {error}");
    }
}

#[test]
fn conditional_expressions_choose_one_arm() {
    let source = "DIM SHARED hits AS INTEGER\nDIM n AS INTEGER, s AS STRING, d AS DOUBLE\nn = 5\n\
        PRINT 1 IF n > 3 ELSE 2; 10 IF n > 9 ELSE 20 IF n > 4 ELSE 30\n\
        d = 1 IF n = 0 ELSE 2.5\nPRINT d\n\
        s = \"big\" IF n > 3 ELSE \"small\"\nPRINT s; LEN(\"x\" IF n < 0 ELSE \"yy\")\n\
        IF n > 3 THEN s = \"a\" IF n > 4 ELSE \"b\" ELSE s = \"c\"\n\
        n = tick% IF n > 100 ELSE 7\nPRINT s; n; hits\n\
        PRINT f\"{n IF n IN (6, 7) ELSE -n}\"\n\
        FUNCTION tick%\nhits += 1\ntick% = 1\nEND FUNCTION\n";
    // f-string fields are parsed apart from their line, and took none of these.
    assert_eq!(printed(source), " 1  20 \n 2.5 \nbig 2 \na 7  0 \n7\n");
}

#[test]
fn chained_comparisons_hold_each_operand_once() {
    // Parenthesized, a comparison is a value again, as in QB.
    let source = "DIM SHARED hits AS INTEGER\nDIM a AS INTEGER, b AS INTEGER\na = 1: b = 5\n\
        PRINT 0 < a < b; a < b < 3; 1 <= a <= 1 < b\n\
        PRINT (a < b) < 0; a < b < 0\n\
        PRINT 5 < tick% < 10; 9 < 1 < tick%; hits\n\
        PRINT a = 1 AND b = 5; \"x\" = \"x\" AND a = 1\n\
        FUNCTION tick%\nhits += 1\ntick% = 7\nEND FUNCTION\n";
    // A comparison after AND starts a new operand, not a chain.
    assert_eq!(printed(source), "-1  0 -1 \n-1  0 \n-1  0  1 \n-1 -1 \n");
}

#[test]
fn in_searches_lists_strings_and_arrays() {
    let source = "DIM SHARED hits AS INTEGER\nDIM x AS INTEGER, w AS STRING, v(2) AS INTEGER\n\
        x = 3: w = \"bc\": v(1) = 3\n\
        PRINT x IN (1, 2, 3); x NOT IN (1, 2, 3); w IN \"abcd\"; \"z\" IN \"abc\"; x IN v(); 4 IN v; x NOT IN v()\n\
        PRINT w IN (\"a\", \"bc\"); x IN (3, tick%); hits\n\
        FUNCTION tick%\nhits += 1\ntick% = 1\nEND FUNCTION\n";
    assert_eq!(printed(source), "-1  0 -1  0 -1  0  0 \n-1 -1  0 \n");
}

#[test]
fn microsoft_profiles_keep_qb_expressions() {
    // QB compares the first comparison's -1 or 0 with the third operand.
    let source = "PRINT 3 < 2 < 1\n";
    assert_eq!(super::test_runtime_model::printed_on(source, "vbdos", "vbdos"), "-1 \n");
    assert_eq!(printed(source), " 0 \n");
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "vbdos.bas", b"x = 1 IF 1 ELSE 2\n");
    qb_driver::parsed(&path, &qb_driver::Frontend::new("vbdos", "vbdos"), None)
        .expect_err("VBDOS has no conditional expression");
}

#[test]
fn return_gives_a_function_its_value() {
    let source = "PRINT fib&(10); greet$(\"ann\"); find%(49); find%(50); sign%(-4); pick%(0)\n\
        FUNCTION fib&(n AS INTEGER)\nIF n < 2 THEN RETURN n\nRETURN fib&(n - 1) + fib&(n - 2)\nEND FUNCTION\n\
        FUNCTION greet$(who AS STRING)\nRETURN \"hi \" + who\nEND FUNCTION\n\
        FUNCTION find%(x AS INTEGER)\nFOR EACH i AS INTEGER IN RANGE(10)\nIF i * i = x THEN RETURN i\nNEXT\nRETURN -1\nEND FUNCTION\n\
        FUNCTION sign%(n AS INTEGER)\nRETURN 1 IF n > 0 ELSE -1 IF n < 0 ELSE 0\nEND FUNCTION\n\
        FUNCTION pick%(n AS INTEGER)\nIF n THEN RETURN 1 ELSE RETURN 2\nEND FUNCTION\n";
    assert_eq!(printed(source), " 55 hi ann 7 -1 -1  2 \n");
}

#[test]
fn a_bare_return_still_ends_a_gosub() {
    let source = "GOSUB inner\nPRINT g%\nEND\ninner:\nPRINT \"in\";\nRETURN\n\
        FUNCTION g%\nRETURN 5\nEND FUNCTION\n";
    assert_eq!(printed(source), "in 5 \n");
}

#[test]
fn microsoft_profiles_return_only_from_gosub() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "vbdos.bas", b"FUNCTION f%\nRETURN 1 + 2\nEND FUNCTION\n");
    qb_driver::parsed(&path, &qb_driver::Frontend::new("vbdos", "vbdos"), None)
        .expect_err("VBDOS returns no value");
}

#[test]
fn tuple_assignment_evaluates_every_value_first() {
    let source = "DIM a AS INTEGER, b AS INTEGER, i AS INTEGER, s AS STRING, t AS STRING, v(2) AS INTEGER\n\
        a = 1: b = 2: s = \"x\": t = \"y\"\n\
        a, b = b, a\ns, t = t, s\nv(0), v(1) = a + 10, a\ni, v(i) = 2, 7\n\
        PRINT a; b; s; t; v(0); v(1); v(2)\n";
    assert_eq!(printed(source), " 2  1 yx 12  2  7 \n");
}

#[test]
fn functions_return_tuples() {
    let source = "DIM q AS INTEGER, r AS INTEGER, w AS STRING, n AS LONG\n\
        q, r = divmod(17, 5)\nPRINT q; r\n\
        w, n = named(3)\nPRINT w; n\nw, n = named(1)\nPRINT w; n\n\
        q, r = pair\nPRINT q; r\n\
        FUNCTION divmod (a AS INTEGER, b AS INTEGER) AS (INTEGER, INTEGER)\nRETURN a \\ b, a MOD b\nEND FUNCTION\n\
        FUNCTION named (k AS INTEGER) AS (STRING, LONG)\nIF k > 2 THEN RETURN \"big\", k * 100000\nRETURN \"small\", k\nEND FUNCTION\n\
        FUNCTION pair AS (INTEGER, INTEGER)\nRETURN divmod(9, 4)\nEND FUNCTION\n";
    assert_eq!(printed(source), " 3  2 \nbig 300000 \nsmall 1 \n 2  1 \n");
}

#[test]
fn tuple_errors() {
    for (source, message) in [
        ("DIM a AS INTEGER, b AS INTEGER\na, b = 1, 2, 3\n", "2 targets for 3 values"),
        ("DIM t AS (INTEGER, INTEGER)\n", "only a FUNCTION's result"),
        ("DIM a AS INTEGER, b AS INTEGER\na, b = LEN(\"x\")\n", "several values"),
    ] {
        let error = compiled(source).expect_err(source);
        assert!(error.contains(message), "{source}: {error}");
    }
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "vbdos.bas", b"a = 1: b = 2\na, b = b, a\n");
    qb_driver::parsed(&path, &qb_driver::Frontend::new("vbdos", "vbdos"), None)
        .expect_err("VBDOS has no tuples");
}
