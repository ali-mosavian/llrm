//! The spill model's facts, each on the smallest function that shows it.

use std::collections::BTreeMap;

use llrm_analysis::testing::DOS;
use llrm_analysis::{cfg, liveness};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Function, InstId, Module, ValueId};

use super::{Pressure, Room, Site, Traffic, View, addressed, cells, forecast, integer, segment_view, sites, spilled, transient, traffic, words};
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
    let points = function.layout().iter().flat_map(|&block| sites(function, &found, block, room, &|_| room.across_call, &|_, _| 0, &cells, &integer, &|_| false, &|_| false)).flat_map(super::Site::points).collect::<Vec<_>>();
    let prices = traffic(function, &looped(function), &cells, &costs(), &|_| true, &|_| 1);
    // Each of `a` and `b` is stored once and loaded once.
    assert_eq!(spilled(points, |one| prices.get(&one).map_or(0, |one| one.price(&costs()))), 2);
}

/// A load through a far pointer takes the register its selector passes
/// through besides the pointer's own: one fewer holds values there, which no
/// point counted, so a loop with four products and a counter live across ten
/// such loads was planned to fit and could not be allocated.
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
    let taken = loads.iter().map(|&inst| {
        let live = function.instruction(inst).operands.iter().filter_map(|one| if let llrm_mir::module::Operand::Value(value) = one { Some(*value) } else { None }).collect();
        super::transient(&module.context, &layout, function, inst, room, &live)
    }).collect::<Vec<_>>();
    assert_eq!(taken, [1, 0]);
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
    let views = |value: ValueId| segment_view(&module.context, &layout, llrm_x86_m16::spaces(), function, value);
    let at = |segments: i64| {
        let room = Room { registers: 6, across_call: 2, segments, ..Room::default() };
        let block = function.layout()[0];
        sites(function, &found, block, room, &|_| 2, &|_, _| 0, &cells, &integer, &views, &|_| false).into_iter().find(|site| matches!(function.instruction(site.inst).opcode, llrm_mir::opcode::Opcode::Load { .. })).expect("a load")
    };
    let held = at(3);
    assert_eq!(held.segments.residents.len(), 2, "both views are in segment registers");
    assert!(held.before.residents.iter().all(|&one| !views(one)));
    let none = at(0);
    assert!(none.segments.residents.is_empty() && none.before.residents.iter().filter(|&&one| views(one)).count() == 2, "no segment registers: general ones");
}

fn _peak(text: &str, room: Room) -> i64 {
    let module = module(text);
    let function = function(&module);
    let layout = llrm_analysis::testing::layout(&module);
    let found = liveness::live(function);
    let cells = cells(function);
    let integer = |value: ValueId| integer(&module.context, function, value);
    let views = |value: ValueId| segment_view(&module.context, &layout, llrm_x86_m16::spaces(), function, value);
    let addressed = addressed(function);
    let routed = |value: ValueId| addressed.contains(&value);
    let points = function.layout().iter().flat_map(|&block| sites(function, &found, block, room, &|_| 2, &|inst, live| transient(&module.context, &layout, function, inst, room, live), &cells, &integer, &views, &routed)).flat_map(Site::points);
    forecast(points, |_| 1).peak
}

fn _walk(registers: i64) -> i64 {
    _peak("define i16 @f(ptr %p) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %n, %loop ]
  %g = getelementptr i16, ptr %p, i16 %i
  %v = load i16, ptr %g
  %n = add i16 %i, 1
  %more = icmp ne i16 %n, 100
  br i1 %more, label %loop, label %exit
exit:
  ret i16 %n
}
", Room { registers, across_call: 2, ..Room::default() })
}

/// An address only its own access takes is folded into the access: it is not
/// held in a register besides its pointer and index, which the access reads.
/// Counted as a third, a loop holding a pointer and an index forecast a spill
/// on two registers and the allocator made none: 54% of the corpus's loops
/// had a forecast spill that was never made.
#[test]
fn test_an_address_folded_into_its_access_takes_no_register_of_its_own() {
    assert_eq!(_walk(2), 0, "the pointer and the index fit two registers");
    assert_eq!(_walk(1), 1, "one more than one register holds");
}

/// `p[i + 8]` is two `getelementptr`s, one the other's base, and one access:
/// folded together, the chain holds no register but its pointer and index. The
/// inner one counted as a third, so a loop of one pointer and one index
/// forecast a spill on two registers.
#[test]
fn test_a_chain_of_addresses_folds_into_its_access() {
    let text = "define i16 @f(ptr %p) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %n, %loop ]
  %a = getelementptr i8, ptr %p, i16 %i
  %b = getelementptr i8, ptr %a, i16 16
  %v = load i16, ptr %b
  %n = add i16 %i, 2
  %more = icmp ne i16 %n, 100
  br i1 %more, label %loop, label %exit
exit:
  ret i16 %n
}
";
    assert_eq!(_peak(text, Room { registers: 2, across_call: 2, ..Room::default() }), 0);
}

/// 16-bit addresses hold a pointer in BX, SI or DI: three pointers and an
/// index live through a loop did not fit three address registers though six
/// registers held them, and the allocator reloaded one every trip (conc3's
/// 40 reloads) where the model forecast no spill.
#[test]
fn test_pointers_and_an_index_fit_the_registers_an_address_may_use() {
    let text = "define i16 @f(ptr %a, ptr %b, ptr %c) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %n, %loop ]
  %s = phi i16 [ 0, %entry ], [ %s3, %loop ]
  %ga = getelementptr i16, ptr %a, i16 %i
  %va = load i16, ptr %ga
  %s1 = add i16 %s, %va
  %gb = getelementptr i16, ptr %b, i16 %i
  %vb = load i16, ptr %gb
  %s2 = add i16 %s1, %vb
  %gc = getelementptr i16, ptr %c, i16 %i
  %vc = load i16, ptr %gc
  %s3 = add i16 %s2, %vc
  %n = add i16 %i, 1
  %more = icmp ne i16 %n, 100
  br i1 %more, label %loop, label %exit
exit:
  ret i16 %s3
}
";
    let room = |addresses| Room { registers: 6, across_call: 2, addresses, ..Room::default() };
    assert_eq!(_peak(text, room(0)), 0, "no address constraint");
    assert_eq!(_peak(text, room(4)), 0, "three pointers and the index fit four");
    assert_eq!(_peak(text, room(3)), 1, "one more than three hold");
}

const HALVED: &str = "define i32 @f(ptr %base, i16 %bound) {
b1:
  br label %b2

b2:
  %iv = phi i16 [ 0, %b1 ], [ %next, %b3 ]
  %i = phi i16 [ 0, %b1 ], [ %i1, %b3 ]
  %sum = phi i32 [ 0, %b1 ], [ %sum1, %b3 ]
  %more = icmp slt i16 %i, %bound
  br i1 %more, label %b3, label %b4

b3:
  %sign = ashr i16 %i, 15
  %biased = sub i16 %i, %sign
  %half = ashr i16 %biased, 1
  %at = getelementptr i8, ptr %base, i16 %half
  %v = load i16, ptr %at
  %w = sext i16 %v to i32
  %sum1 = add i32 %sum, %w
  %i1 = add i16 %i, 1
  %next = add i16 %iv, 2
  br label %b2

b4:
  ret i32 %sum
}
";

/// A result made in its first operand's register copies an operand that stays
/// live: `x / 2` is `mov cx, dx; sub cx, di` with `dx` (the index) live after, so
/// base, bound, index, sum, a second counter and the sign held six values and the
/// copy made a seventh. No point held it: lsr then added the counter, the bound
/// went to `[bp+8]`, and the loop reloaded it each trip (#242).
#[test]
fn test_an_arithmetic_result_copies_the_first_operand_that_stays_live() {
    let room = |two_address| Room { registers: 6, across_call: 2, two_address, ..Room::default() };
    assert_eq!(_peak(HALVED, room(false)), 0, "six values in six registers");
    assert_eq!(_peak(HALVED, room(true)), 1, "the copy is a seventh");
}

/// A frame object's address and a constant offset into it, made before a loop and
/// read in its blocks, are displacements in each access (`[bp+si-188]`), not a value in a
/// register. Counted as one, nbody's four arrays' sixteen element addresses
/// forecast 23 residents on six registers, charged lsr 3000 for a counter
/// the listing kept in a register, and sent its stride-8 counter away (#386).
#[test]
fn test_a_constant_offset_into_a_frame_object_takes_no_register() {
    let text = "define i16 @f(i16 %n) {
entry:
  %a = alloca [32 x i8]
  %p = getelementptr inbounds i16, ptr %a, i16 3
  %q = getelementptr inbounds i16, ptr %a, i16 5
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %next, %latch ]
  %v = load i16, ptr %p
  %at = getelementptr i16, ptr %a, i16 %i
  %u = load i16, ptr %at
  %more = icmp ult i16 %i, %n
  br i1 %more, label %latch, label %exit
latch:
  %t = add i16 %v, %u
  %w = add i16 %t, %i
  store i16 %w, ptr %q
  %next = add i16 %i, 1
  br label %loop
exit:
  ret i16 %n
}
";
    assert_eq!(_peak(text, Room { registers: 4, across_call: 2, ..Room::default() }), 0, "%n, %i, %v and %u fit four registers; %a, %p and %q are [bp+disp]");
}

/// A frame object's address cast to the stack's space is a displacement from
/// BP, made where it is read, as the alloca itself is. Counted as a value, one
/// held across a loop forecast a spill the allocator never made, and hoist
/// refused to leave it before the loop: scanner.nib +14 B at -Os (#529).
#[test]
fn test_a_frame_address_in_the_stack_space_takes_no_register_across_a_loop() {
    let text = "declare void @g(ptr addrspace(5))
define i16 @f(i16 %n) {
entry:
  %slot = alloca [8 x i8]
  %s = addrspacecast ptr %slot to ptr addrspace(5)
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %j, %loop ]
  call void @g(ptr addrspace(5) %s)
  %j = add i16 %i, 1
  %more = icmp ult i16 %j, %n
  br i1 %more, label %loop, label %exit
exit:
  ret i16 %j
}
";
    assert_eq!(_peak(text, Room { registers: 2, across_call: 2, ..Room::default() }), 0);
}

/// Every access asked whether its pointer is folded, which looks at every user of the pointer, and a frame
/// slot has a user for each access to it: 34% of compiling a function of 1600 statements (#560). Each
/// pointer is asked once.
#[test]
fn test_each_pointer_is_asked_whether_it_is_folded_once_however_many_accesses_use_it() {
    let accesses: String = (0..40).map(|at| format!("  store i16 {at}, ptr %slot\n  %v{at} = load i16, ptr %slot\n")).collect();
    let module = crate::testing::parsed(&format!("{}define i16 @f() {{\nentry:\n  %slot = alloca i16\n{accesses}  ret i16 %v39\n}}\n", llrm_analysis::testing::DOS));
    let function = llrm_analysis::testing::function(&module, "f");
    let before = super::folded_runs();
    super::addressed(function);
    assert!(super::folded_runs() - before <= 2, "{} asks for 80 accesses of one slot", super::folded_runs() - before);
}

/// nbody's `x[i]`, made in the outer loop and read in the inner one: on a target whose address takes a scale
/// of 8 beside any registers it is `[ebp+esi*8+disp]` at each read, and holds no register of its own; the
/// model counted four of them live across the inner loop, and hoisted the floats those pointers read so as
/// to free them (#698: -Os m32 +4 B and two `fstp st(0)`). Where the address takes no such scale it is a value.
#[test]
fn test_a_frame_object_indexed_by_a_scaled_integer_is_folded_wherever_it_is_read() {
    let module = module("define double @f(i32 %n) {
entry:
  %buf = alloca [32 x double]
  br label %outer
outer:
  %i = phi i32 [ 0, %entry ], [ %next, %latch ]
  %scaled = mul i32 %i, 8
  %p = getelementptr inbounds i8, ptr %buf, i32 %scaled
  %q = getelementptr inbounds i8, ptr %p, i32 32
  br label %inner
inner:
  %j = phi i32 [ 0, %outer ], [ %j2, %inner ]
  %v = load double, ptr %q
  %j2 = add i32 %j, 1
  %more = icmp slt i32 %j2, %n
  br i1 %more, label %inner, label %latch
latch:
  %next = add i32 %i, 1
  %again = icmp slt i32 %next, %n
  br i1 %again, label %outer, label %done
done:
  ret double 0.0
}
");
    let function = self::function(&module);
    let (p, q) = (named(function, "p"), named(function, "q"));
    // The scale of 8 is bit 3.
    use crate::spill::folded_in;
    assert!(folded_in(&module.context, function, p, 0b1000) && folded_in(&module.context, function, q, 0b1000));
    assert!(!folded_in(&module.context, function, p, 0) && !folded_in(&module.context, function, q, 0));
    // A scale the address does not take is no fold.
    assert!(!folded_in(&module.context, function, q, 0b0011));
}

/// nbody_single's `&pos_x[k]`, made once and read in a loop: a symbol plus a constant is a displacement at each
/// read, as a frame object plus a constant is, but only the latter was: the model held 21 such addresses as
/// registers across the nest, forecast 5389056 clocks of spills and the allocator spilled nothing.
#[test]
fn test_a_symbol_plus_a_constant_is_no_register_wherever_it_is_read() {
    let module = module("@table = global [8 x i32] zeroinitializer
define i32 @f(i32 %n) {
entry:
  %p = getelementptr inbounds i8, ptr @table, i32 8
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %v = load i32, ptr %p
  %next = add i32 %i, %v
  %more = icmp slt i32 %next, %n
  br i1 %more, label %loop, label %done
done:
  ret i32 %next
}
");
    let function = self::function(&module);
    assert!(!integer(&module.context, function, named(function, "p")));
}

/// Lsr took its sites from a `Pressure` the manager made, the hoist from one made for the candidate: the same
/// function gets the same forecast either way, and `hide` removes a value from what is live.
#[test]
fn test_a_forecast_is_the_same_from_the_managers_pressure_as_from_one_made_for_the_candidate() {
    let module = module(COUNTED);
    let function = self::function(&module);
    let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap_or("")).expect("a layout");
    let room = Room { registers: 1, across_call: 1, ..Room::default() };
    let across = |_: InstId| 1;
    let costs = OperationCosts { load: 3, store: 5, ..OperationCosts::default() };
    let frequency = function.layout().iter().map(|&block| (cfg::id(block), 256)).collect();
    let made = View::of(&module.context, &layout, function, room, &across);
    let pressure = Pressure::of(&module.context, function, room.index_scales);
    let kept = View::over(&pressure, &module.context, &layout, function, room, &across);
    let (one, two) = (made.forecast(&costs, &frequency), kept.forecast(&costs, &frequency));
    assert!(one.peak > 0, "the room of one register is crowded");
    assert_eq!((one.cost, &one.spilled, one.peak), (two.cost, &two.spilled, two.peak));
    let hidden = |_: ValueId| true;
    let residents = |view: &View, hide: &dyn Fn(ValueId) -> bool| function.layout().iter().flat_map(|&block| view.sites(block, hide)).map(|site| site.before.residents.len()).sum::<usize>();
    assert!(residents(&kept, &hidden) < residents(&kept, &|_| false));
}
