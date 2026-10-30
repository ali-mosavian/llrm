//! One extending load for a narrow load and its extension: LLVM's
//! DAGCombiner
//! [`ExtendUsesToFormExtLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L15365)
//! and
//! [`ExtendSetCCUses`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L15421),
//! as the rewrite of MIR before isel.
//!
//! # How it works
//! For a load whose value `E = sext/zext L` is made in the same block, the
//! compares of `L` with a constant, in that block or any block the load
//! dominates, compare `E` instead, with the constant extended the same way;
//! the same extension made again is `E`, and a reader in the load's own block
//! reads `trunc E`; any other reader leaves the load alone. `L`
//! is then read only by `E`, and isel selects the pair as one `movsx`/`movzx`
//! from memory: the compare is on the wide register, not on a byte register
//! that `E` must extend again.
//!
//! A `zext` is not made where a signed compare reads `L` (its sign bits would
//! be lost). `E` is moved to just after `L`, so that it precedes the compares
//! it now feeds.
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | Done on MIR before isel, not in the DAG: isel selects one instruction at a time (`isel/combined.rs`) and has no combine stage. Owner mir-undef; revisit when isel gains one. |
//! | D2 | A reader of the load that is neither a compare with a constant, the same extension again nor in the load's own block (a store or a call in another block) leaves the load alone. The pin gives it `trunc E` (`isTruncateFree`); there the narrow value lives beside the wide one: bench/grep.nib `_main` grew 58 B of spills. Owner mir-undef; revisit when isel can fold the trunc. |
//! | D3 | A compare with a constant in a block the load dominates, not only in its own, is widened (the pin's DAG is one block): x_life -O2 727 -> 711 B, -O3 the same, m16 -O1 668 -> 661, -O2 644 -> 641. Cost: bench/grep.nib is +33 / +29 / +25 B over parity at -O1 / -O2 / -Os (and 115 / 173 / 159 B under main): `_main` spills one more range (greedy trace: spilled 31 -> 32, stores 8 -> 12). The cause is an UNTESTED hypothesis: a long range that moves from the byte class to the word class can take SI or DI, which grep uses as base registers. render/d_alias is above main already (-O2 12522 main, 14921 parity; -Os 11836, 12232); the rule adds +12 B at -O2 (0.08%: bytes do not decide there) and +2 B at -Os (0.016% of 12232, under the 0.05% bar): nothing beyond those bytes. Exit: a pressure-aware gate for the cross-block case. Owner mir-undef; review 2026-11-14. |
//!
//! # Example
//! Before:
//!
//! ```mir
//! b0:
//!   %c = load i8, ptr %p
//!   %lt = icmp sgt i8 %c, 0
//!   %w = sext i8 %c to i16
//!   br i1 %lt, label %b1, label %b2
//! ```
//!
//! After:
//!
//! ```mir
//! b0:
//!   %c = load i8, ptr %p
//!   %w = sext i8 %c to i16
//!   %lt = icmp sgt i16 %w, 0
//!   br i1 %lt, label %b1, label %b2
//! ```
//!
//! From unit test `test_a_compare_of_a_loaded_byte_compares_its_extension` in
//! `extload_tests.rs`.
//!
//! # Runs
//! Once per body, after `cgpaddr`, where the other passes that make the
//! forms isel sees run (the pin has none: its DAGCombiner does it). With
//! `wanted("extload")`.
//!
//! # Needs
//! Nothing.
//!
//! # Keeps
//! Blocks, dominators and loops.
//!
//! # See also
//! Tests: `extload_tests.rs`. Levels: `docs/reference/levels.md`.

use llrm_mir::context::{ConstantKind, Context, signed};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};

pub struct ExtLoad;

impl FunctionPass for ExtLoad {
    fn name(&self) -> &'static str {
        "extload"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        _analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if widened(unit.context, unit.function) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// What the compares and the other readers of one load become.
struct Plan {
    extension: InstId,
    /// Other extensions of the load, of the same kind and type: they are `E`.
    same: Vec<InstId>,
    /// Readers in the load's block that read `trunc E`.
    others: Vec<(InstId, u32)>,
    /// `(compare, index of the load in its operands, index of the constant)`.
    compares: Vec<(InstId, u32, u32)>,
}

/// `load`'s plan, or `None` where the pin's `ExtendUsesToFormExtLoad` says no
/// or there is nothing to change.
fn plan(
    context: &Context,
    function: &Function,
    home: BlockId,
    load: InstId,
) -> Option<Plan> {
    let Opcode::Load { volatile: false, .. } = function.instruction(load).opcode else { return None };
    let result = function.instruction(load).result?;
    context.types.int_bits(function.instruction(load).ty)?;
    let extension = first_extension(function, home, load, result)?;
    let zero_extended = function.instruction(extension).opcode == Opcode::Cast(CastOp::ZExt);
    let mut compares = Vec::new();
    let mut same = Vec::new();
    let mut others = Vec::new();
    for used in function.users(result) {
        if used.user == extension {
            continue;
        }
        let user = function.instruction(used.user);
        let here = function.parent(used.user) == Some(home);
        if let Opcode::ICmp(predicate) = &user.opcode {
            let other = 1 - used.index;
            if int_constant(context, user.operands[other as usize]).is_some() {
                // A zero extension loses the sign bits a signed compare reads.
                if zero_extended && is_signed(*predicate) {
                    return None;
                }
                compares.push((used.user, used.index, other));
                continue;
            }
        }
        // The same extension made again (another block): it is `E`, which
        // dominates it as the load does. Any other reader would read `trunc E`,
        // and the narrow value would live beside the wide one (D2).
        if user.opcode == function.instruction(extension).opcode
            && user.ty == function.instruction(extension).ty
            && used.index == 0
        {
            same.push(used.user);
            continue;
        }
        // A reader in the load's own block reads `trunc E`: nothing of the
        // narrow value is live out of the block. One in another block
        // or a phi would keep it live beside the wide value (grep.nib
        // `_main`, +58 B of spills): left.
        if here && user.opcode != Opcode::Phi {
            others.push((used.user, used.index));
            continue;
        }
        return None;
    }
    if compares.is_empty() {
        return None;
    }
    Some(Plan { extension, same, others, compares })
}

/// The first extension of `result` after `load` in its block. The readers of
/// the load name the candidates, so a load nothing extends costs its readers,
/// not a scan of the block after it (a block of N loads was N^2/2 steps).
fn first_extension(
    function: &Function,
    home: BlockId,
    load: InstId,
    result: llrm_mir::module::ValueId,
) -> Option<InstId> {
    let extends = |one: InstId| {
        let made = function.instruction(one);
        matches!(made.opcode, Opcode::Cast(CastOp::SExt | CastOp::ZExt)) && made.operands[0] == Operand::Value(result)
    };
    let mut candidates = function
        .users(result)
        .iter()
        .map(|used| used.user)
        .filter(|&one| one != load && function.parent(one) == Some(home) && extends(one));
    let first = candidates.next()?;
    if candidates.next().is_none() {
        return Some(first);
    }
    // Several: the earliest in the block, as the scan found it.
    scan_after(function, home, load, extends)
}

fn scan_after(
    function: &Function,
    home: BlockId,
    load: InstId,
    extends: impl Fn(InstId) -> bool,
) -> Option<InstId> {
    function
        .iter_block(home)
        .skip_while(|&one| one != load)
        .skip(1)
        .find(
            |&one| {
                #[cfg(test)]
                SCANNED.with(|steps| steps.set(steps.get() + 1));
                extends(one)
            },
        )
}

#[cfg(test)]
thread_local! {
    /// Instructions looked at by block scans for an extension.
    pub(crate) static SCANNED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn int_constant(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        _ => None,
    }
}

fn is_signed(predicate: IntPredicate) -> bool {
    matches!(
        predicate,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    )
}

fn widened(
    context: &Context,
    function: &mut Function,
) -> bool {
    let loads: Vec<(BlockId, InstId)> =
        function.walk().filter(|&(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Load { .. })).collect();
    let mut changed = false;
    for (home, load) in loads {
        if function.is_erased(load) {
            continue;
        }
        let Some(plan) = plan(context, function, home, load) else { continue };
        let result = function.instruction(load).result.expect("a value");
        let narrow = function.instruction(load).ty;
        let narrow_bits = context.types.int_bits(narrow).expect("an integer");
        let wide = function.instruction(plan.extension).ty;
        let extended = Operand::Value(function.instruction(plan.extension).result.expect("a value"));
        let zero_extended = function.instruction(plan.extension).opcode == Opcode::Cast(CastOp::ZExt);
        function.move_to(plan.extension, Position::After(load)).expect("a placed instruction");
        for again in plan.same {
            let value = function.instruction(again).result.expect("a value");
            function.replace_value(value, extended);
            function.erase(again).expect("no readers");
        }
        if !plan.others.is_empty() {
            let cut = function.create_instruction(
                Opcode::Cast(CastOp::Trunc),
                narrow,
                vec![extended],
                Flags::default(),
                None,
            );
            function.insert(cut, Position::After(plan.extension)).expect("a placed instruction");
            let cut = Operand::Value(function.instruction(cut).result.expect("a value"));
            for (user, at) in plan.others {
                let mut operands = function.instruction(user).operands.clone();
                operands[at as usize] = cut;
                function.set_operands(user, operands);
            }
        }
        for (compare, at, constant) in plan.compares {
            let mut operands = function.instruction(compare).operands.clone();
            let bits = int_constant(context, operands[constant as usize]).expect("an integer constant");
            let value = if zero_extended { bits as i128 } else { signed(bits, narrow_bits) };
            operands[at as usize] = extended;
            operands[constant as usize] = Operand::Constant(context.int(wide, value));
            function.set_operands(compare, operands);
        }
        debug_assert!(function.users(result).iter().all(|used| used.user == plan.extension));
        changed = true;
    }
    changed
}

#[cfg(test)]
#[path = "extload_tests.rs"]
mod tests;
