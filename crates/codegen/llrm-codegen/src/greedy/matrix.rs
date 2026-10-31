//! `LiveRegMatrix`: which virtual registers hold which register units, and
//! whether a virtual register can be assigned a physical register.
//! Translated from llvm/include/llvm/CodeGen/LiveRegMatrix.h and
//! llvm/lib/CodeGen/LiveRegMatrix.cpp at d1106deb71cc.
//!
//! # How it works
//! There is one `LiveIntervalUnion` per register unit. Assigning a virtual
//! register to a physical register unifies its live range into the union of
//! every unit of the register; unassigning extracts it. An interference check
//! asks, in order of cost, whether the range crosses a call that destroys the
//! register (`IK_RegMask`), meets a fixed range of one of its units
//! (`IK_RegUnit`), or meets a virtual register already assigned to a unit
//! (`IK_VirtReg`, the only kind eviction can resolve). The queries against the
//! unions are cached per unit and valid until `invalidateVirtRegs`.
//!
//! ```text
//!   assign(%7 -> ax)    units eax.0, eax.1 hold %7
//!   check(%9, al)       al = eax.0: %7 overlaps %9 there  -> IK_VirtReg
//!   check(%9, ah)       eax.1: also %7 (it holds ax)       -> IK_VirtReg
//!   check(%9, bl)       ebx.0: free                        -> IK_Free
//! ```
//!
//! DEVIATE (target interface, no number): the regmask and fixed-range checks
//! are asked of `PhysInterference` (this target's calls and pinned registers
//! are in the allocator's `Masks` and `fixed`), where C++ reads them from
//! `LiveIntervals` (`checkRegMaskInterference`, `getRegUnit`). Sub-range
//! handling (`foreachUnit` over lane masks) is not translated: a value is one
//! range over every unit of its register.
//!
//! # Example
//! After `assign(%1, ax)` the unions of `eax.0` and `eax.1` each hold `%1`'s
//! segments; `check_interference(%2, al)` is `IK_VirtReg` when the ranges meet.
//!
//! # Used by
//! the greedy allocator (`greedy`).

use llrm_lir::registers::RegId;

use super::virtregmap::{RegUnit, RegisterInfo, VirtRegMap};
use crate::liveness::liveintervalunion::{LiveIntervalUnion, Query, VReg};
use crate::liveness::liverange::{LiveInterval, LiveRange};
use crate::liveness::slotindex::SlotIndex;

/// `LiveRegMatrix::InterferenceKind`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InterferenceKind {
    /// No interference, go ahead and assign.
    Free = 0,
    /// Interfering virtual registers are assigned to the register or its
    /// aliases; unassigning them resolves it.
    VirtReg,
    /// A fixed live range is in the way, typically argument registers for a
    /// call. Unassigning virtual registers cannot resolve it.
    RegUnit,
    /// The range crosses an instruction with a regmask operand that doesn't
    /// preserve the register: it is live across a call.
    RegMask,
}

/// What the matrix asks of the target's fixed constraints.
pub trait PhysInterference {
    /// `LiveIntervals::checkRegMaskInterference` (LiveIntervals.cpp#L1007): the
    /// registers (roots) destroyed by some instruction `li` is live across, or
    /// None if it is live across none.
    fn reg_mask_clobbered(
        &self,
        li: &LiveInterval,
    ) -> Option<llrm_lir::registers::RegSet>;
    /// `checkRegUnitInterference`: `li` overlaps a fixed live range of one of
    /// the units of `phys`.
    fn reg_unit_interferes(
        &self,
        li: &LiveInterval,
        phys: RegId,
    ) -> bool;

    /// `LiveIntervals::getRegUnit`: the fixed live range of `unit`, if any.
    fn fixed_range(
        &self,
        _unit: RegUnit,
    ) -> Option<&LiveRange> {
        None
    }

    /// `getRegMaskSlotsInBlock`: the numbers (for `reg_mask_slot` and
    /// `reg_mask_clobbers`) of the regmask slots in `[start, stop)`, ascending.
    fn reg_mask_range(
        &self,
        _start: SlotIndex,
        _stop: SlotIndex,
    ) -> std::ops::Range<usize> {
        0..0
    }

    /// `LiveIntervals::getRegMaskSlotsInBlock`: the same numbers for block
    /// `mbb`, whose slots are `[start, stop)`. A set that keeps its runs per
    /// block gives them without a search.
    fn reg_mask_range_in_block(
        &self,
        _mbb: usize,
        start: SlotIndex,
        stop: SlotIndex,
    ) -> std::ops::Range<usize> {
        self.reg_mask_range(start, stop)
    }

    /// The value a split is being tried for, when what a regmask clobbers of
    /// `phys` depends on it (`reg_mask_clobbers`): None for a register it does
    /// not depend for.
    fn lane_of(
        &self,
        _phys: RegId,
    ) -> Option<u32> {
        None
    }

    /// The value `lane_of` answers for, while a split is tried for it.
    fn set_lane(
        &self,
        _virt: Option<u32>,
    ) {
    }

    /// Whether regmask number `i` destroys any register: an entry that only
    /// reads one (the data segment an access takes without naming it) is
    /// not a regmask operand of the instruction.
    fn reg_mask_destroys(
        &self,
        _i: usize,
    ) -> bool {
        true
    }

    /// The slot of regmask number `i`.
    fn reg_mask_slot(
        &self,
        _i: usize,
    ) -> SlotIndex {
        unreachable!("no regmask")
    }

    /// `MachineOperand::clobbersPhysReg(RegMaskBits[i], phys)`.
    fn reg_mask_clobbers(
        &self,
        _i: usize,
        _phys: RegId,
    ) -> bool {
        false
    }
}

/// No calls and no fixed ranges.
pub struct NoFixed;

impl PhysInterference for NoFixed {
    fn reg_mask_clobbered(
        &self,
        _li: &LiveInterval,
    ) -> Option<llrm_lir::registers::RegSet> {
        None
    }

    fn reg_unit_interferes(
        &self,
        _li: &LiveInterval,
        _phys: RegId,
    ) -> bool {
        false
    }
}

/// `LiveRegMatrix` (LiveRegMatrix.h#L44).
pub struct LiveRegMatrix {
    tri: RegisterInfo,
    pub vrm: VirtRegMap,
    fixed: Box<dyn PhysInterference>,
    /// The unions, by register unit (`Matrix`, a `LiveIntervalUnion::Array`).
    matrix: Vec<LiveIntervalUnion>,
    /// The queries, by register unit (`Queries`).
    queries: Vec<Query>,
    /// Bumped by `invalidateVirtRegs`: query results from before are stale.
    user_tag: u32,
    /// `RegMaskVirtReg`, `RegMaskTag`, `RegMaskUsable`: the registers a call
    /// destroys under the last range asked about, kept for the next ask of it.
    reg_mask: Option<(VReg, u32, Option<llrm_lir::registers::RegSet>)>,
    num_assigned: usize,
    num_unassigned: usize,
}

impl LiveRegMatrix {
    pub fn new(
        tri: RegisterInfo,
        fixed: Box<dyn PhysInterference>,
    ) -> Self {
        Self {
            tri,
            vrm: VirtRegMap::new(),
            fixed,
            matrix: Vec::new(),
            queries: Vec::new(),
            user_tag: 0,
            reg_mask: None,
            num_assigned: 0,
            num_unassigned: 0,
        }
    }

    pub fn tri(&self) -> RegisterInfo {
        self.tri
    }

    /// The fixed constraints (`LIS` regmasks and register units).
    pub fn fixed(&self) -> &dyn PhysInterference {
        &*self.fixed
    }

    /// `Matrix[unit]`, None while no register of it was assigned.
    pub fn union(
        &self,
        unit: RegUnit,
    ) -> Option<&LiveIntervalUnion> {
        self.matrix.get(unit as usize)
    }

    /// The arrays cover `unit` (`LiveIntervalUnion::Array::init`).
    fn grow(
        &mut self,
        unit: RegUnit,
    ) {
        let need = unit as usize + 1;
        if self.matrix.len() < need {
            // A target has a few dozen register units: made in one step.
            self.matrix.reserve(need.max(32) - self.matrix.len());
            self.queries.reserve(need.max(32) - self.queries.len());
            self.matrix.resize_with(need, LiveIntervalUnion::new);
            self.queries.resize_with(need, Query::new);
        }
    }

    fn union_of(
        &mut self,
        unit: RegUnit,
    ) -> &mut LiveIntervalUnion {
        self.grow(unit);
        &mut self.matrix[unit as usize]
    }

    /// `invalidateVirtRegs`: live ranges could have changed.
    pub fn invalidate_virt_regs(&mut self) {
        self.user_tag += 1;
    }

    /// `assign` (LiveRegMatrix.cpp#L91): assigns `virt` to `phys` and unifies
    /// its range into the union of each unit of `phys`.
    pub fn assign(
        &mut self,
        virt: &LiveInterval,
        phys: RegId,
    ) {
        self.vrm.assign_virt2phys(virt.reg(), phys);
        for unit in self.tri.regunits(phys) {
            self.union_of(unit).unify(virt.reg(), &virt.range);
        }
        self.num_assigned += 1;
    }

    /// `unassign` (LiveRegMatrix.cpp#L108): extracts the register from the
    /// unions of the units of its physical register.
    pub fn unassign(
        &mut self,
        virt: &LiveInterval,
    ) {
        let phys = self.vrm.get_phys(virt.reg()).expect("unassigning a register that has none");
        self.vrm.clear_virt(virt.reg());
        for unit in self.tri.regunits(phys) {
            if let Some(union) = self.matrix.get_mut(unit as usize) {
                union.extract(virt.reg(), &virt.range);
            }
        }
        self.num_unassigned += 1;
    }

    /// `isPhysRegUsed`: any unit of the register holds a segment.
    pub fn is_phys_reg_used(
        &self,
        phys: RegId,
    ) -> bool {
        self.tri.regunits(phys).any(|unit| self.matrix.get(unit as usize).is_some_and(|union| !union.empty()))
    }

    /// `query` (LiveRegMatrix.cpp#L160) followed by `checkInterference()`: does
    /// `lr` meet a virtual register in the union of `unit`? The result is kept
    /// until the union changes or the tag moves.
    pub fn query_check(
        &mut self,
        vreg: VReg,
        lr: &LiveRange,
        unit: RegUnit,
    ) -> bool {
        self.grow(unit);
        let union = &self.matrix[unit as usize];
        let q = &mut self.queries[unit as usize];
        q.init(self.user_tag, u64::from(vreg), u64::from(unit), union);
        q.check_interference(lr, union)
    }

    /// `query(...).interferingVRegs(max)`: the virtual registers in the union
    /// of `unit` that `lr` meets, up to `max`.
    pub fn query_vregs(
        &mut self,
        vreg: VReg,
        lr: &LiveRange,
        unit: RegUnit,
        max: usize,
    ) -> Vec<VReg> {
        self.grow(unit);
        let union = &self.matrix[unit as usize];
        let q = &mut self.queries[unit as usize];
        q.init(self.user_tag, u64::from(vreg), u64::from(unit), union);
        q.interfering_vregs(lr, union, max).to_vec()
    }

    /// The register `reg` is a view of, in the target's description.
    pub fn root(
        &self,
        reg: RegId,
    ) -> RegId {
        self.tri.regs.root(reg)
    }

    /// `checkRegMaskInterference` (LiveRegMatrix.cpp#L127): `virt` crosses an
    /// instruction that destroys `phys`. The answer for one range is worked out
    /// once and kept (`RegMaskUsable`), whatever register is asked.
    fn check_reg_mask_interference(
        &mut self,
        virt: &LiveInterval,
        phys: RegId,
    ) -> bool {
        self.reg_mask_of(virt).is_some_and(|roots| roots.contains(&self.tri.regs.root(phys)))
    }

    /// The registers `virt` loses to the instructions it is live across, kept
    /// for `virt` until the next assignment (`RegMaskVirtReg`, `RegMaskTag`).
    fn reg_mask_of(
        &mut self,
        virt: &LiveInterval,
    ) -> Option<llrm_lir::registers::RegSet> {
        let valid = self.reg_mask.as_ref().is_some_and(|(reg, tag, _)| *reg == virt.reg() && *tag == self.user_tag);
        if !valid {
            self.reg_mask = Some((virt.reg(), self.user_tag, self.fixed.reg_mask_clobbered(virt)));
        }
        self.reg_mask.as_ref().expect("just made").2
    }

    /// `checkRegMaskInterference(VirtReg)` with no register: the range is live
    /// across an instruction that destroys some register.
    pub fn crosses_reg_mask(
        &mut self,
        virt: &LiveInterval,
    ) -> bool {
        self.reg_mask_of(virt).is_some()
    }

    /// `checkInterference(VirtReg, PhysReg)` (LiveRegMatrix.cpp#L178).
    pub fn check_interference(
        &mut self,
        virt: &LiveInterval,
        phys: RegId,
    ) -> InterferenceKind {
        if virt.empty() {
            return InterferenceKind::Free;
        }
        // Regmask interference is the fastest check.
        if self.check_reg_mask_interference(virt, phys) {
            return InterferenceKind::RegMask;
        }
        // Check for fixed interference.
        if self.fixed.reg_unit_interferes(virt, phys) {
            return InterferenceKind::RegUnit;
        }
        // Check the matrix for virtual register interference.
        for unit in self.tri.regunits(phys) {
            if self.query_check(virt.reg(), &virt.range, unit) {
                return InterferenceKind::VirtReg;
            }
        }
        InterferenceKind::Free
    }

    /// `getOneVReg`: any virtual register assigned to a unit of the register.
    pub fn get_one_vreg(
        &self,
        phys: RegId,
    ) -> Option<VReg> {
        self.tri
            .regunits(phys)
            .find_map(|unit| self.matrix.get(unit as usize).and_then(LiveIntervalUnion::get_one_vreg))
    }

    pub fn num_assigned(&self) -> usize {
        self.num_assigned
    }

    pub fn num_unassigned(&self) -> usize {
        self.num_unassigned
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::registers::Regs;

    use super::*;
    use crate::liveness::liverange::Segment;
    use crate::liveness::slotindex::{SlotIndex, SlotIndexes};
    use crate::model::lir::{Block, Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn info() -> RegisterInfo {
        RegisterInfo::new(Regs(&llrm_x86_m16::REGISTER_INFO))
    }

    fn indexes() -> Vec<SlotIndex> {
        let insns: Vec<Arc<Insn>> = (0..100).map(|_| Arc::new(Insn::new(0, None, None, vec![], vec![]))).collect();
        let body = LirBody::new(
            "matrix",
            0,
            vec![Block::from(LirBlock::new(0, insns.clone()))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let indexes = SlotIndexes::new(&body);
        insns.iter().map(|one| indexes.get_instruction_index(one.id()).get_reg_slot(false)).collect()
    }

    fn interval(
        reg: u32,
        from: usize,
        to: usize,
        at: &[SlotIndex],
    ) -> LiveInterval {
        let mut li = LiveInterval::new(reg, 1.0);
        let v = li.get_next_value(at[from]);
        li.add_segment(Segment::new(at[from], at[to], Some(v)));
        li
    }

    /// A register's units are the lanes of its root it covers: the two low
    /// bytes and the upper half.
    #[test]
    fn test_registers_share_the_units_they_overlap_in() {
        let tri = info();
        let (ax, al, ah, eax, bl) = (RegId::AX, RegId::AL, RegId::AH, RegId::EAX, RegId::BL);
        assert_eq!(tri.regunits(ax).len(), 2);
        assert_eq!(tri.regunits(eax).len(), 3);
        let shared = |a: RegId, b: RegId| tri.regunits(a).any(|u| tri.regunits(b).any(|v| v == u));
        assert!(shared(ax, al) && shared(ax, ah) && shared(eax, al) && !shared(al, ah) && !shared(al, bl));
        for (a, b) in [(ax, al), (al, ah), (ax, bl), (eax, ax)] {
            assert_eq!(shared(a, b), tri.overlaps(a, b), "{a:?} {b:?} against the file's overlap");
        }
    }

    /// The register units of X86RegisterInfo.td, as `llvm-tblgen
    /// -gen-register-info` writes them at d1106deb71cc (`X86RegUnitRoots`,
    /// decoded through `X86RegDiffLists`): AL, AH and one unit for the high
    /// half (`HAX`) make EAX three, AX is two, a byte register or a segment
    /// register is one. The byte-lane model gave EAX four (bytes 2 and 3
    /// are never named apart), so an assignment took four union entries
    /// where LLVM's takes three.
    #[test]
    fn test_the_units_of_a_register_are_the_table_gen_units_of_x86() {
        let tri = info();
        let count = |reg: RegId| tri.regunits(reg).count();
        for wide in [RegId::EAX, RegId::EBX, RegId::ECX, RegId::EDX, RegId::ESI, RegId::EDI, RegId::EBP, RegId::ESP] {
            assert_eq!(count(wide), 3, "{wide:?}: low byte, high byte, upper half");
        }
        for word in [RegId::AX, RegId::BX, RegId::CX, RegId::DX, RegId::SI, RegId::DI, RegId::BP, RegId::SP] {
            assert_eq!(count(word), 2, "{word:?}");
        }
        for byte in [RegId::AL, RegId::AH, RegId::BL, RegId::BH, RegId::CL, RegId::CH, RegId::DL, RegId::DH] {
            assert_eq!(count(byte), 1, "{byte:?}");
        }
        for segment in [RegId::DS, RegId::ES, RegId::FS, RegId::GS, RegId::SS, RegId::CS] {
            assert_eq!(count(segment), 1, "{segment:?}");
        }
        // EAX is AX and one more unit, the upper half HAX.
        let ax: Vec<_> = tri.regunits(RegId::AX).collect();
        let extra: Vec<_> = tri.regunits(RegId::EAX).filter(|unit| !ax.contains(unit)).collect();
        assert_eq!(extra.len(), 1);
    }

    #[test]
    fn test_a_register_interferes_with_a_virtual_register_that_holds_a_unit_of_it() {
        let at = indexes();
        let mut matrix = LiveRegMatrix::new(info(), Box::new(NoFixed));
        let held = interval(1, 10, 20, &at);
        matrix.assign(&held, RegId::AX);
        let meets = interval(2, 15, 25, &at);
        let after = interval(3, 20, 30, &at);
        matrix.invalidate_virt_regs();
        assert_eq!(matrix.check_interference(&meets, RegId::AL), InterferenceKind::VirtReg, "al is a unit of ax");
        assert_eq!(matrix.check_interference(&meets, RegId::AH), InterferenceKind::VirtReg);
        assert_eq!(matrix.check_interference(&meets, RegId::BX), InterferenceKind::Free);
        assert_eq!(matrix.check_interference(&meets, RegId::EAX), InterferenceKind::VirtReg);
        assert_eq!(
            matrix.check_interference(&after, RegId::AX),
            InterferenceKind::Free,
            "segments are half open: adjacent do not meet"
        );
        assert_eq!(matrix.get_one_vreg(RegId::AL), Some(1));
        assert!(matrix.is_phys_reg_used(RegId::EAX) && !matrix.is_phys_reg_used(RegId::EBX));
        matrix.unassign(&held);
        matrix.invalidate_virt_regs();
        assert_eq!(matrix.check_interference(&meets, RegId::AL), InterferenceKind::Free);
        assert!(!matrix.is_phys_reg_used(RegId::AX));
    }

    /// An interference answer is kept until the union changes: a register
    /// assigned after the first check is seen by the next (the union's tag).
    #[test]
    fn test_a_cached_answer_does_not_outlive_a_change_to_the_union() {
        let at = indexes();
        let mut matrix = LiveRegMatrix::new(info(), Box::new(NoFixed));
        let probe = interval(9, 10, 20, &at);
        matrix.invalidate_virt_regs();
        assert_eq!(matrix.check_interference(&probe, RegId::CX), InterferenceKind::Free);
        matrix.assign(&interval(4, 12, 14, &at), RegId::CX);
        assert_eq!(matrix.check_interference(&probe, RegId::CX), InterferenceKind::VirtReg);
    }

    struct Calls;

    impl PhysInterference for Calls {
        fn reg_mask_clobbered(
            &self,
            _li: &LiveInterval,
        ) -> Option<llrm_lir::registers::RegSet> {
            Some([crate::model::ir::root(RegId::AX)].into_iter().collect())
        }

        fn reg_unit_interferes(
            &self,
            _li: &LiveInterval,
            phys: RegId,
        ) -> bool {
            phys == RegId::BX
        }
    }

    /// The kinds are tried in order of cost: a mask before a fixed range before
    /// a virtual register.
    #[test]
    fn test_regmask_then_fixed_then_virtual_interference() {
        let at = indexes();
        let mut matrix = LiveRegMatrix::new(info(), Box::new(Calls));
        matrix.assign(&interval(1, 10, 20, &at), RegId::AX);
        matrix.assign(&interval(5, 10, 20, &at), RegId::CX);
        matrix.invalidate_virt_regs();
        let probe = interval(2, 12, 18, &at);
        assert_eq!(matrix.check_interference(&probe, RegId::AX), InterferenceKind::RegMask);
        assert_eq!(matrix.check_interference(&probe, RegId::BX), InterferenceKind::RegUnit);
        assert_eq!(matrix.check_interference(&probe, RegId::CX), InterferenceKind::VirtReg);
        assert_eq!(matrix.check_interference(&probe, RegId::DX), InterferenceKind::Free);
    }

    /// A description that roots `BX` at `EAX`, which iced's table does not.
    fn rooted_elsewhere() -> Regs {
        use llrm_lir::registers::{Entry, Info};
        let base = &llrm_x86_m16::REGISTER_INFO;
        let mut table = base.table;
        let bx = table[RegId::BX.index()].expect("bx is listed");
        table[RegId::BX.index()] = Some(Entry { root: RegId::EAX, ..bx });
        Regs(Box::leak(Box::new(Info {
            table,
            frame: base.frame,
            stack: base.stack,
            loads: base.loads,
            scratch: base.scratch,
            bytes: base.bytes,
            data_segment: base.data_segment,
            stack_segment: base.stack_segment,
            code_segment: base.code_segment,
            far_segment: base.far_segment,
            widths: base.widths,
            effects: base.effects,
            cache: Default::default(),
            views: base.views,
        })))
    }

    /// The matrix asked iced's `ROOT_OF` whether a call destroyed a register,
    /// so a target whose ids are not iced's got an x86 family: a register the
    /// description roots at the destroyed one was reported free.
    #[test]
    fn test_a_call_mask_is_read_through_the_descriptions_roots() {
        let at = indexes();
        let mut matrix = LiveRegMatrix::new(RegisterInfo::new(rooted_elsewhere()), Box::new(Calls));
        let probe = interval(2, 12, 18, &at);
        assert_eq!(matrix.check_interference(&probe, RegId::BX), InterferenceKind::RegMask);
    }

    /// Greedy reads roots from the description; x86 results are unchanged only
    /// while each x86 file roots a register where iced's table does.
    #[test]
    fn test_the_x86_files_root_every_register_as_iced_does() {
        for (name, info) in [
            ("m16", &llrm_x86_m16::REGISTER_INFO),
            ("m32", &llrm_x86_m32::REGISTER_INFO),
            ("m64", &llrm_x86_m64::REGISTER_INFO),
        ] {
            for entry in info.table.iter().flatten() {
                assert_eq!(info.root(entry.id), entry.id.root(), "{name}: {} is rooted differently", entry.name);
            }
        }
    }
}
