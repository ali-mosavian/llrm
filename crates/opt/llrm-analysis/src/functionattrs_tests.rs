//! What `infer` stamps, against `FunctionAttrs.cpp`'s rules.

use llrm_mir::module::{GlobalValue, Module};

use super::*;
use crate::testing::parsed;

fn declarations(module: &Module) -> Vec<GlobalValue> {
    module.globals.iter().map(GlobalValue::declaration).collect()
}

/// What `infer` says of the SCC `names`.
fn inferred(
    text: &str,
    names: &[&str],
) -> Vec<Inferred> {
    let module = parsed(text);
    let members: Vec<GlobalId> = names.iter().map(|one| module.named(one).expect("a global")).collect();
    infer(&module, &declarations(&module), &members).into_iter().map(|(_, one)| one).collect()
}

fn all_of(
    no_recurse: bool,
    no_unwind: bool,
    will_return: bool,
    no_return: bool,
    no_free: bool,
    no_sync: bool,
) -> Inferred {
    Inferred { no_recurse, no_unwind, will_return, no_return, no_free, no_sync }
}

#[test]
fn test_a_body_over_norecurse_callees_is_stamped_all_three() {
    let text = "declare i32 @leaf(i32) norecurse nounwind willreturn

define i32 @f(i32 %x) {
b0:
  %a = call i32 @leaf(i32 %x)
  %b = add i32 %a, 1
  ret i32 %b
}
";
    assert_eq!(inferred(text, &["f"]), [all_of(true, true, true, false, false, false)]);
}

/// A call to a body nothing states leaves the function recursive, throwing and
/// not returning; a call to itself leaves it recursive, but a call to an SCC
/// member does not break nounwind.
#[test]
fn test_an_unstated_callee_and_a_self_call_leave_nothing_stamped() {
    let text = "declare void @unknown()

define void @calls_unknown() {
b0:
  call void @unknown()
  ret void
}

define void @itself() {
b0:
  call void @itself()
  ret void
}
";
    assert_eq!(inferred(text, &["calls_unknown"]), [all_of(false, false, false, false, false, false)]);
    assert_eq!(inferred(text, &["itself"]), [all_of(false, true, false, false, true, true)]);
}

/// Mutual recursion: norecurse needs a one-member SCC; nounwind is the whole
/// SCC's, so one throwing instruction takes it from both.
#[test]
fn test_nounwind_is_all_or_none_of_an_scc_and_norecurse_needs_one_member() {
    let quiet = "define void @a() {
b0:
  call void @b()
  ret void
}

define void @b() {
b0:
  call void @a()
  ret void
}
";
    assert_eq!(
        inferred(quiet, &["a", "b"]),
        [all_of(false, true, false, false, true, true), all_of(false, true, false, false, true, true)]
    );
    let throwing = "declare void @unknown()

define void @a() {
b0:
  call void @b()
  ret void
}

define void @b() {
b0:
  call void @a()
  call void @unknown()
  ret void
}
";
    assert_eq!(
        inferred(throwing, &["a", "b"]),
        [all_of(false, false, false, false, false, false), all_of(false, false, false, false, false, false)]
    );
}

/// A loop may not come back; with `mustprogress` and no writes it does.
#[test]
fn test_a_loop_returns_only_by_mustprogress_without_writes() {
    let body = "define void @spin(i1 %c) {
b0:
  br label %loop

loop:
  br i1 %c, label %loop, label %done

done:
  ret void
}
";
    assert!(!inferred(body, &["spin"])[0].will_return);
    let promised = body.replace("define void @spin(i1 %c) {", "define void @spin(i1 %c) mustprogress memory(read) {");
    assert!(inferred(&promised, &["spin"])[0].will_return);
}

/// Only a path to `ret` that no `noreturn` call cuts makes the body return.
#[test]
fn test_a_body_with_no_path_to_a_ret_past_a_noreturn_call_is_noreturn() {
    let text = "declare void @exit() noreturn

define void @stops() {
b0:
  call void @exit()
  ret void
}

define void @returns(i1 %c) {
b0:
  br i1 %c, label %quit, label %go

quit:
  call void @exit()
  ret void

go:
  ret void
}
";
    assert!(inferred(text, &["stops"])[0].no_return);
    assert!(!inferred(text, &["returns"])[0].no_return);
}

/// An `invoke` may unwind whatever its callee states.
#[test]
fn test_an_invoke_takes_nounwind_away() {
    let text = "declare void @leaf() nounwind

define void @f() personality ptr null {
b0:
  invoke void @leaf() to label %ok unwind label %bad

ok:
  ret void

bad:
  %p = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %p
}
";
    assert!(!inferred(text, &["f"])[0].no_unwind);
}

/// What `infer_memory` says of the SCC `names`, as `(name, argmem, other)`
/// mod/ref words; a member whose attribute does not narrow is left out.
fn memory_of(
    text: &str,
    names: &[&str],
) -> Vec<(String, ModRefInfo, ModRefInfo)> {
    let module = parsed(text);
    let members: Vec<GlobalId> = names.iter().map(|one| module.named(one).expect("a global")).collect();
    infer_memory(&module, &members)
        .into_iter()
        .map(|(id, me)| {
            (
                module.global(id).name.owned().expect("a name"),
                me.mod_ref(IRMemLocation::ArgMem),
                me.mod_ref(IRMemLocation::Other),
            )
        })
        .collect()
}

const NO: ModRefInfo = ModRefInfo::NoModRef;

/// `Stamp` stated these through `Summaries` and `alias::initialized`; the
/// translation reads the body with BasicAA's `getModRefInfoMask`.
#[test]
fn a_store_through_a_parameter_is_argmem_write_and_a_constant_read_is_nothing() {
    let text = "@k = internal constant i32 7
define void @put(ptr %p) {
b0:
  store i32 1, ptr %p
  ret void
}
define i32 @table() {
b0:
  %v = load i32, ptr @k
  ret i32 %v
}
define i32 @local() {
b0:
  %a = alloca i32
  store i32 1, ptr %a
  %v = load i32, ptr %a
  ret i32 %v
}
";
    assert_eq!(memory_of(text, &["put"]), [("put".to_owned(), ModRefInfo::Mod, NO)]);
    assert_eq!(memory_of(text, &["table"]), [("table".to_owned(), NO, NO)]);
    assert_eq!(memory_of(text, &["local"]), [("local".to_owned(), NO, NO)]);
}

/// A write to a mutable global is `Other`, not argument memory.
#[test]
fn a_store_to_a_mutable_global_is_other_memory() {
    let text = "@g = internal global i32 0
define void @set() {
b0:
  store i32 1, ptr @g
  ret void
}
";
    assert_eq!(memory_of(text, &["set"]), [("set".to_owned(), NO, ModRefInfo::Mod)]);
}

/// Calls inside the SCC are free, but their pointer arguments count once the
/// SCC reads argument memory (`RecursiveArgME`): `walk` reads `%p` and passes
/// `@g` on to itself, so it also reads `@g`, which is `Other`.
#[test]
fn a_recursive_call_adds_its_arguments_locations_when_the_scc_reads_argument_memory() {
    let text = "@g = internal global i32 0
define i32 @walk(ptr %p, i32 %n) {
b0:
  %v = load i32, ptr %p
  %m = sub i32 %n, 1
  %r = call i32 @walk(ptr @g, i32 %m)
  %s = add i32 %v, %r
  ret i32 %s
}
";
    assert_eq!(memory_of(text, &["walk"]), [("walk".to_owned(), ModRefInfo::Ref, ModRefInfo::Ref)]);
}

/// A member that writes narrows every member of its SCC (`ME` is the SCC's).
#[test]
fn one_writing_member_states_its_whole_scc() {
    let text = "define void @ping(ptr %p, i32 %n) {
b0:
  call void @pong(ptr %p, i32 %n)
  ret void
}
define void @pong(ptr %p, i32 %n) {
b0:
  store i32 %n, ptr %p
  call void @ping(ptr %p, i32 %n)
  ret void
}
";
    let found = memory_of(text, &["ping", "pong"]);
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|(_, argument, other)| *argument == ModRefInfo::Mod && *other == NO));
}

/// Stating the result replaces `readonly` by `memory(...)` and a callee's new
/// attribute reaches its caller in the same bottom-up pass: `outer` reads
/// nothing of its own, so it is `memory(none)` only because `inner` was
/// stated first.
#[test]
fn a_stated_callee_narrows_its_caller_in_one_pass() {
    let mut module = parsed(
        "@k = internal constant i32 7
define i32 @inner() readonly {
b0:
  %v = load i32, ptr @k
  ret i32 %v
}
define i32 @outer() {
b0:
  %v = call i32 @inner()
  ret i32 %v
}
",
    );
    let (inner, outer) = (module.named("inner").unwrap(), module.named("outer").unwrap());
    assert_eq!(stamp_memory(&mut module, &[inner]), [inner]);
    assert_eq!(stamp_memory(&mut module, &[outer]), [outer]);
    for id in [inner, outer] {
        let attrs = &module.global(id).function().unwrap().attrs;
        assert!(MemoryEffects::of_attrs(attrs).does_not_access_memory());
        assert!(!Facts::of(attrs).read_only(), "readonly is replaced by memory(...)");
    }
}

/// DEVIATE F1: a loop whose trip count is proved returns, where LLVM's
/// `willreturn` needs a body without back edges; the flags are stated in
/// `deriveAttrsInPostOrder`'s order. Without F1 the counted loop came out as
/// `norecurse nounwind` and every caller kept the loop.
#[test]
fn post_order_states_willreturn_for_a_counted_loop_after_noreturn_and_before_nounwind() {
    let mut module = parsed(
        "define i32 @count() {
b0:
  br label %b1
b1:
  %i = phi i32 [ 0, %b0 ], [ %n, %b1 ]
  %n = add i32 %i, 1
  %more = icmp ult i32 %n, 10
  br i1 %more, label %b1, label %b2
b2:
  ret i32 %n
}
",
    );
    let id = module.named("count").unwrap();
    assert_eq!(post_order(&mut module, &DataLayout::default(), Spaces::FLAT, &[id]), [id]);
    let attrs = &module.global(id).function().unwrap().attrs;
    let order: Vec<Fact> = attrs
        .iter()
        .filter_map(Fact::of_attribute)
        .filter(|one| matches!(one, Fact::WillReturn | Fact::NoUnwind | Fact::NoRecurse))
        .collect();
    assert_eq!(order, [Fact::WillReturn, Fact::NoUnwind, Fact::NoRecurse]);
}

/// `will_return_by_loops` derived what is known of the whole body (0.94 M
/// instructions a body at d_faces -O2, 11 M over the stamps) before it asked
/// whether the calls state `willreturn`; with one that does not, the answer
/// is no whatever the loops are.
#[test]
fn a_call_that_does_not_state_willreturn_costs_no_derivation_of_the_loops() {
    let mut module = parsed(
        "declare void @external(i32)

define i32 @count() {
b0:
  br label %b1
b1:
  %i = phi i32 [ 0, %b0 ], [ %n, %b1 ]
  call void @external(i32 %i)
  %n = add i32 %i, 1
  %more = icmp ult i32 %n, 10
  br i1 %more, label %b1, label %b2
b2:
  ret i32 %n
}
",
    );
    let id = module.named("count").unwrap();
    let before = crate::consts::scratch_derivations();
    post_order(&mut module, &DataLayout::default(), Spaces::FLAT, &[id]);
    assert_eq!(
        crate::consts::scratch_derivations(),
        before,
        "the body's values were derived for a loop proof nothing reads"
    );
    assert!(!Facts::of(&module.global(id).function().unwrap().attrs).will_return());
}

/// What F1 may not do: a loop nothing proves finite is no `willreturn`
/// function (callers delete an unused call of one). An empty loop, a loop that
/// ends on a loaded value and a `!=` loop stepping by two past its bound all
/// can run for ever.
#[test]
fn a_loop_that_may_not_end_is_not_willreturn() {
    let shapes = [
        ("empty", "define i32 @f() {\nb0:\n  br label %b1\nb1:\n  br label %b1\n}\n"),
        (
            "loaded exit",
            "define i32 @f(ptr %p) {\nb0:\n  br label %b1\nb1:\n  %q = phi ptr [ %p, %b0 ], [ %r, %b1 ]\n  %v = load i32, ptr %q\n  %r = getelementptr i32, ptr %q, i32 1\n  %z = icmp ne i32 %v, 0\n  br i1 %z, label %b1, label %b2\nb2:\n  ret i32 %v\n}\n",
        ),
        (
            "step two",
            "define i32 @f(i32 %n) {\nb0:\n  br label %b1\nb1:\n  %i = phi i32 [ 0, %b0 ], [ %j, %b1 ]\n  %j = add i32 %i, 2\n  %c = icmp ne i32 %j, %n\n  br i1 %c, label %b1, label %b2\nb2:\n  ret i32 %j\n}\n",
        ),
    ];
    for (name, text) in shapes {
        let mut module = parsed(text);
        let id = module.named("f").unwrap();
        post_order(&mut module, &DataLayout::default(), Spaces::FLAT, &[id]);
        let attrs = &module.global(id).function().unwrap().attrs;
        assert!(!Facts::of(attrs).will_return(), "{name}: {attrs:?}");
    }
}

/// `nofree` and `nosync` were never inferred: 111 functions of 180 in the
/// dyn-work programs had neither where the pin gives both (a body that calls
/// only what states them), and a call that states neither keeps them off.
#[test]
fn test_nofree_and_nosync_follow_what_the_calls_state() {
    let text = "declare void @quiet() nofree nosync nounwind
declare void @loud()

define void @a() {
b0:
  call void @quiet()
  ret void
}

define void @b() {
b0:
  call void @loud()
  ret void
}

define i32 @leaf(i32 %x) {
b0:
  %y = add i32 %x, 1
  ret i32 %y
}
";
    let flags = |name: &str| {
        let one = inferred(text, &[name])[0];
        (one.no_free, one.no_sync)
    };
    assert_eq!((flags("a"), flags("b"), flags("leaf")), ((true, true), (false, false), (true, true)));
}
