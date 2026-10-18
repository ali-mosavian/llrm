//! `joined`'s PRE and the `Gvn` pass, each body MIR text run by llrm-mir's
//! interpreter before and after.

use llrm_analysis::manager::Summaries;
use llrm_mir::passes::PassManager;

use super::{Gvn, joined};
use crate::testing::{f, parsed, printed, results};

const INPUTS: &[&[i128]] = &[&[0, 0, 0], &[3, 5, 1], &[-7, 2, 0], &[0x7fff, 1, 1]];

/// A diamond on %c whose arms are `left` and `right`, joining at b3 to
/// compute `join` into %r.
fn diamond(
    left: &str,
    right: &str,
    join: &str,
) -> String {
    format!(
        "define i16 @f(i16 %x, i16 %y, i1 %c) {{
b0:
  br i1 %c, label %b1, label %b2

b1:
{left}  br label %b3

b2:
{right}  br label %b3

b3:
{join}  ret i16 %r
}}
"
    )
}

/// `text` joined: its printed form, and whether anything changed. What
/// `@f` returns is what it returned before.
fn joined_once(
    text: &str,
    insert: bool,
) -> (String, bool) {
    let before = parsed(text);
    let mut module = before.clone();
    let changed = joined(f(&mut module), insert).unwrap();
    let text = printed(&module);
    assert_eq!(results(&module, INPUTS), results(&before, INPUTS), "{text}");
    (text, changed)
}

fn kept(
    text: &str,
    insert: bool,
) {
    let (after, changed) = joined_once(text, insert);
    assert!(!changed);
    assert_eq!(after, printed(&parsed(text)));
}

#[test]
fn test_providers_on_every_edge_become_a_phi() {
    let (text, changed) =
        joined_once(&diamond("  %a = add i16 %x, %y\n", "  %b = add i16 %y, %x\n", "  %r = add i16 %x, %y\n"), false);
    assert!(changed);
    assert!(
        text.ends_with("b3:\n  %r.pre-phi = phi i16 [ %a, %b1 ], [ %b, %b2 ]\n  ret i16 %r.pre-phi\n}\n"),
        "{text}"
    );
}

#[test]
fn test_a_join_expression_translates_through_its_phis() {
    let (text, changed) = joined_once(
        &diamond(
            "  %a = mul i16 %x, 3\n",
            "  %b = mul i16 %y, 3\n",
            "  %p = phi i16 [ %x, %b1 ], [ %y, %b2 ]\n  %r = mul i16 %p, 3\n",
        ),
        false,
    );
    assert!(changed);
    assert!(text.contains("  %r.pre-phi = phi i16 [ %a, %b1 ], [ %b, %b2 ]\n  ret i16 %r.pre-phi\n"), "{text}");
}

#[test]
fn test_a_missing_provider_is_inserted_on_an_unconditional_edge() {
    let text = diamond("  %a = xor i16 %x, %y\n", "", "  %r = xor i16 %x, %y\n");
    kept(&text, false);
    let (text, changed) = joined_once(&text, true);
    assert!(changed);
    assert!(
        text.ends_with(
            "b2:
  %r.pre = xor i16 %x, %y
  br label %b3

b3:
  %r.pre-phi = phi i16 [ %a, %b1 ], [ %r.pre, %b2 ]
  ret i16 %r.pre-phi
}
"
        ),
        "{text}"
    );
}

/// A divide by %y on the path that did not divide could trap there.
#[test]
fn test_a_division_is_not_inserted_on_a_path_that_did_not_divide() {
    let text = diamond("  %a = udiv i16 %x, %y\n", "", "  %r = udiv i16 %x, %y\n");
    let mut module = parsed(&text);
    assert!(!joined(f(&mut module), true).unwrap());
    assert_eq!(printed(&module), printed(&parsed(&text)));
}

#[test]
fn test_no_provider_on_any_edge_leaves_the_join_alone() {
    kept(&diamond("  %a = add i16 %x, 1\n", "  %b = add i16 %y, 1\n", "  %r = add i16 %x, %y\n"), true);
}

/// `x - y` on one edge and `y - x` on the other: the second edge has no
/// provider.
#[test]
fn test_an_arm_computing_another_expression_serves_nothing() {
    kept(&diamond("  %a = sub i16 %x, %y\n", "  %b = sub i16 %y, %x\n", "  %r = sub i16 %x, %y\n"), false);
}

/// Inserting on the entry edge would hoist into the preheader what the
/// latch computes; the latch, dominated by the header, serves nothing.
#[test]
fn test_a_loop_header_is_not_served_from_its_own_body() {
    let text = "define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %n, %b2 ]
  %r = add i16 %x, %y
  %more = icmp ult i16 %i, 3
  br i1 %more, label %b2, label %b3

b2:
  %a = add i16 %x, %y
  %n = add i16 %i, 1
  br label %b1

b3:
  ret i16 %r
}
";
    kept(text, true);
}

/// Numbering serves the dominated repeat; the join then takes a phi of
/// the arms' values.
#[test]
fn test_gvn_numbers_then_joins() {
    let text = diamond(
        "  %a = shl i16 %x, 2\n  %a2 = shl i16 %x, 2\n  %s = add i16 %a, %a2\n",
        "  %b = shl i16 %x, 2\n",
        "  %r = shl i16 %x, 2\n",
    );
    let before = parsed(&text);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.verify_invalidation = true;
    manager.require::<Summaries>();
    manager.add(Gvn::default());
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
    let after = printed(&module);
    assert!(after.contains("  %s = add i16 %a, %a\n"), "{after}");
    assert!(after.contains("  %r.pre-phi = phi i16 [ %a, %b1 ], [ %b, %b2 ]\n  ret i16 %r.pre-phi\n"), "{after}");
    assert_eq!(results(&module, INPUTS), results(&before, INPUTS));
}

/// Under the pass manager a load through a global is served across a
/// call to a `readonly` callee. The pass once saw no globals, so it took
/// the callee for one that may write and loaded @g again.
#[test]
fn a_load_through_a_global_is_reused_across_a_readonly_call() {
    let mut module = parsed(
        "@g = global i16 0

declare i16 @peek() readonly

define i16 @f() {
b0:
  %a = load i16, ptr @g
  %p = call i16 @peek()
  %b = load i16, ptr @g
  %r = add i16 %a, %b
  ret i16 %r
}
",
    );
    let mut manager = PassManager::default();
    manager.require::<Summaries>();
    manager.add(Gvn::default());
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
    let after = printed(&module);
    assert!(after.contains("  %r = add i16 %a, %a\n"), "{after}");
}

/// `text` through `Gvn` under the pass manager, printed; what `@f` returns
/// stays.
fn managed(text: &str) -> String {
    managed_with(text, Gvn::default())
}

fn managed_with(
    text: &str,
    gvn: Gvn,
) -> String {
    let before = parsed(text);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.verify_invalidation = true;
    manager.require::<Summaries>();
    manager.add(gvn);
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
    let after = printed(&module);
    assert_eq!(results(&module, INPUTS), results(&before, INPUTS), "{after}");
    after
}

/// The pass forwards a stored value to the load of its cell.
#[test]
fn test_gvn_forwards_a_stored_value_to_its_load() {
    let text = "@g = global i16 0

define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  store i16 %x, ptr @g
  %r = load i16, ptr @g
  ret i16 %r
}
";
    assert!(managed(text).contains("  store i16 %x, ptr @g\n  ret i16 %x\n"));
    let volatile = text.replace("load i16", "load volatile i16");
    assert_eq!(managed(&volatile), printed(&parsed(&volatile)));
}

/// The pass makes a join's load the phi of what each arm stored.
#[test]
fn test_gvn_joins_the_values_each_arm_stored() {
    let text = format!(
        "@g = global i16 0\n\n{}",
        diamond("  store i16 %x, ptr @g\n", "  store i16 %y, ptr @g\n", "  %r = load i16, ptr @g\n")
    );
    assert!(
        managed(&text).contains("b3:\n  %r1 = phi i16 [ %x, %b1 ], [ %y, %b2 ]\n  ret i16 %r1\n"),
        "{}",
        managed(&text)
    );
    let volatile = text.replace("load i16", "load volatile i16");
    assert_eq!(managed(&volatile), printed(&parsed(&volatile)));
}

/// `@x` loaded before a loop of `bound` trips and again after a store
/// inside it, where serving the reload holds `%a` through a point that
/// then spills. Priced at the conventional ten trips even when proven
/// one, the reload was always served: a spill for one saved load. The
/// registers are the target's; gvn used to ignore them. Where `%a` spills
/// in the loop, each reload of it costs the load it saves.
#[test]
fn a_loop_of_proven_trips_prices_the_reload_it_serves() {
    let text = |bound: &str| {
        format!(
            "@x = global i16 0
@y = global i16 0
@z = global i16 0

define i16 @f(i16 %n, i16 %p) {{
b0:
  %a = load i16, ptr @x
  store i16 %a, ptr @z
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  %more = icmp slt i16 %i, {bound}
  br i1 %more, label %b2, label %b3

b2:
  %t1 = mul i16 %i, 3
  %t2 = mul i16 %i, 5
  %t3 = add i16 %t1, %t2
  store i16 %t3, ptr @y
  %b = load i16, ptr @x
  store i16 %b, ptr @z
  %i.next = add i16 %i, 1
  br label %b1

b3:
  %q = mul i16 %p, %p
  %r = add i16 %q, %p
  ret i16 %r
}}
"
        )
    };
    let reloads = |bound: &str, registers: i64| {
        let before = parsed(&text(bound));
        let mut module = before.clone();
        let mut manager = PassManager::default();
        manager.require::<Summaries>();
        manager.add(Gvn::default());
        manager
            .run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned { registers, ..Default::default() }))
            .unwrap();
        let inputs: &[&[i128]] = &[&[0, 1], &[1, 2], &[5, 3]];
        assert_eq!(results(&module, inputs), results(&before, inputs));
        printed(&module).contains("%b = load i16, ptr @x")
    };
    assert!(reloads("1", 4));
    assert!(!reloads("%n", 6), "room for %a: serving the reload saves a load per trip");
}

/// Without `Summaries` required the pass runs, as an LLVM function pass
/// does without a cached outer result: every call unknown, so less
/// precise. It panicked.
#[test]
fn a_bare_pass_manager_takes_every_call_for_unknown() {
    let module = crate::testing::parsed(&format!(
        "{}{}{}",
        llrm_analysis::testing::DOS,
        crate::testing::WRITES_ITS_ARGUMENT,
        "define i16 @f() {\nb0:\n  %a = load i16, ptr @g\n  call void @h(ptr @k)\n  %b = load i16, ptr @g\n  %r = add i16 %a, %b\n  ret i16 %r\n}\n"
    ));
    let precise = crate::testing::summarized(&module, Gvn::default(), true, &[&[]]);
    let bare = crate::testing::summarized(&module, Gvn::default(), false, &[&[]]);
    assert!(precise.contains("%r = add i16 %a, %a") && !bare.contains("%r = add i16 %a, %a"), "{precise}\n{bare}");
}

/// @e may call back into @f, so @h's summary took in @f's: the memset's
/// write to @f's own frame came with it, and instantiated at @f's call to
/// @h named @f's %a, which the reload after the call then had to wait for.
#[test]
fn a_summary_carries_no_frame_object_of_a_call_back() {
    let mut module = parsed(&format!(
        "{}define internal void @h(ptr addrspace(1) %p) {{
b0:
  %q = load ptr addrspace(1), ptr addrspace(1) %p
  store i16 1, ptr addrspace(1) %q
  call void @e()
  ret void
}}

declare void @e()

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

define i16 @f() {{
b0:
  %a = alloca [4 x i8]
  %v = alloca ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr %a, i8 0, i16 4, i1 false)
  %c = getelementptr inbounds i8, ptr %a, i16 1
  store i8 7, ptr %c
  %vf = addrspacecast ptr %v to ptr addrspace(1)
  call void @h(ptr addrspace(1) %vf)
  %x = load i8, ptr %c
  %y = zext i8 %x to i16
  ret i16 %y
}}
",
        llrm_analysis::testing::DOS
    ));
    let mut manager = PassManager::default();
    manager.require::<Summaries>();
    manager.add(Gvn::default());
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
    let text = printed(&module);
    // The load is the stored 7, and the widening of a constant is simplified.
    assert!(text.contains("ret i16 7"), "{text}");
}

/// A view lent to @bump holds a pointer to %values; @bump reads it out
/// and writes through it, so %values changes though only the view's
/// address was passed. The reload after the call took the stored 20.
#[test]
fn what_a_lent_pointer_holds_is_written_by_the_callee() {
    let module = parsed(&format!(
        "{}define internal void @bump(ptr addrspace(1) nocapture %view) {{
b0:
  %p = load ptr addrspace(1), ptr addrspace(1) %view
  %e = getelementptr inbounds i16, ptr addrspace(1) %p, i16 1
  %x = load i16, ptr addrspace(1) %e
  %y = add i16 %x, 3
  store i16 %y, ptr addrspace(1) %e
  ret void
}}

define i16 @f() {{
b0:
  %values = alloca [3 x i16]
  %view = alloca ptr addrspace(1)
  %e = getelementptr inbounds i16, ptr %values, i16 1
  store i16 20, ptr %e
  %far = addrspacecast ptr %values to ptr addrspace(1)
  store ptr addrspace(1) %far, ptr %view
  %v = addrspacecast ptr %view to ptr addrspace(1)
  call void @bump(ptr addrspace(1) %v)
  %r = load i16, ptr %e
  ret i16 %r
}}
",
        llrm_analysis::testing::DOS
    ));
    crate::testing::summarized(&module, Gvn::default(), true, &[&[]]);
}

/// GVN's propagateEquality: below the edge a branch takes, its condition
/// is that edge's constant. A bounds check CSE had merged was branched on
/// again where it always held (T028's `update`), and lowering kept the
/// bit alive in a register to test it twice.
#[test]
fn a_condition_is_known_below_the_edge_it_took() {
    let after = managed(
        "define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  %k = icmp ugt i16 %x, 1
  br i1 %k, label %b1, label %b4

b1:
  br i1 %k, label %b2, label %b3

b2:
  %m = select i1 %k, i16 %y, i16 0
  ret i16 %m

b3:
  ret i16 2

b4:
  %n = zext i1 %k to i16
  ret i16 %n
}
",
    );
    // The select and the widening are simplified on the way.
    assert!(
        after.contains("br i1 true, label %b2, label %b3")
            && after.contains("b2:\n  ret i16 %y")
            && after.contains("b4:\n  ret i16 0"),
        "{after}"
    );
}

/// The edge's own phi uses count: a phi in an arm that takes the branch's
/// condition from the branch's block reads that arm's constant, though the
/// arm's block does not dominate the block the phi's operand comes from (the
/// branch's own).
#[test]
fn a_condition_is_known_in_a_phi_of_the_arm_on_its_own_edge() {
    let after = managed(
        "define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  %k = icmp ugt i16 %x, 1
  br i1 %k, label %b1, label %b2

b1:
  br label %b2

b2:
  %p = phi i1 [ %k, %b0 ], [ true, %b1 ]
  %n = zext i1 %p to i16
  ret i16 %n
}
",
    );
    assert!(!after.contains("phi i1 [ %k"), "{after}");
}

/// A segment made a far pointer twice is one pointer: segld's load of
/// `a(i)` went through a second cast pair, so no store was seen to reach it.
#[test]
fn a_pointer_cast_twice_is_one_value() {
    let text = format!(
        "{}@d = internal global i16 0

define i16 @f(i16 %v) {{
b0:
  %s = load i16, ptr @d
  %p1 = inttoptr i16 %s to ptr addrspace(2)
  %f1 = addrspacecast ptr addrspace(2) %p1 to ptr addrspace(1)
  store i16 %v, ptr addrspace(1) %f1
  %p2 = inttoptr i16 %s to ptr addrspace(2)
  %f2 = addrspacecast ptr addrspace(2) %p2 to ptr addrspace(1)
  %x = load i16, ptr addrspace(1) %f2
  ret i16 %x
}}
",
        llrm_analysis::testing::DOS
    );
    let mut module = parsed(&text);
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.require::<Summaries>();
    manager.add(Gvn::default());
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
    let after = printed(&module);
    // One cast, and the load reads what the store wrote.
    assert!(after.matches("inttoptr").count() == 1 && !after.contains("load i16, ptr addrspace(1)"), "{after}");
}

/// A load of what the language says is written once and never again is the
/// value an earlier load of it read, whatever a call between may write.
#[test]
fn a_load_the_language_says_is_invariant_is_reused_across_a_call_that_may_write() {
    let gvn = |second: &str| {
        let mut module = parsed(&format!(
            "@g = global i16 0

declare void @poke()

define i16 @f() {{
b0:
  %a = load i16, ptr @g
  call void @poke()
  %b = {second}
  %r = add i16 %a, %b
  ret i16 %r
}}

!0 = !{{}}
"
        ));
        let mut manager = PassManager::default();
        manager.require::<Summaries>();
        manager.add(Gvn::default());
        manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
        printed(&module)
    };
    assert!(gvn("load i16, ptr @g").contains("  %r = add i16 %a, %b\n"), "a plain load is read again");
    let stated = gvn("load i16, ptr @g, !invariant.load !0");
    assert!(stated.contains("  %r = add i16 %a, %a\n"), "{stated}");
}

/// The program's global is read again after a store to a device: through a
/// pointer in the fixed-address space the store cannot change it, so the
/// second read is the first; through a far pointer it may, and stays.
#[test]
fn test_a_store_at_a_fixed_address_keeps_a_global_loaded_before() {
    for (space, reloaded) in [(4, false), (1, true)] {
        let text = format!(
            "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p4:32:16:16:16-i32:16-i64:16-n8:16:32\"

@g = global i16 0

define i16 @f(ptr addrspace({space}) %p) {{
b0:
  %a = load i16, ptr @g
  store i8 1, ptr addrspace({space}) %p
  %b = load i16, ptr @g
  %r = add i16 %a, %b
  ret i16 %r
}}
"
        );
        let mut module = parsed(&text);
        let mut manager = PassManager::default();
        manager.require::<Summaries>();
        manager.add(Gvn::default());
        manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
        let after = printed(&module);
        assert_eq!(after.matches("load i16, ptr @g").count() == 2, reloaded, "space {space}\n{after}");
    }
}

/// The device is read again after a store to the program's global: through
/// the fixed-address space the store cannot have changed it, so the second
/// read is the first; through a far pointer it may have, and stays.
#[test]
fn test_a_store_to_a_global_keeps_a_fixed_address_read_before() {
    for (space, reloaded) in [(4, false), (1, true)] {
        let text = format!(
            "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p4:32:16:16:16-i32:16-i64:16-n8:16:32\"

@g = global i16 0

define i16 @f(ptr addrspace({space}) %p) {{
b0:
  %a = load i16, ptr addrspace({space}) %p
  store i16 1, ptr @g
  %b = load i16, ptr addrspace({space}) %p
  %r = add i16 %a, %b
  ret i16 %r
}}
"
        );
        let mut module = parsed(&text);
        let mut manager = PassManager::default();
        manager.require::<Summaries>();
        manager.add(Gvn::default());
        manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
        let after = printed(&module);
        assert_eq!(
            after.matches(&format!("load i16, ptr addrspace({space})")).count() == 2,
            reloaded,
            "space {space}\n{after}"
        );
    }
}

/// Two pointers a constructor returns are apart: a store through one does
/// not change what the other holds, so a second read of the other is the
/// first. A callee that states its result `noalias` is a constructor; one that
/// does not may return the same object twice, and the read stays.
#[test]
fn test_a_noalias_result_is_apart_from_every_other_object() {
    for (attribute, reloaded) in [("noalias ", false), ("", true)] {
        let text = format!(
            "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16-n8:16:32\"

declare {attribute}ptr @make()

define i16 @f() {{
b0:
  %p = call ptr @make()
  %q = call ptr @make()
  store i16 1, ptr %p
  %a = load i16, ptr %q
  store i16 2, ptr %p
  %b = load i16, ptr %q
  %r = add i16 %a, %b
  ret i16 %r
}}
"
        );
        let mut module = parsed(&text);
        let mut manager = PassManager::default();
        manager.require::<Summaries>();
        manager.add(Gvn::default());
        manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
        let after = printed(&module);
        assert_eq!(after.matches("load i16, ptr %q").count() == 2, reloaded, "{attribute:?}\n{after}");
    }
}

/// `text` through `Gvn` with the module's summaries, printed.
fn numbered(text: &str) -> String {
    let mut module = parsed(text);
    let mut manager = PassManager::default();
    manager.require::<Summaries>();
    manager.add(Gvn::default());
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
    printed(&module)
}

/// A volatile store writes only the bytes it addresses, as LLVM's: @g is
/// read once across one to a fixed address or to another global, and again
/// across one through a far pointer that may be @g. It was a barrier, and
/// TEXTFILL read its variables again after every POKE (#257).
#[test]
fn test_a_volatile_store_elsewhere_keeps_a_global_loaded_before() {
    for (store, reloaded) in [("ptr addrspace(4) %dev", false), ("ptr @h", false), ("ptr addrspace(1) %far", true)] {
        let text = format!(
            "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p4:32:16:16:16-i32:16-i64:16-n8:16:32\"

@g = global i16 0
@h = global i8 0

define i16 @f(ptr addrspace(4) %dev, ptr addrspace(1) %far) {{
b0:
  %a = load i16, ptr @g
  store volatile i8 1, {store}
  %b = load i16, ptr @g
  %r = add i16 %a, %b
  ret i16 %r
}}
"
        );
        let after = numbered(&text);
        assert_eq!(after.matches("load i16, ptr @g").count() == 2, reloaded, "{store}\n{after}");
        assert!(after.contains(&format!("store volatile i8 1, {store}")), "{after}");
    }
}

/// Volatile accesses keep their number and order: two stores of one address
/// both stay, and a second volatile read is not the first's value.
#[test]
fn test_volatile_accesses_keep_their_order_and_count() {
    let text = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p4:32:16:16:16-i32:16-i64:16-n8:16:32\"

define i16 @f(ptr addrspace(4) %dev) {
b0:
  store volatile i8 1, ptr addrspace(4) %dev
  store volatile i8 2, ptr addrspace(4) %dev
  %a = load volatile i8, ptr addrspace(4) %dev
  %b = load volatile i8, ptr addrspace(4) %dev
  %c = sub i8 %a, %b
  %r = zext i8 %c to i16
  ret i16 %r
}
";
    let after = numbered(text);
    let volatile = after.lines().filter(|one| one.contains("volatile")).map(str::trim).collect::<Vec<_>>();
    assert_eq!(
        volatile,
        [
            "store volatile i8 1, ptr addrspace(4) %dev",
            "store volatile i8 2, ptr addrspace(4) %dev",
            "%a = load volatile i8, ptr addrspace(4) %dev",
            "%b = load volatile i8, ptr addrspace(4) %dev"
        ],
        "{after}"
    );
}

/// Each ask of a unit for its shape derived dominance and loops of the body
/// again: 20 sequential loops under gvn took 17 derivations, 5 now. The pass
/// asks the manager for it once.
#[test]
fn a_pass_derives_the_shape_of_its_body_once_however_many_loops_it_has() {
    let loops = 20;
    let mut text = String::from("@y = global i16 0\n\ndefine i16 @f(i16 %n) {\nb0:\n  br label %h0\n\n");
    for at in 0..loops {
        let next = if at + 1 == loops { "end".to_owned() } else { format!("h{}", at + 1) };
        let from = if at == 0 { "b0".to_owned() } else { format!("h{}", at - 1) };
        text += &format!(
            "h{at}:\n  %i{at} = phi i16 [ 0, %{from} ], [ %n{at}, %l{at} ]\n  %c{at} = icmp slt i16 %i{at}, %n\n  br i1 %c{at}, label %l{at}, label %{next}\n\nl{at}:\n  %v{at} = load i16, ptr @y\n  store i16 %i{at}, ptr @y\n  %n{at} = add i16 %i{at}, 1\n  br label %h{at}\n\n"
        );
    }
    text += "end:\n  ret i16 %n\n}\n";
    let mut module = parsed(&text);
    let before = llrm_analysis::cfg::shapes_derived();
    let mut manager = PassManager::default();
    manager.require::<Summaries>();
    manager.add(Gvn::default());
    manager
        .run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned { registers: 4, ..Default::default() }))
        .unwrap();
    let derived = llrm_analysis::cfg::shapes_derived() - before;
    assert!(derived <= 8, "{derived} shapes derived for one pass over a body of {loops} loops");
}

/// Gvn priced the loops from trip counts it proved for itself, which
/// `Annotated` had proved already for the same body: 20 sequential loops were
/// proved 60 times, 20 now. The counts are the manager's, proved once.
#[test]
fn a_pass_takes_the_trip_counts_the_manager_proved() {
    // The check proves them again to compare, and is counted.
    if std::env::var_os("LLRM_CHECK_COUNTED").is_some() {
        return;
    }
    let loops = 20;
    let mut text = String::from("@y = global i16 0\n\ndefine i16 @f(i16 %n) {\nb0:\n  br label %h0\n\n");
    for at in 0..loops {
        let next = if at + 1 == loops { "end".to_owned() } else { format!("h{}", at + 1) };
        let from = if at == 0 { "b0".to_owned() } else { format!("h{}", at - 1) };
        text += &format!(
            "h{at}:\n  %i{at} = phi i16 [ 0, %{from} ], [ %n{at}, %l{at} ]\n  %c{at} = icmp slt i16 %i{at}, 9\n  br i1 %c{at}, label %l{at}, label %{next}\n\nl{at}:\n  %v{at} = load i16, ptr @y\n  store i16 %i{at}, ptr @y\n  %n{at} = add i16 %i{at}, 1\n  br label %h{at}\n\n"
        );
    }
    text += "end:\n  ret i16 %n\n}\n";
    let mut module = parsed(&text);
    let before = llrm_analysis::induction::proved();
    let mut manager = PassManager::default();
    manager.require::<Summaries>();
    manager.add(Gvn::default());
    manager
        .run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned { registers: 4, ..Default::default() }))
        .unwrap();
    let proved = llrm_analysis::induction::proved() - before;
    assert!(proved <= loops, "{proved} loops proved for one pass over a body of {loops} loops");
}

/// A function numbered twice walks once (the walk does not depend on
/// `avoid_store_crossing`).
#[test]
fn a_function_numbered_twice_walks_once() {
    let text = "@x = global i16 0
@y = global i16 0

define i16 @f(i16 %p) {
b0:
  %a = load i16, ptr @x
  store i16 %p, ptr @y
  %b = load i16, ptr @x
  %r = add i16 %a, %b
  ret i16 %r
}
";
    let mut module = parsed(text);
    let mut manager = PassManager::default();
    manager.require::<Summaries>();
    manager.add(Gvn::default());
    let (walked, numbered) = (llrm_analysis::avail::walked(), super::numberings());
    manager
        .run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned { registers: 1, ..Default::default() }))
        .unwrap();
    assert_eq!(super::numberings() - numbered, 2, "the function is numbered both ways");
    assert_eq!(llrm_analysis::avail::walked() - walked, 1, "the walk ran again for the second numbering");
}

/// The walk forwards a stored value to its load.
#[test]
fn the_walk_alone_forwards_a_stored_value_to_its_load() {
    let text = "@g = global i16 0

define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  store i16 %x, ptr @g
  %r = load i16, ptr @g
  ret i16 %r
}
";
    let before = parsed(text);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.require::<Summaries>();
    manager.add(Gvn::default());
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
    assert!(printed(&module).contains("  store i16 %x, ptr @g\n  ret i16 %x\n"), "{}", printed(&module));
}

/// A function numbered both ways (crossing stores and not) and priced twice,
/// 4067 times over QCport and the programs at -O2: 72% of them priced alike and
/// the second won 2.3%. The ways differ only where a load is served across a
/// store; where none is, the second numbering is the first and is neither made
/// nor priced.
#[test]
fn test_a_function_with_no_load_served_across_a_store_is_numbered_once() {
    let numberings = |text: &str, registers: i64| {
        let mut module = parsed(text);
        let mut manager = PassManager::default();
        manager.require::<Summaries>();
        manager.add(Gvn::default());
        let before = super::numberings();
        manager
            .run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned { registers, ..Default::default() }))
            .unwrap();
        super::numberings() - before
    };
    let plain = "@x = global i16 0
@z = global i16 0

define i16 @f(i16 %p) {
b0:
  %a = load i16, ptr @x
  %b = load i16, ptr @x
  %r = add i16 %a, %b
  store i16 %r, ptr @z
  ret i16 %r
}
";
    let across = "@x = global i16 0
@y = global i16 0

define i16 @f(i16 %p) {
b0:
  %a = load i16, ptr @x
  store i16 %p, ptr @y
  %b = load i16, ptr @x
  %r = add i16 %a, %b
  ret i16 %r
}
";
    assert_eq!(numberings(plain, 6), 1, "the second numbering is the first");
    assert_eq!(numberings(across, 1), 2, "a load served across a store is numbered both ways where crossing spills");
}

/// Crossing the store is dearer only through the registers it holds a value in:
/// where the function fits its registers everywhere, the careful numbering
/// (which keeps the load) can only cost more work, and is not made or priced.
/// The forecast it needed was 40% of gvn on a 2048-statement function.
#[test]
fn a_load_served_across_a_store_is_numbered_once_where_crossing_spills_nothing() {
    let across = "@x = global i16 0
@y = global i16 0

define i16 @f(i16 %p) {
b0:
  %a = load i16, ptr @x
  store i16 %p, ptr @y
  %b = load i16, ptr @x
  %r = add i16 %a, %b
  ret i16 %r
}
";
    let mut module = parsed(across);
    let mut manager = PassManager::default();
    manager.require::<Summaries>();
    manager.add(Gvn::default());
    let before = super::numberings();
    manager
        .run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned { registers: 6, ..Default::default() }))
        .unwrap();
    assert_eq!(super::numberings() - before, 1);
}

/// The three kinds of load the deleted availability map forwarded and the
/// MemorySSA walk first did not: each is pinned here so a loss is seen, not
/// measured later.
///
/// (a) A join whose arms store the same operand, the load below the join.
#[test]
fn a_load_below_a_join_whose_arms_stored_the_same_value_takes_it() {
    let text = "@g = global i16 0

define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 %x, ptr @g
  br label %b3

b2:
  store i16 %x, ptr @g
  br label %b3

b3:
  br label %b4

b4:
  %r = load i16, ptr @g
  ret i16 %r
}
";
    let after = managed(text);
    assert!(!after.contains("load i16"), "the load survives: {after}");
}

/// (b) A store through an index a register fact places away from the cell.
#[test]
fn a_load_is_served_across_a_store_whose_index_a_constant_places_away() {
    let text = "@g = global [8 x i16] zeroinitializer

define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  store i16 %x, ptr @g
  %k = and i16 12, 6
  %q = getelementptr i16, ptr @g, i16 %k
  store i16 %y, ptr %q
  %r = load i16, ptr @g
  ret i16 %r
}
";
    let after = managed(text);
    assert!(!after.contains("load i16"), "the load survives: {after}");
}

/// (c) A load marked invariant, after the store that initialises what it reads.
#[test]
fn an_invariant_load_takes_the_value_stored_before_it() {
    let text = "@g = global i16 0

define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  store i16 %x, ptr @g
  %r = load i16, ptr @g, !invariant.load !0
  ret i16 %r
}

!0 = !{}
";
    let after = managed(text);
    assert!(!after.contains("load i16"), "the load survives: {after}");
}

/// Two calls with the same arguments to a function that touches no memory are
/// one call, as LLVM's GVN numbers a call that `doesNotAccessMemory` and gcc's
/// FRE a const function's. A callee that reads or writes, an argument that
/// differs, or another callee keeps both. hanoi's `hanoi(n - 1) + 1 +
/// hanoi(n - 1)` was two recursions, 2^13 calls for what one makes.
#[test]
fn test_equal_calls_to_a_function_that_touches_no_memory_are_one() {
    let calls = |attrs: &str, second: &str, callee: &str| {
        numbered(&format!(
            "@g = global i16 0

define i16 @h(i16 %n) {attrs} {{
b0:
  %m = mul i16 %n, 3
  ret i16 %m
}}

define i16 @k(i16 %n) {attrs} {{
b0:
  %m = mul i16 %n, 5
  ret i16 %m
}}

define i16 @f(i16 %x, i16 %y) {{
b0:
  %a = call i16 @h(i16 %x)
  %b = call i16 @{callee}(i16 {second})
  %r = add i16 %a, %b
  ret i16 %r
}}
"
        ))
    };
    let count = |text: &str| text.lines().filter(|line| line.contains("call i16 @")).count();
    assert_eq!(count(&calls("memory(none)", "%x", "h")), 1);
    assert_eq!(count(&calls("memory(none)", "%y", "h")), 2, "another argument is another call");
    assert_eq!(count(&calls("memory(none)", "%x", "k")), 2, "another callee is another call");
    assert_eq!(count(&calls("memory(read)", "%x", "h")), 2, "a callee that reads is not numbered here");
    assert_eq!(count(&calls("", "%x", "h")), 2, "nor one that says nothing");
}

/// Equal calls are merged before `tailrec` runs: it makes the second of two
/// calls a loop's trip, and then the first has no equal to merge with.
/// `hanoi(n - 1) + 1 + hanoi(n - 1)` ran as a nest of loops and 8,191 calls'
/// work, 143,747 clocks; one call, `2 * h + 1`, is 103.
#[test]
fn test_two_equal_calls_are_one_before_tail_recursion_makes_a_loop_of_one() {
    use super::EqualCalls;
    use crate::tailrec::TailRecursion;
    let text = "define internal i32 @h(i32 %n) memory(none) {
b0:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %b1, label %b2
b1:
  ret i32 0
b2:
  %m = sub i32 %n, 1
  %a = call i32 @h(i32 %m)
  %p = add i32 %a, 1
  %b = call i32 @h(i32 %m)
  %q = add i32 %p, %b
  ret i32 %q
}
";
    let run = |merge: bool| {
        let mut module = parsed(text);
        let mut manager = PassManager::default();
        manager.require::<Summaries>();
        if merge {
            manager.add(EqualCalls);
        }
        manager.add(TailRecursion);
        manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
        printed(&module)
    };
    let calls = |text: &str| text.lines().filter(|line| line.contains("call i32 @h")).count();
    let (merged, looped) = (run(true), run(false));
    assert!(looped.contains("tailrecurse"), "premise: tail recursion makes the second call a loop\n{looped}");
    assert_eq!((calls(&merged), merged.contains("tailrecurse")), (1, false), "{merged}");
}

/// -O1's `-ftree-fre`: a load a dominating load or store already holds is
/// served on every path, but nothing is inserted to make a partial redundancy
/// full. Before, -O1 had no gvn at all (it was gated with `-fgcse`), and the
/// swap of a sift reloaded the two cells its compare had just loaded.
#[test]
fn without_pre_a_load_a_dominator_holds_is_still_served() {
    let reloaded = "define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  %first = load i16, ptr @g
  br i1 %c, label %b1, label %b2

b1:
  %second = load i16, ptr @g
  br label %b3

b2:
  br label %b3

b3:
  %r = add i16 %first, 1
  ret i16 %r
}
";
    let after = managed_with(
        &format!("@g = global i16 0\n\n{}", reloaded.replace("%r = add i16 %first, 1", "%r = load i16, ptr @g")),
        Gvn { pre: false },
    );
    assert!(!after.contains("%r = load"), "{after}");
    assert!(!after.contains("%second = load"), "{after}");
}

#[test]
fn without_pre_a_partial_redundancy_is_left_and_with_pre_it_is_made_full() {
    let text = diamond("  %a = xor i16 %x, %y\n", "", "  %r = xor i16 %x, %y\n");
    assert_eq!(managed_with(&text, Gvn { pre: false }), printed(&parsed(&text)));
    assert!(managed_with(&text, Gvn { pre: true }).contains("%r.pre = xor i16 %x, %y"));
}

/// A block that dominates another but stands after it in the layout (a join
/// removed, as `decide` does) was met second, so its computation never served
/// the one it dominates: shellsort -O2 kept `sub`, its compare and a load
/// (4 loads, 3 wanted). The dominating computation is met first.
#[test]
fn test_gvn_serves_a_computation_from_a_dominator_laid_out_after_it() {
    let text = "define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  br i1 %c, label %b1, label %b3

b2:
  %b = sub i16 %x, %y
  ret i16 %b

b1:
  %a = sub i16 %x, %y
  %u = add i16 %a, 1
  br label %b2

b3:
  ret i16 0
}
";
    let out = managed(text);
    assert_eq!(out.matches("sub i16").count(), 1, "{out}");
}

/// The same for calls: an equal call in a dominator laid out after the one it
/// dominates served nothing, as `sub` above.
#[test]
fn test_equal_calls_are_one_when_the_dominator_is_laid_out_after() {
    let text = numbered(
        "define i16 @h(i16 %n) memory(none) {
b0:
  %m = mul i16 %n, 3
  ret i16 %m
}

define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b3

b2:
  %b = call i16 @h(i16 %x)
  ret i16 %b

b1:
  %a = call i16 @h(i16 %x)
  %u = add i16 %a, 1
  br label %b2

b3:
  ret i16 0
}
",
    );
    assert_eq!(text.lines().filter(|line| line.contains("call i16 @")).count(), 1, "{text}");
}

/// A pointer a loop's copy is told is restrict (`llrm.noalias`) is apart from
/// another so told: a store through one does not change what the other holds,
/// so a second read of it is the first. Told of one only, the read stays.
#[test]
fn a_restrict_pointer_is_apart_from_another_the_pass_made_restrict() {
    for (both, reloaded) in [(true, false), (false, true)] {
        let second = if both { "call ptr @llrm.noalias.p0(ptr %q)" } else { "getelementptr i8, ptr %q, i16 0" };
        let text = format!(
            "declare ptr @llrm.noalias.p0(ptr)

define i16 @f(ptr %p, ptr %q) {{
b0:
  %pp = call ptr @llrm.noalias.p0(ptr %p)
  %qq = {second}
  %a = load i16, ptr %qq
  store i16 2, ptr %pp
  %b = load i16, ptr %qq
  %r = add i16 %a, %b
  ret i16 %r
}}
"
        );
        let after = numbered(&text);
        // Told of one only, `%qq` is `%q` itself: the zero GEP is simplified.
        let read = if both { "load i16, ptr %qq" } else { "load i16, ptr %q" };
        assert_eq!(after.matches(read).count() == 2, reloaded, "{both}\n{after}");
    }
}

/// One register short of room for the reload's provider, crossing the store
/// spills and the careful numbering is made and priced. Its price cannot be
/// less than its work, yet it was forecast for spills (a whole-function
/// forecast; skipping it takes 61 M off the 3,540 M d_faces -O2 compile) though
/// that work alone is already past what crossing costs. The answer is the same,
/// and one forecast is made, not two.
#[test]
fn a_numbering_dearer_in_work_than_the_crossing_one_is_not_forecast_for_spills() {
    let text = "@x = global i16 0\n@y = global i16 0\n@z = global i16 0\n\ndefine i16 @f(i16 %n, i16 %p) {\nb0:\n  %a = load i16, ptr @x\n  store i16 %a, ptr @z\n  br label %b1\n\nb1:\n  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]\n  %more = icmp slt i16 %i, %n\n  br i1 %more, label %b2, label %b3\n\nb2:\n  %t1 = mul i16 %i, 3\n  %t2 = mul i16 %i, 5\n  %t3 = add i16 %t1, %t2\n  store i16 %t3, ptr @y\n  %b = load i16, ptr @x\n  store i16 %b, ptr @z\n  %i.next = add i16 %i, 1\n  br label %b1\n\nb3:\n  %q = mul i16 %p, %p\n  %r = add i16 %q, %p\n  ret i16 %r\n}\n";
    let mut module = parsed(text);
    let mut manager = PassManager::default();
    manager.require::<Summaries>();
    manager.add(Gvn::default());
    crate::profit::PRICED.with(|count| count.set((0, 0)));
    manager
        .run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned { registers: 5, ..Default::default() }))
        .unwrap();
    let (_, forecasts) = crate::profit::PRICED.with(std::cell::Cell::get);
    assert!(!printed(&module).contains("%b = load i16, ptr @x"), "the reload is served");
    assert_eq!(forecasts, 1, "{forecasts} whole-function forecasts for the two numberings");
}

/// Two loads of one cell are one load once the pass has run, and the two
/// products of them one product: the scalar pass runs again over what load
/// numbering merged. Run once, before it, the products stayed two, and a later
/// pass saw no square (x_primes -O2 +4% clocks).
#[test]
fn a_product_of_two_loads_of_one_cell_is_one_with_a_product_of_the_same() {
    let text = "define i16 @f(ptr %p) {
b0:
  %a = load i16, ptr %p
  %b = load i16, ptr %p
  %m = mul i16 %a, %b
  %c = load i16, ptr %p
  %d = load i16, ptr %p
  %n = mul i16 %c, %d
  %r = sub i16 %m, %n
  ret i16 %r
}
";
    let mut module = parsed(text);
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.require::<Summaries>();
    manager.add(Gvn::default());
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
    let after = printed(&module);
    assert_eq!(after.matches("mul i16").count(), 1, "{after}");
}
