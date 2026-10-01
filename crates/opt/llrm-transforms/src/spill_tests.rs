//! The spill model's facts, each on the smallest function that shows it.

use std::collections::BTreeMap;

use llrm_analysis::testing::DOS;
use llrm_analysis::{cfg, liveness};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Function, Module, ValueId};

use super::{Room, Traffic, cells, integer, segment_view, sites, spilled, traffic, words};
use crate::profit::OperationCosts;

fn module(text: &str) -> Module {
    llrm_mir::parse::module(&format!("{DOS}{text}")).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn function(module: &Module) -> &Function {
    module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f").2
}

fn named(function: &Function, name: &str) -> ValueId {
    function.walk().filter_map(|(_, inst)| function.instruction(inst).result).find(|&value| function.value(value).name.as_deref() == Some(name)).expect(name)
}

fn costs() -> OperationCosts {
    OperationCosts { load: 1, store: 1, memory_update: 3, address: 1, r#move: 1, ..OperationCosts::default() }
}

/// Every block once, the loop's ten times.
fn looped(function: &Function) -> BTreeMap<i64, i64> {
    function.layout().iter().map(|&block| (cfg::id(block), if function.block(block).name.as_deref() == Some("body") { 10 } else { 1 })).collect()
}

const COUNTED: &str = "define i16 @f(i16 %n) {
entry:
  br label %body
body:
  %i = phi i16 [ 0, %entry ], [ %j, %body ]
  %j = add i16 %i, 1
  %c = icmp ult i16 %j, %n
  br i1 %c, label %body, label %done
done:
  ret i16 %j
}
";

/// A counter kept in memory is stepped there, `add [m],1`, and its step
/// shares its cell: priced as a store and a load a trip, a spilled
/// counter read as dearer than a spilled invariant read as often.
#[test]
fn test_a_spilled_counter_is_stepped_in_its_cell() {
    let module = module(COUNTED);
    let function = function(&module);
    let (i, j) = (named(function, "i"), named(function, "j"));
    let cells = cells(function);
    assert_eq!(cells.get(&j), Some(&i));
    let found = traffic(function, &looped(function), &cells, &costs(), &|_| true, &|_| 1);
    // Stored once on entry, stepped ten times, read by the exit test ten
    // times and once after.
    assert_eq!(found[&i], Traffic { stores: 1, updates: 10, loads: 11, rebuild: None });
}

/// A truth value only its own block's branch reads is flags; one kept
/// past its block is a register: GVN kept `i < n` across level's loop exit
/// in `dl`, priced as free.
#[test]
fn test_a_truth_value_takes_a_register_only_past_its_branch() {
    let counted = module(COUNTED);
    let function = self::function(&counted);
    assert!(!integer(&counted.context, function, named(function, "c")));
    let kept = module("define i16 @f(i16 %n) {
entry:
  %c = icmp ult i16 %n, 7
  br i1 %c, label %yes, label %no
yes:
  br label %no
no:
  %r = select i1 %c, i16 1, i16 2
  ret i16 %r
}
");
    let function = self::function(&kept);
    assert!(integer(&kept.context, function, named(function, "c")));
}

/// A far pointer spills as a segment and an offset: two stores.
#[test]
fn test_a_far_pointer_is_stored_a_word_at_a_time() {
    let module = module("define ptr addrspace(1) @f(ptr addrspace(1) %p, ptr %q) {
entry:
  %a = getelementptr i8, ptr addrspace(1) %p, i16 2
  %b = getelementptr i8, ptr %q, i16 2
  ret ptr addrspace(1) %a
}
");
    let function = function(&module);
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a layout")).expect("a layout");
    assert_eq!(words(&module.context, &layout, function, named(function, "a")), 2);
    assert_eq!(words(&module.context, &layout, function, named(function, "b")), 1);
}

/// A far view of a frame object is rebuilt with its address and its
/// segment: laps's loop rebuilt one each trip where a walking pointer
/// would have loaded both at once, and it was priced as free.
#[test]
fn test_a_far_view_of_a_frame_is_rebuilt_with_its_segment() {
    let module = module("define i16 @f() {
entry:
  %a = alloca [8 x i8]
  %far = addrspacecast ptr %a to ptr addrspace(1)
  %v = load i16, ptr addrspace(1) %far
  %w = load i16, ptr %a
  %s = add i16 %v, %w
  ret i16 %s
}
");
    let function = function(&module);
    let found = traffic(function, &looped(function), &cells(function), &costs(), &|_| true, &|_| 1);
    assert_eq!(found[&named(function, "a")].rebuild, Some(1));
    assert_eq!(found[&named(function, "far")].rebuild, Some(2));
}

/// Across a call only the registers a call leaves hold values: two live
/// across one that leaves one spill the cheaper, where the whole body's
/// registers had seemed enough.
#[test]
fn test_a_call_leaves_only_its_registers() {
    let module = module("declare void @g()

define i16 @f(i16 %x, i16 %y) {
entry:
  %a = add i16 %x, 1
  %b = add i16 %y, 2
  call void @g()
  %s = add i16 %a, %b
  ret i16 %s
}
");
    let function = function(&module);
    let found = liveness::live(function);
    let cells = cells(function);
    let room = Room { registers: 3, across_call: 1, ..Room::default() };
    let integer = |value: ValueId| integer(&module.context, function, value);
    let points = function.layout().iter().flat_map(|&block| sites(function, &found, block, room, &|_| room.across_call, &|_| 0, &cells, &integer, &|_| false)).flat_map(super::Site::points).collect::<Vec<_>>();
    let prices = traffic(function, &looped(function), &cells, &costs(), &|_| true, &|_| 1);
    // Each of `a` and `b` is stored once and loaded once.
    assert_eq!(spilled(points, |one| prices.get(&one).map_or(0, |one| one.price(&costs()))), 2);
}

/// A load through a far pointer takes the register its selector passes
/// through: one fewer holds values there, which no point counted, so a loop
/// with four products and a counter live across ten such loads was planned
/// to fit and could not be allocated.
#[test]
fn test_a_far_access_takes_a_register_of_its_own() {
    let module = module("define i16 @f(ptr addrspace(1) %far, ptr %near) {
entry:
  %a = load i16, ptr addrspace(1) %far
  %b = load i16, ptr %near
  %s = add i16 %a, %b
  ret i16 %s
}
");
    let function = function(&module);
    let layout = llrm_analysis::testing::layout(&module);
    let room = Room { registers: 6, across_call: 2, far_access: 1, ..Room::default() };
    let loads = function.layout().iter().flat_map(|&block| function.block(block).instructions().to_vec()).filter(|&inst| matches!(function.instruction(inst).opcode, llrm_mir::opcode::Opcode::Load { .. })).collect::<Vec<_>>();
    let taken = loads.iter().map(|&inst| super::transient(&module.context, &layout, function, inst, room)).collect::<Vec<_>>();
    assert_eq!(taken, [2, 1]);
}

/// A far view of a segment is held in a segment register: counted against the
/// target's segment registers, not its general ones, and a general one
/// where the target has none. Lsr alone knew it, and gvn priced the same
/// pointer as a register.
#[test]
fn test_a_far_view_of_a_segment_is_counted_in_the_segment_registers() {
    let module = module("define i16 @f(i16 %a, i16 %b) {
entry:
  %sa = inttoptr i16 %a to ptr addrspace(2)
  %fa = addrspacecast ptr addrspace(2) %sa to ptr addrspace(1)
  %sb = inttoptr i16 %b to ptr addrspace(2)
  %fb = addrspacecast ptr addrspace(2) %sb to ptr addrspace(1)
  %x = load i16, ptr addrspace(1) %fa
  %y = load i16, ptr addrspace(1) %fb
  %s = add i16 %x, %y
  ret i16 %s
}
");
    let function = function(&module);
    let layout = llrm_analysis::testing::layout(&module);
    let found = liveness::live(function);
    let cells = cells(function);
    let integer = |value: ValueId| integer(&module.context, function, value);
    let views = |value: ValueId| segment_view(&module.context, &layout, function, value);
    let at = |segments: i64| {
        let room = Room { registers: 6, across_call: 2, segments, ..Room::default() };
        let block = function.layout()[0];
        sites(function, &found, block, room, &|_| 2, &|_| 0, &cells, &integer, &views).into_iter().find(|site| matches!(function.instruction(site.inst).opcode, llrm_mir::opcode::Opcode::Load { .. })).expect("a load")
    };
    let held = at(3);
    assert_eq!(held.segments.residents.len(), 2, "both views are in segment registers");
    assert!(held.before.residents.iter().all(|&one| !views(one)));
    let none = at(0);
    assert!(none.segments.residents.is_empty() && none.before.residents.iter().filter(|&&one| views(one)).count() == 2, "no segment registers: general ones");
}
