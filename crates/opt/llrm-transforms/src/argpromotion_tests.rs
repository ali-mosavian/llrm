use llrm_mir::datalayout::DataLayout;
use llrm_mir::target::Machine;

use crate::argpromotion::promoted;
use crate::testing::{parsed, printed};

/// `text` after the pass, priced in clocks on the real-mode target.
fn run(text: &str) -> String {
    let mut module = parsed(&format!("{}{text}", llrm_analysis::testing::DOS));
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a layout")).expect("parses");
    promoted(&mut module, &layout, &llrm_x86_m16::Dos::default().costs(), false);
    printed(&module)
}

/// A slice descriptor: the data pointer at 0, its length at 4. `sum` reads the length and
/// passes the descriptor on, as Nib's recursive `fn f(a: &[i16], n: i16)` did: every call
/// pushed the descriptor's far pointer and each callee `les`'d it to read one word.
fn sum(attrs: &str, linkage: &str, body: &str) -> String {
    format!(
        "define {linkage}i16 @sum(ptr addrspace(1) {attrs} %d, i16 %n) {{
b0:
  %at = getelementptr i8, ptr addrspace(1) %d, i16 4
  %len = load i16, ptr addrspace(1) %at
  {body}
  %done = icmp sle i16 %n, 0
  br i1 %done, label %out, label %again
again:
  %m = sub i16 %n, 1
  %r = call i16 @sum(ptr addrspace(1) %d, i16 %m)
  %s = add i16 %r, %len
  ret i16 %s
out:
  ret i16 %len
}}
define i16 @top(ptr addrspace(1) %p) {{
b0:
  %r = call i16 @sum(ptr addrspace(1) %p, i16 3)
  ret i16 %r
}}
declare void @sink(ptr addrspace(1))
"
    )
}

const READS: &str = "noalias readonly dereferenceable(6) nocapture";

/// The caller loads the length and the function takes it in the descriptor's place.
#[test]
fn a_recursive_function_reading_a_descriptor_takes_its_fields_instead() {
    let after = run(&sum(READS, "internal ", ""));
    assert!(after.contains("@sum(i16 %") && after.contains("i16 %n)"), "{after}");
    assert!(after.contains("%len = load i16") || after.matches("load i16, ptr addrspace(1)").count() == 1, "{after}");
    // The recursive call passes the length on; only the outside call loads it.
    assert_eq!(after.matches("load i16").count(), 1, "{after}");
    assert!(!after.contains("call i16 @sum(ptr addrspace(1)"), "{after}");
}

/// A callee that writes through the pointer, or lets it escape, would see its own change or
/// another's: it keeps the pointer.
#[test]
fn a_function_that_stores_through_or_escapes_the_pointer_keeps_it() {
    let stored = run(&sum("noalias dereferenceable(6) nocapture", "internal ", "store i16 0, ptr addrspace(1) %d"));
    assert!(stored.contains("@sum(ptr addrspace(1)"), "{stored}");
    let escaped = run(&sum("noalias dereferenceable(6)", "internal ", "call void @sink(ptr addrspace(1) %d)"));
    assert!(escaped.contains("@sum(ptr addrspace(1)"), "{escaped}");
}

/// Where the function loads after a branch, the load a caller makes before the call is the
/// callee's own promise that the bytes are there; without it the caller may fault where the
/// callee did not, and with it a load made on entry is the same load.
#[test]
fn a_load_past_a_branch_is_hoisted_only_where_the_bytes_are_promised() {
    let late = |attrs: &str| {
        format!(
            "define internal i16 @get(ptr addrspace(1) {attrs} %d, i16 %n) {{
b0:
  %c = icmp sle i16 %n, 0
  br i1 %c, label %out, label %read
read:
  %at = getelementptr i8, ptr addrspace(1) %d, i16 4
  %len = load i16, ptr addrspace(1) %at
  ret i16 %len
out:
  ret i16 0
}}
define i16 @top(ptr addrspace(1) %p, i16 %n) {{
b0:
  %r = call i16 @get(ptr addrspace(1) %p, i16 %n)
  ret i16 %r
}}
"
        )
    };
    assert!(run(&late("noalias readonly nocapture")).contains("@get(ptr addrspace(1)"));
    assert!(run(&late(READS)).contains("@get(i16 %"));
}

/// Code outside the module may call it with the pointer.
#[test]
fn an_exported_function_keeps_its_signature() {
    let after = run(&sum(READS, "", ""));
    assert!(after.contains("@sum(ptr addrspace(1)"), "{after}");
}
