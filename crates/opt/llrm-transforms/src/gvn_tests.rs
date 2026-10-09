//! `joined`'s PRE and the `Gvn` pass, each body MIR text run by llrm-mir's
//! interpreter before and after.

use llrm_analysis::manager::Summaries;
use llrm_mir::passes::PassManager;

use crate::testing::{f, parsed, printed, results};

use super::{Gvn, joined};

const INPUTS: &[&[i128]] = &[&[0, 0, 0], &[3, 5, 1], &[-7, 2, 0], &[0x7fff, 1, 1]];

/// A diamond on %c whose arms are `left` and `right`, joining at b3 to
/// compute `join` into %r.
fn diamond(left: &str, right: &str, join: &str) -> String {
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
fn joined_once(text: &str, insert: bool) -> (String, bool) {
    let before = parsed(text);
    let mut module = before.clone();
    let changed = joined(f(&mut module), insert).unwrap();
    let text = printed(&module);
    assert_eq!(results(&module, INPUTS), results(&before, INPUTS), "{text}");
    (text, changed)
}

fn kept(text: &str, insert: bool) {
    let (after, changed) = joined_once(text, insert);
    assert!(!changed);
    assert_eq!(after, printed(&parsed(text)));
}

#[test]
fn test_providers_on_every_edge_become_a_phi() {
    let (text, changed) = joined_once(&diamond("  %a = add i16 %x, %y\n", "  %b = add i16 %y, %x\n", "  %r = add i16 %x, %y\n"), false);
    assert!(changed);
    assert!(text.ends_with("b3:\n  %r.pre-phi = phi i16 [ %a, %b1 ], [ %b, %b2 ]\n  ret i16 %r.pre-phi\n}\n"), "{text}");
}

#[test]
fn test_a_join_expression_translates_through_its_phis() {
    let (text, changed) = joined_once(
        &diamond("  %a = mul i16 %x, 3\n", "  %b = mul i16 %y, 3\n", "  %p = phi i16 [ %x, %b1 ], [ %y, %b2 ]\n  %r = mul i16 %p, 3\n"),
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
    let before = parsed(text);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.verify_invalidation = true;
    manager.require::<Summaries>();
    manager.add(Gvn::default());
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
    let text = format!("@g = global i16 0\n\n{}", diamond("  store i16 %x, ptr @g\n", "  store i16 %y, ptr @g\n", "  %r = load i16, ptr @g\n"));
    assert!(managed(&text).contains("b3:\n  %r1 = phi i16 [ %x, %b1 ], [ %y, %b2 ]\n  ret i16 %r1\n"), "{}", managed(&text));
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
        manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned { registers, ..Default::default() })).unwrap();
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
    let module = crate::testing::parsed(&format!("{}{}{}", llrm_analysis::testing::DOS, crate::testing::WRITES_ITS_ARGUMENT, "define i16 @f() {\nb0:\n  %a = load i16, ptr @g\n  call void @h(ptr @k)\n  %b = load i16, ptr @g\n  %r = add i16 %a, %b\n  ret i16 %r\n}\n"));
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
    assert!(text.contains("%y = zext i8 7 to i16"), "{text}");
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
    assert!(after.contains("br i1 true, label %b2, label %b3") && after.contains("select i1 true") && after.contains("zext i1 false"), "{after}");
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
    assert!(after.matches("inttoptr").count() == 1 && after.contains("load i16, ptr addrspace(1) %f1"), "{after}");
}

/// A load of what the language says is written once and never again is the value an earlier
/// load of it read, whatever a call between may write.
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
        manager.add(Gvn);
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
        manager.add(Gvn);
        manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
        let after = printed(&module);
        assert_eq!(after.matches(&format!("load i16, ptr addrspace({space})")).count() == 2, reloaded, "space {space}\n{after}");
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
        manager.add(Gvn);
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
    manager.add(Gvn);
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

/// Each ask of a unit for its shape derived dominance and loops of the body again: 20 sequential loops under gvn took
/// 17 derivations, 5 now. The pass asks the manager for it once.
#[test]
fn a_pass_derives_the_shape_of_its_body_once_however_many_loops_it_has() {
    let loops = 20;
    let mut text = String::from("@y = global i16 0\n\ndefine i16 @f(i16 %n) {\nb0:\n  br label %h0\n\n");
    for at in 0..loops {
        let next = if at + 1 == loops { "end".to_owned() } else { format!("h{}", at + 1) };
        let from = if at == 0 { "b0".to_owned() } else { format!("h{}", at - 1) };
        text += &format!("h{at}:\n  %i{at} = phi i16 [ 0, %{from} ], [ %n{at}, %l{at} ]\n  %c{at} = icmp slt i16 %i{at}, %n\n  br i1 %c{at}, label %l{at}, label %{next}\n\nl{at}:\n  %v{at} = load i16, ptr @y\n  store i16 %i{at}, ptr @y\n  %n{at} = add i16 %i{at}, 1\n  br label %h{at}\n\n");
    }
    text += "end:\n  ret i16 %n\n}\n";
    let mut module = parsed(&text);
    let before = llrm_analysis::cfg::shapes_derived();
    let mut manager = PassManager::default();
    manager.require::<Summaries>();
    manager.add(Gvn::default());
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned { registers: 4, ..Default::default() })).unwrap();
    let derived = llrm_analysis::cfg::shapes_derived() - before;
    assert!(derived <= 8, "{derived} shapes derived for one pass over a body of {loops} loops");
}

/// Gvn priced the loops from trip counts it proved for itself, which `Annotated` had proved already for the same body: 20
/// sequential loops were proved 60 times, 20 now. The counts are the manager's, proved once.
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
        text += &format!("h{at}:\n  %i{at} = phi i16 [ 0, %{from} ], [ %n{at}, %l{at} ]\n  %c{at} = icmp slt i16 %i{at}, 9\n  br i1 %c{at}, label %l{at}, label %{next}\n\nl{at}:\n  %v{at} = load i16, ptr @y\n  store i16 %i{at}, ptr @y\n  %n{at} = add i16 %i{at}, 1\n  br label %h{at}\n\n");
    }
    text += "end:\n  ret i16 %n\n}\n";
    let mut module = parsed(&text);
    let before = llrm_analysis::induction::proved();
    let mut manager = PassManager::default();
    manager.require::<Summaries>();
    manager.add(Gvn::default());
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned { registers: 4, ..Default::default() })).unwrap();
    let proved = llrm_analysis::induction::proved() - before;
    assert!(proved <= loops, "{proved} loops proved for one pass over a body of {loops} loops");
}

/// Where the machine prices registers, Gvn numbers the function twice (crossing stores, and not) and keeps the cheaper; each run solved
/// what every block holds again, for the same instructions: 638 solutions for 343 runs compiling `mdl_ai.c`, 12.7% of its compile. It
/// is solved once for the function as it comes in.
#[test]
fn test_availability_is_solved_once_when_a_function_is_numbered_twice() {
    let text = "@x = global i16 0
@y = global i16 0
@z = global i16 0

define i16 @f(i16 %n, i16 %p) {
b0:
  %a = load i16, ptr @x
  store i16 %a, ptr @z
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  %more = icmp slt i16 %i, %n
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
}
";
    let mut module = parsed(text);
    let mut manager = PassManager::default();
    manager.require::<Summaries>();
    manager.add(Gvn::default());
    let before = llrm_analysis::avail::solved();
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned { registers: 6, ..Default::default() })).unwrap();
    assert_eq!(llrm_analysis::avail::solved() - before, 1, "availability solved again for the second numbering");
}

/// A function numbered both ways (crossing stores and not) and priced twice, 4067 times over QCport and the programs at -O2: 72% of
/// them priced alike and the second won 2.3%. The ways differ only where a load is served across a store; where none is, the second
/// numbering is the first and is neither made nor priced.
#[test]
fn test_a_function_with_no_load_served_across_a_store_is_numbered_once() {
    let numberings = |text: &str| {
        let mut module = parsed(text);
        let mut manager = PassManager::default();
        manager.require::<Summaries>();
        manager.add(Gvn::default());
        let before = super::numberings();
        manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned { registers: 6, ..Default::default() })).unwrap();
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
    assert_eq!(numberings(plain), 1, "the second numbering is the first");
    assert_eq!(numberings(across), 2, "a load served across a store is numbered both ways");
}
