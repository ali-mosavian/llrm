//! What `add_initializes` states, against `inferInitializes`.

use super::*;
use crate::testing::{layout, parsed};

/// The `initializes` ranges of parameter 0 of `@f` after stating `text`.
fn stated(text: &str) -> Vec<(i64, i64)> {
    let mut module = parsed(text);
    let layout = layout(&module);
    let members: BTreeSet<GlobalId> = module.functions().map(|(id, _, _)| id).collect();
    // Callees' own `initializes` first, as the walk does.
    let order = llrm_mir::callgraph::CallGraph::new(&module).bottom_up_components();
    for (component, _) in order {
        let set: BTreeSet<GlobalId> = component.into_iter().filter(|id| members.contains(id)).collect();
        add_initializes(&mut module, &layout, &set);
    }
    let f = module.named("f").expect("@f");
    llrm_mir::memory::initializes(&module.global(f).function().unwrap().parameter_attrs[0]).to_vec()
}

#[test]
fn a_store_through_the_parameter_initializes_its_bytes() {
    assert_eq!(stated("define void @f(ptr %p) {\nb0:\n  store i16 1, ptr %p\n  ret void\n}\n"), [(0, 2)]);
}

#[test]
fn a_read_before_the_write_leaves_nothing_initialized() {
    let text = "define i16 @f(ptr %p) {\nb0:\n  %v = load i16, ptr %p\n  store i16 1, ptr %p\n  ret i16 %v\n}\n";
    assert_eq!(stated(text), []);
}

#[test]
fn a_constant_gep_shifts_the_range() {
    let text = "define void @f(ptr %p) {\nb0:\n  %q = getelementptr i8, ptr %p, i16 4\n  store i16 1, ptr %q\n  store i16 1, ptr %p\n  ret void\n}\n";
    assert_eq!(stated(text), [(0, 2), (4, 6)]);
}

#[test]
fn only_what_both_arms_write_is_initialized() {
    let text = "define void @f(ptr %p, i1 %c) {\nb0:\n  br i1 %c, label %a, label %b\na:\n  store i32 1, ptr %p\n  br label %m\nb:\n  store i16 1, ptr %p\n  br label %m\nm:\n  ret void\n}\n";
    assert_eq!(stated(text), [(0, 2)]);
}

#[test]
fn a_memset_of_a_constant_length_initializes_that_many_bytes() {
    let text = "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n\ndefine void @f(ptr %p) {\nb0:\n  call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 6, i1 false)\n  ret void\n}\n";
    assert_eq!(stated(text), [(0, 6)]);
}

#[test]
fn a_callee_that_initializes_its_argument_initializes_the_callers() {
    let text = "define void @g(ptr nocapture writeonly %p) {\nb0:\n  store i32 1, ptr %p\n  ret void\n}\n\ndefine void @f(ptr %p) {\nb0:\n  call void @g(ptr %p)\n  ret void\n}\n";
    assert_eq!(stated(text), [(0, 4)]);
}
