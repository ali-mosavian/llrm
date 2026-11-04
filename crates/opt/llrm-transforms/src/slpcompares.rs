//! Compares of adjacent bytes against constants become one wide compare: the
//! part of LLVM's `SLPVectorizer` that matters here, where a vector of
//! compares is `reduce.and`ed, `VectorCombine` and `InstCombine` then make
//! it a compare of one integer.
//!
//! # How it works
//! A conjunction (`and`, or `select c, x, false`, of one block) whose
//! conjuncts include `icmp eq (load p+k), c_k` for adjacent cells of one
//! pointer is `icmp eq (load p), C`, `C` the constants little-endian. The
//! leaves are the one-use compares of a one-use load; a leaf stands for the
//! whole conjunction only through `and`s, so a leaf under an `or` is left
//! alone. The widest chunk of 8 or 4 bytes is taken from the lowest cell,
//! the rest tried again. Nothing between the first and the last load of a
//! chunk may write memory, so the wide load can stand at the first. The first
//! leaf of a chunk is replaced by the wide compare and the others by `true`,
//! which the combine drops.
//!
//! # Example
//! ```text
//!   %a = icmp eq i8 %b0, 67                %w = load i32, ptr %p
//!   %b = icmp eq i8 %b1, 79                %r = icmp eq i32 %w, 1297044291
//!   %c = icmp eq i8 %b2, 77        ==>     (the chain is %r)
//!   %d = icmp eq i8 %b3, 83
//! ```
//!
//! MIRROR: the outcome of `SLPVectorizerPass` (`PassBuilderPipelines.cpp`
//! l.1455) on an `and` reduction of equalities of consecutive loads, with
//! `VectorCombine` and `InstCombine` after it. Chunks of fewer than four
//! bytes are not made: the pin's cost model keeps two and three byte chains
//! scalar (`two_bytes_stay_bytes` in `llrm-arm64`'s `slp_compares.rs`, measured
//! against the pin).
//!
//! DEVIATE 1: no vectors. A chain the pin makes a `<16 x i8>` compare is
//! two 8-byte compares here. Exit: vector types in the backend.
//!
//! DEVIATE 2: only equalities under `and`; `ne` under `or` is the same fact
//! the other way. Exit: the dual.
//!
//! DEVIATE 3: the wide load's alignment is the narrow load's; a target is
//! asked only whether it has vector registers (`vector_register_bits`), not
//! whether a misaligned access is fast.
//!
//! # Runs
//! At the pin's `slp-vectorizer` slot, after `simplifycfg` has made the
//! chain one block.
//!
//! # Used by
//! `pipeline` (Late).

use llrm_analysis::memory::{MemRef, Unit as MemUnit};
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, Outer, PreservedAnalyses, Unit};

/// Fewest bytes a wide compare is made of: the pin keeps two and three.
const LEAST_BYTES: i64 = 4;
/// Most bytes: the widest legal integer of the targets with vector registers.
const MOST_BYTES: i64 = 8;

/// `icmp eq (load cell), constant`.
#[derive(Clone)]
struct Leaf {
    compare: InstId,
    load: InstId,
    /// The load's place in its block.
    order: usize,
    root: Operand,
    disp: i64,
    /// Bytes loaded.
    width: i64,
    value: u128,
}

/// The cell `pointer` names: what it is added to and the displacement.
fn cell(
    unit: &MemUnit,
    pointer: Operand,
    width: u32,
) -> Option<(Operand, i64)> {
    let reference = MemRef::at(unit, pointer, width);
    let root = reference.root.filter(|_| reference.base.is_none() && reference.segment.is_none())?;
    Some((root, reference.disp))
}

/// The integer `operand` is, as bits.
fn bits_of(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            ConstantKind::Zero | ConstantKind::Null => Some(0),
            _ => None,
        },
        _ => None,
    }
}

/// The instruction that makes `value`.
fn defined_by(
    function: &Function,
    value: ValueId,
) -> Option<InstId> {
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

/// The value at the top of the conjunction `value` is a conjunct of: climbs
/// through the one-use `and`s and `select c, x, false`s of its block.
fn conjunction_root(
    context: &Context,
    function: &Function,
    mut value: ValueId,
) -> ValueId {
    loop {
        let users = function.users(value);
        let Some(only) = users.only() else { return value };
        let user = function.instruction(only.user);
        let joins = match user.opcode {
            Opcode::Binary(BinaryOp::And) => true,
            Opcode::Select => only.index < 2 && bits_of(context, user.operands[2]) == Some(0),
            _ => false,
        };
        let Some(result) = user.result.filter(|_| joins && context.types.int_bits(user.ty) == Some(1)) else {
            return value;
        };
        if defined_by(function, value).map(|inst| function.parent(inst)) != Some(function.parent(only.user)) {
            return value;
        }
        value = result;
    }
}

/// The wide compares of `function`'s conjunctions; whether any was made.
pub fn merged(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let mut plan: Vec<Vec<Leaf>> = Vec::new();
    {
        let unit = MemUnit::within(context, layout, function, outer);
        for block in function.layout() {
            let mut groups: Vec<(ValueId, Vec<Leaf>)> = Vec::new();
            let mut writes: Vec<usize> = Vec::new();
            for (order, inst) in function.iter_block(*block).enumerate() {
                let instruction = function.instruction(inst);
                match &instruction.opcode {
                    Opcode::Store { .. } | Opcode::Call(_) | Opcode::Invoke(_) => writes.push(order),
                    Opcode::ICmp(IntPredicate::Eq) => {
                        let Some(leaf) = leaf_of(&unit, function, inst, *block) else { continue };
                        let result = instruction.result.expect("a compare's value");
                        let root = conjunction_root(unit.context, function, result);
                        match groups.iter_mut().find(|group| group.0 == root) {
                            Some(group) => group.1.push(leaf),
                            None => groups.push((root, vec![leaf])),
                        }
                    }
                    _ => {}
                }
            }
            for (_, leaves) in groups {
                plan.extend(chunks(leaves, &writes));
            }
        }
    }
    for chunk in &plan {
        make(context, function, chunk);
    }
    !plan.is_empty()
}

/// `inst` as a leaf: an equality of a one-use load of an integer cell with a
/// constant.
fn leaf_of(
    unit: &MemUnit,
    function: &Function,
    inst: InstId,
    block: llrm_mir::module::BlockId,
) -> Option<Leaf> {
    let compare = function.instruction(inst);
    let (loaded, constant) = match (compare.operands[0], compare.operands[1]) {
        (Operand::Value(value), constant) | (constant, Operand::Value(value)) => (value, constant),
        _ => return None,
    };
    let value = bits_of(unit.context, constant)?;
    if function.users(compare.result?).only().is_none() || !function.users(loaded).has_one_use() {
        return None;
    }
    let load = defined_by(function, loaded)?;
    let instruction = function.instruction(load);
    if !matches!(instruction.opcode, Opcode::Load { volatile: false, .. }) || function.parent(load) != Some(block) {
        return None;
    }
    let bits = unit.context.types.int_bits(instruction.ty).filter(|bits| matches!(bits, 8 | 16 | 32))?;
    let (root, disp) = cell(unit, instruction.operands[0], bits / 8)?;
    let order = function.iter_block(block).position(|one| one == load)?;
    Some(Leaf { compare: inst, load, order, root, disp, width: i64::from(bits / 8), value })
}

/// The runs of adjacent cells of `leaves`, each cut into chunks of eight or
/// four bytes with nothing written between its loads.
fn chunks(
    mut leaves: Vec<Leaf>,
    writes: &[usize],
) -> Vec<Vec<Leaf>> {
    let mut made = Vec::new();
    leaves.sort_by_key(|leaf| (leaf.disp, leaf.order));
    let mut at = 0;
    while at < leaves.len() {
        let mut end = at + 1;
        while end < leaves.len()
            && leaves[end].root == leaves[at].root
            && leaves[end].width == leaves[at].width
            && leaves[end].disp == leaves[end - 1].disp + leaves[end].width
        {
            end += 1;
        }
        let run = &leaves[at..end];
        let width = run[0].width;
        let mut taken = 0;
        while taken < run.len() {
            let bytes = (run.len() - taken) as i64 * width;
            let size = if bytes >= MOST_BYTES {
                MOST_BYTES
            } else if bytes >= LEAST_BYTES {
                LEAST_BYTES
            } else {
                break;
            };
            let count = (size / width) as usize;
            let chunk = &run[taken..taken + count];
            taken += count;
            let first = chunk.iter().map(|leaf| leaf.order).min().expect("a chunk has leaves");
            let last = chunk.iter().map(|leaf| leaf.order).max().expect("a chunk has leaves");
            if count >= 2 && !writes.iter().any(|&write| write > first && write < last) {
                made.push(chunk.to_vec());
            }
        }
        // A leaf already taken is not a leaf of the next run.
        at = end;
    }
    made
}

/// The wide compare of `chunk`, its leaves replaced by it.
fn make(
    context: &mut Context,
    function: &mut Function,
    chunk: &[Leaf],
) {
    let bits = (chunk.iter().map(|leaf| leaf.width).sum::<i64>() * 8) as u32;
    let wide_type = context.types.int(bits);
    let flag_type = context.types.int(1);
    let lowest = &chunk[0];
    let earliest = chunk.iter().min_by_key(|leaf| leaf.order).expect("a chunk has leaves");
    let align = match function.instruction(lowest.load).opcode {
        Opcode::Load { align, .. } => align.or(Some(lowest.width as u64)),
        _ => None,
    };
    let pointer = function.instruction(lowest.load).operands[0];
    let load = function.create_instruction(
        Opcode::Load { align, volatile: false },
        wide_type,
        vec![pointer],
        Flags::default(),
        None,
    );
    function.insert(load, Position::Before(earliest.load)).expect("a placed load");
    let wide = function.instruction(load).result.expect("a load's value");
    let mut constant = 0u128;
    let mut shift = 0;
    for leaf in chunk {
        constant |= leaf.value << shift;
        shift += leaf.width as u32 * 8;
    }
    let equal = function.create_instruction(
        Opcode::ICmp(IntPredicate::Eq),
        flag_type,
        vec![Operand::Value(wide), Operand::Constant(context.int(wide_type, constant as i128))],
        Flags::default(),
        None,
    );
    function.insert(equal, Position::After(load)).expect("a placed compare");
    let made = function.instruction(equal).result.expect("a compare's value");
    let yes = Operand::Constant(context.int(flag_type, 1));
    for (index, leaf) in chunk.iter().enumerate() {
        let result = function.instruction(leaf.compare).result.expect("a compare's value");
        function.replace_all_uses_with(result, if index == 0 { Operand::Value(made) } else { yes });
        function.erase(leaf.compare).expect("a compare with no use left");
        function.erase(leaf.load).expect("a load with no use left");
    }
}

/// Wide compares in place of chains of narrow ones, on a target with vector
/// registers.
pub struct SlpCompares;

impl FunctionPass for SlpCompares {
    fn name(&self) -> &'static str {
        "slpcompares"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("slp-vectorizer")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = analyses.outer().clone();
        let vectors = outer.target().tti().is_some_and(|tti| tti.vector_register_bits() > 0);
        if vectors && merged(unit.context, unit.layout, unit.function, &outer) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}
