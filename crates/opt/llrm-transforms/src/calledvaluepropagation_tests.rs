use crate::calledvaluepropagation::propagated;
use crate::testing::{parsed, printed};

fn run(text: &str) -> String {
    let mut module = parsed(text);
    propagated(&mut module);
    printed(&module)
}

const FUNCTIONS: &str = "define void @a() {\nb0:\n  ret void\n}\n\ndefine void @b() {\nb0:\n  ret void\n}\n\n";

/// An indirect call through a global that only ever holds `@a` and `@b` may
/// reach those two: `!callees` lists them by name. Without it the call graph
/// takes the call to reach any function, and every caller's attributes and
/// bottom-up order lose what they could know.
#[test]
fn a_call_through_a_global_holding_two_functions_lists_them() {
    let text = format!(
        "{FUNCTIONS}@slot = internal global ptr @a\n\ndefine void @set() {{\nb0:\n  store ptr @b, ptr @slot\n  ret void\n}}\n\n\
         define void @f() {{\nb0:\n  %p = load ptr, ptr @slot\n  call void %p()\n  ret void\n}}\n"
    );
    let out = run(&text);
    assert!(out.contains("call void %p(), !callees !0"), "{out}");
    assert!(out.contains("!0 = !{ptr @a, ptr @b}"), "{out}");
}

/// A global whose address escapes can hold anything: no `!callees`.
#[test]
fn a_global_whose_address_escapes_adds_nothing() {
    let text = format!(
        "{FUNCTIONS}declare void @take(ptr)\n\n@slot = internal global ptr @a\n\n\
         define void @f() {{\nb0:\n  call void @take(ptr @slot)\n  %p = load ptr, ptr @slot\n  call void %p()\n  ret void\n}}\n"
    );
    assert!(!run(&text).contains("!callees"));
}

/// More than four possible targets is overdefined.
#[test]
fn five_targets_are_too_many() {
    let mut text = String::new();
    for name in ["a", "b", "c", "d", "e"] {
        text += &format!("define void @{name}() {{\nb0:\n  ret void\n}}\n\n");
    }
    text += "@slot = internal global ptr @a\n\ndefine void @set() {\nb0:\n";
    for name in ["b", "c", "d", "e"] {
        text += &format!("  store ptr @{name}, ptr @slot\n");
    }
    text += "  ret void\n}\n\ndefine void @f() {\nb0:\n  %p = load ptr, ptr @slot\n  call void %p()\n  ret void\n}\n";
    assert!(!run(&text).contains("!callees"));
}

/// The argument of a local function called only with `@a` and `@b`.
#[test]
fn a_local_functions_argument_is_the_functions_it_is_called_with() {
    let text = format!(
        "{FUNCTIONS}define internal void @apply(ptr %fn) {{\nb0:\n  call void %fn()\n  ret void\n}}\n\n\
         define void @top() {{\nb0:\n  call void @apply(ptr @a)\n  call void @apply(ptr @b)\n  ret void\n}}\n"
    );
    let out = run(&text);
    assert!(out.contains("call void %fn(), !callees !0"), "{out}");
}
