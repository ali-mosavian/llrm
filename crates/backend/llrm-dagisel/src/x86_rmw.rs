//! `foldLoadStoreIntoMemOperand`: `store (op (load p), x), p` as one
//! read-modify-write instruction (`add [m], r`, `inc [m]`, `neg [m]`).
//!
//! # How it works
//! A translation of [L3925][fold] and `isFusableLoadOpStorePattern`
//! ([L3787][fusable]) for the 8, 16 and 32-bit memory types. The store's stored
//! value must be an `X86ISD` arithmetic node with one use, whose one operand is
//! a normal load of the store's own address that nothing else uses, and whose
//! chain reaches the store with no path from the load round the arithmetic.
//! Then the load, the arithmetic and the store become one machine node on the
//! address `selectAddr` makes; `i64` memory types do not occur at 32 bits
//! (SD35).
//!
//! # Used by
//! The `X86DAGToDAGISel::Select` loop, before the table, for `ISD::STORE`.
//!
//! [fold]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L3925
//! [fusable]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L3787

use crate::dag::{LoadExt, SDValue};
use crate::generated::ids::{isd, mvt, x86, x86isd};
use crate::matcher::Vt;
use crate::x86_addr::{AddrDag, Matcher};
use crate::x86_hooks::X86Hooks;

/// The memory opcode of an operation by operand width: `[8, 16, 32]`.
struct Forms {
    reg: [u16; 3],
    imm: [u16; 3],
}

fn forms(op: u16) -> Forms {
    match op {
        x86isd::ADD => {
            Forms { reg: [x86::ADD8mr, x86::ADD16mr, x86::ADD32mr], imm: [x86::ADD8mi, x86::ADD16mi, x86::ADD32mi] }
        }
        x86isd::ADC => {
            Forms { reg: [x86::ADC8mr, x86::ADC16mr, x86::ADC32mr], imm: [x86::ADC8mi, x86::ADC16mi, x86::ADC32mi] }
        }
        x86isd::SUB => {
            Forms { reg: [x86::SUB8mr, x86::SUB16mr, x86::SUB32mr], imm: [x86::SUB8mi, x86::SUB16mi, x86::SUB32mi] }
        }
        x86isd::SBB => {
            Forms { reg: [x86::SBB8mr, x86::SBB16mr, x86::SBB32mr], imm: [x86::SBB8mi, x86::SBB16mi, x86::SBB32mi] }
        }
        x86isd::AND => {
            Forms { reg: [x86::AND8mr, x86::AND16mr, x86::AND32mr], imm: [x86::AND8mi, x86::AND16mi, x86::AND32mi] }
        }
        x86isd::OR => {
            Forms { reg: [x86::OR8mr, x86::OR16mr, x86::OR32mr], imm: [x86::OR8mi, x86::OR16mi, x86::OR32mi] }
        }
        x86isd::XOR => {
            Forms { reg: [x86::XOR8mr, x86::XOR16mr, x86::XOR32mr], imm: [x86::XOR8mi, x86::XOR16mi, x86::XOR32mi] }
        }
        _ => unreachable!("not a read-modify-write operation"),
    }
}

fn width_index(vt: Vt) -> Option<usize> {
    [mvt::i8, mvt::i16, mvt::i32].iter().position(|&w| w == vt)
}

fn is_constant<D: AddrDag>(
    dag: &D,
    v: SDValue,
) -> Option<i64> {
    (dag.opcode(v.node) == isd::Constant).then(|| dag.constant(v)).flatten()
}

/// `isFusableLoadOpStorePattern`: the load and the chain the new node starts
/// from.
fn fusable<D: AddrDag>(
    dag: &mut D,
    store: u32,
    stored: SDValue,
    load_op_no: usize,
) -> Option<(u32, SDValue)> {
    if stored.res != 0 || !dag.has_n_uses_of_value(SDValue::new(stored.node, 0), 1) {
        return None;
    }
    if dag.is_truncating_store(store) || dag.is_indexed(store) || dag.is_non_temporal(store) {
        return None;
    }
    let load = dag.operand(stored.node, load_op_no);
    if dag.opcode(load.node) != isd::LOAD
        || dag.load_ext_type(load.node) != Some(LoadExt::NonExt)
        || dag.is_indexed(load.node)
        || !dag.has_one_use(load)
    {
        return None;
    }
    // The load's pointer and offset are the store's.
    if dag.operand(load.node, 1) != dag.operand(store, 2) || dag.operand(load.node, 2) != dag.operand(store, 3) {
        return None;
    }

    let load_chain = SDValue::new(load.node, 1);
    let chain = dag.operand(store, 0);
    let mut chain_ops = Vec::new();
    let mut worklist = Vec::new();
    let mut found = false;
    if chain == load_chain {
        found = true;
        chain_ops.push(dag.operand(load.node, 0));
    } else if dag.opcode(chain.node) == isd::TokenFactor {
        for i in 0..dag.num_operands(chain.node) {
            let op = dag.operand(chain.node, i);
            if op == load_chain {
                found = true;
                chain_ops.push(dag.operand(load.node, 0));
                continue;
            }
            worklist.push(op.node);
            chain_ops.push(op);
        }
    }
    if !found {
        return None;
    }
    // Fusing must not put the arithmetic on a path from the load to the store's
    // chain.
    for i in 0..dag.num_operands(stored.node) {
        let op = dag.operand(stored.node, i);
        if op.node != load.node {
            worklist.push(op.node);
        }
    }
    if dag.has_predecessor_helper(load.node, &mut crate::dag::Visited::new(), &mut worklist, 1024) {
        return None;
    }
    let input_chain = if chain_ops.len() == 1 { chain_ops[0] } else { dag.token_factor(&chain_ops) };
    Some((load.node, input_chain))
}

/// Folds the store `store` with its load and arithmetic; `true` when it did.
pub fn fold_load_store_into_mem_operand<D: AddrDag>(
    dag: &mut D,
    hooks: &X86Hooks,
    store: u32,
) -> bool {
    let stored = dag.operand(store, 1);
    let opc = dag.opcode(stored.node);
    let mem_vt = dag.memory_vt(store);
    let Some(width) = width_index(mem_vt) else { return false };

    let (commutable, negate) = match opc {
        x86isd::SUB => (false, is_constant(dag, dag.operand(stored.node, 0)) == Some(0)),
        x86isd::SBB => (false, false),
        x86isd::ADD | x86isd::ADC | x86isd::AND | x86isd::OR | x86isd::XOR => (true, false),
        _ => return false,
    };

    let mut load_op_no = usize::from(negate);
    let mut found = fusable(dag, store, stored, load_op_no);
    if found.is_none() && commutable {
        load_op_no = 1;
        found = fusable(dag, store, stored, load_op_no);
    }
    let Some((load, input_chain)) = found else { return false };

    let ptr = dag.operand(load, 1);
    let Some(a) = (Matcher {
        dag,
        pointer_type: hooks.options.pointer_type(),
        mode: hooks.options.address_mode,
        size: hooks.options.opt_for_size,
    })
    .select_addr_of(Some(load), ptr) else {
        return false;
    };
    let address = [a.base, a.scale, a.index, a.disp, a.segment];
    let results = [mvt::i32, mvt::Other];

    let mut opc = opc;
    let result = 'build: {
        if opc == x86isd::SUB && negate {
            let neg = [x86::NEG8m, x86::NEG16m, x86::NEG32m][width];
            let ops = [address[0], address[1], address[2], address[3], address[4], input_chain];
            break 'build dag.machine_node(neg, &results, &ops);
        }
        if (opc == x86isd::ADD || opc == x86isd::SUB) && (!hooks.slow_inc_dec() || hooks.options.opt_for_size) {
            let rhs = dag.operand(stored.node, 1);
            let is_one = is_constant(dag, rhs) == Some(1);
            let is_neg_one = is_constant(dag, rhs) == Some(-1);
            if (is_one || is_neg_one) && hooks.has_no_carry_flag_uses(dag, SDValue::new(stored.node, 1)) {
                let inc = (opc == x86isd::ADD) == is_one;
                let new =
                    if inc { [x86::INC8m, x86::INC16m, x86::INC32m] } else { [x86::DEC8m, x86::DEC16m, x86::DEC32m] }
                        [width];
                let ops = [address[0], address[1], address[2], address[3], address[4], input_chain];
                break 'build dag.machine_node(new, &results, &ops);
            }
        }
        let f = forms(opc);
        let mut new = f.reg[width];
        let mut operand = dag.operand(stored.node, 1 - load_op_no);
        if let Some(mut value) = is_constant(dag, operand) {
            // `add m, -128` shorter as `sub m, 128`.
            let fits8 = |v: i64| i8::try_from(v).is_ok();
            if (opc == x86isd::ADD || opc == x86isd::SUB)
                && mem_vt != mvt::i8
                && !fits8(value)
                && fits8(value.wrapping_neg())
                && hooks.has_no_carry_flag_uses(dag, SDValue::new(stored.node, 1))
            {
                value = value.wrapping_neg();
                opc = if opc == x86isd::ADD { x86isd::SUB } else { x86isd::ADD };
            }
            operand = dag.target_constant(value, mem_vt);
            new = forms(opc).imm[width];
        }
        if opc == x86isd::ADC || opc == x86isd::SBB {
            let flags_in = dag.operand(stored.node, 2);
            let copy = dag.copy_to_reg(input_chain, x86::EFLAGS, flags_in, None);
            let ops = [
                address[0],
                address[1],
                address[2],
                address[3],
                address[4],
                operand,
                SDValue::new(copy, 0),
                SDValue::new(copy, 1),
            ];
            dag.machine_node(new, &results, &ops)
        } else {
            let ops = [address[0], address[1], address[2], address[3], address[4], operand, input_chain];
            dag.machine_node(new, &results, &ops)
        }
    };

    dag.set_node_mem_refs(result, &[store, load]);
    dag.replace_uses(SDValue::new(load, 1), SDValue::new(result, 1));
    dag.replace_uses(SDValue::new(store, 0), SDValue::new(result, 1));
    dag.replace_uses(SDValue::new(stored.node, 1), SDValue::new(result, 0));
    dag.remove_dead_node(store);
    true
}
