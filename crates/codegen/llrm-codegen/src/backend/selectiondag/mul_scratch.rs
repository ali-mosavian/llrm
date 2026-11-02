//! Which `i16` constant multiplies keep their `imul` because the chain that
//! replaces it needs a register the function has not got to spare.
//!
//! # How it works
//! A chain of shifts and adds that reads its source again needs a register
//! besides the result: an `imul r,r,imm` or the three-operand form needs
//! none. Where the registers are full at that point the extra one is a spill.
//! `keep_imul` asks the one spill model (`llrm_transforms::spill`) what the
//! function spills with one more register wanted at each such multiply and
//! keeps the `imul` of those where it spills more than without: the clocks
//! the chain saves do not pay for the memory operands it adds.
//!
//! # Provides
//! `keep_imul`.

use std::collections::{BTreeMap, HashSet};

use llrm_analysis::blockfreq::BlockFrequencies;
use llrm_analysis::cfg;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Function, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_transforms::{profit, spill};

use crate::backend::arithmetic;

/// The registers `chain` takes beside its source and result: one where it reads
/// the source again after its first step, none for a lone shift.
pub fn chain_temporaries(chain: &[(&'static str, i64)]) -> i64 {
    i64::from(chain.len() > 1)
}

/// The results of the `i16` multiplies by a constant in `function` whose
/// priced chain would spill: they stay `imul`.
pub fn keep_imul(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    cpu: &crate::backend::cpu::Profile,
    arch: &dyn llrm_target::Target,
    frequencies: &BlockFrequencies,
    milliclocks_per_byte: i64,
) -> HashSet<ValueId> {
    let mut found = HashSet::new();
    let mut candidates = Vec::new();
    for &block in function.layout() {
        let freq = frequencies.relative_to_entry(block);
        for inst in function.iter_block(block) {
            let op = function.instruction(inst);
            if op.opcode != Opcode::Binary(BinaryOp::Mul) {
                continue;
            }
            let Some(result) = op.result else { continue };
            let Some(16) = context.types.int_bits(function.value(result).ty) else { continue };
            let [Operand::Value(_), Operand::Constant(constant)] = op.operands[..] else { continue };
            let ConstantKind::Int(bits) = context.get(constant).kind else { continue };
            let Ok(Some(chain)) =
                arithmetic::chain_at_speed(bits as u16 as i16 as i64, 2, cpu, arch, freq, milliclocks_per_byte)
            else {
                continue;
            };
            let registers = chain_temporaries(&chain);
            if registers > 0 {
                candidates.push((inst, result, registers));
            }
        }
    }
    llrm_support::debug!("mulscratch", "{} candidate(s)", candidates.len());
    if candidates.is_empty() {
        return found;
    }
    let machine = cpu.target();
    let room = spill::Room::of_machine(&*machine);
    if !room.priced() {
        return found;
    }
    let costs = machine.costs();
    let frequency: BTreeMap<i64, i64> = function
        .layout()
        .iter()
        .map(|&block| (cfg::id(block), (frequencies.relative_to_entry(block) * profit::UNIT as f64) as i64))
        .collect();
    let across = |_| room.across_call;
    let pressure = spill::Pressure::of(context, function, room.index_scales);
    let view = || spill::View::over(&pressure, context, layout, function, room, &across);
    let base = view().forecast(&costs, &frequency).cost;
    for (inst, result, registers) in candidates {
        let with = view().with_scratch(inst, registers).forecast(&costs, &frequency).cost;
        llrm_support::debug!("mulscratch", "{result:?}: {registers} more register(s), price {base} -> {with}");
        if with > base {
            found.insert(result);
        }
    }
    found
}
