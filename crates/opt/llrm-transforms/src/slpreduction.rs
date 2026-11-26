//! `SLPVectorizer`'s horizontal reductions: a chain of one associative
//! integer operation over loads (or extensions of loads) of consecutive
//! cells becomes vector loads and one `llvm.vector.reduce.*`.
//!
//! # How it works
//! `vectorizeHorReduction` (l.34447) takes the last operation of a chain as
//! the root. `matchAssociativeReduction` (l.30842) walks the chain's links (an
//! operand of the same operation with one use, in the root's block) and
//! collects the other operands, the reduced values, in root-to-leaf order;
//! those that are loads, alike in their extension and their object, form a
//! slice, each slice read from its last value to its first (ascending cells).
//! `tryToReduce` (l.31874) cuts a slice into windows: the width starts at
//! `GetVectorFactor(n)` (the largest the registers hold) and falls where a
//! window is not profitable; after a window the rest is cut again. A window is
//! a tree of two entries, the loads and their extension; its cost is the
//! vector load less the scalar ones, the extension (free: the reduction
//! absorbs it, `IsArithmeticExtendedReduction`), and the reduction (the first
//! part: the extended reduction, less the scalar chain; the later parts: one
//! vector operation and one extension). It is taken below the
//! `slp-threshold` of 0. The parts are joined by `emitReduction` (l.34077):
//! the narrower part is added to the low lanes of the wider, and
//! `llvm.vector.reduce.*` of the result is combined with the values left over
//! as `op.rdx` (flags other than wrap are what `createOp` keeps).
//!
//! DEVIATE SLP-RDX1 (owner w11, exit 2026-11-24): the old load-slice path
//! runs first and the tree path only when it finds nothing; the tree's total
//! on jpeg_fixed is -6 where the pin's is -4 (no per-entry oracle yet); the
//! windows of loads that are already in ascending order and whose scalars
//! have one use; no
//! reordered or externally used lanes (`ExternalUses`, the reorder
//! shuffle), no scale by the loop's trip count (it multiplies every entry and
//! the reduction alike, so the sign of the total is the one used here), no
//! `computeMinimumValueSizes` narrowing, the move of a load past a store by
//! alias analysis (here: nothing between the first and last load of a window
//! touches memory), the reductions of floats, min/max, booleans, the loop
//! accumulator form and `AdjustReducedVals`' shifted windows.
//!
//! # Runs
//! With the store chains, at the pin's `slp-vectorizer` slot.
//!
//! # Used by
//! `slpvectorize`.
//!
//! # Example
//! Forty `sext i16` loads of consecutive cells, summed by a chain of `add i64`,
//! become two vector loads and one reduction (the tile test).
//!
//! ```text
//! %x0 = sext i16 (load [i]) to i64 ; ... x39, summed by add i64
//!         =>
//! load <32 x i16>, load <8 x i16>; the narrower is added into the low
//! lanes of the wider; one @llvm.vector.reduce.add.v32i64 of the result
//! ```

use llrm_analysis::memory::{MemRef, Unit as MemUnit};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::ReduceKind;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CallInfo, CastOp, Flags, Opcode, Tail};
use llrm_mir::passes::{Declared, Outer};
use llrm_mir::tti::{ArithOp, CastKind, CostKind, MemoryOp, OperandInfo, Tti, Ty, VectorTy};
use llrm_mir::types::{Type, TypeId};

/// `-slp-threshold`.
const COST_THRESHOLD: i64 = 0;
/// `HorizontalReduction::ReductionLimit` (`VectorizeNonPowerOf2` off).
const REDUCTION_LIMIT: usize = 4;
/// `RegMaxNumber * RedValsMaxNumber`, the widest window.
const MAX_ELTS: usize = 128;

/// The reduction a chain of `op` makes.
fn reduce_kind(op: BinaryOp) -> Option<ReduceKind> {
    match op {
        BinaryOp::Add => Some(ReduceKind::Add),
        BinaryOp::Mul => Some(ReduceKind::Mul),
        BinaryOp::And => Some(ReduceKind::And),
        BinaryOp::Or => Some(ReduceKind::Or),
        BinaryOp::Xor => Some(ReduceKind::Xor),
        _ => None,
    }
}

fn arith_of(op: BinaryOp) -> ArithOp {
    match op {
        BinaryOp::Add => ArithOp::Add,
        BinaryOp::Mul => ArithOp::Mul,
        BinaryOp::And => ArithOp::And,
        BinaryOp::Or => ArithOp::Or,
        BinaryOp::Xor => ArithOp::Xor,
        other => unreachable!("not a reduction operation: {other:?}"),
    }
}

fn int_ty(
    context: &Context,
    ty: TypeId,
) -> Option<u32> {
    match context.types.get(ty) {
        Type::Int(bits @ (8 | 16 | 32 | 64)) => Some(*bits),
        _ => None,
    }
}

/// One reduced value: a load, or an extension of one.
#[derive(Clone)]
struct Leaf {
    value: Operand,
    load: InstId,
    cast: Option<(InstId, CastOp)>,
    /// The loaded type and the load's cell: the object, the offset.
    loaded: TypeId,
    root: Operand,
    /// The variable index the cells share, and its scale.
    base: (Option<llrm_mir::module::ValueId>, i64),
    disp: i64,
    align: Option<u64>,
}

/// A chain of one operation: its links, the root, and the reduced values.
struct Chain {
    op: BinaryOp,
    root: InstId,
    links: Vec<InstId>,
    /// In `matchAssociativeReduction`'s order: from the root's last operand.
    values: Vec<Operand>,
}

fn is_link(
    function: &Function,
    block: BlockId,
    op: BinaryOp,
    value: Operand,
) -> Option<InstId> {
    let Operand::Value(id) = value else { return None };
    let ValueDef::Instruction(inst) = function.value(id).def else { return None };
    let instruction = function.instruction(inst);
    (function.parent(inst) == Some(block)
        && instruction.opcode == Opcode::Binary(op)
        && function.users(id).iter().count() == 1)
        .then_some(inst)
}

/// `matchAssociativeReduction`: the chain `root` ends.
fn match_chain(
    context: &Context,
    function: &Function,
    block: BlockId,
    root: InstId,
) -> Option<Chain> {
    let Opcode::Binary(op) = function.instruction(root).opcode else { return None };
    reduce_kind(op)?;
    int_ty(context, function.instruction(root).ty)?;
    let mut links = Vec::new();
    let mut values = Vec::new();
    let mut work = vec![root];
    while let Some(link) = work.pop() {
        links.push(link);
        let operands = function.instruction(link).operands.clone();
        for operand in operands.iter().rev() {
            match is_link(function, block, op, *operand) {
                Some(inner) => work.push(inner),
                None => values.push(*operand),
            }
        }
    }
    (links.len() >= REDUCTION_LIMIT - 1).then_some(Chain { op, root, links, values })
}

/// The reduced value as a leaf of a slice, when it is a simple load or the
/// extension of one, each with one use.
fn leaf_of(
    unit: &MemUnit,
    function: &Function,
    block: BlockId,
    value: Operand,
) -> Option<Leaf> {
    let Operand::Value(id) = value else { return None };
    let ValueDef::Instruction(first) = function.value(id).def else { return None };
    if function.parent(first) != Some(block) || function.users(id).iter().count() != 1 {
        return None;
    }
    let (cast, load) = match function.instruction(first).opcode {
        Opcode::Cast(op @ (CastOp::SExt | CastOp::ZExt)) => {
            let Operand::Value(inner) = function.instruction(first).operands[0] else { return None };
            let ValueDef::Instruction(load) = function.value(inner).def else { return None };
            if function.parent(load) != Some(block) || function.users(inner).iter().count() != 1 {
                return None;
            }
            (Some((first, op)), load)
        }
        _ => (None, first),
    };
    let Opcode::Load { align, volatile: false } = function.instruction(load).opcode else { return None };
    let loaded = function.instruction(load).ty;
    // `LoadInst::getAlign`: no `align` is the ABI alignment of the loaded type.
    let align = align.or_else(|| Some(unit.layout.align(&unit.context.types, loaded)));
    let bits = int_ty(unit.context, loaded)?;
    let reference = MemRef::at(unit, function.instruction(load).operands[0], bits / 8);
    let root = reference.root.filter(|_| reference.segment.is_none())?;
    Some(Leaf { value, load, cast, loaded, root, base: (reference.base, reference.scale), disp: reference.disp, align })
}

/// `GetVectorFactor`: the width `n` values are cut to so that they fill
/// registers (`getFloorFullVectorNumberOfElements`).
fn vector_factor(
    tti: &dyn Tti,
    lane_bits: u32,
    n: usize,
) -> usize {
    if n < 2 {
        return n;
    }
    let register = tti.vector_register_bits().max(lane_bits);
    let parts = (n as u64 * u64::from(lane_bits)).div_ceil(u64::from(register)) as usize;
    let floor = 1usize << (usize::BITS - 1 - n.leading_zeros());
    let mut width = if parts == 0 || parts >= n {
        floor
    } else {
        let per_register = n.div_ceil(parts).next_power_of_two();
        if per_register > n { floor } else { n / per_register * per_register }
    };
    let registers = tti.number_of_registers(llrm_mir::tti::RegisterClass::Vector) as usize;
    let parts_of = |width: usize| (width as u64 * u64::from(lane_bits)).div_ceil(u64::from(register)) as usize;
    while registers != 0 && parts_of(width) > registers && width > 1 {
        width = 1usize << (usize::BITS - 1 - (width - 1).leading_zeros());
    }
    if registers != 0 && parts_of(width) > registers / 2 {
        width = 1usize << (usize::BITS - 1 - width.leading_zeros());
    }
    width.min(MAX_ELTS)
}

fn lane_ty(bits: u32) -> Ty {
    Ty::Int(bits)
}

/// The cost of the window's entries and of the reduction (the scalar chain
/// less the vector reduction): the sum is below `-COST_THRESHOLD`.
#[allow(clippy::too_many_arguments)]
fn worthwhile(
    context: &Context,
    function: &Function,
    tti: &dyn Tti,
    op: BinaryOp,
    reduced_bits: u32,
    window: &[Leaf],
    first_part: bool,
) -> bool {
    let kind = crate::slptree::cost_kind(function);
    let width = window.len() as u32;
    let load_bits = int_ty(context, window[0].loaded).expect("an integer load");
    let load_lane = lane_ty(load_bits);
    let valid = |cost: llrm_mir::tti::Cost| cost.valid.then_some(cost.value);
    // The loads entry: the vector load less the scalar ones.
    let align = window[0].align.unwrap_or(u64::from(load_bits / 8));
    let Some(vector_load) = tti
        .vector_memory_op_cost(MemoryOp::Load, VectorTy { lane: load_lane, count: width }, align, kind)
        .and_then(valid)
    else {
        llrm_support::debug!("slp", "reduction x{width}: the vector load is not priced");
        {
            llrm_support::debug!("slp", "reduction x{width}: a cost is not priced (line {})", line!());
            return false;
        }
    };
    let Some(scalar_load) = valid(tti.memory_op_cost(MemoryOp::Load, load_lane, kind, false)) else { return false };
    let mut total = vector_load - scalar_load * i64::from(width);
    // The extension entry is free where the reduction absorbs it.
    let reduced = lane_ty(reduced_bits);
    let narrow = VectorTy { lane: load_lane, count: width };
    let wide = VectorTy { lane: reduced, count: width };
    let extend = window[0]
        .cast
        .map(
            |(_, op)| match op {
                CastOp::SExt => CastKind::SExt,
                _ => CastKind::ZExt,
            },
        );
    // `getReductionCost`: the vector side less the scalar chain.
    let Some(arith) =
        valid(tti.arithmetic_cost(arith_of(op), reduced, kind, OperandInfo::default(), OperandInfo::default()))
    else {
        {
            llrm_support::debug!("slp", "reduction x{width}: a cost is not priced (line {})", line!());
            return false;
        }
    };
    let scalar_chain = arith * i64::from(width - 1);
    let cast_of = |extend: Option<CastKind>| -> Option<i64> {
        match extend {
            Some(cast_kind) => tti.vector_cast_cost(cast_kind, wide, narrow, kind).and_then(valid),
            None => Some(0),
        }
    };
    let vector = if first_part {
        // `getExtendedReductionCost`: the target's native form, else `BaseT`'s,
        // the reduction of the extended vector and the extension.
        let native = extend.and_then(|_| {
            tti.vector_extended_reduction_cost(
                reduce_kind(op).expect("a reduction"),
                extend == Some(CastKind::ZExt),
                reduced,
                narrow,
                kind,
            )
            .and_then(valid)
        });
        match native {
            Some(cost) => cost,
            None => {
                let Some(reduce) = tti
                    .vector_reduction_cost(
                        reduce_kind(op).expect("a reduction"),
                        wide_or(narrow, wide, extend),
                        false,
                        kind,
                    )
                    .and_then(valid)
                else {
                    llrm_support::debug!("slp", "reduction x{width}: the reduction is not priced");
                    return false;
                };
                let Some(cast) = cast_of(extend) else {
                    llrm_support::debug!("slp", "reduction x{width}: the extension is not priced");
                    return false;
                };
                reduce + cast
            }
        }
    } else {
        let Some(add) = tti.vector_arithmetic_cost(arith_of(op), narrow, kind).and_then(valid) else {
            llrm_support::debug!("slp", "reduction x{width}: the vector operation is not priced");
            return false;
        };
        let Some(cast) = cast_of(extend) else {
            llrm_support::debug!("slp", "reduction x{width}: the extension is not priced");
            return false;
        };
        add + cast
    };
    total += vector - scalar_chain;
    llrm_support::debug!(
        "slp",
        "reduction x{width}: loads {}, reduction {} less chain {scalar_chain}: {total}",
        vector_load - scalar_load * i64::from(width),
        vector
    );
    total < -COST_THRESHOLD
}

/// The vector type the reduction is made over.
fn wide_or(
    narrow: VectorTy,
    wide: VectorTy,
    extend: Option<CastKind>,
) -> VectorTy {
    if extend.is_some() { wide } else { narrow }
}

/// Nothing between the first and last load of a window writes memory.
fn movable(
    function: &Function,
    block: BlockId,
    window: &[Leaf],
) -> bool {
    let first = window.iter().map(|leaf| position(function, block, leaf.load)).min().expect("a window");
    let last = window.iter().map(|leaf| position(function, block, leaf.load)).max().expect("a window");
    function
        .iter_block(block)
        .skip(first)
        .take(last - first + 1)
        .all(
            |inst| !matches!(
                function.instruction(inst).opcode,
                Opcode::Store { .. } | Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Atomic(_) | Opcode::VaArg
            ),
        )
}

fn position(
    function: &Function,
    block: BlockId,
    inst: InstId,
) -> usize {
    function.iter_block(block).position(|one| one == inst).expect("an instruction of the block")
}

/// A window taken: its place in the slice.
struct Part {
    start: usize,
    width: usize,
}

/// `tryToReduce`'s loop over one slice: the windows taken.
fn windows(
    context: &Context,
    tti: &dyn Tti,
    function: &Function,
    block: BlockId,
    chain: &Chain,
    slice: &[Leaf],
) -> Vec<Part> {
    let Some(reduced_bits) = int_ty(context, function.instruction(chain.root).ty) else { return Vec::new() };
    let load_bits = int_ty(context, slice[0].loaded).expect("an integer load");
    let n = slice.len();
    if n < REDUCTION_LIMIT {
        return Vec::new();
    }
    let lane_bits = reduced_bits.max(load_bits);
    let mut parts: Vec<Part> = Vec::new();
    let mut width = vector_factor(tti, lane_bits, n).min(MAX_ELTS);
    let mut pos = 0;
    while pos + width <= n && width >= REDUCTION_LIMIT {
        let window = &slice[pos..pos + width];
        llrm_support::debug!("slp", "reduction window {pos}..{} of {n}", pos + width);
        let consecutive = window.windows(2).all(|pair| pair[1].disp == pair[0].disp + i64::from(load_bits / 8));
        if consecutive
            && movable(function, block, window)
            && worthwhile(context, function, tti, chain.op, reduced_bits, window, parts.is_empty())
        {
            parts.push(Part { start: pos, width });
            pos += width;
            width = if n - pos > 1 { vector_factor(tti, lane_bits, n - pos) } else { 0 };
        } else {
            width = if width > 1 { vector_factor(tti, lane_bits, width - 1) } else { 0 };
        }
    }
    parts
}

fn vector_of(
    context: &mut Context,
    element: TypeId,
    count: usize,
) -> TypeId {
    context.types.intern(Type::Vector { element, count: count as u32 })
}

fn result_of(
    function: &Function,
    inst: InstId,
) -> Operand {
    Operand::Value(function.instruction(inst).result.expect("a value"))
}

/// The vector load of a window (stands after the window's last load), and its
/// extension to the reduced type.
fn emit_part(
    context: &mut Context,
    function: &mut Function,
    block: BlockId,
    window: &[Leaf],
    reduced: TypeId,
    root: InstId,
) -> Operand {
    let last = window.iter().max_by_key(|leaf| position(function, block, leaf.load)).expect("a window");
    let first = &window[0];
    let vector_ty = vector_of(context, first.loaded, window.len());
    let pointer = function.instruction(first.load).operands[0];
    let load = function.create_instruction(
        Opcode::Load { align: first.align, volatile: false },
        vector_ty,
        vec![pointer],
        Flags::default(),
        None,
    );
    function.insert(load, Position::After(last.load)).expect("the loads' block");
    let loaded = result_of(function, load);
    match first.cast {
        None => loaded,
        Some((_, op)) => {
            let wide = vector_of(context, reduced, window.len());
            let cast = function.create_instruction(Opcode::Cast(op), wide, vec![loaded], Flags::default(), None);
            function.insert(cast, Position::Before(root)).expect("the root's block");
            result_of(function, cast)
        }
    }
}

fn shuffle(
    context: &mut Context,
    function: &mut Function,
    root: InstId,
    element: TypeId,
    first: Operand,
    second: Operand,
    mask: Vec<i32>,
) -> Operand {
    let ty = vector_of(context, element, mask.len());
    let made =
        function.create_instruction(Opcode::ShuffleVector(mask), ty, vec![first, second], Flags::default(), None);
    function.insert(made, Position::Before(root)).expect("the root's block");
    result_of(function, made)
}

fn poison_of(
    context: &mut Context,
    ty: TypeId,
) -> Operand {
    Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Poison }))
}

/// `emitReduction`: the parts joined, reduced, and the chain replaced.
#[allow(clippy::too_many_arguments)]
fn rewrite(
    context: &mut Context,
    declared: &mut Declared,
    function: &mut Function,
    block: BlockId,
    chain: &Chain,
    slice: &[Leaf],
    parts: &[Part],
) {
    let root = chain.root;
    let result_ty = function.instruction(root).ty;
    let mut total: Option<(Operand, usize)> = None;
    for part in parts {
        let window = &slice[part.start..part.start + part.width];
        let vector = emit_part(context, function, block, window, result_ty, root);
        total = Some(match total {
            None => (vector, part.width),
            Some((so_far, so_far_width)) => {
                // The wider part keeps the lanes: the narrower is added to its
                // low ones.
                let ((wide, wide_width), (narrow, narrow_width)) = if so_far_width >= part.width {
                    ((so_far, so_far_width), (vector, part.width))
                } else {
                    ((vector, part.width), (so_far, so_far_width))
                };
                let wide_ty = vector_of(context, result_ty, wide_width);
                let poison = poison_of(context, wide_ty);
                let mut op = wide;
                if wide_width != narrow_width {
                    op = shuffle(context, function, root, result_ty, wide, poison, (0..narrow_width as i32).collect());
                }
                let narrow_ty = vector_of(context, result_ty, narrow_width);
                let added = function.create_instruction(
                    Opcode::Binary(chain.op),
                    narrow_ty,
                    vec![op, narrow],
                    Flags::default(),
                    Some("rdx.op"),
                );
                function.insert(added, Position::Before(root)).expect("the root's block");
                op = result_of(function, added);
                if wide_width != narrow_width {
                    let narrow_poison = poison_of(context, narrow_ty);
                    let resized = shuffle(
                        context,
                        function,
                        root,
                        result_ty,
                        op,
                        narrow_poison,
                        (0..wide_width as i32)
                            .map(|lane| if (lane as usize) < narrow_width { lane } else { -1 })
                            .collect(),
                    );
                    let mask = (0..wide_width as i32)
                        .map(|lane| if (lane as usize) < narrow_width { wide_width as i32 + lane } else { lane })
                        .collect();
                    op = shuffle(context, function, root, result_ty, wide, resized, mask);
                }
                (op, wide_width)
            }
        });
    }
    let (vector, width) = total.expect("a part");
    let mut done = reduce_call(context, declared, function, vector, width, chain.op, result_ty, root);
    // The values left over: `op.rdx`, the vector total first.
    let taken: Vec<InstId> = parts
        .iter()
        .flat_map(|part| slice[part.start..part.start + part.width].iter())
        .flat_map(|leaf| [leaf.cast.map(|(inst, _)| inst), Some(leaf.load)])
        .flatten()
        .collect();
    // A cast goes before the load it reads.
    let taken = {
        let (casts, loads): (Vec<InstId>, Vec<InstId>) =
            taken.into_iter().partition(|inst| matches!(function.instruction(*inst).opcode, Opcode::Cast(_)));
        [casts, loads].concat()
    };
    let taken_values: Vec<Operand> = parts
        .iter()
        .flat_map(|part| slice[part.start..part.start + part.width].iter())
        .map(|leaf| leaf.value)
        .collect();
    for value in chain.values.iter().rev().filter(|value| !taken_values.contains(value)) {
        let made = function.create_instruction(
            Opcode::Binary(chain.op),
            result_ty,
            vec![done, *value],
            Flags::default(),
            Some("op.rdx"),
        );
        function.insert(made, Position::Before(root)).expect("the root's block");
        done = result_of(function, made);
    }
    let root_value = function.instruction(root).result.expect("a chain root's value");
    function.replace_all_uses_with(root_value, done);
    for link in &chain.links {
        function.erase(*link).expect("a chain link");
    }
    for inst in taken {
        let operands = function.instruction(inst).operands.clone();
        function.erase(inst).expect("a vectorized scalar");
        for operand in operands {
            erase_if_dead(function, operand);
        }
    }
}

/// `removeInstructionsAndOperands`: an address computation nothing reads
/// any more goes with the load it fed, and so on up its operands.
fn erase_if_dead(
    function: &mut Function,
    operand: Operand,
) {
    let Operand::Value(id) = operand else { return };
    let ValueDef::Instruction(inst) = function.value(id).def else { return };
    if function.is_erased(inst)
        || !matches!(
            function.instruction(inst).opcode,
            Opcode::GetElementPtr { .. } | Opcode::Cast(_)
        )
        || !function.users(id).is_empty()
    {
        return;
    }
    let operands = function.instruction(inst).operands.clone();
    function.erase(inst).expect("a dead address computation");
    for operand in operands {
        erase_if_dead(function, operand);
    }
}

/// `llvm.vector.reduce.<op>` of `vector` (`width` lanes), before `root`.
#[allow(clippy::too_many_arguments)]
fn reduce_call(
    context: &mut Context,
    declared: &mut Declared,
    function: &mut Function,
    vector: Operand,
    width: usize,
    op: BinaryOp,
    result_ty: TypeId,
    root: InstId,
) -> Operand {
    let vector_ty = vector_of(context, result_ty, width);
    let function_type =
        context.types.intern(Type::Function { returns: result_ty, parameters: vec![vector_ty], variadic: false });
    let kind = match reduce_kind(op).expect("a reduction") {
        ReduceKind::Add => "add",
        ReduceKind::Mul => "mul",
        ReduceKind::And => "and",
        ReduceKind::Or => "or",
        ReduceKind::Xor => "xor",
        other => unreachable!("not an integer reduction: {other:?}"),
    };
    let name = format!("llvm.vector.reduce.{kind}.{}", llrm_mir::intrinsics::mangle(&context.types, vector_ty));
    let callee = declared.declare(&name, function_type);
    let pointer = context.types.ptr(0);
    let callee = Operand::Constant(context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }));
    let call = function.create_instruction(
        Opcode::Call(Box::new(CallInfo {
            function_type,
            calling_convention: 0,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new()],
            attrs: Vec::new(),
            tail: Tail::default(),
        })),
        result_ty,
        vec![vector, callee],
        Flags::default(),
        Some("rdx"),
    );
    function.insert(call, Position::Before(root)).expect("the root's block");
    result_of(function, call)
}

/// `tryToReduce` over the reduced values of `chain`, grouped by what they are
/// (the same opcode and type, constants, and the rest): each group of
/// [`REDUCTION_LIMIT`] or more is cut into windows, the width from
/// `GetVectorFactor` down and each width slid along the group, the first
/// window whose tree pays is made (DEVIATE SLP-RDX1: one window of a chain; the
/// rest of the values are combined as `op.rdx`, in the chain's order, not the
/// pin's later windows and its identity-operation scales).
#[allow(clippy::too_many_arguments)]
fn try_tree(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
    tti: &dyn Tti,
    block: BlockId,
    chain: &Chain,
    facts: &mut Option<crate::slptree::BlockFacts>,
) -> bool {
    let groups = {
        let mut groups: Vec<Vec<Operand>> = Vec::new();
        for value in chain.values.iter().rev() {
            let key = |operand: &Operand| -> (u8, Option<Opcode>, Option<TypeId>) {
                match operand {
                    Operand::Value(id) => match function.value(*id).def {
                        ValueDef::Instruction(inst) if function.parent(inst) == Some(block) => {
                            (0, Some(function.instruction(inst).opcode.clone()), Some(function.value(*id).ty))
                        }
                        _ => (1, None, Some(function.value(*id).ty)),
                    },
                    _ => (2, None, None),
                }
            };
            match groups.iter_mut().find(|group| key(&group[0]) == key(value)) {
                Some(group) => group.push(*value),
                None => groups.push(vec![*value]),
            }
        }
        groups
    };
    let info = crate::slptree::ReductionInfo { op: chain.op, links: chain.links.clone(), leftovers: Vec::new() };
    let lane_bits = int_ty(context, function.instruction(chain.root).ty).expect("an integer chain");
    let tree = {
        let unit = MemUnit::within(context, layout, function, outer);
        let facts_now = &*facts.get_or_insert_with(|| crate::slptree::BlockFacts::of(&unit, function, block));
        let mut session = llrm_analysis::scev::analysis::ScevSession::of(unit.context, function, outer);
        session.ask_within(unit.context, layout, &outer.globals, &outer.metadata, function, |se, cx| {
            let scaler = crate::slptree::Scaler::new(se, cx);
            for group in &groups {
                if group.len() < REDUCTION_LIMIT || matches!(group[0], Operand::Constant(_)) {
                    continue;
                }
                let n = group.len();
                let mut width = vector_factor(tti, lane_bits, n).min(MAX_ELTS);
                let mut pos = 0;
                while width >= REDUCTION_LIMIT {
                    while pos + width <= n {
                        let window = &group[pos..pos + width];
                        if let Some(tree) = crate::slptree::try_reduction(
                            &unit, function, block, &info, window, facts_now, tti, &scaler,
                        ) {
                            let mut info = info.clone();
                            // The values left out: the chain's order, the last
                            // operand first.
                            info.leftovers =
                                chain.values.iter().rev().filter(|value| !window.contains(value)).copied().collect();
                            return Some(crate::slptree::with_reduction(tree, info));
                        }
                        pos += 1;
                    }
                    pos = 0;
                    width -= 1;
                    if width > 1 {
                        width = vector_factor(tti, lane_bits, width);
                    }
                }
            }
            None
        })
    };
    match tree {
        Some(tree) => {
            crate::slptree::emit(context, layout, declared, function, &tree, None);
            true
        }
        None => false,
    }
}

/// The horizontal reductions of `function`; whether any was made.
pub fn run_function(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
    tti: &dyn Tti,
) -> bool {
    let mut changed = false;
    for block in function.layout().to_vec() {
        // The last operations of the block first, as `vectorizeChainsInBlock`
        // visits the roots that are not operands of another chain's link.
        let candidates: Vec<InstId> = function.iter_block(block).collect();
        let mut taken: Vec<InstId> = Vec::new();
        // Read by every window tried, until a tree changes the function.
        let mut facts: Option<crate::slptree::BlockFacts> = None;
        for root in candidates.into_iter().rev() {
            if function.is_erased(root) || taken.contains(&root) {
                continue;
            }
            let Some(chain) = match_chain(context, function, block, root) else { continue };
            // The slice of loads: the reduced values from the last, alike.
            let plan = {
                let unit = MemUnit::within(context, layout, function, outer);
                let mut slices: Vec<Vec<Leaf>> = Vec::new();
                for value in &chain.values {
                    let Some(leaf) = leaf_of(&unit, function, block, *value) else { continue };
                    let alike = |other: &Leaf| {
                        other.root == leaf.root
                            && other.base == leaf.base
                            && other.loaded == leaf.loaded
                            && other.cast.map(|one| one.1) == leaf.cast.map(|one| one.1)
                    };
                    match slices.iter_mut().find(|slice| alike(&slice[0])) {
                        Some(slice) => slice.push(leaf),
                        None => slices.push(vec![leaf]),
                    }
                }
                // Each slice from its last value to its first.
                let mut best: Option<(Vec<Leaf>, Vec<Part>)> = None;
                slices.sort_by_key(|slice| std::cmp::Reverse(slice.len()));
                for mut slice in slices {
                    slice.reverse();
                    let parts = windows(unit.context, tti, function, block, &chain, &slice);
                    if !parts.is_empty() {
                        best = Some((slice, parts));
                        break;
                    }
                }
                best
            };
            let Some((slice, parts)) = plan else {
                // Not consecutive loads: the tree of the reduced values.
                if try_tree(context, layout, function, declared, outer, tti, block, &chain, &mut facts) {
                    taken.extend(chain.links.iter().copied());
                    changed = true;
                    facts = None;
                }
                continue;
            };
            taken.extend(chain.links.iter().copied());
            rewrite(context, declared, function, block, &chain, &slice, &parts);
            changed = true;
            facts = None;
        }
    }
    changed
}

/// `getReductionCost` of a reduction tree's window of `width` values of `lane`:
/// the vector reduction less the scalar chain it replaces.
pub(crate) fn tree_reduction_cost(
    tti: &dyn Tti,
    kind: CostKind,
    op: BinaryOp,
    lane: Ty,
    width: usize,
) -> Option<i64> {
    let valid = |cost: llrm_mir::tti::Cost| cost.valid.then_some(cost.value);
    let arith = valid(tti.arithmetic_cost(arith_of(op), lane, kind, OperandInfo::default(), OperandInfo::default()))?;
    let reduce =
        valid(tti.vector_reduction_cost(reduce_kind(op)?, VectorTy { lane, count: width as u32 }, false, kind)?)?;
    Some(reduce - arith * (width as i64 - 1))
}

/// The vector reduction of a reduction tree's root vector, the values left out
/// combined into it as `op.rdx`, in place of the chain's result.
pub(crate) fn emit_tree_reduction(
    context: &mut Context,
    declared: &mut Declared,
    function: &mut Function,
    info: &crate::slptree::ReductionInfo,
    vector: Operand,
    width: usize,
) {
    let root = info.links[0];
    let result_ty = function.instruction(root).ty;
    let mut done = reduce_call(context, declared, function, vector, width, info.op, result_ty, root);
    for value in &info.leftovers {
        let made = function.create_instruction(
            Opcode::Binary(info.op),
            result_ty,
            vec![done, *value],
            Flags::default(),
            Some("op.rdx"),
        );
        function.insert(made, Position::Before(root)).expect("the root's block");
        done = result_of(function, made);
    }
    let root_value = function.instruction(root).result.expect("a chain root's value");
    function.replace_all_uses_with(root_value, done);
}
