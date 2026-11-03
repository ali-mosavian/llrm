//! `BaseIndexOffset`: a memory access's address as a base, an index and a
//! constant offset (the pin's `SelectionDAGAddressAnalysis`,
//! [SelectionDAGAddressAnalysis.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAGAddressAnalysis.cpp)).
//!
//! # How it works
//! [`BaseIndexOffset::matching`] strips constant adds, ors that act as adds
//! and one `base + index` from a load's or store's address.
//! [`BaseIndexOffset::equal_base_index`] says whether two share base and index
//! and how many bytes apart they are; [`compute_aliasing`] turns that, or the
//! kinds of their bases (frame index, global, constant pool), into a verdict.
//! The verdict itself is `llrm_analysis::placealias::alias_of`, shared with
//! arm64's load/store pairing; this file only reads the DAG into its inputs.
//!
//! DEVIATE AA1: no indexed loads or stores (x86 has none) and no lifetime
//! nodes (the builder drops the markers): `matchLSNode`'s pre-inc/pre-dec and
//! indexed-base cases, and `LifetimeSDNode`, are not translated. Exit: none.
//! Owner: w12.
//!
//! DEVIATE AA2: a constant pool entry has no offset and a fixed frame index's
//! object offset is `SelectionDAG::fixed_offsets` (the builder fills it from
//! the convention). Exit: none. Owner: w12.
//!
//! # Used by
//! The combiner's chain walk (`mayAlias`, `GatherAllAliases`).

use llrm_analysis::aa::AliasResult;
use llrm_analysis::placealias::{Object, alias_of};

use super::dag::{NodeData, SDValue, SelectionDAG};
use super::isd::ISD;
use super::known_bits::masked_value_is_zero;
use super::x86_lowering::x86isd;

/// `BaseIndexOffset`; no `base` is a failed match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BaseIndexOffset {
    pub base: Option<SDValue>,
    pub index: Option<SDValue>,
    pub offset: i64,
    pub index_sign_ext: bool,
}

/// `bits` as a two's-complement number `width` bits wide.
fn signed(
    bits: u128,
    width: u32,
) -> i64 {
    let shift = 128 - width;
    (((bits << shift) as i128) >> shift) as i64
}

/// `getSExtValue` of a `ConstantSDNode` operand.
fn constant(
    dag: &SelectionDAG,
    value: SDValue,
) -> Option<i64> {
    match dag.node(value).data {
        NodeData::Const { bits, .. } if dag.opcode(value) == ISD::Constant => {
            Some(signed(bits, dag.value_type(value).size_in_bits()))
        }
        _ => None,
    }
}

/// `TLI.unwrapAddress` (X86): the symbol under a `Wrapper`.
fn unwrap(
    dag: &SelectionDAG,
    value: SDValue,
) -> SDValue {
    if matches!(dag.opcode(value), x86isd::WRAPPER | x86isd::WRAPPER_RIP) { dag.operand(value, 0) } else { value }
}

fn is_any_add(
    dag: &SelectionDAG,
    value: SDValue,
) -> bool {
    dag.opcode(value) == ISD::Add
}

impl BaseIndexOffset {
    const NONE: Self = Self { base: None, index: None, offset: 0, index_sign_ext: false };

    /// `matchLSNode`, by `BaseIndexOffset::match`: only a load or a store has
    /// an address.
    #[must_use]
    pub fn matching(
        dag: &SelectionDAG,
        node: SDValue,
    ) -> Self {
        let ptr = match dag.opcode(node) {
            ISD::Load => dag.operand(node, 1),
            ISD::Store => dag.operand(node, 2),
            _ => return Self::NONE,
        };
        // (((B + I*M) + c)) + c ...
        let mut base = unwrap(dag, ptr);
        let mut offset = 0i64;
        // Consume constant adds and ors with appropriate masking.
        loop {
            match dag.opcode(base) {
                // Only consider ORs which act as adds.
                ISD::Or => {
                    if let Some(c) = constant(dag, dag.operand(base, 1)) {
                        let width = dag.value_type(base).size_in_bits();
                        let mask = (c as u128) & super::fold::mask(width);
                        if masked_value_is_zero(dag, dag.operand(base, 0), mask) {
                            offset += c;
                            base = unwrap(dag, dag.operand(base, 0));
                            continue;
                        }
                    }
                }
                ISD::Add => {
                    if let Some(c) = constant(dag, dag.operand(base, 1)) {
                        offset += c;
                        base = unwrap(dag, dag.operand(base, 0));
                        continue;
                    }
                }
                _ => {}
            }
            break;
        }
        let index = None;
        let mut index_sign_ext = false;
        if is_any_add(dag, base) {
            // Inside a loop the base pointer is an add and a multiply: it is
            // the base.
            if dag.opcode(dag.operand(base, 1)) == ISD::Mul {
                return Self { base: Some(base), index, offset, index_sign_ext };
            }
            // Base + Index + Offset.
            let mut at = dag.operand(base, 1);
            let potential = dag.operand(base, 0);
            if dag.opcode(at) == ISD::SignExtend {
                at = dag.operand(at, 0);
                index_sign_ext = true;
            }
            // Index + constant.
            let Some(c) = is_any_add(dag, at).then(|| constant(dag, dag.operand(at, 1))).flatten() else {
                return Self { base: Some(potential), index: Some(at), offset, index_sign_ext };
            };
            offset += c;
            at = dag.operand(at, 0);
            if dag.opcode(at) == ISD::SignExtend {
                at = dag.operand(at, 0);
                index_sign_ext = true;
            } else {
                index_sign_ext = false;
            }
            return Self { base: Some(potential), index: Some(at), offset, index_sign_ext };
        }
        Self { base: Some(base), index, offset, index_sign_ext }
    }

    /// `equalBaseIndex`: the bytes from `self` to `other`, when both share a
    /// base and an index the walk can compare.
    #[must_use]
    pub fn equal_base_index(
        &self,
        other: &Self,
        dag: &SelectionDAG,
    ) -> Option<i64> {
        let (base, other_base) = (self.base?, other.base?);
        let mut off = other.offset - self.offset;
        if other.index != self.index || other.index_sign_ext != self.index_sign_ext {
            return None;
        }
        // Trivial match.
        if other_base == base {
            return Some(off);
        }
        match (dag.node(base).data, dag.node(other_base).data) {
            (NodeData::Global { symbol: a, offset: at }, NodeData::Global { symbol: b, offset: bt }) if a == b => {
                off += bt - at;
                Some(off)
            }
            (NodeData::Global { .. }, _) => None,
            (NodeData::PoolIndex(a, at), NodeData::PoolIndex(b, bt)) => (dag.constant_pool.get(a as usize)
                == dag.constant_pool.get(b as usize))
            .then_some(off + i64::from(bt) - i64::from(at)),
            (NodeData::PoolIndex(..), _) => None,
            (NodeData::FrameIndex(a), NodeData::FrameIndex(b)) => {
                // Equal frame indices: the offsets compare directly. Two
                // fixed objects have known places; an alloca's is not.
                if a == b {
                    return Some(off);
                }
                match (dag.fixed_offsets.get(&a), dag.fixed_offsets.get(&b)) {
                    (Some(at), Some(bt)) => Some(off + bt - at),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// `computeAliasing`: whether two accesses of `bytes0` and `bytes1` bytes
/// (`None`: unknown) alias, or `None` when the addresses do not say.
#[must_use]
pub fn compute_aliasing(
    dag: &SelectionDAG,
    op0: SDValue,
    bytes0: Option<u64>,
    op1: SDValue,
    bytes1: Option<u64>,
) -> Option<bool> {
    let ptr0 = BaseIndexOffset::matching(dag, op0);
    let ptr1 = BaseIndexOffset::matching(dag, op1);
    let (base0, base1) = (ptr0.base?, ptr1.base?);
    let object = |value: SDValue| match dag.node(value).data {
        NodeData::FrameIndex(index) => {
            Some(Object::Frame { id: i64::from(index), fixed: dag.fixed_offsets.contains_key(&index) })
        }
        NodeData::Global { symbol, .. } => Some(Object::Global(symbol)),
        NodeData::PoolIndex(..) => Some(Object::Pool),
        _ => None,
    };
    match alias_of(ptr0.equal_base_index(&ptr1, dag), object(base0).as_ref(), bytes0, object(base1).as_ref(), bytes1) {
        AliasResult::NoAlias => Some(false),
        AliasResult::MayAlias => None,
        AliasResult::PartialAlias { .. } | AliasResult::MustAlias => Some(true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::selectiondag::dag::{DagConfig, MemBase, MemInfo};
    use crate::backend::selectiondag::mvt::EVT;

    fn info() -> MemInfo {
        MemInfo {
            mem_vt: EVT::I32,
            align: 4,
            volatile: false,
            base: MemBase::Unknown,
            ext: None,
            space: 0,
            ordering: None,
        }
    }

    struct Access {
        dag: SelectionDAG,
        entry: SDValue,
    }

    fn setup() -> Access {
        let dag = SelectionDAG::new(DagConfig::default());
        let entry = dag.entry_node();
        Access { dag, entry }
    }

    impl Access {
        fn load_at(
            &mut self,
            ptr: SDValue,
        ) -> SDValue {
            self.dag.get_load(EVT::I32, self.entry, ptr, info())
        }

        fn plus(
            &mut self,
            base: SDValue,
            bytes: i64,
        ) -> SDValue {
            let c = self.dag.get_constant(bytes as u128, EVT::I32);
            self.dag.get_node(ISD::Add, EVT::I32, &[base, c])
        }

        fn alias(
            &self,
            a: SDValue,
            b: SDValue,
        ) -> Option<bool> {
            compute_aliasing(&self.dag, a, Some(4), b, Some(4))
        }
    }

    /// Two words of one object, 4 apart, do not alias; 2 apart they do.
    #[test]
    fn accesses_off_one_base_are_compared_by_offset() {
        let mut t = setup();
        let base = t.dag.get_register(EVT::I32, 100);
        let (p4, p2) = (t.plus(base, 4), t.plus(base, 2));
        let (a, b, c) = (t.load_at(base), t.load_at(p4), t.load_at(p2));
        assert_eq!(t.alias(a, b), Some(false));
        assert_eq!(t.alias(a, c), Some(true));
        assert_eq!(t.alias(c, a), Some(true));
        assert_eq!(t.alias(b, a), Some(false));
    }

    /// Two unrelated pointers say nothing: the walk must keep the order.
    #[test]
    fn unrelated_pointers_are_not_decided() {
        let mut t = setup();
        let (p, q) = (t.dag.get_register(EVT::I32, 100), t.dag.get_register(EVT::I32, 101));
        let (a, b) = (t.load_at(p), t.load_at(q));
        assert_eq!(t.alias(a, b), None);
    }

    /// An alloca, a global and a constant pool entry are different objects.
    #[test]
    fn objects_of_different_kinds_or_symbols_do_not_alias() {
        let mut t = setup();
        let (slot, other) = (t.dag.get_frame_index(EVT::I32, 0), t.dag.get_frame_index(EVT::I32, 1));
        let (g, h) = (t.dag.get_global_address(EVT::I32, 7, 0), t.dag.get_global_address(EVT::I32, 8, 0));
        let [a, b, c, d] = [slot, other, g, h].map(|p| t.load_at(p));
        assert_eq!(t.alias(a, b), Some(false), "two allocas");
        assert_eq!(t.alias(a, c), Some(false), "frame and global");
        assert_eq!(t.alias(c, d), Some(false), "two globals");
        let g4 = t.dag.get_global_address(EVT::I32, 7, 4);
        let e = t.load_at(g4);
        assert_eq!(t.alias(c, e), Some(false), "one global, 4 apart");
    }

    /// Fixed objects have places: the slot above is 4 bytes on.
    #[test]
    fn fixed_objects_compare_by_their_offsets() {
        let mut t = setup();
        t.dag.fixed_offsets.insert(-1, 8);
        t.dag.fixed_offsets.insert(-2, 12);
        let (one, two) = (t.dag.get_frame_index(EVT::I32, -1), t.dag.get_frame_index(EVT::I32, -2));
        let (a, b) = (t.load_at(one), t.load_at(two));
        assert_eq!(t.alias(a, b), Some(false));
        let mid = t.plus(one, 6);
        let c = t.load_at(mid);
        assert_eq!(t.alias(c, b), Some(true), "[14,18) overlaps [12,16)");
    }
}
