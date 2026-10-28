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
