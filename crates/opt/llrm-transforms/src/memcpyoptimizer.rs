//! Adjacent stores of one repeated byte become a `llvm.memset`: LLVM's
//! `MemCpyOptPass`, stage 1 (`runImpl`, `iterateOnFunction`, `processStore`,
//! `processMemSet`, `tryMergingIntoMemset`, `MemsetRanges`).
//!
//! # How it works
//! Each block is walked in order. A simple store whose value is a repeated byte
//! (`isBytewiseValue`), or a `llvm.memset` of a constant length, starts a scan
//! of the instructions after it, up to the terminator. The scan keeps the
//! stores and memsets of the same byte at a constant offset from the start
//! pointer in `MemsetRanges`, the sorted byte ranges they cover, and stops at
//! the first instruction that touches memory otherwise (a store of another
//! byte, a load, a call). Every range of at least two instructions that is
//! worth it (`isProfitableToUseMemset`: four stores or sixteen bytes; fewer
//! only where the memset needs fewer stores of the widest native integer, and
//! always where a memset is among them) becomes one memset placed where the
//! scan stopped, and its stores go. The walk goes on from the last memset made.
//! The function is walked again until a walk changes nothing.
//!
//! ```text
//!   store i8 0, ptr %p                call memset(%p, 0, 4)
//!   store i8 0, ptr %p+1              (three stores gone)
//!   store i8 0, ptr %p+2        ==>
//!   store i8 0, ptr %p+3
//!   <first instruction that is not one of them>
//! ```
//!
//! MIRROR: `MemsetRanges::addRange`, `isProfitableToUseMemset`,
//! `tryMergingIntoMemset`, `processStore`, `processMemSet`,
//! `iterateOnFunction`, `runImpl`.
//!
//! DEVIATE 1: `getPointerOffsetFrom` is `MemRef`'s root and displacement: two
//! pointers of one root with no variable index, no segment. Exit: none needed.
//! DEVIATE 2: a call that only accesses inaccessible memory ends the scan
//! (LLVM steps over it); `memory::of` has no such effect. Exit: model the
//! effect.
//! DEVIATE 3: only integer stores are bytewise (LLVM's `isBytewiseValue` also
//! reads floats, null and aggregates), and `processStore`'s promotion of an
//! aggregate store to a memset is absent. Exit: S2, with the float constants.
//! DEVIATE 4: unreachable blocks are walked. Exit: none needed.
//! DEVIATE 5: `processStoreOfLoad`, `processMemCpy` and the rest are stages 2
//! and 3.
//!
//! # Example
//! Four byte stores:
//!
//! ```mir
//! store i8 0, ptr %p
//! %q1 = getelementptr i8, ptr %p, i16 1
//! store i8 0, ptr %q1
//! ...
//! ```
//!
//! become `call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 4, i1 false)`.
//!
//! # Runs
//! In the slot of `merge@818`, where `loopidioms::Merge` ran.
//!
//! # Keeps
//! The blocks and edges: only instructions change.
//!
//! # See also
//! LLVM
//! [`MemCpyOptimizer.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MemCpyOptimizer.cpp#L348).

use llrm_analysis::memory::{MemRef, Unit};
use llrm_analysis::num::Num as BigInt;
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::memory::{self, Effects};
use llrm_mir::module::{Function, InstId, Operand};
use llrm_mir::opcode::{CallInfo, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, Declared, FunctionPass, Outer, PreservedAnalyses};
use llrm_mir::types::{FloatKind, Type};

use crate::counting;
use crate::loopidioms::_memset;

thread_local! {
    static STEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The instructions the scans of this thread have looked at, for a test that a
/// block of many runs is not scanned once for each.
pub fn steps() -> usize {
    STEPS.with(std::cell::Cell::get)
}

pub struct MemCpyOptimizer;

impl FunctionPass for MemCpyOptimizer {
    fn name(&self) -> &'static str {
        "memcpyopt"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = analyses.outer();
        let changed = run_impl(unit.context, unit.layout, unit.function, outer, unit.declared);
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// The byte a store or memset repeats: a constant, or the one value of an
/// `i8` (`isBytewiseValue`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Byte {
    Constant(u128),
    Value(Operand),
}

/// `MemsetRange`: bytes `start..end` of one object stored by `stores`, the
/// pointer of the lowest one.
struct Range {
    start: i64,
    end: i64,
    start_ptr: Operand,
    /// The alignment of the access at `start_ptr`: the memset made keeps it
    /// (`MemsetRange::Alignment`).
    align: Option<u64>,
    stores: Vec<(InstId, bool)>,
}

/// `MemsetRange::isProfitableToUseMemset`; `widest` is the largest legal
/// integer's bytes.
fn profitable(
    range: &Range,
    widest: i64,
) -> bool {
    // More than four stores or sixteen bytes.
    if range.stores.len() >= 4 || range.end - range.start >= 16 {
        return true;
    }
    // Nothing to merge.
    if range.stores.len() < 2 {
        return false;
    }
    // A memset among them is always worth extending.
    if range.stores.iter().any(|(_, store)| !store) {
        return true;
    }
    // The code generator pairs stores itself.
    if range.stores.len() == 2 {
        return false;
    }
    // Fewer stores of the widest integer and single bytes than there are.
    let bytes = range.end - range.start;
    let widest = widest.max(1);
    range.stores.len() as i64 > bytes / widest + bytes % widest
}

/// `MemsetRanges::addRange`: the sorted ranges, one more instruction merged in.
fn add_range(
    ranges: &mut Vec<Range>,
    start: i64,
    size: i64,
    ptr: Operand,
    align: Option<u64>,
    inst: (InstId, bool),
) {
    let end = start + size;
    let at = ranges.partition_point(|one| one.end < start);
    // Nothing to merge with: a new range.
    if at == ranges.len() || end < ranges[at].start {
        ranges.insert(at, Range { start, end, start_ptr: ptr, align, stores: vec![inst] });
        return;
    }
    ranges[at].stores.push(inst);
    // Contained.
    if ranges[at].start <= start && ranges[at].end >= end {
        return;
    }
    if start < ranges[at].start {
        ranges[at].start = start;
        ranges[at].start_ptr = ptr;
        ranges[at].align = align;
    }
    if end > ranges[at].end {
        ranges[at].end = end;
        while at + 1 < ranges.len() && end >= ranges[at + 1].start {
            let next = ranges.remove(at + 1);
            ranges[at].stores.extend(next.stores);
            ranges[at].end = ranges[at].end.max(next.end);
        }
    }
}

/// `isBytewiseValue` of an integer or float `value`: a float is the integer
/// of its bits (`+0.0` is the zero byte, `-0.0` is not).
fn bytewise(
    unit: &Unit,
    value: Operand,
) -> Option<Byte> {
    let repeated = |bits: u128, width: u32| {
        let byte = bits & 0xFF;
        (width % 8 == 0 && width != 0 && (0..width / 8).all(|at| (bits >> (8 * at)) & 0xFF == byte))
            .then_some(Byte::Constant(byte))
    };
    if let Some(width) = unit.int_bits(value) {
        if let Some(bits) = unit.int_constant(value) {
            return repeated(bits, width);
        }
        return (width == 8).then_some(Byte::Value(value));
    }
    let Operand::Constant(id) = value else { return None };
    let ty = unit.operand_type(value)?;
    let width = match unit.context.types.get(ty) {
        Type::Float(FloatKind::Float) => 32,
        Type::Float(FloatKind::Double) => 64,
        _ => return None,
    };
    match unit.context.get(id).kind {
        ConstantKind::Float(bits) => repeated(u128::from(bits), width),
        ConstantKind::Null | ConstantKind::Zero => repeated(0, width),
        _ => None,
    }
}

/// The bytes a store of `value` writes: an integer's or a `float`'s or
/// `double`'s.
fn stored_bytes(
    unit: &Unit,
    value: Operand,
) -> Option<i64> {
    if let Some(bits) = unit.int_bits(value) {
        return Some(i64::from(bits / 8));
    }
    match unit.context.types.get(unit.operand_type(value)?) {
        Type::Float(FloatKind::Float) => Some(4),
        Type::Float(FloatKind::Double) => Some(8),
        _ => None,
    }
}

/// The store `inst` is, when it is simple: its value and pointer.
fn simple_store(
    function: &Function,
    inst: InstId,
) -> Option<(Operand, Operand)> {
    let op = function.instruction(inst);
    let Opcode::Store { volatile: false, .. } = op.opcode else { return None };
    Some((op.operands[0], op.operands[1]))
}

/// The alignment a store or memset states of its address, a store without one
/// the type's (`StoreInst::getAlign`).
fn alignment(
    unit: &Unit,
    inst: InstId,
) -> Option<u64> {
    let op = unit.function.instruction(inst);
    match &op.opcode {
        Opcode::Store { align, .. } => {
            align.or_else(|| Some(unit.layout.align(&unit.context.types, unit.operand_type(op.operands[0])?)))
        }
        Opcode::Call(info) => info.argument_attrs.first()?.iter().find_map(|attr| match attr {
            llrm_mir::opcode::Attribute::Int(name, bytes) if name == "align" => Some(*bytes),
            _ => None,
        }),
        _ => None,
    }
}

/// A `llvm.memset` of a constant length that is not volatile: its destination,
/// byte and length.
fn constant_memset(
    unit: &Unit,
    callees: &memory::Callees,
    function: &Function,
    inst: InstId,
) -> Option<(Operand, Operand, i64)> {
    let (dest, byte, length) = memory::memset(unit.context, callees, function, inst)?;
    let volatile = function.instruction(inst).operands[3];
    if unit.int_constant(volatile) != Some(0) {
        return None;
    }
    let length = i64::try_from(unit.int_constant(length)?).ok()?;
    Some((dest, byte, length))
}

/// `byte` as the operand a memset takes.
fn byte_operand(
    context: &Context,
    byte: Byte,
) -> Operand {
    match byte {
        Byte::Constant(byte) => counting::constant(context, &BigInt::from(byte), 8),
        Byte::Value(value) => value,
    }
}

/// A call to `llvm.memset` before `before`.
fn create_memset(
    context: &mut Context,
    declared: &mut Declared,
    function: &mut Function,
    (space, width): (u32, u32),
    (pointer, byte, length): (Operand, Operand, i64),
    align: Option<u64>,
    before: InstId,
) -> InstId {
    let (callee, function_type) = _memset(context, declared, space, width);
    let ty = context.types.ptr(0);
    let callee = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Global(callee) }));
    let count = counting::constant(context, &BigInt::from(length), width);
    let off = counting::constant(context, &BigInt::from(0), 1);
    let void = context.types.void();
    let info = CallInfo {
        function_type,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs: vec![
            align.map(|bytes| llrm_mir::opcode::Attribute::Int("align".to_owned(), bytes)).into_iter().collect(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ],
        attrs: Vec::new(),
        tail: Default::default(),
    };
    let call = function.create_instruction(
        Opcode::Call(Box::new(info)),
        void,
        vec![pointer, byte, count, off, callee],
        Flags::default(),
        None,
    );
    function.insert(call, Position::Before(before)).expect("a placed instruction");
    call
}

/// What `try_merging_into_memset` decided: the memsets to make, in range order,
/// and the instruction they go before.
struct Plan {
    memsets: Vec<(Operand, i64, (u32, u32), Option<u64>)>,
    stores: Vec<Vec<InstId>>,
    before: InstId,
}

/// `tryMergingIntoMemset`: `start` is a store or memset of `byte` at `ptr`.
/// The last memset made.
fn try_merging_into_memset(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
    declared: &mut Declared,
    start: InstId,
    start_ptr: Operand,
    byte: Byte,
) -> Option<InstId> {
    let callees = outer.callees();
    let plan = {
        let unit = Unit::within(context, layout, function, outer);
        let widest = i64::from(layout.largest_legal_integer() / 8);
        let reference = |ptr: Operand, size: i64| -> Option<MemRef> {
            let found = MemRef::at(&unit, ptr, size as u32);
            (found.root.is_some() && found.base.is_none() && found.segment.is_none()).then_some(found)
        };
        let (start_size, start_inst) = match simple_store(function, start) {
            Some((value, _)) => (stored_bytes(&unit, value)?, (start, true)),
            None => (constant_memset(&unit, callees, function, start)?.2, (start, false)),
        };
        let base = reference(start_ptr, start_size)?;
        let mut ranges: Vec<Range> = Vec::new();
        let mut at = function.next_in_block(start)?;
        let stopped = loop {
            STEPS.with(|steps| steps.set(steps.get() + 1));
            let inst = function.instruction(at);
            if inst.opcode.is_terminator() {
                break at;
            }
            let found = if let Some((value, pointer)) = simple_store(function, at) {
                // A store of the same byte at a constant offset from the start.
                (bytewise(&unit, value) == Some(byte)).then(|| (pointer, stored_bytes(&unit, value).unwrap_or(1), true))
            } else if let Some((dest, other, length)) = constant_memset(&unit, callees, function, at) {
                let same = match byte {
                    Byte::Constant(wanted) => unit.int_constant(other) == Some(wanted),
                    Byte::Value(value) => other == value,
                };
                same.then_some((dest, length, false))
            } else if memory::of(unit.context, callees, function, at) == Effects::NONE {
                // Touches no memory: stepped over.
                at = function.next_in_block(at)?;
                continue;
            } else {
                None
            };
            let Some((pointer, size, is_store)) = found else { break at };
            let Some(next) = reference(pointer, size) else { break at };
            if next.root != base.root {
                break at;
            }
            add_range(&mut ranges, next.disp - base.disp, size, pointer, alignment(&unit, at), (at, is_store));
            at = function.next_in_block(at)?;
        };
        // Only the start: a single store with nothing to merge.
        if ranges.is_empty() {
            return None;
        }
        add_range(&mut ranges, 0, start_size, start_ptr, alignment(&unit, start), start_inst);
        let mut plan = Plan { memsets: Vec::new(), stores: Vec::new(), before: stopped };
        for range in &ranges {
            if range.stores.len() == 1 || !profitable(range, widest) {
                continue;
            }
            let space = unit.space(range.start_ptr).unwrap_or(0);
            let width = layout.pointer(space).index_bits;
            plan.memsets.push((range.start_ptr, range.end - range.start, (space, width), range.align));
            plan.stores.push(range.stores.iter().map(|(inst, _)| *inst).collect());
        }
        plan
    };
    let mut last = None;
    for ((pointer, length, space, align), stores) in plan.memsets.into_iter().zip(plan.stores) {
        let operand = byte_operand(context, byte);
        last = Some(create_memset(context, declared, function, space, (pointer, operand, length), align, plan.before));
        for store in stores {
            function.erase(store).expect("a store or call has no result to use");
        }
    }
    last
}

/// `iterateOnFunction`: whether anything changed.
fn iterate_on_function(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
    declared: &mut Declared,
) -> bool {
    let callees = outer.callees();
    let mut changed = false;
    for block in function.layout().to_vec() {
        let mut next = function.block(block).first();
        while let Some(inst) = next {
            next = function.next_in_block(inst);
            let candidate = {
                let unit = Unit::within(context, layout, function, outer);
                if let Some((value, pointer)) = simple_store(function, inst) {
                    bytewise(&unit, value).map(|byte| (pointer, byte))
                } else {
                    // `processMemSet`.
                    constant_memset(&unit, callees, function, inst).and_then(|(dest, byte, _)| {
                        let byte = match unit.int_constant(byte) {
                            Some(constant) => Byte::Constant(constant & 0xFF),
                            None => Byte::Value(byte),
                        };
                        Some((dest, byte))
                    })
                }
            };
            let Some((pointer, byte)) = candidate else { continue };
            if let Some(made) = try_merging_into_memset(context, layout, function, outer, declared, inst, pointer, byte)
            {
                changed = true;
                // Not invalidated: the walk goes on from the memset.
                next = Some(made);
            }
        }
    }
    changed
}

/// `runImpl`: the function walked until a walk changes nothing.
pub fn run_impl(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
    declared: &mut Declared,
) -> bool {
    let mut changed = false;
    while iterate_on_function(context, layout, function, outer, declared) {
        changed = true;
    }
    changed
}

#[cfg(test)]
#[path = "memcpyoptimizer_tests.rs"]
mod tests;
