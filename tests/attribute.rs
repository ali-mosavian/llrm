//! gcc's `__attribute__((...))`: Open Watcom's front end stopped at the `(` of
//! the first one (#935). The attributes that state a fact (`noinline`,
//! `always_inline`) reach the optimizer; those that change no meaning are read
//! and dropped; the rest are an error, not ignored.

mod common;

use std::process::Output;

fn compile(source: &str) -> (Output, String) {
    compiled("-m16", source)
}

fn compiled(
    target: &str,
    source: &str,
) -> (Output, String) {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("t.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args([target, "-O2", "-S", "-o", "t.s", "t.c"])
        .output()
        .unwrap();
    let listing = std::fs::read_to_string(scratch.path().join("t.s")).unwrap_or_default();
    (output, listing)
}

/// The text of `main_` (`_main` at m16, whose default is C's for `main`), the
/// one routine that calls.
fn main_of(listing: &str) -> &str {
    let from = listing.find("main_ proc").or_else(|| listing.find("_main proc")).expect("main");
    &listing[from..listing[from..].find("endp").map_or(listing.len(), |at| from + at)]
}

/// Calls in `main_`, a tail call (`jmp k_`, not a jump to a local `L1_1`) among
/// them.
fn calls(listing: &str) -> usize {
    let is_call = |line: &str| {
        let line = line.trim_start();
        line.starts_with("call")
            || line.strip_prefix("jmp ").is_some_and(|target| !target.trim_start().starts_with('L'))
    };
    main_of(listing).lines().filter(|line| is_call(line)).count()
}

const PLAIN: &str = "int k(int x) { return x + 1; }\nint main(void) { return k(2); }\n";

/// `k` is one add: at -O2 it is inlined into `main`, which then makes no call.
#[test]
fn test_without_an_attribute_a_one_add_function_is_inlined() {
    let (output, listing) = compile(PLAIN);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(calls(&listing), 0, "{listing}");
}

/// `__attribute__((noinline))` was `E1009: Expecting ')' but found '('`; once
/// read, it was dropped and `k` was inlined.
#[test]
fn test_noinline_before_the_type_keeps_the_call() {
    let (output, listing) =
        compile("__attribute__((noinline)) int k(int x) { return x + 1; }\nint main(void) { return k(2); }\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(calls(&listing), 1, "{listing}");
}

/// The flat target reads the same header.
#[test]
fn test_noinline_keeps_the_call_at_m32_too() {
    let (output, listing) =
        compiled("-m32", "__attribute__((noinline)) int k(int x) { return x + 1; }\nint main(void) { return k(2); }\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(calls(&listing), 1, "{listing}");
}

/// After a prototype's declarator, and the definition that follows it says
/// nothing: gcc merges the two.
#[test]
fn test_noinline_after_a_prototype_holds_for_the_definition() {
    let (output, listing) = compile(
        "int k(int x) __attribute__((noinline));\nint k(int x) { return x + 1; }\nint main(void) { return k(2); }\n",
    );
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(calls(&listing), 1, "{listing}");
}

/// A list, with commas and attributes that take arguments, and the
/// double-underscore spellings gcc allows.
/// `always_inline` was read and dropped: eight calls of a routine too big for
/// the budget stayed eight calls, where gcc and clang inline them all.
#[test]
fn test_always_inline_replaces_every_call_of_a_big_routine() {
    // Big enough that the inliner's threshold refuses it at eight calls, as the
    // pin's does (the smaller routine of two loops is inlined without the
    // attribute by both).
    let body = format!(
        "{{ long s = 0; int i; for (i = 0; i < x; i++) s += i * x + (s >> 3); if (s > 100) s ^= x; {}return s * 3 + x; }}\n",
        "for (i = 0; i < x; i++) s += i * s - (s >> 2); if (s > 100) s ^= x; ".repeat(4)
    );
    let main = "volatile long a = 3;\nint main(void) { return (int)(f(a) + f(a + 1) + f(a + 2) + f(a + 3) + f(a + 4) + f(a + 5) + f(a + 6) + f(a + 7)); }\n";
    let (_, plain) = compiled("-m32", &format!("static long f(long x) {body}{main}"));
    let (_, forced) = compiled("-m32", &format!("static __attribute__((always_inline)) long f(long x) {body}{main}"));
    assert!(calls(&plain) > 0, "{}", main_of(&plain));
    assert_eq!(calls(&forced), 0, "{}", main_of(&forced));
}

#[test]
fn test_a_list_of_attributes_with_arguments_is_read() {
    let (output, listing) = compile(
        "static __attribute__((__unused__, always_inline)) int u(int x) { return x; }\n\
         int p(const char *f, ...) __attribute__((format(printf, 1, 2), __noinline__));\n\
         int p(const char *f, ...) { return f[0]; }\n\
         int main(void) { return p(\"x\"); }\n",
    );
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(calls(&listing), 1, "{listing}");
}

/// An attribute that changes the program is an error: `packed` ignored would
/// lay the struct out as another one.
/// llrm-cc: clang honours these instead of refusing them, so the test asserts
/// the program changed: the layout of `packed` and `aligned(8)` (a size check
/// the compiler evaluates) and the segment `section` names. `weak` is not
/// asserted: the assembler has no weak symbols and the listing drops it.
#[test]
fn test_an_attribute_that_changes_the_program_is_honoured() {
    // m16 `int` is two bytes: the plain struct is 4, packed is 3, aligned(8) is
    // 8.
    for (attribute, size) in [("packed", 3), ("aligned(8)", 8)] {
        let (output, _) = compile(&format!(
            "struct S {{ char c; int i; }} __attribute__(({attribute})) s;\n\
             int check[sizeof(struct S) == {size} ? 1 : -1];\nint main(void) {{ return s.i; }}\n"
        ));
        assert!(output.status.success(), "{attribute} was ignored: {}", String::from_utf8_lossy(&output.stderr));
    }
    let (output, listing) = compile("int v __attribute__((section(\"xx\")));\nint main(void) { return v; }\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(listing.contains("xx segment"), "{listing}");
}
