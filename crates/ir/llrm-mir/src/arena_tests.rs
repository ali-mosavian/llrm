//! `Arena`: an entry's address survives every later push.

use std::rc::Rc;

use crate::arena::Arena;

/// A `Vec`-backed table moved its entries when it grew, so a `&Constant` held
/// across an intern pointed at freed memory. 10,000 pushes cross nine chunk
/// boundaries; every earlier reference must still read its own value.
#[test]
fn an_entry_stays_where_it_was_put_while_the_arena_grows() {
    let arena = Arena::<String>::default();
    let mut held = Vec::new();
    for at in 0..10_000 {
        let index = arena.push(format!("entry {at}"));
        assert_eq!(index, at);
        held.push((arena.get(index) as *const String, at));
    }
    for (address, at) in held {
        assert_eq!(arena.get(at) as *const String, address, "entry {at} moved");
        assert_eq!(*arena.get(at), format!("entry {at}"));
    }
    assert_eq!(arena.len(), 10_000);
}

/// A clone is a copy, not a view: pushes to one do not show in the other, and
/// both drop what they hold exactly once.
#[test]
fn a_clone_and_its_original_own_their_entries() {
    let counted = Rc::new(());
    let arena = Arena::<Rc<()>>::default();
    for _ in 0..200 {
        arena.push(Rc::clone(&counted));
    }
    let copy = arena.clone();
    copy.push(Rc::clone(&counted));
    assert_eq!((arena.len(), copy.len()), (200, 201));
    assert_eq!(Rc::strong_count(&counted), 1 + 200 + 201);
    drop(arena);
    assert_eq!(Rc::strong_count(&counted), 1 + 201);
    drop(copy);
    assert_eq!(Rc::strong_count(&counted), 1);
}

#[test]
fn interning_through_a_shared_reference_gives_one_id_per_value() {
    let context = crate::context::Context::new();
    let shared = &context;
    let i32_ty = shared.types.int(32);
    let (a, b) = (shared.int(i32_ty, 7), shared.int(i32_ty, 7));
    assert_eq!(a, b);
    assert_ne!(a, shared.int(i32_ty, 8));
    assert_eq!(shared.get(a).ty, i32_ty);
}
