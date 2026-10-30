//! A call of `sin` twice stayed two calls: the declaration `clang` writes of
//! `sin` says `nounwind` and no more, so nothing knew a call sets `errno` and
//! reads nothing, and the pin's CSE merges the second.

use super::InferFunctionAttrs;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"\n\n";

fn run(text: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add_module(InferFunctionAttrs);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    printed(&module)
}

#[test]
fn test_sin_declared_is_nounwind_willreturn_and_writes_only_errno() {
    let after = run("declare double @sin(double)\n");
    for want in ["nounwind", "willreturn", "nofree", "nosync", "nocallback", "memory(errnomem: write)"] {
        assert!(after.contains(want), "`{want}` missing:\n{after}");
    }
}

#[test]
fn test_strlen_reads_only_its_argument() {
    let after = run("declare i16 @strlen(ptr)\n");
    assert!(after.contains("memory(argmem: read)"), "{after}");
    assert!(after.contains("nocapture"), "{after}");
}

#[test]
fn test_a_function_that_is_not_a_library_function_is_left() {
    let text = "declare double @sin(double, double)\ndeclare double @mine(double)\n";
    let after = run(text);
    assert_eq!(after, printed(&parsed(&format!("{HEAD}{text}"))));
}

#[test]
fn test_a_stated_attribute_is_kept_and_not_repeated() {
    let after = run("declare double @sqrt(double) nounwind\n");
    assert_eq!(after.matches("nounwind").count(), 1, "{after}");
}

/// viewfx's `_v_render_effects` called `sin`, `cos`, `atan2` and `sqrt` 13
/// times where the pin calls 4: the declarations said nothing of memory, so
/// two calls with the same argument were two. The symbols are the front end's
/// (`_sin@3`).
#[test]
fn test_two_calls_of_sin_with_the_same_argument_are_one_after_the_pipeline() {
    use llrm_mir::program::Program;

    use crate::pipeline::{self, Applied};

    let text = format!(
        "{HEAD}declare cc 1005 double @\"_sin@3\"(double)\n\n\
define double @f(double %x) {{\nb0:\n  %a = call cc 1005 double @\"_sin@3\"(double %x)\n  \
%b = call cc 1005 double @\"_sin@3\"(double %x)\n  %r = fmul double %a, %b\n  ret double %r\n}}\n"
    );
    let mut module = parsed(&text);
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    let after = printed(&module);
    assert_eq!(after.matches("call cc 1005 double @\"_sin@3\"").count(), 1, "{after}");
}
