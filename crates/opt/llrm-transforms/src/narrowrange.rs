//! An unsigned divide or remainder whose operands ScalarEvolution proves narrow
//! is done in the narrow type.
//!
//! # How it works
//! A loop counter `a` of 0..255 reaches `udiv i32 %a, %b` as an `i32`: the
//! loop's exit test bounds it, and no instruction in the body says so. Where
//! the target has a narrow divide or multiply (`Tti::range_narrowing_width`:
//! m68k `divu.w`, 16 bits) the pass reads each operand's unsigned range from
//! ScalarEvolution: `udiv`/`urem` with both operands in the width become `zext
//! (udiv iN (trunc a), (trunc b))`, N the   smallest power of two (8 or more)
//! that holds them: the instruction selector then sees a narrow   divide, whose
//! result and remainder are known narrow in turn;
//!
//! DEVIATE NRW-1 (owner m68k, exit 2026-12-05): the pin narrows a divide by
//! LazyValueInfo ranges only (`CorrelatedValuePropagation::narrowUDivOrURem`),
//! which carry no loop bound, so its `div8_os`-like loops call `__udivsi3` and
//! `__mulsi3` as ours did. The source is gcc's VRP, which refines the range of
//! an induction variable by its number of iterations (`adjust_range_with_scev`,
//! tree-vrp.cc) and expands the divide by the operands' `nonzero_bits`
//! (`expand_divmod`, expmed.cc). Exit: LVI reads loop bounds, or this is
//! deleted.
//!
//! # Runs
//! After `remidiom`, in the strength slot ([`crate::pipeline::strength_slot`]),
//! where the target gives a width, at -O2 and -O3: at -O1 and -Os the libcall's
//! pushes are fewer bytes than the extensions the narrow divide needs (div8_os
//! 68000: +20 B at both, -14% and -38% clocks).

use llrm_analysis::scev::analysis::ScevSession;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};

pub struct NarrowByRange;

/// A candidate: the instruction, its operand widths' unsigned active bits, the
/// type's width.
struct Candidate {
    inst: InstId,
    wide: u32,
    bits: u32,
}

fn insert_before(
    function: &mut Function,
    at: InstId,
    opcode: Opcode,
    ty: llrm_mir::types::TypeId,
    operands: Vec<Operand>,
) -> Operand {
    let made = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(made, Position::Before(at)).expect("a position before the instruction");
    Operand::Value(function.instruction(made).result.expect("a cast or division has a value"))
}

/// Which instructions are narrowed.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// `udiv`/`urem`: done in the narrow type.
    Divides,
    /// `mul`: the operands are said to be narrow, the product stays wide.
    Multiplies,
}

impl FunctionPass for NarrowByRange {
    fn name(&self) -> &'static str {
        "narrowrange"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let Some(limit) = outer.target().tti().and_then(|tti| tti.range_narrowing_width()) else {
            return PreservedAnalyses::all();
        };
        if narrowed(unit.context, unit.layout, unit.function, &outer, limit, Kind::Divides) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// `mul i32 a, b` of operands ScalarEvolution proves `limit` bits wide, as
/// `mul (zext (trunc a)), (zext (trunc b))`: the selector's known bits then see
/// the half-by-half multiply (`mulu.w`).
///
/// A separate entry, not a pipeline pass: the target's own strength reduction
/// after the pipeline (`lsr_pass`) replaces `zext (trunc iv)` by the counter
/// itself, as ScalarEvolution proves them equal, so the wrapper is made after
/// it. DEVIATE NRW-2 (owner m68k, exit 2026-12-05): the pin does not narrow a
/// multiply by ranges; gcc's `expand_mult` does by `nonzero_bits`.
pub fn multiplies_narrowed(
    context: &mut llrm_mir::context::Context,
    layout: &llrm_mir::datalayout::DataLayout,
    function: &mut Function,
    outer: &llrm_mir::passes::Outer,
    limit: u32,
) -> bool {
    narrowed(context, layout, function, outer, limit, Kind::Multiplies)
}

fn narrowed(
    context: &mut llrm_mir::context::Context,
    layout: &llrm_mir::datalayout::DataLayout,
    function: &mut Function,
    outer: &llrm_mir::passes::Outer,
    limit: u32,
    kind: Kind,
) -> bool {
    let wanted: Vec<InstId> = {
        let (context, function) = (&*context, &*function);
        function
            .walk()
            .map(|(_, inst)| inst)
            .filter(|&inst| {
                let one = function.instruction(inst);
                let opcode = match kind {
                    Kind::Divides => matches!(one.opcode, Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem)),
                    Kind::Multiplies => one.opcode == Opcode::Binary(BinaryOp::Mul),
                };
                // a constant operand is the magic multiply's or the shifts'
                // (`x * 11`, `x / 7`)
                opcode
                    && one.operands.iter().all(|operand| !matches!(operand, Operand::Constant(_)))
                    && context
                        .types
                        .int_bits(one.ty)
                        .is_some_and(|bits| bits > limit && (kind == Kind::Divides || bits <= 2 * limit))
            })
            .collect()
    };
    if wanted.is_empty() {
        return false;
    }
    let mut session = ScevSession::of(context, function, outer);
    let mut found = Vec::new();
    for inst in wanted {
        let operands = function.instruction(inst).operands.clone();
        let wide = context.types.int_bits(function.instruction(inst).ty).unwrap_or(0);
        let bits = session.ask(context, layout, function, outer, |se, cx| {
            operands
                .iter()
                .map(|&operand| {
                    let scev = se.get_scev(cx, operand);
                    se.get_unsigned_range(cx, scev).get_active_bits()
                })
                .max()
        });
        if let Some(bits) = bits.filter(|&bits| bits <= limit) {
            found.push(Candidate { inst, wide, bits });
        }
    }
    let mut changed = false;
    for Candidate { inst, wide, bits } in found {
        let opcode = function.instruction(inst).opcode.clone();
        let ty = function.instruction(inst).ty;
        let [a, b] = function.instruction(inst).operands[..] else { continue };
        if kind == Kind::Multiplies {
            let half = context.types.int(limit);
            let mut halves = Vec::new();
            for operand in [a, b] {
                let low = insert_before(function, inst, Opcode::Cast(CastOp::Trunc), half, vec![operand]);
                halves.push(insert_before(function, inst, Opcode::Cast(CastOp::ZExt), ty, vec![low]));
            }
            let flags = function.instruction(inst).flags;
            let made = function.create_instruction(opcode, ty, halves, flags, None);
            function.insert(made, Position::Before(inst)).expect("a position before the multiply");
            let product = Operand::Value(function.instruction(made).result.expect("a multiply has a value"));
            let old = function.instruction(inst).result.expect("a multiply has a value");
            function.replace_all_uses_with(old, product);
            function.erase(inst).expect("the multiply's users were replaced");
            changed = true;
            continue;
        }
        let width = bits.next_power_of_two().max(8);
        if width >= wide {
            continue;
        }
        let narrow_ty = context.types.int(width);
        let low_a = insert_before(function, inst, Opcode::Cast(CastOp::Trunc), narrow_ty, vec![a]);
        let low_b = insert_before(function, inst, Opcode::Cast(CastOp::Trunc), narrow_ty, vec![b]);
        let flags = function.instruction(inst).flags;
        let made = function.create_instruction(opcode, narrow_ty, vec![low_a, low_b], flags, None);
        function.insert(made, Position::Before(inst)).expect("a position before the division");
        let narrow = Operand::Value(function.instruction(made).result.expect("a division has a value"));
        let wide_again = insert_before(function, inst, Opcode::Cast(CastOp::ZExt), ty, vec![narrow]);
        let old = function.instruction(inst).result.expect("a division has a value");
        function.replace_all_uses_with(old, wide_again);
        function.erase(inst).expect("the division's users were replaced");
        changed = true;
    }
    changed
}
