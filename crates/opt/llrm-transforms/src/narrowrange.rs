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
        let wanted: Vec<InstId> = {
            let (context, function) = (&*unit.context, &*unit.function);
            function
                .walk()
                .map(|(_, inst)| inst)
                .filter(|&inst| {
                    let one = function.instruction(inst);
                    // a constant operand is the magic multiply's or the shifts'
                    // (`x * 11`, `x / 7`)
                    matches!(one.opcode, Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem))
                        && one.operands.iter().all(|operand| !matches!(operand, Operand::Constant(_)))
                        && context.types.int_bits(one.ty).is_some_and(|bits| bits > limit)
                })
                .collect()
        };
        if wanted.is_empty() {
            return PreservedAnalyses::all();
        }
        let mut session = ScevSession::of(unit.context, unit.function, &outer);
        let mut found = Vec::new();
        for inst in wanted {
            let operands = unit.function.instruction(inst).operands.clone();
            let wide = unit.context.types.int_bits(unit.function.instruction(inst).ty).unwrap_or(0);
            let bits = session.ask(unit.context, unit.layout, unit.function, &outer, |se, cx| {
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
            let opcode = unit.function.instruction(inst).opcode.clone();
            let ty = unit.function.instruction(inst).ty;
            let [a, b] = unit.function.instruction(inst).operands[..] else { continue };
            let width = bits.next_power_of_two().max(8);
            if width >= wide {
                continue;
            }
            let narrow_ty = unit.context.types.int(width);
            let low_a = insert_before(unit.function, inst, Opcode::Cast(CastOp::Trunc), narrow_ty, vec![a]);
            let low_b = insert_before(unit.function, inst, Opcode::Cast(CastOp::Trunc), narrow_ty, vec![b]);
            let flags = unit.function.instruction(inst).flags;
            let made = unit.function.create_instruction(opcode, narrow_ty, vec![low_a, low_b], flags, None);
            unit.function.insert(made, Position::Before(inst)).expect("a position before the division");
            let narrow = Operand::Value(unit.function.instruction(made).result.expect("a division has a value"));
            let wide_again = insert_before(unit.function, inst, Opcode::Cast(CastOp::ZExt), ty, vec![narrow]);
            let old = unit.function.instruction(inst).result.expect("a division has a value");
            unit.function.replace_all_uses_with(old, wide_again);
            unit.function.erase(inst).expect("the division's users were replaced");
            changed = true;
        }
        if changed {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}
