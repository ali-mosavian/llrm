//! A loop version's first reason: an address that steps by an unknown stride is
//! contiguous where the stride is 1.
//!
//! # How it works
//! `Stride::found` takes the recurrences of an innermost loop
//! (`induction::recurrences`): a value that indexes an address and whose step
//! has a term of one invariant unknown (`a[i * s]`) names that unknown, at
//! most `MAX_STRIDES` of them. Its test is `s == 1` for each, and the copy's
//! code reads the constant 1 for each.
//!
//! # Example
//! `a[i * s] = 0` in a loop: the copy for `s == 1` stores `a[i * 1]`, which
//! the pipeline makes the fill `a[i] = 0`. See [`crate::loopversion`].
//!
//! # Provides
//! `Stride`, an [`crate::loopversion::Assumption`].
//!
//! # See also
//! gcc `gimple-loop-versioning.cc` (`loop_versioning::analyze_arbitrary_term`,
//! `-fversion-loops-for-strides`).

use llrm_analysis::cfg::{self, Canonical};
use llrm_analysis::induction;
use llrm_analysis::num::Num as BigInt;
use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Flags, IntPredicate, Opcode};
use llrm_mir::passes::Declared;

use crate::loopclone::LoopCopy;
use crate::loopversion::{Assumption, Within};

/// Strides one loop is copied for.
const MAX_STRIDES: usize = 2;

/// The strides of a loop, each with its width.
pub struct Stride {
    strides: Vec<(ValueId, u32)>,
}

impl Stride {
    /// The strides of `shape`, where it has some.
    pub fn found(
        view: &Within,
        shape: &Canonical,
    ) -> Option<Self> {
        let function = view.function;
        // The cheap look before the loop's recurrences are asked of: some
        // operation multiplies two values neither of which is a constant.
        let multiplies = shape
            .loop_
            .body
            .iter()
            .flat_map(|&at| function.block(cfg::block(at)).iter())
            .any(
                |inst| {
                    let op = function.instruction(inst);
                    op.opcode == Opcode::Binary(llrm_mir::opcode::BinaryOp::Mul)
                        && op.operands.iter().all(|operand| matches!(operand, Operand::Value(_)))
                },
            );
        if !multiplies {
            return None;
        }
        let counters = induction::basics(view.within, &shape.loop_);
        let recurrences = induction::recurrences(view.within, &shape.loop_, &counters);
        let mut strides: Vec<(ValueId, u32)> = Vec::new();
        // A value that steps by an unknown and indexes an address.
        for (&value, steps) in &recurrences.values {
            let indexes = function
                .users(value)
                .iter()
                .any(
                    |used| used.index >= 1 && matches!(
                        function.instruction(used.user).opcode,
                        Opcode::GetElementPtr { .. }
                    ),
                );
            if !indexes {
                continue;
            }
            for monomial in steps.step.terms.keys() {
                let Some(stride) = monomial.single() else { continue };
                let outside = match function.value(stride).def {
                    ValueDef::Argument(_) => true,
                    ValueDef::Instruction(def) => function.parent(def).is_some_and(|block| !shape.holds(block)),
                };
                let Some(width) = view.within.int_bits(Operand::Value(stride)) else { continue };
                if outside && !strides.iter().any(|(seen, _)| *seen == stride) && strides.len() < MAX_STRIDES {
                    strides.push((stride, width));
                }
            }
        }
        (!strides.is_empty()).then_some(Self { strides })
    }
}

fn one(
    context: &Context,
    width: u32,
) -> Operand {
    crate::counting::constant(context, &BigInt::from(1), width)
}

impl Assumption for Stride {
    fn name(&self) -> &'static str {
        "a stride of 1"
    }

    fn condition(
        &mut self,
        context: &mut Context,
        _: &mut Declared,
        function: &mut Function,
        before: InstId,
    ) -> Result<Operand, String> {
        let bit = context.types.int(1);
        let mut holds: Option<Operand> = None;
        for &(stride, width) in &self.strides {
            let compare = function.create_instruction(
                Opcode::ICmp(IntPredicate::Eq),
                bit,
                vec![Operand::Value(stride), one(context, width)],
                Flags::default(),
                None,
            );
            function.insert(compare, Position::Before(before))?;
            let test = Operand::Value(function.instruction(compare).result.expect("a compare's value"));
            holds = Some(match holds {
                None => test,
                Some(both) => {
                    let and = function.create_instruction(
                        Opcode::Binary(llrm_mir::opcode::BinaryOp::And),
                        bit,
                        vec![both, test],
                        Flags::default(),
                        None,
                    );
                    function.insert(and, Position::Before(before))?;
                    Operand::Value(function.instruction(and).result.expect("an and's value"))
                }
            });
        }
        holds.ok_or_else(|| "no stride".to_owned())
    }

    fn assumed(
        &self,
        context: &Context,
        function: &mut Function,
        copy: &LoopCopy,
    ) -> Result<(), String> {
        for (_, &inst) in copy.instructions.iter() {
            let operands = function
                .instruction(inst)
                .operands
                .iter()
                .map(|&operand| match operand {
                    Operand::Value(value) => match self.strides.iter().find(|(stride, _)| *stride == value) {
                        Some(&(_, width)) => one(context, width),
                        None => operand,
                    },
                    other => other,
                })
                .collect();
            function.set_operands(inst, operands);
        }
        Ok(())
    }
}
