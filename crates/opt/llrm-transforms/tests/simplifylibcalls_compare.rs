//! `strcmp`, `strncmp`, `memcmp` and `bcmp` of constant strings stayed calls:
//! `strcmp("abc", "abd")` was not -1, `memcmp(p, q, 1)` not a byte difference.

use llrm_mir::passes::PassManager;
use llrm_mir::target::Neutral;
use llrm_transforms::simplifylibcalls::SimplifyLibCalls;

fn run(body: &str) -> String {
    let text = format!(
        "@a = private constant [4 x i8] c\"abc\\00\"
@b = private constant [4 x i8] c\"abd\\00\"
@e = private constant [1 x i8] c\"\\00\"
declare i32 @strcmp(ptr, ptr)
declare i32 @strncmp(ptr, ptr, i64)
declare i32 @memcmp(ptr, ptr, i64)
declare i32 @bcmp(ptr, ptr, i64)
define i32 @f(ptr %p, ptr %q, i64 %n) {{
b0:
{body}
}}
"
    );
    let mut module = llrm_mir::parse::module(&text).expect("parses");
    let mut manager = PassManager::default();
    manager.add(SimplifyLibCalls::with_routines(Vec::new()));
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
