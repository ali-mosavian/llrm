//! What `summaries` states of a function, from its attributes.

use super::*;
use crate::testing::parsed;

fn of(
    text: &str,
    name: &str,
) -> Summary {
    let module = parsed(text);
    let tracked = std::collections::BTreeSet::new();
    let found = crate::globalsmodref::compute(&module);
    summaries(&module, &crate::testing::layout(&module), &tracked, &found)
        .get(name)
        .expect("a defined function")
        .as_ref()
        .clone()
}

fn parameters(slices: &crate::sliceset::SliceSet) -> Vec<i64> {
    slices
        .iter()
        .map(|one| {
            assert_eq!(one.object.kind, MemoryKind::Parameter);
            let Key::Int(index) = one.object.key else { panic!("a parameter is numbered") };
            index
        })
        .collect()
}

#[test]
fn argmem_readwrite_is_a_read_and_a_write_of_each_pointer_parameter() {
    let one = of("define void @f(ptr %p, i16 %n, ptr %q) memory(argmem: readwrite) {\nb0:\n  ret void\n}\n", "f");
    assert_eq!((parameters(&one.reads), parameters(&one.writes)), (vec![0, 2], vec![0, 2]));
    assert!(!one.unknown_read && !one.unknown_write);
}

#[test]
fn a_readonly_parameter_is_read_not_written() {
    let one = of("define void @f(ptr readonly %p) memory(argmem: readwrite) {\nb0:\n  ret void\n}\n", "f");
    assert_eq!((parameters(&one.reads), parameters(&one.writes)), (vec![0], vec![]));
}

#[test]
fn memory_none_states_no_access_and_nocapture_is_not_a_capture() {
    let one = of("define void @f(ptr nocapture %p, ptr %q) memory(none) {\nb0:\n  ret void\n}\n", "f");
    assert!(one.reads.is_empty() && one.writes.is_empty());
    assert_eq!(one.captures, std::collections::BTreeSet::from([Some(Identity::Int(1))]));
}

/// A1: what `other` reaches, tracked globals included, is the unknown access.
#[test]
fn the_unknown_access_stands_for_globals() {
    let one = of("@g = internal global i16 0\ndefine void @f() memory(readwrite) {\nb0:\n  ret void\n}\n", "f");
    assert!(one.unknown_read && one.unknown_write);
    let reads = of("define void @f() memory(read) {\nb0:\n  ret void\n}\n", "f");
    assert!(reads.unknown_read && !reads.unknown_write);
}

/// The `(low, high)` of the slices of parameter 0 in `slices`; `None` is the whole pointee.
fn bytes(slices: &crate::sliceset::SliceSet) -> Vec<Option<(i64, i64)>> {
    slices
        .iter()
        .map(|one| (one.low != crate::memory::WHOLE_LOW || one.high != crate::memory::WHOLE_HIGH).then_some((one.low, one.high)))
        .collect()
}

const PARAM: &str = "define void @f(ptr %p) memory(argmem: readwrite) {\nb0:\n";

/// A caller reloaded a field the callee never touches: `f` wrote two fields of
/// its record and the summary said the whole record.
#[test]
fn constant_stores_write_those_bytes_only() {
    let text = format!(
        "{PARAM}  %q = getelementptr i8, ptr %p, i16 4\n  store i16 1, ptr %p\n  store i16 2, ptr %q\n  ret void\n}}\n"
    );
    let one = of(&text, "f");
    assert_eq!((bytes(&one.writes), bytes(&one.reads)), (vec![Some((0, 2)), Some((4, 6))], vec![]));
}

#[test]
fn a_load_reads_its_bytes_only() {
    let text = format!("{PARAM}  %q = getelementptr i8, ptr %p, i16 6\n  %v = load i16, ptr %q\n  store i16 %v, ptr %p\n  ret void\n}}\n");
    let one = of(&text, "f");
    assert_eq!((bytes(&one.writes), bytes(&one.reads)), (vec![Some((0, 2))], vec![Some((6, 8))]));
}

#[test]
fn a_variable_index_is_the_whole_pointee() {
    let text = "define void @f(ptr %p, i16 %i) memory(argmem: readwrite) {\nb0:\n  %q = getelementptr i8, ptr %p, i16 %i\n  store i16 1, ptr %q\n  store i16 1, ptr %p\n  ret void\n}\n";
    assert_eq!(bytes(&of(text, "f").writes), vec![None]);
}

#[test]
fn an_escaping_pointer_is_the_whole_pointee() {
    let text = "@g = global ptr null\n\ndefine void @f(ptr %p) memory(argmem: readwrite) {\nb0:\n  store ptr %p, ptr @g\n  store i16 1, ptr %p\n  ret void\n}\n";
    assert_eq!(bytes(&of(text, "f").writes), vec![None]);
}

#[test]
fn a_volatile_access_is_the_whole_pointee() {
    let text = format!("{PARAM}  store volatile i16 1, ptr %p\n  ret void\n}}\n");
    assert_eq!(bytes(&of(&text, "f").writes), vec![None]);
}

#[test]
fn a_call_that_takes_the_pointer_is_the_whole_pointee() {
    let text = "declare void @h(ptr)\n\ndefine void @f(ptr %p) memory(argmem: readwrite) {\nb0:\n  store i16 1, ptr %p\n  call void @h(ptr %p)\n  ret void\n}\n";
    assert_eq!(bytes(&of(text, "f").writes), vec![None]);
}

#[test]
fn a_memset_of_unknown_length_is_the_whole_pointee() {
    let text = "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n\ndefine void @f(ptr %p, i16 %n) memory(argmem: readwrite) {\nb0:\n  call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 %n, i1 false)\n  ret void\n}\n";
    assert_eq!(bytes(&of(text, "f").writes), vec![None]);
}

#[test]
fn a_memset_of_known_length_writes_those_bytes() {
    let text = "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n\ndefine void @f(ptr %p) memory(argmem: readwrite) {\nb0:\n  call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 6, i1 false)\n  ret void\n}\n";
    assert_eq!(bytes(&of(text, "f").writes), vec![Some((0, 6))]);
}

#[test]
fn an_interposable_definition_is_the_whole_pointee() {
    let text = "define weak void @f(ptr %p) memory(argmem: readwrite) {\nb0:\n  store i16 1, ptr %p\n  ret void\n}\n";
    assert_eq!(bytes(&of(text, "f").writes), vec![None]);
}
