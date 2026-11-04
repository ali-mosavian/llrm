//! `strcmp`, `strncmp`, `memcmp` and `bcmp` of constant strings stayed calls:
//! `strcmp("abc", "abd")` was not -1, `memcmp(p, q, 1)` not a byte difference.

use llrm_mir::libfunc::Routine;
use llrm_mir::passes::PassManager;
use llrm_mir::target::Neutral;
use llrm_transforms::simplifylibcalls::SimplifyLibCalls;

/// The library the compile states: the calls the rewrites form.
fn routines() -> Vec<Routine> {
    ["memcmp", "bcmp"]
        .map(|standard| Routine { standard, symbol: standard.to_string(), calling_convention: 0 })
        .to_vec()
}

fn run(body: &str) -> String {
    run_with("", body)
}

fn run_with(
    header: &str,
    body: &str,
) -> String {
    let text = format!(
        "{header}@a = private constant [4 x i8] c\"abc\\00\"
@b = private constant [4 x i8] c\"abd\\00\"
@e = private constant [1 x i8] c\"\\00\"
declare i32 @strcmp(ptr, ptr)
declare i32 @strncmp(ptr, ptr, i64)
declare i32 @memcmp(ptr, ptr, i64)
declare i32 @bcmp(ptr, ptr, i64)
define i32 @f(ptr %p, ptr %q, i64 %n, ptr dereferenceable(8) %d) {{
b0:
{body}
}}
"
    );
    let mut module = llrm_mir::parse::module(&text).expect("parses");
    let mut manager = PassManager::default();
    manager.add(SimplifyLibCalls::with_routines(routines()));
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).expect("runs");
    llrm_mir::print::module(&module)
}

fn after(body: &str) -> String {
    let text = run(body);
    text.split("define i32 @f").nth(1).expect("f is printed").to_string()
}

#[test]
fn test_strcmp_of_two_constants_is_their_order() {
    let text = after("  %r = call i32 @strcmp(ptr @a, ptr @b)\n  ret i32 %r");
    assert!(text.contains("ret i32 -1") && !text.contains("call"), "{text}");
}

#[test]
fn test_strcmp_with_an_empty_string_reads_one_byte() {
    let text = after("  %r = call i32 @strcmp(ptr %p, ptr @e)\n  ret i32 %r");
    assert!(text.contains("load i8, ptr %p") && text.contains("zext") && !text.contains("call"), "{text}");
    let text = after("  %r = call i32 @strcmp(ptr @e, ptr %p)\n  ret i32 %r");
    assert!(text.contains("sub i32 0,") && !text.contains("call"), "{text}");
}

#[test]
fn test_strncmp_of_length_zero_and_of_the_same_pointer_is_zero() {
    let text = after("  %r = call i32 @strncmp(ptr %p, ptr %q, i64 0)\n  ret i32 %r");
    assert!(text.contains("ret i32 0") && !text.contains("call"), "{text}");
    let text = after("  %r = call i32 @strncmp(ptr %p, ptr %p, i64 %n)\n  ret i32 %r");
    assert!(text.contains("ret i32 0") && !text.contains("call"), "{text}");
}

#[test]
fn test_strncmp_cuts_both_strings_at_the_length() {
    let text = after("  %r = call i32 @strncmp(ptr @a, ptr @b, i64 2)\n  ret i32 %r");
    assert!(text.contains("ret i32 0") && !text.contains("call"), "{text}");
    let text = after("  %r = call i32 @strncmp(ptr @a, ptr @b, i64 3)\n  ret i32 %r");
    assert!(text.contains("ret i32 -1") && !text.contains("call"), "{text}");
}

#[test]
fn test_memcmp_of_constants_with_a_size_not_constant_selects_on_the_size() {
    let text = after("  %r = call i32 @memcmp(ptr @a, ptr @b, i64 %n)\n  ret i32 %r");
    assert!(text.contains("icmp ule i64 %n, 2") && text.contains("select") && !text.contains("call"), "{text}");
}

#[test]
fn test_memcmp_and_bcmp_of_one_byte_is_the_byte_difference() {
    for name in ["memcmp", "bcmp"] {
        let text = after(&format!("  %r = call i32 @{name}(ptr %p, ptr %q, i64 1)\n  ret i32 %r"));
        assert!(text.contains("sub i32") && !text.contains("call"), "{name}: {text}");
    }
}

#[test]
fn test_memcmp_of_size_zero_is_zero() {
    let text = after("  %r = call i32 @memcmp(ptr %p, ptr %q, i64 0)\n  ret i32 %r");
    assert!(text.contains("ret i32 0") && !text.contains("call"), "{text}");
}

/// `strcmp(P, "x") == 0` is `memcmp(P, "x", 2) == 0` where two bytes are
/// readable at `P`; otherwise the call stays, as the read could fault.
#[test]
fn test_strcmp_with_a_literal_becomes_memcmp_where_the_bytes_are_readable() {
    let equal = |pointer: &str| {
        after(&format!(
            "  %r = call i32 @strcmp(ptr {pointer}, ptr @a)\n  %c = icmp eq i32 %r, 0\n  %z = zext i1 %c to i32\n  ret i32 %z"
        ))
    };
    let text = equal("%d");
    assert!(text.contains("@memcmp(ptr %d, ptr @a, i64 4)") && !text.contains("@strcmp"), "{text}");
    let text = equal("%p");
    assert!(text.contains("@strcmp") && !text.contains("@memcmp"), "{text}");
}

#[test]
fn test_strcmp_whose_result_is_read_beyond_zero_stays() {
    let text = after("  %r = call i32 @strcmp(ptr %d, ptr @a)\n  ret i32 %r");
    assert!(text.contains("@strcmp") && !text.contains("@memcmp"), "{text}");
}

#[test]
fn test_strncmp_of_one_byte_is_memcmp_and_a_longer_one_is_cut_by_the_length() {
    let text = after("  %r = call i32 @strncmp(ptr %p, ptr %q, i64 1)\n  ret i32 %r");
    assert!(text.contains("@memcmp(ptr %p, ptr %q, i64 1)") && !text.contains("@strncmp"), "{text}");
    let text = after(
        "  %r = call i32 @strncmp(ptr %d, ptr @a, i64 2)\n  %c = icmp ne i32 %r, 0\n  %z = zext i1 %c to i32\n  ret i32 %z",
    );
    assert!(text.contains("@memcmp(ptr %d, ptr @a, i64 2)"), "{text}");
}

#[test]
fn test_memcmp_tested_for_equality_becomes_bcmp() {
    let text = after(
        "  %r = call i32 @memcmp(ptr %p, ptr %q, i64 %n)\n  %c = icmp eq i32 %r, 0\n  %z = zext i1 %c to i32\n  ret i32 %z",
    );
    assert!(text.contains("@bcmp(ptr %p, ptr %q, i64 %n)") && !text.contains("@memcmp"), "{text}");
    let text = after("  %r = call i32 @memcmp(ptr %p, ptr %q, i64 %n)\n  ret i32 %r");
    assert!(text.contains("@memcmp") && !text.contains("@bcmp"), "{text}");
}

#[test]
fn test_memcmp_of_constants_with_a_constant_size_is_a_constant() {
    let text = after("  %r = call i32 @memcmp(ptr @a, ptr @b, i64 3)\n  ret i32 %r");
    assert!(text.contains("ret i32 -1") && !text.contains("select"), "{text}");
    let text = after("  %r = call i32 @memcmp(ptr @a, ptr @b, i64 2)\n  ret i32 %r");
    assert!(text.contains("ret i32 0"), "{text}");
}

/// `memcmp(X, Y, 4) == 0` of two aligned variables is one `i32` compare where
/// the target has `i32`; with `%p` unaligned it stays a call.
#[test]
fn test_memcmp_equality_of_aligned_words_is_one_integer_compare() {
    let header = "target datalayout = \"e-p:64:64-i8:8-i16:16-i32:32-i64:64-n8:16:32:64\"
@x = global [4 x i8] zeroinitializer, align 4
@y = global [4 x i8] zeroinitializer, align 4
";
    let tail = "  %c = icmp eq i32 %r, 0\n  %z = zext i1 %c to i32\n  ret i32 %z";
    let text = run_with(header, &format!("  %r = call i32 @memcmp(ptr @x, ptr @y, i64 4)\n{tail}"));
    let text = text.split("define i32 @f").nth(1).expect("f is printed");
    assert!(text.contains("load i32, ptr @x") && text.contains("icmp ne i32") && !text.contains("call"), "{text}");
    let text = run_with(header, &format!("  %r = call i32 @memcmp(ptr %p, ptr @y, i64 4)\n{tail}"));
    let text = text.split("define i32 @f").nth(1).expect("f is printed");
    assert!(!text.contains("load i32") && text.contains("@bcmp"), "{text}");
}
