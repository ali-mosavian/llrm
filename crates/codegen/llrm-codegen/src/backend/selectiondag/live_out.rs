//! `SelectionDAGISel::ComputeLiveOutVRegInfo`
//! ([L904](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L904)):
//! after a block's DAG is legalized, what it knows of each integer it copies
//! to a virtual register, for the blocks that read the register.
//!
//! [`compute_phi_live_out_reg_info`] is `ComputePHILiveOutRegInfo`: a phi's
//! register knows what all its incoming values know. DEVIATE: x86 and arm64
//! do not call it, so their phi registers carry nothing; the caller walks the
//! blocks in layout order where the pin walks them in reverse post-order.
//! Exit: x86 and arm64 judged, no later than 2026-11-20.

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{Function, InstId, Module, Operand};

use super::dag::{NodeData, SDValue, SelectionDAG};
use super::function_lowering::{FIRST_VIRTUAL, FunctionLoweringInfo, LiveOut};
use super::isd::ISD;
use super::known_bits::{known_bits, num_sign_bits};
use super::mvt::EVT;

/// Record the sign bits and known bits of every integer `CopyToReg` of a
/// virtual register reachable by chain from the root.
pub fn compute_live_out_vreg_info(
    dag: &SelectionDAG,
    info: &mut FunctionLoweringInfo,
) {
    let mut added = std::collections::BTreeSet::new();
    let root = dag.get_root().node;
    let mut work = vec![root];
    added.insert(root);
    while let Some(node) = work.pop() {
        let value = SDValue { node, res: 0 };
        for index in 0..dag.node(value).ops.len() {
            let op = dag.operand(value, index);
            if dag.value_type(op) == EVT::OTHER && added.insert(op.node) {
                work.push(op.node);
            }
        }
        if dag.opcode(value) != ISD::CopyToReg {
            continue;
        }
        let NodeData::Register(reg) = dag.node(dag.operand(value, 1)).data else { continue };
        let src = dag.operand(value, 2);
        if reg < FIRST_VIRTUAL || !dag.value_type(src).is_integer() || dag.value_type(src).size_in_bits() > 64 {
            continue;
        }
        let known = known_bits(dag, src);
        info.add_live_out(reg, LiveOut { sign_bits: num_sign_bits(dag, src), zero: known.zero, one: known.one });
    }
}

/// `RegsForValue::getCopyFromRegs`: a copy of a virtual register the earlier
/// blocks said something about is asserted: a zero, an `AssertZext` or an
/// `AssertSext` of the tightest type. Run on a built DAG, so a block made with
/// another (the cases of a split branch) reads what its predecessor's
/// selection learned.
pub fn assert_live_out_copies(
    dag: &mut SelectionDAG,
    info: &FunctionLoweringInfo,
) {
    for id in dag.node_ids().collect::<Vec<_>>() {
        let copy = SDValue { node: id, res: 0 };
        if dag.opcode(copy) != ISD::CopyFromReg {
            continue;
        }
        let NodeData::Register(reg) = dag.node(dag.operand(copy, 1)).data else { continue };
        let vt = dag.value_type(copy);
        let Some(known) = info.live_out.get(&reg).copied().filter(|_| reg >= FIRST_VIRTUAL && vt.is_integer()) else {
            continue;
        };
        let size = vt.size_in_bits();
        let leading_zeros = (0..size).rev().take_while(|&bit| known.zero >> bit & 1 == 1).count() as u32;
        let (code, bits) = if leading_zeros == size {
            let zero = dag.get_constant(0, vt);
            replace_uses(dag, copy, zero);
            continue;
        } else if leading_zeros > 0 {
            (ISD::AssertZext, size - leading_zeros)
        } else if known.sign_bits > 1 {
            (ISD::AssertSext, size - known.sign_bits + 1)
        } else {
            continue;
        };
        let narrow = dag.get_value_type_node(EVT::integer(bits));
        let asserted = dag.get_node(code, vt, &[copy, narrow]);
        replace_uses_except(dag, copy, asserted);
    }
}

fn replace_uses(
    dag: &mut SelectionDAG,
    from: SDValue,
    to: SDValue,
) {
    replace_uses_except(dag, from, to);
}

/// Every user of `from` but `to` itself reads `to`.
fn replace_uses_except(
    dag: &mut SelectionDAG,
    from: SDValue,
    to: SDValue,
) {
    for user in dag.users(from).collect::<Vec<_>>() {
        if user == to.node {
            continue;
        }
        let user = SDValue { node: user, res: 0 };
        let ops: Vec<SDValue> = (0..dag.node(user).ops.len())
            .map(|at| dag.operand(user, at))
            .map(|op| if op == from { to } else { op })
            .collect();
        dag.update_node_operands(user.node, &ops);
    }
}

/// `FunctionLoweringInfo::ComputePHILiveOutRegInfo`
/// ([L432](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/FunctionLoweringInfo.cpp#L432)):
/// what a phi's register knows is what its incoming values know.
/// `register_bits` is the width of the one register an integer of that many
/// bits is held in, none where it takes several. A phi with an incoming value
/// that is no integer constant and whose register knows nothing is left
/// unknown, as is one with an incoming value of another kind (`undef`, a
/// constant expression).
pub fn compute_phi_live_out_reg_info(
    module: &Module,
    function: &Function,
    phi: InstId,
    register_bits: &dyn Fn(u32) -> Option<u32>,
    info: &mut FunctionLoweringInfo,
) {
    let instruction = function.instruction(phi);
    let Some(result) = instruction.result else { return };
    let Some(&reg) = info.value_regs.get(&result) else { return };
    let Some(width) = module.context.types.int_bits(instruction.ty).and_then(register_bits) else { return };
    let all = super::fold::mask(width);
    let mut known: Option<LiveOut> = None;
    for pair in instruction.operands.chunks(2) {
        let incoming = match pair[0] {
            Operand::Constant(constant) => match module.context.get(constant).kind {
                ConstantKind::Int(value) => {
                    let value = value & all;
                    let sign = value >> (width - 1) & 1 == 1;
                    let same = (0..width).rev().take_while(|&bit| (value >> bit & 1 == 1) == sign).count() as u32;
                    LiveOut { sign_bits: same, zero: !value & all, one: value }
                }
                _ => return,
            },
            Operand::Value(value) => {
                let Some(source) = info.value_regs.get(&value).and_then(|reg| info.live_out.get(reg)) else { return };
                *source
            }
            Operand::Block(_) => return,
        };
        known = Some(match known {
            None => incoming,
            Some(so_far) => LiveOut {
                sign_bits: so_far.sign_bits.min(incoming.sign_bits),
                zero: so_far.zero & incoming.zero,
                one: so_far.one & incoming.one,
            },
        });
    }
    if let Some(known) = known {
        info.add_live_out(reg, known);
    }
}
