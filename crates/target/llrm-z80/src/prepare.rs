//! What the module needs before selection: a block that ends in a conditional
//! branch (or a `switch`) to a block with phis puts the phi copies on its own
//! edge, so that edge gets a block.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{GlobalValue, Module, Operand};
use llrm_mir::opcode::{Attribute, BinaryOp, Opcode};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::pattern::constant_int_u64;
use llrm_transforms::edges::split_critical_edge;

/// The name of a block split onto a phi edge: the pin lays these out only after
/// selection, so the switch lowering looks past them.
pub const PHI_EDGE: &str = "phi.edge";

/// Splits every critical edge into a block with phis, in every function with a
/// body. The selector refuses a block of several machine blocks (a split
/// branch, a `switch`) that exports phi copies, as one copy list cannot serve
/// its several edges.
pub fn split_phi_edges(module: &mut Module) {
    let names: Vec<String> = module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .filter_map(|(_, global, _)| global.name.owned())
        .collect();
    for name in names {
        let Some((_, function)) = module.function_mut(&name) else { continue };
        for from in function.layout().to_vec() {
            let Some(terminator) = function.terminator(from) else { continue };
            let targets: Vec<_> = function
                .instruction(terminator)
                .operands
                .iter()
                .filter_map(|one| if let Operand::Block(to) = one { Some(*to) } else { None })
                .collect();
            for to in targets {
                let phi = function
                    .iter_block(to)
                    .next()
                    .is_some_and(|first| function.instruction(first).opcode == Opcode::Phi);
                if phi && let Some(bridge) = split_critical_edge(function, from, to) {
                    function.set_block_name(bridge, PHI_EDGE);
                }
            }
        }
    }
}

/// The attribute that lets a function tuned for size call `__mulsi3`.
pub const LONG_MULTIPLY: &str = "llrm-mulsi3";

/// Marks every function with a body when the module multiplies longs often
/// enough to pay for `__mulsi3` (58 B): a long multiply counts one, a long
/// long three (its cross products). A lone small multiply is cheaper inline
/// (copy_runs, switch_jump_table: +10 and +31 B at -Os with the call).
pub fn mark_long_multiplies(module: &mut Module) {
    let bits = |ty| module.context.types.int_bits(ty);
    let sites: u32 = module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .map(|(_, _, function)| {
            function
                .layout()
                .iter()
                .flat_map(|&block| function.iter_block(block))
                .filter(|&inst| function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Mul))
                .map(|inst| match bits(function.instruction(inst).ty) {
                    Some(32) => 1,
                    Some(64) => 3,
                    _ => 0,
                })
                .sum::<u32>()
        })
        .sum();
    if sites < 2 {
        return;
    }
    let names: Vec<String> = module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .filter_map(|(_, global, _)| global.name.owned())
        .collect();
    for name in names {
        if let Some((_, function)) = module.function_mut(&name) {
            function.attrs.push(Attribute::Flag(LONG_MULTIPLY.into()));
        }
    }
}

/// The attribute that marks a function with a shift of a 64-bit value by a
/// variable amount: it is also selected with each such shift an in-place loop
/// (`driver::selections_with_alternatives`).
pub const WIDE_SHIFT: &str = "llrm-wshift";

/// Marks every function with a body that shifts an `i64` by an amount that is
/// not a constant.
pub fn mark_wide_shifts(module: &mut Module) {
    let Some(Ok(layout)) = module.datalayout.as_deref().map(DataLayout::parse) else { return };
    let names: Vec<String> = module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .filter(|(_, _, function)| {
            let query = Query { globals: &module.globals, ..Query::bare(&module.context, function, &layout) };
            function
                .layout()
                .iter()
                .flat_map(|&block| function.iter_block(block))
                .map(|inst| function.instruction(inst))
                .any(|instruction| {
                    matches!(
                        instruction.opcode,
                        Opcode::Binary(BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr)
                    )
                        && module.context.types.int_bits(instruction.ty) == Some(64)
                        && instruction.operands.get(1).is_some_and(|&amount| constant_int_u64(&query, amount).is_none())
                })
        })
        .filter_map(|(_, global, _)| global.name.owned())
        .collect();
    for name in names {
        if let Some((_, function)) = module.function_mut(&name) {
            function.attrs.push(Attribute::Flag(WIDE_SHIFT.into()));
        }
    }
}

/// The attribute that lets every word times a constant of a function be a
/// shift-add chain whatever its bytes: no word multiply of the module needs
/// `__mulhi3`, and the chains together are smaller than the routine and the
/// calls.
pub const WORD_CHAINS: &str = "llrm-mulhi3-chains";

/// Marks every function with a body when each word multiply of the module is by
/// a constant and has a chain, and the chains add fewer bytes over the calls (6
/// B each) than the 24 B routine is (`runtime/sizes.csv`). Strict levels only
/// (above them the rate prices each site, `dagisel::pipeline::word_chain`).
/// Pin-decomposed constants need no call.
pub fn mark_word_chains(module: &mut Module) {
    const ROUTINE: i64 = 24;
    let Some(Ok(layout)) = module.datalayout.as_deref().map(DataLayout::parse) else { return };
    let mut extra = 0;
    let mut sites = 0;
    for (_, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
        let query = Query { globals: &module.globals, ..Query::bare(&module.context, function, &layout) };
        let muls = function
            .layout()
            .iter()
            .flat_map(|&block| function.iter_block(block))
            .filter(|&inst| function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Mul))
            .filter(|&inst| module.context.types.int_bits(function.instruction(inst).ty) != Some(8));
        for inst in muls {
            let Some(&operand) = function.instruction(inst).operands.get(1) else { return };
            let Some(c) = constant_int_u64(&query, operand) else { return };
            let wide = module.context.types.int_bits(function.instruction(inst).ty) != Some(16);
            // a long product is expanded to word products, some of them calls:
            // the routine stays
            if wide {
                if crate::dagisel::pipeline::pin_decomposes(c) {
                    continue;
                }
                return;
            }
            let c = c & 0xFFFF;
            if crate::dagisel::pipeline::pin_decomposes(c) {
                continue;
            }
            let Some((_, (bytes, _), (call, _))) = crate::dagisel::pipeline::word_chain(c as i64, true, true) else {
                return;
            };
            extra += bytes - call;
            sites += 1;
        }
    }
    if sites == 0 || extra >= ROUTINE {
        return;
    }
    let names: Vec<String> = module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .filter_map(|(_, global, _)| global.name.owned())
        .collect();
    for name in names {
        if let Some((_, function)) = module.function_mut(&name) {
            function.attrs.push(Attribute::Flag(WORD_CHAINS.into()));
        }
    }
}

/// The attribute that marks a function with a divide or remainder of a long or
/// a long long: it is also selected with the speed routines in place of the
/// size ones (`driver::selections_with_alternatives`).
pub const WIDE_DIVIDE: &str = "llrm-wdiv";

/// Marks every function with a body that divides or takes a remainder of an
/// `i32` or an `i64`.
pub fn mark_wide_divides(module: &mut Module) {
    let names: Vec<String> = module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .filter(|(_, _, function)| {
            function
                .layout()
                .iter()
                .flat_map(|&block| function.iter_block(block))
                .map(|inst| function.instruction(inst))
                .any(|instruction| {
                    matches!(
                        instruction.opcode,
                        Opcode::Binary(BinaryOp::SDiv | BinaryOp::UDiv | BinaryOp::SRem | BinaryOp::URem)
                    ) && matches!(module.context.types.int_bits(instruction.ty), Some(32 | 64))
                })
        })
        .filter_map(|(_, global, _)| global.name.owned())
        .collect();
    for name in names {
        if let Some((_, function)) = module.function_mut(&name) {
            function.attrs.push(Attribute::Flag(WIDE_DIVIDE.into()));
        }
    }
}

/// The attribute that marks a function with a `memset` of a constant count of
/// two or more: it is also selected with each such fill an inline `ldir` fill
/// (`driver::selections_with_alternatives`).
pub const CONSTANT_FILL: &str = "llrm-fill";

/// Marks every function with a body that calls `llvm.memset` with a constant
/// count of two or more. The selection decides which of them reach
/// `emit_memset` (a short fill is stored inline); one that does not leaves the
/// two selections identical and the alternative is dropped.
pub fn mark_constant_fills(module: &mut Module) {
    let Some(Ok(layout)) = module.datalayout.as_deref().map(DataLayout::parse) else { return };
    let names: Vec<String> = module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .filter(|(_, _, function)| {
            let query = Query { globals: &module.globals, ..Query::bare(&module.context, function, &layout) };
            function.walk().any(|(_, call)| {
                let Some(callee) = llrm_mir::memory::callee(&module.context, function, call) else { return false };
                let is_memset =
                    module.globals.get(callee.0 as usize).and_then(GlobalValue::intrinsic) == Some(Intrinsic::MemSet);
                is_memset
                    && function
                        .instruction(call)
                        .operands
                        .get(2)
                        .is_some_and(|&length| constant_int_u64(&query, length).is_some_and(|bytes| bytes >= 2))
            })
        })
        .filter_map(|(_, global, _)| global.name.owned())
        .collect();
    for name in names {
        if let Some((_, function)) = module.function_mut(&name) {
            function.attrs.push(Attribute::Flag(CONSTANT_FILL.into()));
        }
    }
}
