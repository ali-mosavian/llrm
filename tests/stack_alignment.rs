//! The stack is a multiple of 8 at a call to a function that holds a double:
//! its quad spills are then 8-aligned, and a 486 takes 3 clocks more for each
//! access off it (Intel486 data sheet 240440-002, section 10.1 and Table 2.1).

mod common;

fn listing(source: &str) -> String {
    listing_with(source, None)
}

fn listing_with(
    source: &str,
    selector: Option<&str>,
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(scratch.path())
        // The call to `h` is the premise: inlined (the pin inlines it), `f`
        // calls nothing that holds a double and needs no pad.
        .args(selector)
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O2", "-fno-inline-functions", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// The bytes a function's entry takes the stack down: the return address, its
/// pushes and `sub esp`.
fn entry_bytes(
    text: &str,
    name: &str,
) -> i64 {
    let start = text.find(&format!("{name} proc")).unwrap();
    let body = &text[start..text[start..].find("endp").map_or(text.len(), |at| start + at)];
    let mut bytes = 4;
    for line in body.lines().skip(1).map(str::trim) {
        if line.starts_with("push ") {
            bytes += 4;
        } else if let Some(count) = line.strip_prefix("sub esp, ") {
            bytes += count.parse::<i64>().unwrap();
        } else if line.starts_with('L') {
            // the first label ends the entry; pushes of the saved registers
            // follow it only in wrapped frames
            if !line.starts_with("L0_0") {
                break;
            }
        } else if line.starts_with("call") || line.starts_with("mov") || line.starts_with("fld") {
            break;
        }
    }
    bytes
}

const MODULE: &str = "extern int g(int);
double h(int x) { return x * 2.5 + 1.0; }
int f(int a) { return g(a) + g(a + 1) + (int)h(a); }
";

/// Where the module holds a double, a function that calls it is entered with,
/// and keeps, the stack a multiple of 8: the entry takes it down by a multiple
/// of 8 (the return address included). llrm-cc: the DAG selector stores the
/// arguments in the area it reserves, as the pin does, so no call pushes a pad.
#[test]
fn a_function_that_reaches_a_double_keeps_the_stack_a_multiple_of_8() {
    let text = listing(MODULE);
    assert_eq!(entry_bytes(&text, "_f") % 8, 0, "{text}");
    assert!(!text.contains("pushd"), "{text}");
}

/// A module that holds no double pays nothing for it: no pad, no rounding.
#[test]
fn a_module_of_integers_pays_nothing_for_it() {
    // llrm-cc: the old selector allocates no callee-saved register across the
    // two calls and reserves a frame for the result; the pin and the DAG
    // selector do not. Gated on the DAG default (#4512), so it runs
    // `--isel=dag`.
    let text = listing_with("extern int g(int);\nint f(int a) { return g(a) + g(a + 1); }\n", Some("--isel=dag"));
    assert!(!text.contains("pushd 0"), "{text}");
    assert!(!text.contains("sub esp"), "{text}");
}

/// The constant pool put an 8-byte constant after a 4-byte one at 4 mod 8
/// (nbody: 1200 operands at one address, 3 clocks each on a 486): a double
/// constant is on an 8-byte boundary.
#[test]
fn a_double_constant_is_on_an_8_byte_boundary() {
    let text = listing("float f(float x) { return x + 0.125f; }\ndouble g(double x) { return x * 0.1; }\n");
    let at = text.find("db 09ah,099h,099h,099h,099h,099h,0b9h,03fh").expect("the double 0.1");
    let before = &text[..at];
    assert!(
        before.trim_end().ends_with("label byte")
            && before.trim_end().lines().rev().nth(1).is_some_and(|line| line.trim() == "align 8"),
        "{text}"
    );
}

/// A 4-byte constant before an 8-byte one cost 4 bytes of padding in every
/// language of nbody (data 28 -> 36, 31 -> 35, 16 -> 20): the 8-byte constants
/// come first.
#[test]
fn the_wide_constants_are_laid_out_before_the_narrow_ones() {
    let text = listing("float f(float x) { return x + 0.125f; }\ndouble g(double x) { return x * 0.1; }\n");
    let wide = text.find("db 09ah,099h,099h,099h,099h,099h,0b9h,03fh").expect("the double 0.1");
    let narrow = text.find("db 000h,000h,000h,03eh").expect("the float 0.125");
    assert!(wide < narrow, "{text}");
}
