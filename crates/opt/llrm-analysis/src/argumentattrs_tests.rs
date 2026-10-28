//! What `add_argument_attrs` states, against FunctionAttrs.cpp's rules.

use llrm_mir::facts::Facts;

use super::*;
use crate::testing::parsed;

/// The module after stating the SCC `names`, with the members it changed.
fn stated(
    text: &str,
    names: &[&str],
) -> (Module, BTreeSet<String>) {
    let mut module = parsed(text);
    let members: BTreeSet<GlobalId> = names.iter().map(|one| module.named(one).expect("a global")).collect();
    let changed = add_argument_attrs(&mut module, &members);
    let names = changed.into_iter().map(|id| module.global(id).name.owned().expect("a name")).collect();
    (module, names)
}

fn facts<'a>(
    module: &'a Module,
    name: &str,
    index: usize,
) -> Facts {
    Facts::of(&module.global(module.named(name).unwrap()).function().unwrap().parameter_attrs[index])
}

#[test]
fn a_load_only_parameter_is_nocapture_readonly() {
    let (module, changed) = stated(
        "define i32 @get(ptr %p) {
b0:
  %v = load i32, ptr %p
  ret i32 %v
}
",
        &["get"],
    );
    assert_eq!(changed, BTreeSet::from(["get".to_owned()]));
    let one = facts(&module, "get", 0);
    assert!(one.no_capture() && one.read_only() && !one.write_only());
}

#[test]
fn a_stored_through_parameter_is_nocapture_writeonly_and_a_kept_one_is_neither() {
    let (module, _) = stated(
        "@slot = internal global ptr null
define void @put(ptr %p, ptr %q) {
b0:
  store i32 1, ptr %p
  store ptr %q, ptr @slot
  ret void
}
",
        &["put"],
    );
    let (p, q) = (facts(&module, "put", 0), facts(&module, "put", 1));
    assert!(p.no_capture() && p.write_only());
    assert!(!q.no_capture() && !q.read_only() && !q.write_only(), "kept in a global, then read through by anyone");
}

/// `addAccessAttrs`'s `IsFree`: no parameter got `nofree` (the pin's `h` has
/// `captures(none)` alone, `hn` and `get` also `nofree`).
#[test]
fn a_parameter_is_nofree_unless_a_call_that_may_free_writes_through_it() {
    let (module, _) = stated(
        "declare void @g(ptr captures(none))
declare void @gn(ptr captures(none)) nofree
define i32 @get(ptr %p) {
b0:
  %v = load i32, ptr %p
  ret i32 %v
}
define void @h(ptr %p) {
b0:
  call void @g(ptr %p)
  ret void
}
define void @hn(ptr %p) {
b0:
  call void @gn(ptr %p)
  ret void
}
",
        &["get", "h", "hn"],
    );
    let nofree = |name| facts(&module, name, 0).no_free();
    assert!(nofree("get") && !nofree("h") && nofree("hn"));
}

/// `addArgumentAttrsFromCallsites`: the pin's `opt -passes=function-attrs`
/// gives `a`'s `%p` `nonnull` and nothing else here.
#[test]
fn a_parameter_passed_to_a_nonnull_noundef_one_of_a_guaranteed_call_is_nonnull() {
    let (module, _) = stated(
        "declare void @use(ptr noundef nonnull)
declare void @plain(ptr nonnull)
declare void @may_not_return()
define void @a(ptr %p, ptr %q) {
b0:
  call void @use(ptr %p)
  call void @plain(ptr %q)
  ret void
}
define void @b(ptr %p) {
b0:
  call void @may_not_return()
  call void @use(ptr %p)
  ret void
}
",
        &["a", "b"],
    );
    let nonnull = |name, index| facts(&module, name, index).non_null();
    assert!(nonnull("a", 0) && !nonnull("a", 1) && !nonnull("b", 0));
}

#[test]
fn an_unused_parameter_is_nocapture_readnone() {
    let (module, _) = stated("define void @f(ptr %p) {\nb0:\n  ret void\n}\n", &["f"]);
    let one = facts(&module, "f", 0);
    assert!(one.no_capture() && one.read_none());
}

/// A call into the SCC is no capture and no access by itself: `even` passes
/// `%p` on to `odd` which reads it; both end `readonly nocapture`.
#[test]
fn a_pointer_passed_around_an_scc_takes_what_the_scc_does_to_it() {
    let text = "define i32 @even(ptr %p, i32 %n) {
b0:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %done, label %go
done:
  %v = load i32, ptr %p
  ret i32 %v
go:
  %m = sub i32 %n, 1
  %r = call i32 @odd(ptr %p, i32 %m)
  ret i32 %r
}
define i32 @odd(ptr %p, i32 %n) {
b0:
  %m = sub i32 %n, 1
  %r = call i32 @even(ptr %p, i32 %m)
  ret i32 %r
}
";
    let (module, changed) = stated(text, &["even", "odd"]);
    assert_eq!(changed.len(), 2);
    for name in ["even", "odd"] {
        let one = facts(&module, name, 0);
        assert!(one.no_capture() && one.read_only(), "@{name}");
    }
}

/// A member that keeps the pointer takes the whole SCC's with it.
#[test]
fn one_member_that_keeps_the_pointer_denies_the_scc() {
    let text = "@slot = internal global ptr null
define void @a(ptr %p, i32 %n) {
b0:
  call void @b(ptr %p, i32 %n)
  ret void
}
define void @b(ptr %p, i32 %n) {
b0:
  store ptr %p, ptr @slot
  call void @a(ptr %p, i32 %n)
  ret void
}
";
    let (module, _) = stated(text, &["a", "b"]);
    for name in ["a", "b"] {
        assert!(!facts(&module, name, 0).no_capture(), "@{name}");
    }
}

/// DEVIATE A3. The pinned opt (24.0.0git d1106deb71cc,
/// `-passes=function-attrs`) leaves `walk` bare and states `walk1` (one self
/// call): the two recursive calls make two graph edges and its test wants
/// exactly one. Stamp stated both, so bench `bintree`'s `walk` and
/// `quicksort`'s `sort` lost `nocapture readonly`.
#[test]
fn a_parameter_passed_to_itself_twice_is_stated() {
    let text = "define i64 @walk(ptr %t, i32 %at) {
b0:
  %c = icmp eq i32 %at, 0
  br i1 %c, label %done, label %go
done:
  %v = load i32, ptr %t
  %e = sext i32 %v to i64
  ret i64 %e
go:
  %m = sub i32 %at, 1
  %a = call i64 @walk(ptr %t, i32 %m)
  %b = call i64 @walk(ptr %t, i32 %m)
  %s = add i64 %a, %b
  ret i64 %s
}
";
    let (module, _) = stated(text, &["walk"]);
    let one = facts(&module, "walk", 0);
    assert!(one.no_capture() && one.read_only());
}
