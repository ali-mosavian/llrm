//! `strcpy` and `stpcpy` of a string of known length stayed calls:
//! `strcpy(p, "abc")` was not a 4-byte `memcpy`.

use llrm_mir::libfunc::Routine;
use llrm_mir::passes::PassManager;
use llrm_mir::target::Neutral;
use llrm_transforms::simplifylibcalls::SimplifyLibCalls;

fn after(body: &str) -> String {
    run(body, Vec::new())
}

/// With `strlen` stated, as on an x86 System V target.
fn after_with_strlen(body: &str) -> String {
    run(body, vec![Routine { standard: "strlen", symbol: "strlen".to_owned(), calling_convention: 0 }])
}

fn run(
    body: &str,
    routines: Vec<Routine>,
) -> String {
    let text = format!(
        "@a = private constant [4 x i8] c\"abc\\00\"
@e = private constant [1 x i8] c\"\\00\"
declare ptr @strcpy(ptr, ptr)
declare ptr @stpcpy(ptr, ptr)
declare ptr @strcat(ptr, ptr)
declare ptr @strncat(ptr, ptr, i64)
declare ptr @strncpy(ptr, ptr, i64)
define ptr @f(ptr %p, ptr %q) {{
b0:
{body}
}}
"
    );
    let mut module = llrm_mir::parse::module(&text).expect("parses");
    let mut manager = PassManager::default();
    manager.add(SimplifyLibCalls::with_routines(routines));
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).expect("runs");
    llrm_mir::print::module(&module)
}

#[test]
fn test_strcpy_of_a_known_string_is_a_memcpy_with_its_nul() {
    let text = after("  %r = call ptr @strcpy(ptr %p, ptr @a)\n  ret ptr %r");
    assert!(
        text.contains("@llvm.memcpy.p0.p0.i64(ptr") && text.contains("i64 4") && text.contains("ret ptr %p"),
        "{text}"
    );
    assert!(!text.contains("call ptr @strcpy"), "{text}");
}

#[test]
fn test_strcpy_of_one_string_to_itself_is_that_string() {
    let text = after("  %r = call ptr @strcpy(ptr %p, ptr %p)\n  ret ptr %r");
    assert!(text.contains("ret ptr %p") && !text.contains("call"), "{text}");
}

#[test]
fn test_strcpy_of_an_unknown_string_stays() {
    let text = after("  %r = call ptr @strcpy(ptr %p, ptr %q)\n  ret ptr %r");
    assert!(text.contains("call ptr @strcpy") && !text.contains("memcpy"), "{text}");
}

#[test]
fn test_stpcpy_of_a_known_string_is_a_memcpy_and_the_address_of_its_nul() {
    let text = after("  %r = call ptr @stpcpy(ptr %p, ptr @a)\n  ret ptr %r");
    assert!(text.contains("getelementptr inbounds i8, ptr %p, i64 3") && text.contains("i64 4"), "{text}");
    assert!(!text.contains("call ptr @stpcpy"), "{text}");
    // An unused result is the pin's strcpy call, which is not translated.
    let text = after("  %r = call ptr @stpcpy(ptr %p, ptr @a)\n  ret ptr %p");
    assert!(text.contains("call ptr @stpcpy"), "{text}");
}

#[test]
fn test_strcat_of_a_known_string_finds_the_end_with_strlen_and_copies() {
    let text = after_with_strlen("  %r = call ptr @strcat(ptr %p, ptr @a)\n  ret ptr %r");
    assert!(text.contains("call i64 @strlen(ptr %p)") && text.contains("getelementptr inbounds i8, ptr %p"), "{text}");
    assert!(text.contains("@llvm.memcpy.p0.p0.i64") && text.contains("i64 4") && text.contains("ret ptr %p"), "{text}");
    assert!(!text.contains("call ptr @strcat"), "{text}");
    // Where the compile does not state strlen the call stays.
    let text = after("  %r = call ptr @strcat(ptr %p, ptr @a)\n  ret ptr %r");
    assert!(text.contains("call ptr @strcat") && !text.contains("memcpy"), "{text}");
}

#[test]
fn test_strcat_and_strncat_of_nothing_are_the_destination() {
    let text = after("  %r = call ptr @strcat(ptr %p, ptr @e)\n  ret ptr %r");
    assert!(text.contains("ret ptr %p") && !text.contains("call"), "{text}");
    let text = after("  %r = call ptr @strncat(ptr %p, ptr %q, i64 0)\n  ret ptr %r");
    assert!(text.contains("ret ptr %p") && !text.contains("call"), "{text}");
}

#[test]
fn test_strncat_with_a_bound_short_of_the_source_stays() {
    let text = after_with_strlen("  %r = call ptr @strncat(ptr %p, ptr @a, i64 2)\n  ret ptr %r");
    assert!(text.contains("call ptr @strncat") && !text.contains("memcpy"), "{text}");
    let text = after_with_strlen("  %r = call ptr @strncat(ptr %p, ptr @a, i64 3)\n  ret ptr %r");
    assert!(text.contains("i64 4") && text.contains("call i64 @strlen"), "{text}");
}

#[test]
fn test_strncpy_of_bound_zero_one_or_within_the_source_is_a_copy() {
    let text = after("  %r = call ptr @strncpy(ptr %p, ptr %q, i64 0)\n  ret ptr %r");
    assert!(text.contains("ret ptr %p") && !text.contains("call"), "{text}");
    let text = after("  %r = call ptr @strncpy(ptr %p, ptr %q, i64 1)\n  ret ptr %r");
    assert!(text.contains("load i8, ptr %q") && text.contains("store i8") && !text.contains("call"), "{text}");
    let text = after("  %r = call ptr @strncpy(ptr %p, ptr @a, i64 3)\n  ret ptr %r");
    assert!(text.contains("i64 3") && text.contains("@llvm.memcpy") && !text.contains("call ptr @strncpy"), "{text}");
    // Past the source and its NUL the pin pads the source; not translated.
    let text = after("  %r = call ptr @strncpy(ptr %p, ptr @a, i64 9)\n  ret ptr %r");
    assert!(text.contains("call ptr @strncpy"), "{text}");
}
