//! `Local.cpp`'s utilities for GVN, each run on MIR text; a change is also run
//! by the interpreter before and after.

use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::memory;
use llrm_mir::module::{BlockId, InstId, Module, Operand, Use, ValueId};
use llrm_mir::types::Type;
use llrm_mir::valuetracking;

use super::local::*;
use crate::testing::{parsed, printed, results};

/// The instruction of @f whose result is `%name`.
fn inst(
    module: &Module,
    name: &str,
) -> InstId {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    function
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| {
            function.instruction(inst).result.is_some_and(|value| function.value(value).name.as_deref() == Some(name))
        })
        .unwrap_or_else(|| panic!("%{name}"))
}

fn value(
    module: &Module,
    name: &str,
) -> ValueId {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    match function.parameters().iter().find(|&&one| function.value(one).name.as_deref() == Some(name)) {
        Some(&parameter) => parameter,
        None => function.instruction(inst(module, name)).result.expect("a result"),
    }
}

/// @f of the printed module, from `define` on.
fn body(module: &Module) -> String {
    let text = printed(module);
    let at = text.find("define").expect("a function");
    let end = text[at..].find("\n}\n").map_or(text.len(), |end| at + end + 3);
    text[at..end].to_owned()
}

/// The lines of `text` that hold `needle`, trimmed: what a test asserts on.
fn lines(
    text: &str,
    needle: &str,
) -> Vec<String> {
    text.lines().filter(|line| line.contains(needle)).map(|line| line.trim().to_owned()).collect()
}

/// `inst` replaced by `repl` the way GVN does it, and erased.
fn patched(
    module: &mut Module,
    inst_name: &str,
    repl_name: &str,
) {
    let declarations = module.declarations();
    let metadata = module.metadata.clone();
    let (inst, repl) = (inst(module, inst_name), Operand::Value(value(module, repl_name)));
    let (context, function) = module.function_mut("f").expect("@f");
    patch_and_replace_all_uses_with(context, &declarations, &metadata, function, inst, repl);
    function.erase(inst).expect("unused after the replacement");
}

// ---- patchReplacementInstruction

/// The replacement keeps the flags both instructions had: `%a` was `nsw nuw`,
/// `%b` `nsw`, and `%a` stood in for `%b` still claiming no unsigned wrap.
#[test]
fn a_replacement_keeps_only_the_flags_both_had() {
    let before = parsed(
        "define i16 @f(i16 %x) {
b0:
  %a = add nsw nuw i16 %x, 1
  %b = add nsw i16 %x, 1
  %r = mul i16 %a, %b
  ret i16 %r
}
",
    );
    let mut after = before.clone();
    patched(&mut after, "b", "a");
    let text = body(&after);
    assert_eq!(lines(&text, "%a ="), ["%a = add nsw i16 %x, 1"], "{text}");
    assert_eq!(results(&after, &[&[3], &[-7]]), results(&before, &[&[3], &[-7]]));
}

/// A flag the replaced instruction lacks leaves the replacement without it.
#[test]
fn a_replacement_loses_a_flag_the_replaced_instruction_lacked() {
    let mut module = parsed(
        "define i16 @f(i16 %x) {
b0:
  %a = add nsw nuw i16 %x, 1
  %b = add i16 %x, 1
  %r = mul i16 %a, %b
  ret i16 %r
}
",
    );
    patched(&mut module, "b", "a");
    assert_eq!(lines(&body(&module), "%a ="), ["%a = add i16 %x, 1"]);
}

/// A flag only one class has is not touched by an instruction of another:
/// an `add nsw` replacing an `sdiv exact` keeps `nsw`.
#[test]
fn a_flag_of_another_class_is_left_alone() {
    let mut module = parsed(
        "define i16 @f(i16 %x) {
b0:
  %a = add nsw i16 %x, 2
  %b = sdiv exact i16 %x, 2
  %r = sub i16 %a, %b
  ret i16 %r
}
",
    );
    let before = body(&module);
    patched(&mut module, "b", "a");
    assert_eq!(lines(&body(&module), "%a ="), lines(&before, "%a ="));
}

/// A load replaced by an arithmetic operation does not take the operation's
/// flags away: they say nothing of the load.
#[test]
fn a_replaced_load_leaves_the_replacements_flags() {
    let before = parsed(
        "@g = global i16 0

define i16 @f(i16 %v) {
b0:
  store i16 %v, ptr @g
  %s = add nsw i16 %v, 0
  %l = load i16, ptr @g
  %r = mul i16 %s, %l
  ret i16 %r
}
",
    );
    let mut after = before.clone();
    patched(&mut after, "l", "s");
    assert_eq!(lines(&body(&after), "%s ="), ["%s = add nsw i16 %v, 0"]);
    assert_eq!(results(&after, &[&[3], &[-7]]), results(&before, &[&[3], &[-7]]));
}

/// The value of `llvm.sadd.with.overflow` wraps: an `add nsw nuw` that stands
/// for it lost both flags, or `32767 + 1` is poison where it was `-32768`.
#[test]
fn a_with_overflow_value_replaced_by_an_add_drops_its_wrap_flags() {
    let before = parsed(
        "declare { i16, i1 } @llvm.sadd.with.overflow.i16(i16, i16)

define i16 @f(i16 %x) {
b0:
  %o = call { i16, i1 } @llvm.sadd.with.overflow.i16(i16 %x, i16 1)
  %v = extractvalue { i16, i1 } %o, 0
  %s = add nsw nuw i16 %x, 1
  %r = add i16 %v, %x
  ret i16 %r
}
",
    );
    let mut after = before.clone();
    patched(&mut after, "v", "s");
    assert_eq!(lines(&body(&after), "%s ="), ["%s = add i16 %x, 1"]);
    assert_eq!(results(&after, &[&[32767]]), results(&before, &[&[32767]]));
}

/// The call attributes both calls have, and no more: `readnone` and `noundef`
/// on one call only are gone, `nounwind willreturn` stay.
#[test]
fn a_replacement_call_keeps_the_attributes_both_calls_have() {
    let mut module = parsed(
        "declare i16 @h(i16)

define i16 @f(i16 %x) {
b0:
  %a = call i16 @h(i16 noundef %x) readnone nounwind willreturn
  %b = call i16 @h(i16 %x) nounwind willreturn
  %r = add i16 %a, %b
  ret i16 %r
}
",
    );
    patched(&mut module, "b", "a");
    assert_eq!(lines(&body(&module), "%a ="), ["%a = call i16 @h(i16 %x) nounwind willreturn"]);
}

/// `dereferenceable` and `align` are the smaller of the two calls', and
/// `memory` the union of what they may do.
#[test]
fn a_replacement_call_takes_the_smaller_bounds_and_the_wider_memory() {
    let mut module = parsed(
        "declare i16 @h(ptr)

define i16 @f(ptr %p) {
b0:
  %a = call i16 @h(ptr align 4 dereferenceable(8) %p) memory(read)
  %b = call i16 @h(ptr align 2 dereferenceable(4) %p) memory(write)
  %r = add i16 %a, %b
  ret i16 %r
}
",
    );
    patched(&mut module, "b", "a");
    assert_eq!(
        lines(&body(&module), "%a ="),
        ["%a = call i16 @h(ptr align 2 dereferenceable(4) %p) memory(readwrite)"]
    );
}

/// A `memory` attribute only one call states is dropped: the other may do
/// anything.
#[test]
fn a_memory_attribute_only_one_call_states_is_dropped() {
    let mut module = parsed(
        "declare i16 @h(i16)

define i16 @f(i16 %x) {
b0:
  %a = call i16 @h(i16 %x) memory(none)
  %b = call i16 @h(i16 %x)
  %r = add i16 %a, %b
  ret i16 %r
}
",
    );
    patched(&mut module, "b", "a");
    assert_eq!(lines(&body(&module), "%a ="), ["%a = call i16 @h(i16 %x)"]);
}

/// An attribute that must be preserved (`noinline`) on one call only does not
/// intersect: the call is left as it was, and the answer is no.
#[test]
fn attributes_that_must_be_preserved_do_not_intersect() {
    let mut module = parsed(
        "declare i16 @h(i16)

define i16 @f(i16 %x) {
b0:
  %a = call i16 @h(i16 %x) noinline nounwind
  %b = call i16 @h(i16 %x) nounwind
  %r = add i16 %a, %b
  ret i16 %r
}
",
    );
    let (a, b) = (inst(&module, "a"), inst(&module, "b"));
    let before = body(&module);
    let (_, function) = module.function_mut("f").expect("@f");
    assert!(!try_intersect_attributes(function, a, b));
    assert_eq!(body(&module), before);
}

const LOADS: &str = "@g = global i16 0

define i16 @f() {
b0:
  %k = load i16, ptr @g, KMETA
  %j = load i16, ptr @g, JMETA
  %r = add i16 %k, %j
  ret i16 %r
}

!0 = !{i16 0, i16 10}
!1 = !{i16 0, i16 20}
!2 = !{!\"x\"}
!3 = !{!\"int\"}
!4 = !{!3, !3, i64 0}
!5 = !{!3, !3, i64 0}
!6 = !{}
";

/// The kinds of metadata on `%name` of @f.
fn kinds(
    module: &Module,
    name: &str,
) -> Vec<String> {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let mut kinds: Vec<String> =
        function.instruction(inst(module, name)).metadata.iter().map(|(kind, _)| kind.clone()).collect();
    kinds.sort();
    kinds
}

/// `%k` after `combine_metadata_for_cse` with `%j`.
fn combined(
    k: &str,
    j: &str,
    does_k_move: bool,
) -> Vec<String> {
    let mut module = parsed(&LOADS.replace("KMETA", k).replace("JMETA", j));
    let (k_inst, j_inst) = (inst(&module, "k"), inst(&module, "j"));
    let metadata = module.metadata.clone();
    let (_, function) = module.function_mut("f").expect("@f");
    combine_metadata_for_cse(&metadata, function, k_inst, j_inst, does_k_move);
    printed(&module);
    kinds(&module, "k")
}

/// A replacement that stays where it is keeps its alias metadata and loses a
/// range the replaced load did not state the same, and a kind LLVM does not
/// know.
#[test]
fn a_replacement_that_stays_loses_a_range_the_other_load_did_not_state() {
    assert_eq!(combined("!range !0, !tbaa !4, !foo !2", "!range !1, !tbaa !4", false), ["tbaa"]);
    assert_eq!(combined("!range !0, !tbaa !4", "!tbaa !4", false), ["tbaa"]);
    assert_eq!(combined("!range !0, !tbaa !4", "!range !0, !tbaa !4", false), ["range", "tbaa"]);
}

/// Two nodes with the same operands are one, as LLVM uniques them.
#[test]
fn equal_nodes_are_one() {
    assert_eq!(combined("!tbaa !4, !range !0", "!tbaa !5, !range !0", true), ["range", "tbaa"]);
    assert_eq!(combined("!tbaa !4", "!tbaa !3", true), Vec::<String>::new());
}

/// A replacement that moves keeps an alias tag only where the other load
/// states the same, and an invariant mark only where both do.
#[test]
fn a_replacement_that_moves_keeps_only_what_both_loads_state() {
    assert_eq!(
        combined("!tbaa !4, !invariant.load !6", "!tbaa !4, !invariant.load !6", true),
        ["invariant.load", "tbaa"]
    );
    assert_eq!(combined("!tbaa !4, !invariant.load !6", "!tbaa !4", true), ["tbaa"]);
    assert_eq!(combined("!tbaa !4", "!invariant.load !6", true), Vec::<String>::new());
    assert_eq!(combined("!tbaa !4", "!tbaa !4", false), ["tbaa"]);
}

/// A load that says its value is not undef (`!noundef`) keeps its range
/// where it does not move.
#[test]
fn a_noundef_replacement_that_stays_keeps_its_range() {
    assert_eq!(combined("!range !0, !noundef !6", "!tbaa !4", false), ["noundef", "range"]);
    assert_eq!(combined("!range !0, !noundef !6", "!tbaa !4", true), Vec::<String>::new());
}

// ---- the dominator tree queries and replaceDominatedUsesWith

fn tree_of(module: &Module) -> DomTree {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    DomTree::of(function)
}

fn block(
    module: &Module,
    name: &str,
) -> BlockId {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some(name)).expect("a block")
}

const BRANCH: &str = "define i16 @f(i16 %x, i16 %y) {
b0:
  %c = icmp eq i16 %x, %y
  br i1 %c, label %t, label %e

t:
  %a = add i16 %x, 1
  %b = add i16 %x, 2
  br label %u

u:
  %d = add i16 %x, 3
  br label %m

e:
  br label %m

m:
  %p = phi i16 [ %x, %u ], [ %x, %e ]
  %q = add i16 %p, %x
  ret i16 %q
}
";

/// Where a replacement of `%x` by `%y` starts.
#[derive(Clone, Copy)]
enum Root {
    Edge(&'static str, &'static str),
    Block(&'static str),
    Instruction(&'static str),
}

/// `text` with the uses of `%x` that `root` dominates replaced by `%y`, and
/// `only_adds` the uses that are an `add`: how many, and the lines that read
/// `%x` after. The interpreter finds the same results for `inputs`, all of
/// which have `%x == %y` where the root is entered.
fn replaced(
    text: &str,
    root: Root,
    only_adds: bool,
    inputs: &[&[i128]],
) -> (usize, Vec<String>) {
    let before = parsed(text);
    let mut after = before.clone();
    let tree = tree_of(&before);
    let (x, y) = (value(&before, "x"), Operand::Value(value(&before, "y")));
    let is_add = |function: &llrm_mir::module::Function, at: Use| {
        !only_adds || matches!(
            function.instruction(at.user).opcode,
            llrm_mir::opcode::Opcode::Binary(_)
        )
    };
    let (context, function) = after.function_mut("f").expect("@f");
    let count = match root {
        Root::Edge(start, end) => {
            let edge = BasicBlockEdge { start: block(&before, start), end: block(&before, end) };
            replace_dominated_uses_with_if_edge(context, function, x, y, &tree, edge, |f, at, _| is_add(f, at))
        }
        Root::Block(name) => {
            replace_dominated_uses_with_if_block(context, function, x, y, &tree, block(&before, name), |f, at, _| {
                is_add(f, at)
            })
        }
        Root::Instruction(name) => replace_dominated_uses_with_if_instruction(
            context,
            function,
            x,
            y,
            &tree,
            inst(&before, name),
            |f, at, _| is_add(f, at),
        ),
    };
    assert_eq!(results(&after, inputs), results(&before, inputs), "{}", body(&after));
    (count, lines(&body(&after), "%x").into_iter().filter(|line| !line.starts_with("define")).collect())
}

/// An edge into a block only it enters dominates the block and what that block
/// dominates: `%x` is `%y` in `t` and `u` and the phi arm from `u`, not in `e`
/// or the join.
#[test]
fn an_edge_replaces_the_uses_it_dominates() {
    let (count, left) = replaced(BRANCH, Root::Edge("b0", "t"), false, &[&[4, 4], &[9, 9]]);
    assert_eq!(count, 4);
    assert_eq!(left, ["%c = icmp eq i16 %x, %y", "%p = phi i16 [ %y, %u ], [ %x, %e ]", "%q = add i16 %p, %x"]);
}

/// A block dominates the uses in the blocks below it, not its own.
#[test]
fn a_block_replaces_the_uses_below_it_not_its_own() {
    let (count, left) = replaced(BRANCH, Root::Block("t"), false, &[&[4, 4]]);
    assert_eq!(count, 2);
    assert_eq!(
        left,
        [
            "%c = icmp eq i16 %x, %y",
            "%a = add i16 %x, 1",
            "%b = add i16 %x, 2",
            "%p = phi i16 [ %y, %u ], [ %x, %e ]",
            "%q = add i16 %p, %x"
        ]
    );
}

/// An instruction dominates the uses after it in its block and the blocks
/// below it, not itself or what comes before.
#[test]
fn an_instruction_replaces_the_uses_after_it() {
    let (count, left) = replaced(BRANCH, Root::Instruction("a"), false, &[&[4, 4]]);
    assert_eq!(count, 3);
    assert_eq!(
        left,
        ["%c = icmp eq i16 %x, %y", "%a = add i16 %x, 1", "%p = phi i16 [ %y, %u ], [ %x, %e ]", "%q = add i16 %p, %x"]
    );
}

/// The `If` forms also ask the caller: only the adds.
#[test]
fn the_if_forms_replace_only_what_the_caller_accepts() {
    let counts: Vec<usize> = [Root::Edge("b0", "t"), Root::Block("t"), Root::Instruction("a")]
        .into_iter()
        .map(|root| replaced(BRANCH, root, true, &[&[4, 4]]).0)
        .collect();
    assert_eq!(counts, [3, 1, 2]);
}

/// A phi's use is at the end of its incoming block: the arm from the block of
/// the root is dominated though the phi is in a block the root does not
/// dominate.
#[test]
fn a_phi_arm_is_read_at_the_end_of_its_incoming_block() {
    let text = "define i16 @f(i16 %x, i16 %y) {
b0:
  %c = icmp eq i16 %x, %y
  br i1 %c, label %t, label %e

t:
  %a = add i16 %x, 1
  br label %m

e:
  br label %m

m:
  %p = phi i16 [ %x, %t ], [ %x, %e ]
  ret i16 %p
}
";
    let (count, left) = replaced(text, Root::Instruction("a"), false, &[&[4, 4], &[4, 5]]);
    assert_eq!(count, 1);
    assert_eq!(left, ["%c = icmp eq i16 %x, %y", "%a = add i16 %x, 1", "%p = phi i16 [ %y, %t ], [ %x, %e ]"]);
}

/// The edge to a join is critical: the phi arm that comes in on it is the
/// edge's, and the rest of the join is not.
#[test]
fn a_critical_edge_replaces_only_its_phi_arm() {
    let text = "define i16 @f(i16 %x, i16 %y) {
b0:
  %c = icmp eq i16 %x, %y
  br i1 %c, label %m, label %t

t:
  %a = add i16 %x, 1
  br label %m

m:
  %p = phi i16 [ %x, %b0 ], [ %a, %t ]
  %q = add i16 %p, %x
  ret i16 %q
}
";
    let (count, left) = replaced(text, Root::Edge("b0", "m"), false, &[&[4, 4], &[4, 5]]);
    assert_eq!(count, 1);
    assert_eq!(
        left,
        ["%c = icmp eq i16 %x, %y", "%a = add i16 %x, 1", "%q = add i16 %p, %x"],
        "the phi arm from b0 is %y"
    );
}

/// Two edges from one block to a join dominate nothing there: neither is the
/// only way in.
#[test]
fn a_duplicate_edge_dominates_nothing() {
    let text = "define i16 @f(i16 %x, i16 %y) {
b0:
  %c = icmp eq i16 %x, %y
  br i1 %c, label %m, label %m

m:
  %q = add i16 %x, 1
  ret i16 %q
}
";
    let (count, left) = replaced(text, Root::Edge("b0", "m"), false, &[&[4, 4], &[4, 5]]);
    assert_eq!(count, 0);
    assert_eq!(left, ["%c = icmp eq i16 %x, %y", "%q = add i16 %x, 1"]);
}

/// A use in an unreachable block is dominated by anything, even a definition
/// after it in no path.
#[test]
fn an_unreachable_use_is_dominated() {
    let text = "define i16 @f(i16 %x, i16 %y) {
b0:
  %a = add i16 %x, 1
  ret i16 %a

dead:
  %z = add i16 %x, 5
  ret i16 %z
}
";
    let (count, left) = replaced(text, Root::Instruction("a"), false, &[&[4, 4]]);
    assert_eq!(count, 1);
    assert_eq!(left, ["%a = add i16 %x, 1"]);
}

/// An invoke's value is defined on the edge to its normal block: its root
/// reaches that block's uses, not the landing pad's.
#[test]
fn an_invoke_dominates_its_normal_block_not_its_handler() {
    let before = parsed(
        "declare i16 @h()
declare i32 @__gxx_personality_v0(...)

define i16 @f(i16 %x, i16 %y) personality ptr @__gxx_personality_v0 {
b0:
  %v = invoke i16 @h() to label %ok unwind label %pad

ok:
  %a = add i16 %x, 1
  ret i16 %a

pad:
  %lp = landingpad { ptr, i32 } cleanup
  %z = add i16 %x, 3
  resume { ptr, i32 } %lp
}
",
    );
    let mut after = before.clone();
    let tree = tree_of(&before);
    let (x, root) = (value(&before, "x"), inst(&before, "v"));
    let ty = before.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f").2.value(x).ty;
    let seven = after.context.int(ty, 7);
    let (context, function) = after.function_mut("f").expect("@f");
    let count = replace_dominated_uses_with_instruction(context, function, x, Operand::Constant(seven), &tree, root);
    let text = body(&after);
    assert_eq!(count, 1);
    assert_eq!(lines(&text, "%a ="), ["%a = add i16 7, 1"]);
    assert_eq!(lines(&text, "%z ="), ["%z = add i16 %x, 3"]);
}

// ---- isInstructionTriviallyDead

/// Whether each instruction of @f is trivially dead, in order.
fn dead_flags(text: &str) -> Vec<bool> {
    let module = parsed(text);
    let (callees, declarations) = (memory::callees(&module), module.declarations());
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    function
        .walk()
        .map(|(_, inst)| is_instruction_trivially_dead(&module.context, &callees, &declarations, function, inst))
        .collect()
}

/// An unused instruction with no side effect is dead; what is read, what
/// writes, a call that may not come back or throw, and a volatile read are not.
#[test]
fn an_unused_instruction_is_dead_unless_it_has_a_side_effect() {
    let dead = dead_flags(
        "@g = global i16 0
declare i16 @pure(i16) readnone nounwind willreturn
declare i16 @spins(i16) readnone nounwind
declare i16 @throws(i16) readnone willreturn
declare i16 @writes(i16) nounwind willreturn

define i16 @f(i16 %x) {
b0:
  %unused = add i16 %x, 1
  %used = add i16 %x, 2
  %load = load i16, ptr @g
  %volatile = load volatile i16, ptr @g
  store i16 %x, ptr @g
  %pure = call i16 @pure(i16 %x)
  %spins = call i16 @spins(i16 %x)
  %throws = call i16 @throws(i16 %x)
  %writes = call i16 @writes(i16 %x)
  ret i16 %used
}
",
    );
    assert_eq!(dead, [true, false, true, false, false, true, false, false, false, false]);
}

/// A lifetime marker is dead when only markers name its object; a marker of an
/// object something else reads or writes is not.
#[test]
fn a_lifetime_marker_is_dead_when_only_markers_name_the_object() {
    let dead = dead_flags(
        "declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)

define i16 @f(i16 %x) {
b0:
  %a = alloca i16
  call void @llvm.lifetime.start.p0(i64 2, ptr %a)
  call void @llvm.lifetime.end.p0(i64 2, ptr %a)
  %b = alloca i16
  call void @llvm.lifetime.start.p0(i64 2, ptr %b)
  store i16 %x, ptr %b
  call void @llvm.lifetime.end.p0(i64 2, ptr %b)
  ret i16 %x
}
",
    );
    assert_eq!(dead, [false, true, true, false, false, false, false, false]);
}

/// An assumption is dead when its condition is a constant that holds.
#[test]
fn an_assumption_is_dead_when_it_holds_trivially() {
    let dead = dead_flags(
        "declare void @llvm.assume(i1)

define i16 @f(i1 %c) {
b0:
  call void @llvm.assume(i1 true)
  call void @llvm.assume(i1 false)
  call void @llvm.assume(i1 %c)
  ret i16 0
}
",
    );
    assert_eq!(dead, [true, false, false, false]);
}

/// A phi nothing reads is dead; a terminator and a landing pad never are.
#[test]
fn a_phi_is_dead_but_a_terminator_and_a_landing_pad_are_not() {
    let dead = dead_flags(
        "declare void @h()
declare i32 @__gxx_personality_v0(...)

define i16 @f(i16 %x, i1 %c) personality ptr @__gxx_personality_v0 {
b0:
  br i1 %c, label %b1, label %b2

b1:
  invoke void @h() to label %b2 unwind label %pad

b2:
  %p = phi i16 [ %x, %b0 ], [ %x, %b1 ]
  ret i16 0

pad:
  %lp = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %lp
}
",
    );
    assert_eq!(dead, [false, false, true, false, false, false]);
}

// ---- EliminateDuplicatePHINodes

/// A join of `b1` and `b2` with the phis `phis`, then `ret`.
fn join(phis: &str) -> String {
    format!(
        "define i16 @f(i1 %c, i16 %x, i16 %y) {{
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
{phis}  ret i16 %r
}}
"
    )
}

/// The phis of `b3` that `eliminate_duplicate_phi_nodes` removes, and the
/// function after they are erased.
fn deduplicated(text: &str) -> (Vec<String>, Module) {
    let mut module = parsed(text);
    let b3 = block(&module, "b3");
    let (_, function) = module.function_mut("f").expect("@f");
    let mut to_remove = llrm_mir::dense::IdSet::new();
    assert_eq!(eliminate_duplicate_phi_nodes(function, b3, &mut to_remove), !to_remove.is_empty());
    let names: Vec<String> = to_remove
        .iter()
        .map(|phi| function.value(function.instruction(phi).result.expect("a value")).name.clone().expect("named"))
        .collect();
    for phi in to_remove.iter() {
        function.erase(phi).expect("nothing reads it");
    }
    (names, module)
}

/// Two phis with the same arms are one, and the one after the other is the
/// one removed; the phi that read them reads the first.
#[test]
fn a_duplicate_phi_is_replaced_by_the_first() {
    let before = parsed(&join(
        "  %p = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  %q = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  %r = add i16 %p, %q
",
    ));
    let (removed, after) = deduplicated(&body(&before));
    assert_eq!(removed, ["q"]);
    assert_eq!(lines(&body(&after), "%r ="), ["%r = add i16 %p, %p"]);
    assert_eq!(results(&after, &[&[0, 3, 4], &[1, 3, 4]]), results(&before, &[&[0, 3, 4], &[1, 3, 4]]));
}

/// Phis in a different order of the same arms are not identical: LLVM
/// compares them position by position.
#[test]
fn phis_with_the_same_arms_in_another_order_are_kept() {
    let (removed, _) = deduplicated(&join(
        "  %p = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  %q = phi i16 [ %y, %b2 ], [ %x, %b1 ]
  %r = add i16 %p, %q
",
    ));
    assert_eq!(removed, Vec::<String>::new());
}

/// A loop header with `n` phis that differ, and the pairs `%p`, `%q` (the same)
/// and `%s`, `%t` (the same once `%q` is `%p`).
fn looped(n: usize) -> String {
    let distinct: String = (0..n).map(|k| format!("  %d{k} = phi i16 [ {k}, %b0 ], [ %y, %b3 ]\n")).collect();
    format!(
        "define i16 @f(i16 %x, i16 %y) {{
b0:
  br label %b3

b3:
{distinct}  %p = phi i16 [ %x, %b0 ], [ %a, %b3 ]
  %q = phi i16 [ %x, %b0 ], [ %a, %b3 ]
  %s = phi i16 [ %x, %b0 ], [ %p, %b3 ]
  %t = phi i16 [ %x, %b0 ], [ %q, %b3 ]
  %a = add i16 %p, 1
  %c = icmp ult i16 %a, %y
  br i1 %c, label %b3, label %b4

b4:
  %r = add i16 %s, %t
  ret i16 %r
}}
"
    )
}

/// A pair made identical by a replacement is found, in the exhaustive search
/// and, past 32 phis, the set-based one: `%t` reads `%q`, `%s` reads `%p`, and
/// once `%q` is `%p` they are the same.
#[test]
fn a_pair_made_identical_by_a_replacement_is_found() {
    for n in [0, 40] {
        let before = parsed(&looped(n));
        let (removed, after) = deduplicated(&body(&before));
        assert_eq!(removed, ["q", "t"], "{n} other phis");
        assert_eq!(lines(&body(&after), "%r ="), ["%r = add i16 %s, %s"], "{n} other phis");
        let inputs: &[&[i128]] = &[&[1, 5], &[4, 3]];
        assert_eq!(results(&after, inputs), results(&before, inputs), "{n} other phis");
    }
}

/// The exhaustive search ignores a phi's fast-math flags, and the set-based
/// one, which compares `isIdenticalTo`, does not: a block of 33 phis keeps
/// two otherwise equal float phis that a block of 3 merges.
#[test]
fn only_the_exhaustive_search_ignores_fast_math_flags() {
    let phis = "  %p = phi float [ 1.0, %b1 ], [ 2.0, %b2 ]
  %q = phi fast float [ 1.0, %b1 ], [ 2.0, %b2 ]
  %r = fadd float %p, %q
";
    let float_join = |tail: &str| join(tail).replace("ret i16 %r", "%z = fptoui float %r to i16\n  ret i16 %z");
    assert_eq!(deduplicated(&float_join(phis)).0, ["q"], "three phis");
    let crowded: String = (0..31).map(|k| format!("  %d{k} = phi i16 [ {k}, %b1 ], [ %y, %b2 ]\n")).collect();
    assert_eq!(deduplicated(&float_join(&format!("{crowded}{phis}"))).0, Vec::<String>::new(), "thirty-four phis");
}

/// The overload that erases removes the duplicates itself.
#[test]
fn the_erasing_form_removes_the_duplicates() {
    let mut module = parsed(&join(
        "  %p = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  %q = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  %r = add i16 %p, %q
",
    ));
    let b3 = block(&module, "b3");
    let (_, function) = module.function_mut("f").expect("@f");
    assert!(eliminate_duplicate_phi_nodes_and_erase(function, b3));
    assert!(!eliminate_duplicate_phi_nodes_and_erase(function, b3));
    assert_eq!(lines(&body(&module), "phi").len(), 1);
}

// ---- MergeBlockIntoPredecessor

/// `bb` of @f merged into its predecessor: whether it merged, and the module.
fn merged(
    text: &str,
    bb: &str,
    two_successors: bool,
) -> (bool, Module) {
    let mut module = parsed(text);
    let bb = block(&module, bb);
    let (context, function) = module.function_mut("f").expect("@f");
    let merged = merge_block_into_predecessor(context, function, bb, two_successors);
    (merged, module)
}

const CHAIN: &str = "define i16 @f(i16 %x) {
b0:
  %a = add i16 %x, 1
  br label %b1

b1:
  %p = phi i16 [ %a, %b0 ]
  %b = mul i16 %p, 2
  br label %b2

b2:
  ret i16 %b
}
";

/// The records said before the predecessor's branch go with the terminator that
/// takes its place: the merge redirected the uses of the block before it erased
/// the branch, which then named its own block, and the records stayed standing
/// before the erased branch (nib: "a debug record of !8 stands before
/// instruction 5, which is no longer in the function").
#[test]
fn merging_leaves_the_records_of_the_old_branch_before_the_new_terminator() {
    let (merged, module) = merged(
        "define i16 @f(i16 %x) {
b0:
  %a = add i16 %x, 1
  #dbg_value(i16 %a, !0)
  br label %b1

b1:
  ret i16 %a
}

!0 = !{!\"v\"}
",
        "b1",
        false,
    );
    assert!(merged);
    assert!(llrm_mir::verify::verify(&module).is_empty(), "{:?}", llrm_mir::verify::verify(&module));
    let (_, _, function) = module.functions().next().expect("@f");
    let ret = function.terminator(function.entry().expect("entry")).expect("a terminator");
    assert!(function.debug_records().iter().all(|one| one.before == ret), "{:?}", function.debug_records());
    assert_eq!(function.debug_records().len(), 1);
}

/// With instructions in the merged block the records before the old branch
/// stand before the first of them, not after all of them: observed.c's `mix`
/// at -O2 -g had every record of every variable past the last instruction, so
/// gdb read `a`, `b`, `c` and `i` as optimised out where the pin says them.
#[test]
fn merging_keeps_the_records_of_the_old_branch_before_the_moved_instructions() {
    let (merged, module) = merged(
        "define i16 @f(i16 %x) {
b0:
  #dbg_value(i16 %x, !0)
  br label %b1

b1:
  %b = mul i16 %x, 2
  %c = add i16 %b, 1
  ret i16 %c
}

!0 = !{!\"v\"}
",
        "b1",
        false,
    );
    assert!(merged);
    let (_, _, function) = module.functions().next().expect("@f");
    let first = function.iter_block(function.entry().expect("entry")).next().expect("an instruction");
    assert!(function.debug_records().iter().all(|one| one.before == first), "{:?}", function.debug_records());
    assert_eq!(function.debug_records().len(), 1);
}

/// A block its predecessor alone enters joins it: the phi is its one value,
/// the instructions follow the predecessor's, and the block is gone.
#[test]
fn a_block_only_its_predecessor_enters_joins_it() {
    let before = parsed(CHAIN);
    let (merged, after) = merged(CHAIN, "b1", false);
    assert!(merged);
    let text = body(&after);
    assert_eq!(
        text.lines().map(str::trim).collect::<Vec<_>>(),
        [
            "define i16 @f(i16 %x) {",
            "b0:",
            "%a = add i16 %x, 1",
            "%b = mul i16 %a, 2",
            "br label %b2",
            "",
            "b2:",
            "ret i16 %b",
            "}"
        ]
    );
    assert_eq!(results(&after, &[&[3], &[9]]), results(&before, &[&[3], &[9]]));
}

/// A block with two predecessors, a self-loop, the entry, and one whose
/// predecessor ends in an invoke or a conditional branch stay.
#[test]
fn a_block_that_cannot_join_stays() {
    let join = "define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b2

b2:
  ret i16 %x
}
";
    assert!(!merged(join, "b2", false).0, "two predecessors");
    let looping = "define i16 @f(i16 %x) {
b0:
  br label %b1

b1:
  br label %b1
}
";
    assert!(!merged(looping, "b1", false).0, "two predecessors, itself one");
    let invoking = "declare void @h()
declare i32 @__gxx_personality_v0(...)

define i16 @f(i16 %x) personality ptr @__gxx_personality_v0 {
b0:
  invoke void @h() to label %ok unwind label %pad

ok:
  ret i16 %x

pad:
  %lp = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %lp
}
";
    assert!(!merged(invoking, "ok", false).0, "an invoke before it");
    assert!(!merged(invoking, "ok", true).0, "an invoke is not a conditional branch");
    let branching = "define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  ret i16 %x

b2:
  ret i16 0
}
";
    assert!(!merged(branching, "b1", false).0, "a predecessor with two successors");
    assert!(!merged(CHAIN, "b0", false).0, "the entry");
}

/// With `predecessor_with_two_successors` a conditional branch to the block
/// and another goes to where the block jumped.
#[test]
fn a_predecessor_with_two_successors_branches_past_the_block() {
    let text = "define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b3

b1:
  %a = add i16 %x, 1
  br label %b2

b2:
  %p = phi i16 [ %a, %b1 ], [ %x, %b3 ]
  ret i16 %p

b3:
  br label %b2
}
";
    let before = parsed(text);
    let (merged, after) = merged(text, "b1", true);
    assert!(merged);
    let text = body(&after);
    assert_eq!(lines(&text, "br i1"), ["br i1 %c, label %b2, label %b3"]);
    assert_eq!(lines(&text, "%p ="), ["%p = phi i16 [ %a, %b0 ], [ %x, %b3 ]"]);
    assert_eq!(results(&after, &[&[1, 3], &[0, 3]]), results(&before, &[&[1, 3], &[0, 3]]));
}

/// Each phi of a block is its first incoming value.
#[test]
fn a_single_entry_phi_is_its_value() {
    let mut module = parsed(CHAIN);
    let bb = block(&module, "b1");
    let (context, function) = module.function_mut("f").expect("@f");
    assert!(fold_single_entry_phi_nodes(context, function, bb));
    assert!(!fold_single_entry_phi_nodes(context, function, bb));
    assert_eq!(lines(&body(&module), "%b ="), ["%b = mul i16 %a, 2"]);
}

// ---- Loads.cpp

/// The pointer queries' module, with its layout and sizes.
struct Pointers {
    module: Module,
    layout: DataLayout,
    sizes: valuetracking::Sizes,
}

impl Pointers {
    fn new(text: &str) -> Self {
        let module = parsed(text);
        let layout = DataLayout::default();
        let sizes = valuetracking::sizes(&module, &layout);
        Self { module, layout, sizes }
    }

    fn function(&self) -> &llrm_mir::module::Function {
        self.module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f").2
    }

    fn operand(
        &self,
        name: &str,
    ) -> Operand {
        Operand::Value(value(&self.module, name))
    }

    fn null(&mut self) -> Operand {
        let ty = self.module.context.types.intern(Type::Pointer(0));
        Operand::Constant(self.module.context.constant(Constant { ty, kind: ConstantKind::Null }))
    }

    fn global(
        &mut self,
        name: &str,
    ) -> Operand {
        let id = self.module.named(name).expect("a global");
        let ty = self.module.context.types.intern(Type::Pointer(0));
        Operand::Constant(self.module.context.constant(Constant { ty, kind: ConstantKind::Global(id) }))
    }

    fn always(
        &self,
        from: Operand,
        to: Operand,
    ) -> bool {
        can_replace_pointers_if_equal(
            &self.module.context,
            &self.layout,
            &self.module.declarations(),
            &self.sizes,
            self.function(),
            from,
            to,
        )
    }

    fn in_use(
        &self,
        user: &str,
        to: Operand,
    ) -> bool {
        let function = self.function();
        let user = inst(&self.module, user);
        let at = Use { user, index: 0 };
        can_replace_pointers_in_use_if_equal(
            &self.module.context,
            &self.layout,
            &self.module.declarations(),
            &self.sizes,
            function,
            at,
            to,
        )
    }
}

const POINTERS: &str = "@g = global i16 0
@k = constant i16 5
declare void @llvm.lifetime.start.p0(i64, ptr)

define i16 @f(ptr %p, ptr %q, i16 %n) {
b0:
  %near = getelementptr i8, ptr %p, i16 4
  %far = getelementptr i8, ptr %p, i16 8
  %cmp = icmp eq ptr %p, %q
  %load = load i16, ptr %p
  %addr = ptrtoint ptr %p to i16
  call void @llvm.lifetime.start.p0(i64 2, ptr %p)
  ret i16 %load
}
";

/// Two pointers into one object are replaceable by each other; two objects
/// are not, unless the one is null, or a constant global to read.
#[test]
fn pointers_are_replaceable_where_they_name_one_object_or_one_is_null_or_constant() {
    let mut pointers = Pointers::new(POINTERS);
    let (p, q) = (pointers.operand("p"), pointers.operand("q"));
    let (near, far) = (pointers.operand("near"), pointers.operand("far"));
    let null = pointers.null();
    let (g, k) = (pointers.global("g"), pointers.global("k"));
    assert!(pointers.always(near, far), "one object");
    assert!(pointers.always(p, near));
    assert!(!pointers.always(p, q), "two objects");
    assert!(pointers.always(p, null), "to null");
    assert!(pointers.always(null, p), "from null");
    assert!(!pointers.always(p, g), "a global that may change");
    assert!(pointers.always(p, k), "a constant global");
}

/// A use replaceable where it only compares or exposes an address; and never
/// in a lifetime marker.
#[test]
fn a_use_of_an_unequal_pointer_is_replaceable_only_where_it_compares() {
    let mut pointers = Pointers::new(POINTERS);
    let q = pointers.operand("q");
    assert!(pointers.in_use("cmp", q), "an icmp");
    assert!(pointers.in_use("addr", q), "a ptrtoint, the address being all of the pointer");
    assert!(!pointers.in_use("load", q), "a load");
    let null = pointers.null();
    assert!(!pointers.in_use("load", q));
    let lifetime = pointers
        .function()
        .walk()
        .map(|(_, one)| one)
        .find(|&one| matches!(
            pointers.function().instruction(one).opcode,
            llrm_mir::opcode::Opcode::Call(_)
        ));
    let lifetime = Use { user: lifetime.expect("a call"), index: 1 };
    let function = pointers.function();
    assert!(!can_replace_pointers_in_use_if_equal(
        &pointers.module.context,
        &pointers.layout,
        &pointers.module.declarations(),
        &pointers.sizes,
        function,
        lifetime,
        null,
    ));
}

// ---- andIRFlags, dropPoisonGeneratingFlags

/// A function with the instructions `a` and `b` as `%a` and `%b`.
fn pair(
    a: &str,
    b: &str,
) -> Module {
    parsed(&format!(
        "define i16 @f(i16 %x, i16 %y, float %u, float %v, ptr %p) {{
b0:
  %a = {a}
  %b = {b}
  ret i16 0
}}
"
    ))
}

/// The line of `%a` once `a` is made to have only the flags `b` has, as the
/// printer writes it.
fn anded(
    a: &str,
    b: &str,
) -> String {
    let mut module = pair(a, b);
    let (x, y) = (inst(&module, "a"), inst(&module, "b"));
    let (context, function) = module.function_mut("f").expect("@f");
    and_ir_flags(context, function, x, y);
    lines(&body(&module), "%a =").remove(0)
}

/// The line `%a = {text}` as the printer writes it.
fn written(text: &str) -> String {
    lines(&body(&pair(text, text)), "%a =").remove(0)
}

/// In each class both are in, `a` keeps the flags `b` has: the wrap flags of
/// arithmetic and `trunc`, `exact`, `disjoint`, the fast-math flags, a GEP's,
/// `nneg` and `samesign`.
#[test]
fn each_class_of_flags_is_anded() {
    let cases = [
        ("add nsw nuw i16 %x, %y", "mul nsw i16 %x, %y", "add nsw i16 %x, %y"),
        ("trunc nuw nsw i16 %x to i8", "trunc nuw i16 %x to i8", "trunc nuw i16 %x to i8"),
        ("udiv exact i16 %x, %y", "lshr i16 %x, %y", "udiv i16 %x, %y"),
        ("ashr exact i16 %x, %y", "sdiv exact i16 %x, %y", "ashr exact i16 %x, %y"),
        ("or disjoint i16 %x, %y", "or i16 %x, %y", "or i16 %x, %y"),
        ("fadd nnan nsz float %u, %v", "fmul nsz arcp float %u, %v", "fadd nsz float %u, %v"),
        ("fcmp fast olt float %u, %v", "fneg nnan float %u", "fcmp nnan olt float %u, %v"),
        (
            "getelementptr inbounds nuw i8, ptr %p, i16 %x",
            "getelementptr inbounds i8, ptr %p, i16 %y",
            "getelementptr inbounds i8, ptr %p, i16 %x",
        ),
        ("zext nneg i16 %x to i32", "zext i16 %y to i32", "zext i16 %x to i32"),
        ("uitofp nneg i16 %x to float", "zext i16 %y to i32", "uitofp i16 %x to float"),
        ("icmp samesign ult i16 %x, %y", "icmp ult i16 %x, %y", "icmp ult i16 %x, %y"),
    ];
    for (a, b, expected) in cases {
        assert_eq!(anded(a, b), written(expected), "{a} and {b}");
    }
}

/// A flag of a class the other instruction is not in stays: the other has
/// nothing to say of it.
#[test]
fn a_flag_the_other_instruction_has_no_class_for_stays() {
    let cases = [
        ("add nsw nuw i16 %x, %y", "sdiv exact i16 %x, %y"),
        ("or disjoint i16 %x, %y", "add i16 %x, %y"),
        ("udiv exact i16 %x, %y", "add i16 %x, %y"),
        ("zext nneg i16 %x to i32", "trunc i16 %x to i8"),
        ("fadd fast float %u, %v", "add i16 %x, %y"),
    ];
    for (a, b) in cases {
        assert_eq!(anded(a, b), written(a), "{a} and {b}");
    }
}

/// Every flag a poison value can come of is dropped; a float keeps the ones
/// that cannot.
#[test]
fn the_flags_poison_comes_of_are_dropped() {
    let cases = [
        ("add nsw nuw i16 %x, %y", "add i16 %x, %y"),
        ("shl nsw i16 %x, %y", "shl i16 %x, %y"),
        ("trunc nuw nsw i16 %x to i8", "trunc i16 %x to i8"),
        ("sdiv exact i16 %x, %y", "sdiv i16 %x, %y"),
        ("or disjoint i16 %x, %y", "or i16 %x, %y"),
        ("getelementptr inbounds nuw i8, ptr %p, i16 %x", "getelementptr i8, ptr %p, i16 %x"),
        ("zext nneg i16 %x to i32", "zext i16 %x to i32"),
        ("icmp samesign ult i16 %x, %y", "icmp ult i16 %x, %y"),
        ("fadd nnan ninf nsz float %u, %v", "fadd nsz float %u, %v"),
    ];
    for (a, expected) in cases {
        let mut module = pair(a, a);
        let x = inst(&module, "a");
        let (context, function) = module.function_mut("f").expect("@f");
        drop_poison_generating_flags(context, function, x);
        assert_eq!(lines(&body(&module), "%a =").remove(0), written(expected), "{a}");
    }
}

// ---- isPointerUseReplaceable

/// A chain of `n` selects over `%p`, then an `icmp`: `%p` is a pointer a use
/// of which is replaceable by `%q`.
fn selects(n: usize) -> String {
    let chain: String = (0..n)
        .map(|k| {
            let from = if k == 0 { "%p".to_owned() } else { format!("%s{}", k - 1) };
            format!("  %s{k} = select i1 %c, ptr {from}, ptr %q\n")
        })
        .collect();
    let last = if n == 0 { "%p".to_owned() } else { format!("%s{}", n - 1) };
    format!(
        "define i1 @f(ptr %p, ptr %q, i1 %c) {{
b0:
{chain}  %r = icmp eq ptr {last}, %q
  ret i1 %r
}}
"
    )
}

/// The search gives up after 39 users (`Limit` is 40, spent a user at a
/// time): a chain of 38 selects and the compare is followed to its end, one of
/// 39 is not.
#[test]
fn a_chain_of_users_is_followed_only_so_far() {
    for (n, replaceable) in [(0, true), (38, true), (39, false)] {
        let pointers = Pointers::new(&selects(n));
        let p = value(&pointers.module, "p");
        let function = pointers.function();
        let at = *function.users(p).iter().next().expect("a use");
        assert_eq!(is_pointer_use_replaceable(function, at, false), replaceable, "{n} selects");
    }
}

/// A `ptrtoint` of a pointer whose address is not all of it exposes more than
/// the address: not replaceable where the layout's index is narrower than the
/// pointer.
#[test]
fn a_ptrtoint_is_replaceable_only_where_the_address_is_the_pointer() {
    let text = "define i16 @f(ptr %p, ptr %q) {
b0:
  %a = ptrtoint ptr %p to i16
  ret i16 %a
}
";
    for (layout, replaceable) in [("p:32:32:32:32", true), ("p:32:32:32:16", false)] {
        let mut pointers = Pointers::new(text);
        pointers.layout = DataLayout::parse(layout).expect("a layout");
        let q = pointers.operand("q");
        assert_eq!(pointers.in_use("a", q), replaceable, "{layout}");
    }
}

/// `memory` of a call that only touches its arguments, with one that touches
/// nothing, is the argument access.
#[test]
fn memory_attributes_are_united_per_location() {
    let mut module = parsed(
        "declare i16 @h(ptr)

define i16 @f(ptr %p) {
b0:
  %a = call i16 @h(ptr %p) memory(argmem: read)
  %b = call i16 @h(ptr %p) memory(none)
  %r = add i16 %a, %b
  ret i16 %r
}
",
    );
    patched(&mut module, "b", "a");
    assert_eq!(lines(&body(&module), "%a ="), ["%a = call i16 @h(ptr %p) memory(argmem: read)"]);
}

/// A conditional branch with both arms to the block is dropped when the block
/// joins it, and the phi, whose two arms agree, is its value.
#[test]
fn a_branch_with_both_arms_to_the_block_is_dropped_by_the_merge() {
    let text = "define i16 @f(i16 %x, i16 %y) {
b0:
  %c = icmp eq i16 %x, %y
  br i1 %c, label %b1, label %b1

b1:
  %p = phi i16 [ %x, %b0 ], [ %x, %b0 ]
  %r = add i16 %p, 1
  ret i16 %r
}
";
    let before = parsed(text);
    let (merged, after) = merged(text, "b1", false);
    assert!(merged);
    assert_eq!(lines(&body(&after), "br"), Vec::<String>::new());
    assert_eq!(results(&after, &[&[1, 1], &[1, 2]]), results(&before, &[&[1, 1], &[1, 2]]));
}

/// `changeToUnreachable`: the instruction and what follows it in its block go,
/// the block ends in `unreachable`, uses of what went are poison, and the
/// successor's phi loses its input from the block.
#[test]
fn test_change_to_unreachable_cuts_the_block_and_the_successors_phi() {
    let mut module = parsed(
        "define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %d = add i16 %x, 1
  br label %b2

b2:
  %p = phi i16 [ %x, %b0 ], [ %d, %b1 ]
  ret i16 %p
}
",
    );
    let at = inst(&module, "d");
    let (context, function) = module.function_mut("f").expect("@f");
    assert_eq!(change_to_unreachable(context, function, at), 2);
    assert_eq!(
        printed(&module),
        "define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  unreachable

b2:
  ret i16 %x
}
"
    );
}

/// `duplicate_phis_in` built a map and a vector of the phis for every block it
/// was asked of, and most blocks have no two phis (d_faces -O2: 3,844 visits,
/// 2.8 M Ir). A block of none or one is not walked; one of two is.
#[test]
fn a_block_of_fewer_than_two_phis_is_not_walked_for_duplicates() {
    let mut module = parsed(
        "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i16 [ %x, %b1 ], [ 1, %b2 ]
  %q = phi i16 [ %x, %b1 ], [ 1, %b2 ]
  ret i16 %p
}
",
    );
    let (_, function) = module.function_mut("f").expect("@f");
    let walked = || crate::transform::WALKED.with(std::cell::Cell::get);
    let before = walked();
    for block in function.layout().to_vec() {
        if function.layout().last() != Some(&block) {
            assert!(!crate::transform::duplicate_phis_in(function, block).unwrap(), "no duplicates before b3");
        }
    }
    assert_eq!(walked() - before, 0, "a block of fewer than two phis was walked");
    let last = *function.layout().last().expect("b3");
    assert!(crate::transform::duplicate_phis_in(function, last).unwrap(), "b3's two phis are one");
    assert_eq!(walked() - before, 1);
}

/// `EliminateDuplicatePHINodes` compares the phis of a block pair by pair up to
/// `PHICSENumPHISmallSize` (32) of them and hashes only a larger set; every
/// block of two or more was hashed (d_faces -O2: 1,741 blocks, a map and a key
/// vector each, 494 Ir a block against the pin's 141).
#[test]
fn a_block_of_few_phis_is_compared_pair_by_pair_and_a_block_of_many_is_hashed() {
    let join = |phis: usize| {
        let mut text = String::from(
            "define i16 @f(i16 %x, i1 %c) {\nb0:\n  br i1 %c, label %b1, label %b2\n\nb1:\n  br label %b3\n\nb2:\n  br label %b3\n\nb3:\n",
        );
        for n in 0..phis {
            // Every phi is the first one's twin except one that differs.
            let other = if n == 1 { "2" } else { "1" };
            text.push_str(&format!("  %p{n} = phi i16 [ %x, %b1 ], [ {other}, %b2 ]\n"));
        }
        text.push_str("  ret i16 %p0\n}\n");
        text
    };
    let run = |phis: usize| {
        let mut module = parsed(&join(phis));
        let (_, function) = module.function_mut("f").expect("@f");
        let last = *function.layout().last().expect("b3");
        let before = crate::transform::HASHED.with(std::cell::Cell::get);
        let changed = crate::transform::duplicate_phis_in(function, last).unwrap();
        let left = crate::edges::phis(function, last).len();
        (changed, left, crate::transform::HASHED.with(std::cell::Cell::get) - before)
    };
    assert_eq!(run(3), (true, 2, 0), "p0 and p2 are one; p1 stays; compared, not hashed");
    assert_eq!(run(32), (true, 2, 0), "32 phis are still compared");
    assert_eq!(run(33), (true, 2, 1), "33 phis are hashed, with the same result");
}

/// Two array selectors merged lost `!llrm.descriptor`; the survivor read as a
/// forged data-segment pointer and `LLRM_CHECK_STALE` failed bintree.bas -O2
/// -m16.
#[test]
fn merged_array_selectors_keep_their_descriptor_mark() {
    assert_eq!(combined("!llrm.descriptor !6", "!llrm.descriptor !2", true), ["llrm.descriptor"]);
    assert_eq!(combined("!llrm.descriptor !6", "!llrm.descriptor !2", false), ["llrm.descriptor"]);
    assert_eq!(combined("!llrm.descriptor !6", "!tbaa !4", true), Vec::<String>::new());
}
