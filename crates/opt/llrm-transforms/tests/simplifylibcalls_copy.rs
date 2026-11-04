//! `strcpy` and `stpcpy` of a string of known length stayed calls:
//! `strcpy(p, "abc")` was not a 4-byte `memcpy`.

use llrm_mir::passes::PassManager;
use llrm_mir::target::Neutral;
use llrm_transforms::simplifylibcalls::SimplifyLibCalls;

fn after(body: &str) -> String {
    let text = format!(
        "@a = private constant [4 x i8] c\"abc\\00\"
declare ptr @strcpy(ptr, ptr)
declare ptr @stpcpy(ptr, ptr)
define ptr @f(ptr %p, ptr %q) {{
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
