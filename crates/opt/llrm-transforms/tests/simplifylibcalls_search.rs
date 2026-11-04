//! `strchr`, `strpbrk`, `strspn`, `strcspn` and `strstr` of constant strings
//! stayed calls: `strchr("abc", 'b')` was not `"abc" + 1`, `strspn("abc",
//! "ab")` not 2.

use llrm_mir::passes::PassManager;
use llrm_mir::target::Neutral;
use llrm_transforms::simplifylibcalls::SimplifyLibCalls;

fn after(body: &str) -> String {
    let text = format!(
        "@a = private constant [4 x i8] c\"abc\\00\"
@e = private constant [1 x i8] c\"\\00\"
@h = private constant [8 x i8] c\"xxabcyy\\00\"
@s = private constant [3 x i8] c\"ab\\00\"
@z = private constant [3 x i8] c\"xy\\00\"
@c = private constant [3 x i8] c\"xc\\00\"
declare ptr @strchr(ptr, i32)
declare ptr @strpbrk(ptr, ptr)
declare i64 @strspn(ptr, ptr)
declare i64 @strcspn(ptr, ptr)
declare ptr @strstr(ptr, ptr)
define ptr @f(ptr %p) {{
b0:
{body}
}}
"
    );
    run(&text)
}

fn run(text: &str) -> String {
    let mut module = llrm_mir::parse::module(text).expect("parses");
    let mut manager = PassManager::default();
    manager.add(SimplifyLibCalls::with_routines(Vec::new()));
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).expect("runs");
    llrm_mir::print::module(&module)
}

/// The pointer function `f` with `call` returning `%r`.
fn pointer(call: &str) -> String {
    after(&format!("  %r = {call}\n  ret ptr %r"))
}

/// The count function `g` with `call` returning `%r`.
fn count(call: &str) -> String {
    run(&format!(
        "@a = private constant [4 x i8] c\"abc\\00\"
@e = private constant [1 x i8] c\"\\00\"
@s = private constant [3 x i8] c\"ab\\00\"
@c = private constant [3 x i8] c\"xc\\00\"
declare i64 @strspn(ptr, ptr)
declare i64 @strcspn(ptr, ptr)
define i64 @g(ptr %p) {{
b0:
  %r = {call}
  ret i64 %r
}}
"
    ))
}

#[test]
fn test_strchr_of_a_constant_string_and_character_is_the_address_or_null() {
    let text = pointer("call ptr @strchr(ptr @a, i32 98)");
    assert!(text.contains("getelementptr inbounds i8, ptr @a, i64 1") && !text.contains("call"), "{text}");
    let text = pointer("call ptr @strchr(ptr @a, i32 122)");
    assert!(text.contains("ret ptr null") && !text.contains("call"), "{text}");
    let text = pointer("call ptr @strchr(ptr @a, i32 0)");
    assert!(text.contains("i64 3") && !text.contains("call"), "{text}");
}

/// The pin turns a `strchr` tested against its own string into a character
/// compare first; that rewrite is not translated, so the call stays.
#[test]
fn test_strchr_compared_with_its_string_is_left_alone() {
    let text = after(
        "  %r = call ptr @strchr(ptr @a, i32 98)\n  %c = icmp eq ptr %r, @a\n  %z = select i1 %c, ptr %p, ptr null\n  ret ptr %z",
    );
    assert!(text.contains("@strchr"), "{text}");
}

#[test]
fn test_strpbrk_folds_over_constant_sets() {
    let text = pointer("call ptr @strpbrk(ptr @a, ptr @c)");
    assert!(text.contains("getelementptr inbounds i8, ptr @a, i64 2") && !text.contains("call"), "{text}");
    let text = pointer("call ptr @strpbrk(ptr @a, ptr @z)");
    assert!(text.contains("ret ptr null") && !text.contains("call"), "{text}");
    let text = pointer("call ptr @strpbrk(ptr %p, ptr @e)");
    assert!(text.contains("ret ptr null") && !text.contains("call"), "{text}");
}

#[test]
fn test_strstr_of_equal_or_empty_or_constant_strings() {
    let text = pointer("call ptr @strstr(ptr %p, ptr %p)");
    assert!(text.contains("ret ptr %p") && !text.contains("call"), "{text}");
    let text = pointer("call ptr @strstr(ptr %p, ptr @e)");
    assert!(text.contains("ret ptr %p") && !text.contains("call"), "{text}");
    let text = pointer("call ptr @strstr(ptr @h, ptr @a)");
    assert!(text.contains("getelementptr inbounds i8, ptr @h, i64 2") && !text.contains("call"), "{text}");
    let text = pointer("call ptr @strstr(ptr @h, ptr @z)");
    assert!(text.contains("ret ptr null") && !text.contains("call"), "{text}");
}

#[test]
fn test_strspn_and_strcspn_fold_over_constant_strings() {
    let text = count("call i64 @strspn(ptr @a, ptr @s)");
    assert!(text.contains("ret i64 2") && !text.contains("call"), "{text}");
    let text = count("call i64 @strcspn(ptr @a, ptr @c)");
    assert!(text.contains("ret i64 2") && !text.contains("call"), "{text}");
    let text = count("call i64 @strspn(ptr %p, ptr @e)");
    assert!(text.contains("ret i64 0") && !text.contains("call"), "{text}");
    let text = count("call i64 @strcspn(ptr @e, ptr %p)");
    assert!(text.contains("ret i64 0") && !text.contains("call"), "{text}");
}
