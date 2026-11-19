//! The 6502's DAG-to-DAG hooks as the interpreter's [`Hooks`]: the one complex
//! pattern the table names, fold legality and the node predicates.
//!
//! # How it works
//! `SelectIndirect` is the `(zp),Y` address: a pointer in an `Imag16` pair and
//! a byte offset for Y. `(add ptr, c)` with `0 <= c < 256` gives `ptr` and `c`;
//! any other address is its own pointer at offset 0.

use llrm_dagisel::addr::AddrDag;
use llrm_dagisel::addr::find_non_imm_use;
use llrm_dagisel::dag::{Dag, Hooks, PatternContext, SDValue};
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::matcher::Vt;

use super::generated::{ids::isd as own, instrinfo, mos_mos6502, mos_w65c02};

/// The CPUs the tables are generated for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cpu {
    Mos6502,
    W65c02,
}

/// No pattern predicate of the 6502 tables is decided at selection time.
struct NoDynamicPredicates;

impl PatternContext for NoDynamicPredicates {
    fn should_opt_for_size(&self) -> bool {
        false
    }
    fn has_min_size(&self) -> bool {
        false
    }
    fn has_frame_pointer(&self) -> bool {
        false
    }
    fn is_legal_to_call_immediate_addr(&self) -> bool {
        false
    }
}

pub struct MosHooks {
    cpu: Cpu,
    /// `OptLevel == CodeGenOptLevel::None`: nothing folds.
    opt_none: bool,
}

impl MosHooks {
    pub fn new(
        cpu: Cpu,
        opt_none: bool,
    ) -> MosHooks {
        MosHooks { cpu, opt_none }
    }
}

/// `SelectIndirect`: the pointer and the byte offset.
fn select_indirect<D: AddrDag>(
    dag: &mut D,
    value: SDValue,
    results: &mut Vec<(SDValue, Option<u32>)>,
) {
    // A sum that is also wanted as a value (the pointer a loop steps) is the
    // address as it is: folding its constant instead keeps the old pointer
    // alive beside the new one, a copy of the pair each trip.
    let folds = dag.opcode(value.node) == isd::ADD && !dag.has_materializing_use(value);
    let (base, offset) = match folds.then(|| dag.constant(dag.operand(value.node, 1))) {
        Some(Some(c)) if (0..256).contains(&c) => (dag.operand(value.node, 0), c),
        _ => (value, 0),
    };
    let offset = dag.target_constant(offset, mvt::i8);
    // A frame object stays an object (`rc0` plus its offset once the frame is
    // laid out), not a pointer value built by `LEASP16`: the address becomes a
    // `TargetFrameIndex`, as x86's `getAddressOperands` makes it.
    let base = match dag.frame_index(base) {
        Some(index) => dag.target_frame_index(index, mvt::i16),
        None => base,
    };
    results.push((base, None));
    results.push((offset, None));
}

/// `SelectPointer`: `SelectIndirect` for an address that is a pointer. A
/// global's address (or a constant) is left to the absolute forms: `lda g+36`
/// is one instruction where a pointer pair is four to build, and the folds into
/// an ALU op have no absolute form for every carry op.
fn select_pointer<D: AddrDag>(
    dag: &mut D,
    value: SDValue,
    results: &mut Vec<(SDValue, Option<u32>)>,
) -> bool {
    let base = if dag.opcode(value.node) == isd::ADD { dag.operand(value.node, 0) } else { value };
    let fixed = |dag: &D, v: SDValue| dag.opcode(v.node) == own::WRAPPER || dag.constant(v).is_some();
    if fixed(dag, value) || fixed(dag, base) {
        return false;
    }
    select_indirect(dag, value, results);
    true
}

/// `SelectPlain`: a pointer with no constant to add and no frame object, as
/// itself.
fn select_plain<D: AddrDag>(
    dag: &mut D,
    value: SDValue,
    results: &mut Vec<(SDValue, Option<u32>)>,
) -> bool {
    let offset = dag.opcode(value.node) == isd::ADD && dag.constant(dag.operand(value.node, 1)).is_some();
    if offset || dag.frame_index(value).is_some() {
        return false;
    }
    results.push((value, None));
    true
}

/// `SelectIndexed`: `(add pointer, (zext i8 index))` as the pointer and the
/// byte index, either operand order; a global's address is left to
/// `SelectAbsX`. A word whose high byte is known zero is the same index: its
/// low half (`MaskedValueIsZero`, as the pin's address complex patterns ask).
fn select_indexed<D: AddrDag>(
    dag: &mut D,
    value: SDValue,
    results: &mut Vec<(SDValue, Option<u32>)>,
) -> bool {
    if dag.opcode(value.node) != isd::ADD {
        return false;
    }
    let operands = [dag.operand(value.node, 0), dag.operand(value.node, 1)];
    for (base, extend) in [(operands[0], operands[1]), (operands[1], operands[0])] {
        if dag.opcode(base.node) == own::WRAPPER || dag.value_type(base) != mvt::i16 {
            continue;
        }
        let index = if dag.opcode(extend.node) == isd::ZERO_EXTEND {
            dag.operand(extend.node, 0)
        } else if dag.value_type(extend) == mvt::i16
            && dag.constant(extend).is_none()
            && dag.masked_value_is_zero(extend, 0xFF00)
        {
            dag.get_node(isd::TRUNCATE, mvt::i8, &[extend])
        } else {
            continue;
        };
        if dag.value_type(index) == mvt::i8 {
            results.push((base, None));
            results.push((index, None));
            return true;
        }
    }
    false
}

/// The leaves of the adds below `value`: a node that is not an `add`, or
/// whose operands the caller does not follow, is one leaf.
fn add_leaves<D: Dag>(
    dag: &D,
    value: SDValue,
    leaves: &mut Vec<SDValue>,
) {
    if dag.opcode(value.node) == isd::ADD {
        add_leaves(dag, dag.operand(value.node, 0), leaves);
        add_leaves(dag, dag.operand(value.node, 1), leaves);
    } else {
        leaves.push(value);
    }
}

/// `SelectAbsX`: a global, a byte index and constants added in any order and
/// grouping, `(add (wrapper global), (zext i8 index))` with `+ k` anywhere, as
/// the symbol, the constant (mod 2^16: the access wraps there too) and the
/// byte index. A word whose high byte is known zero is the same index, as for
/// `SelectIndexed` (`MaskedValueIsZero`).
fn select_absx<D: AddrDag>(
    dag: &mut D,
    value: SDValue,
    results: &mut Vec<(SDValue, Option<u32>)>,
) -> bool {
    if dag.opcode(value.node) != isd::ADD {
        return false;
    }
    let mut leaves = Vec::new();
    add_leaves(dag, value, &mut leaves);
    let (mut symbol, mut index, mut offset) = (None, None, 0u64);
    let mut word_index = None;
    for leaf in leaves {
        if let Some(c) = dag.constant(leaf) {
            offset = offset.wrapping_add(c as u64);
        } else if dag.opcode(leaf.node) == own::WRAPPER && symbol.is_none() {
            let target = dag.operand(leaf.node, 0);
            if dag.opcode(target.node) != isd::TargetGlobalAddress {
                return false;
            }
            symbol = Some(target);
        } else if dag.opcode(leaf.node) == isd::ZERO_EXTEND && index.is_none() {
            let byte = dag.operand(leaf.node, 0);
            if dag.value_type(byte) != mvt::i8 {
                return false;
            }
            index = Some(byte);
        } else if index.is_none()
            && word_index.is_none()
            && dag.value_type(leaf) == mvt::i16
            && dag.masked_value_is_zero(leaf, 0xFF00)
        {
            word_index = Some(leaf);
        } else {
            return false;
        }
    }
    if let Some(word) = word_index {
        if index.is_some() {
            return false;
        }
        index = Some(dag.get_node(isd::TRUNCATE, mvt::i8, &[word]));
    }
    let (Some(symbol), Some(index)) = (symbol, index) else { return false };
    results.push((symbol, None));
    results.push((dag.target_constant((offset & 0xFFFF) as i64, mvt::i16), None));
    results.push((index, None));
    true
}

/// `SelectAbs`: `(wrapper global)` or `(add (wrapper global), c)` as the symbol
/// and the offset.
fn select_abs<D: Dag>(
    dag: &mut D,
    value: SDValue,
    results: &mut Vec<(SDValue, Option<u32>)>,
) -> bool {
    let (wrapper, offset) = match (dag.opcode(value.node) == isd::ADD).then(|| dag.constant(dag.operand(value.node, 1)))
    {
        Some(Some(c)) if (0..65536).contains(&c) => (dag.operand(value.node, 0), c),
        _ => (value, 0),
    };
    if dag.opcode(wrapper.node) != own::WRAPPER {
        return false;
    }
    let symbol = dag.operand(wrapper.node, 0);
    if dag.opcode(symbol.node) != isd::TargetGlobalAddress {
        return false;
    }
    results.push((symbol, None));
    results.push((dag.target_constant(offset, mvt::i16), None));
    true
}

impl<D: AddrDag> Hooks<D> for MosHooks {
    fn pointer_type(&self) -> Vt {
        mvt::i16
    }

    fn check_pattern_predicate(
        &self,
        predicate: u16,
    ) -> bool {
        match self.cpu {
            Cpu::Mos6502 => mos_mos6502::pattern_predicate(&NoDynamicPredicates, predicate),
            Cpu::W65c02 => mos_w65c02::pattern_predicate(&NoDynamicPredicates, predicate),
        }
    }

    fn check_node_predicate(
        &self,
        dag: &D,
        value: SDValue,
        predicate: u16,
    ) -> bool {
        match self.cpu {
            Cpu::Mos6502 => mos_mos6502::node_predicate(dag, self, value, predicate),
            Cpu::W65c02 => mos_w65c02::node_predicate(dag, self, value, predicate),
        }
    }

    fn check_complex_pattern(
        &mut self,
        dag: &mut D,
        _root: u32,
        _parent: Option<u32>,
        value: SDValue,
        pattern: u8,
        results: &mut Vec<(SDValue, Option<u32>)>,
    ) -> bool {
        // the numbers follow the order the table first uses them in, so the
        // name is the key
        let table = match self.cpu {
            Cpu::Mos6502 => mos_mos6502::COMPLEX_PATTERNS,
            Cpu::W65c02 => mos_w65c02::COMPLEX_PATTERNS,
        };
        let name = table.iter().find(|(number, ..)| *number == pattern).map(|(_, name, _)| *name);
        match name {
            Some("SelectIndirect") => {
                select_indirect(dag, value, results);
                true
            }
            Some("SelectPointer") => select_pointer(dag, value, results),
            Some("SelectPlain") => select_plain(dag, value, results),
            Some("SelectAbs") => select_abs(dag, value, results),
            Some("SelectAbsX") => select_absx(dag, value, results),
            Some("SelectIndexed") => select_indexed(dag, value, results),
            other => unreachable!("the table has no complex pattern {other:?}"),
        }
    }

    /// The one transform of the table is `to_tframeindex`.
    fn run_xform(
        &mut self,
        dag: &mut D,
        value: SDValue,
        _xform: u8,
    ) -> SDValue {
        let index = dag.frame_index(value).expect("to_tframeindex takes a frame index");
        dag.target_frame_index(index, mvt::i16)
    }

    fn is_profitable_to_fold(
        &self,
        _dag: &D,
        _value: SDValue,
        _user: u32,
        _root: u32,
    ) -> bool {
        !self.opt_none
    }

    /// `SelectionDAGISel::IsLegalToFold`.
    fn is_legal_to_fold(
        &self,
        dag: &D,
        value: SDValue,
        user: u32,
        root: u32,
    ) -> bool {
        if self.opt_none {
            return false;
        }
        let mut root = root;
        let mut ignore_chains = true;
        while dag.value_type(SDValue::new(root, (dag.num_values(root) - 1) as u8)) == mvt::Glue {
            let Some(glued) = dag.glued_user(root) else { break };
            root = glued;
            ignore_chains = false;
        }
        !find_non_imm_use(dag, root, value.node, user, ignore_chains)
    }

    fn check_and_mask(
        &self,
        _: &D,
        _: SDValue,
        _: i64,
        _: i64,
    ) -> bool {
        false
    }

    fn check_or_mask(
        &self,
        _: &D,
        _: SDValue,
        _: i64,
        _: i64,
    ) -> bool {
        false
    }

    fn may_load_store(
        &self,
        opcode: u16,
    ) -> (bool, bool) {
        instrinfo::memory_flags(opcode)
    }
}
