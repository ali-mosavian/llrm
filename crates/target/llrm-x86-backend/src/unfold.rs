//! `X86InstrInfo::unfoldMemoryOperand` for a DAG node
//! ([X86InstrInfo.cpp#L9026][pin]): a machine node with a folded load becomes
//! the load and the node on registers.
//!
//! # How it works
//! The pin's memory-fold tables, read backwards (`llrm_dagisel`'s
//! `foldtables::UNFOLD`), name the register form of the node's opcode and the
//! operand the memory reference replaced. The address operands go to a load of
//! that operand's register class, the others to the register form, which also
//! reads the loaded value:
//!
//! ```text
//! ADD64rm a, [base idx..], ch      MOV64rm [base idx..], ch  ->  t
//!                              =>  ADD64rr a, t
//! ```
//!
//! # DEVIATEs
//! FT2. A node with a folded store (`ADD64mr`: a read-modify-write) is not
//! unfolded; the pin also makes the store. Exit: when a program needs it.
//! FT3. Only the general registers (`GR8` to `GR64`): the load opcode of a
//! vector class (`getLoadRegOpcode`) is not here, and no vector node is
//! selected. Exit: vector MIR.
//!
//! # Used by
//! The scheduler's `TryUnfoldSU`, through
//! [`llrm_codegen::backend::selectiondag::schedule::SchedInfo`].
//!
//! [pin]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L9026

use llrm_codegen::backend::selectiondag::dag::{NodeData, SDNodeFlags, SDValue, SelectionDAG};
use llrm_codegen::backend::selectiondag::{EVT, ISD};
use llrm_dagisel::generated::ids::x86;
use llrm_x86_dagisel::generated::foldtables::{FOLDED_LOAD, FOLDED_STORE, INDEX_MASK, UNFOLD};
use llrm_x86_dagisel::generated::instrinfo::desc;

/// The widths a general register class loads: `getLoadRegOpcode`'s `MOV*rm`
/// for `GR8` to `GR64`, with the value type the class holds.
const LOADS: [(u16, EVT); 4] =
    [(x86::MOV8rm, EVT::I8), (x86::MOV16rm, EVT::I16), (x86::MOV32rm, EVT::I32), (x86::MOV64rm, EVT::I64)];

/// The load that fills a register of class `class`, and the type it holds.
fn load_for_class(class: i16) -> Option<(u16, EVT)> {
    LOADS
        .into_iter()
        .find(|&(opcode, _)| desc(opcode).and_then(|d| d.operands.first()).is_some_and(|o| o.reg_class == class))
}

/// The load and the register form of `node`, or None where the pin's
/// `unfoldMemoryOperand` is false or this one is not translated.
pub fn unfold_memory_operand(
    dag: &mut SelectionDAG,
    node: u32,
) -> Option<[u32; 2]> {
    let ISD::Machine(opcode) = dag.opcode(SDValue { node, res: 0 }) else { return None };
    let at = UNFOLD.binary_search_by_key(&(opcode as u16), |&(mem, _, _)| mem).ok()?;
    let (_, register_form, flags) = UNFOLD[at];
    if flags & FOLDED_STORE != 0 || flags & FOLDED_LOAD == 0 {
        return None;
    }
    let index = usize::from(flags & INDEX_MASK);
    let info = desc(register_form)?;
    let num_defs = usize::from(info.num_defs);
    let class = info.operands.get(index)?.reg_class;
    let (load_opcode, load_vt) = load_for_class(class)?;

    let ops: Vec<SDValue> = dag.node(SDValue { node, res: 0 }).ops.to_vec();
    let (chain, rest) = ops.split_last()?;
    // `Index - NumDefs` is the first address operand: `AddrNumOperands` = 5.
    let first = index.checked_sub(num_defs)?;
    if rest.len() < first + 5 {
        return None;
    }
    let mut address: Vec<SDValue> = rest[first..first + 5].to_vec();
    address.push(*chain);
    let mut before: Vec<SDValue> = rest[..first].to_vec();
    let after: Vec<SDValue> = rest[first + 5..].to_vec();

    let load = dag.intern(
        ISD::Machine(u32::from(load_opcode)),
        &[load_vt, EVT::OTHER],
        &address,
        SDNodeFlags::default(),
        NodeData::None,
    );
    // The register form's results: its defs, then the node's other values
    // (the flags) but not the chain.
    let values = dag.node(SDValue { node, res: 0 }).values.to_vec();
    let mut vts = Vec::new();
    if num_defs > 0 {
        vts.push(load_for_class(info.operands.first()?.reg_class)?.1);
    }
    for (i, &vt) in values.iter().enumerate() {
        if vt != EVT::OTHER && i >= num_defs {
            vts.push(vt);
        }
    }
    before.push(load);
    before.extend(after);
    let register =
        dag.intern(ISD::Machine(u32::from(register_form)), &vts, &before, SDNodeFlags::default(), NodeData::None);
    Some([load.node, register.node])
}

#[cfg(test)]
mod tests {
    use llrm_codegen::backend::selectiondag::dag::DagConfig;
    use llrm_x86_dagisel::generated::x86_x64::MACHINE_OPCODES;

    use super::*;

    fn id(name: &str) -> u32 {
        u32::from(MACHINE_OPCODES.iter().find(|(_, n)| *n == name).unwrap_or_else(|| panic!("no {name}")).0)
    }

    /// `ADD64rm a, [b], ch` left the load folded in the add that the scheduler
    /// clones for the flags: sat_builtins -O2 m64 read the cell twice (+32 B).
    #[test]
    fn test_a_folded_load_add_is_a_load_and_an_add_on_registers() {
        let mut dag = SelectionDAG::new(DagConfig::default());
        let entry = dag.entry_node();
        let a = dag.get_copy_from_reg(entry, 1000, EVT::I64);
        let base = dag.get_copy_from_reg(entry, 1001, EVT::I64);
        let (one, zero) = (dag.get_constant(1, EVT::I8), dag.get_constant(0, EVT::I32));
        let ops = [a, base, one, zero, zero, zero, SDValue { node: a.node, res: 1 }];
        let add = dag.intern(
            ISD::Machine(id("ADD64rm")),
            &[EVT::I64, EVT::I32, EVT::OTHER],
            &ops,
            SDNodeFlags::default(),
            NodeData::None,
        );
        let [load, register] = unfold_memory_operand(&mut dag, add.node).expect("unfolds");
        let (load, register) = (SDValue { node: load, res: 0 }, SDValue { node: register, res: 0 });
        assert_eq!(dag.opcode(load), ISD::Machine(id("MOV64rm")));
        assert_eq!(dag.node(load).ops.len(), 6, "base, scale, index, disp, segment, chain");
        assert_eq!(dag.node(load).values, [EVT::I64, EVT::OTHER]);
        assert_eq!(dag.opcode(register), ISD::Machine(id("ADD64rr")));
        assert_eq!(dag.node(register).ops.as_slice(), [a, load]);
        assert_eq!(dag.node(register).values, [EVT::I64, EVT::I32], "the value and the flags, no chain");
    }

    #[test]
    fn test_a_register_form_does_not_unfold() {
        let mut dag = SelectionDAG::new(DagConfig::default());
        let entry = dag.entry_node();
        let a = dag.get_copy_from_reg(entry, 1000, EVT::I64);
        let add = dag.intern(
            ISD::Machine(id("ADD64rr")),
            &[EVT::I64, EVT::I32],
            &[a, a],
            SDNodeFlags::default(),
            NodeData::None,
        );
        assert_eq!(unfold_memory_operand(&mut dag, add.node), None);
    }
}
