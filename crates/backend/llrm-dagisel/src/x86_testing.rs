//! Hooks for the x86-32 tables over [`TestDag`](crate::testing::TestDag).

use crate::dag::{Dag, Hooks, SDValue, X86Predicates};
use crate::generated::ids::mvt;
use crate::matcher::Vt;
use crate::testing::TestDag;

/// Hooks for a 32-bit target whose address pattern the test supplies.
pub struct TestHooks {
    /// What `selectAddr` answers for the address it is asked about: base,
    /// index, scale, disp.
    pub address: Option<(SDValue, SDValue, i64, i64)>,
}

impl Hooks<TestDag> for TestHooks {
    fn pointer_type(&self) -> Vt {
        mvt::i32
    }
    fn check_pattern_predicate(
        &self,
        predicate: u16,
    ) -> bool {
        crate::generated::x86_i486::pattern_predicate(&OptSize, predicate)
    }
    fn check_node_predicate(
        &self,
        dag: &TestDag,
        value: SDValue,
        predicate: u16,
    ) -> bool {
        crate::generated::x86_i486::node_predicate(dag, self, value, predicate)
    }
    fn check_complex_pattern(
        &mut self,
        dag: &mut TestDag,
        _root: u32,
        _parent: Option<u32>,
        _value: SDValue,
        pattern: u8,
        results: &mut Vec<(SDValue, Option<u32>)>,
    ) -> bool {
        // Only `selectAddr` is stubbed; the other complex patterns match
        // nothing.
        let (0, Some((base, index, scale, disp))) = (pattern, self.address) else { return false };
        let scale = dag.target_constant(scale, mvt::i8);
        let disp = dag.target_constant(disp, mvt::i32);
        let segment = dag.register(0, mvt::i16);
        results.extend([base, scale, index, disp, segment].map(|v| (v, None)));
        true
    }
    fn run_xform(
        &mut self,
        _: &mut TestDag,
        value: SDValue,
        _: u8,
    ) -> SDValue {
        value
    }
    fn is_profitable_to_fold(
        &self,
        _: &TestDag,
        _: SDValue,
        _: u32,
        _: u32,
    ) -> bool {
        true
    }
    fn is_legal_to_fold(
        &self,
        _: &TestDag,
        _: SDValue,
        _: u32,
        _: u32,
    ) -> bool {
        true
    }
    fn check_and_mask(
        &self,
        _: &TestDag,
        _: SDValue,
        _: i64,
        _: i64,
    ) -> bool {
        false
    }
    fn check_or_mask(
        &self,
        _: &TestDag,
        _: SDValue,
        _: i64,
        _: i64,
    ) -> bool {
        false
    }
    fn may_load_store(
        &self,
        _: u16,
    ) -> (bool, bool) {
        (true, true)
    }
}

impl X86Predicates<TestDag> for TestHooks {
    fn has_sse_unaligned_mem(&self) -> bool {
        false
    }
    fn should_avoid_immediate_inst_forms_for_size(
        &self,
        _: &TestDag,
        _: u32,
    ) -> bool {
        false
    }
    fn is_unneeded_shift_mask(
        &self,
        _: &TestDag,
        _: u32,
        _: u32,
    ) -> bool {
        false
    }
    fn is_sext_absolute_symbol_ref(
        &self,
        _: &TestDag,
        _: u32,
        _: u32,
    ) -> bool {
        false
    }
    fn has_no_carry_flag_uses(
        &self,
        _: &TestDag,
        _: SDValue,
    ) -> bool {
        false
    }
    fn is_def32(
        &self,
        _: &TestDag,
        _: u32,
    ) -> bool {
        false
    }
    fn check_tc_ret_enough_regs(
        &self,
        _: &TestDag,
        _: u32,
    ) -> bool {
        true
    }
}

pub struct OptSize;

impl crate::dag::PatternContext for OptSize {
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
