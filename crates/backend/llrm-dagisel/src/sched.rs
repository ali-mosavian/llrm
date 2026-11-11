//! The pin's scheduling model of a CPU: `MCSchedModel` and the classes of its
//! opcodes.
//!
//! # How it works
//! `tools/dagisel/schedtable.py` reads `llvm-tblgen -gen-subtarget` and writes
//! `generated/<target>/sched.rs`: one [`SchedModel`] per CPU, with the
//! resources, the class rows the DAG tables' opcodes reach and the pin's own
//! numbers. An opcode's class number is the same in every model; the model
//! gives the class its meaning. [`SchedModel::resolve`] follows a variant class
//! with the caller's answers to the pin's predicates.
//!
//! # Example
//! ```text
//! generic (Cortex-A510): in order, MicroOpBufferSize 0, issue 3
//! ADDWri  -> class 1443 -> 1 micro-op, 1 cycle on CortexA510UnitALU
//! ADDWrs  -> variant 1435 -> HasShiftedReg ? WriteISReg : WriteI
//! ```
//!
//! # Used by
//! The arm64 machine scheduler.

/// `MCProcResourceDesc`.
#[derive(Clone, Copy, Debug)]
pub struct ProcResource {
    pub name: &'static str,
    pub num_units: u8,
    /// The group this unit belongs to; 0 for none.
    pub super_idx: u8,
    /// -1: unbuffered by the model's own buffer (`MicroOpBufferSize`), 0: in
    /// order, n: reservation station.
    pub buffer_size: i32,
    /// The resources a group stands for, one entry per unit.
    pub sub_units: &'static [u8],
}

/// `MCWriteProcResEntry`.
#[derive(Clone, Copy, Debug)]
pub struct WriteProcRes {
    pub resource: u8,
    pub release_at_cycle: u16,
    pub acquire_at_cycle: u16,
}

/// `MCWriteLatencyEntry`: `write_idx` is the def operand's write; 0 stands for
/// any.
#[derive(Clone, Copy, Debug)]
pub struct WriteLatency {
    pub write_idx: u16,
    pub cycles: i32,
}

/// `MCReadAdvanceEntry`.
#[derive(Clone, Copy, Debug)]
pub struct ReadAdvance {
    pub use_idx: u16,
    pub write_idx: u16,
    pub cycles: i32,
}

/// `MCSchedClassDesc` with its table slices resolved. The pin's `Name` is
/// debug-only there and left out here.
#[derive(Clone, Copy, Debug)]
pub struct ClassData {
    pub num_micro_ops: u16,
    pub begin_group: bool,
    pub end_group: bool,
    pub retire_ooo: bool,
    pub write_proc_res: &'static [WriteProcRes],
    pub write_latency: &'static [WriteLatency],
    pub read_advance: &'static [ReadAdvance],
}

/// `ClassRow::flags` bits.
const BEGIN_GROUP: u8 = 1;
const END_GROUP: u8 = 2;
const RETIRE_OOO: u8 = 4;

/// A distinct class as generated: spans `(at, len)` into the model's pools, which
/// models share.
#[derive(Clone, Copy, Debug)]
pub struct ClassRow {
    pub num_micro_ops: u16,
    pub flags: u8,
    pub proc_at: u16,
    pub proc_len: u8,
    pub lat_at: u16,
    pub lat_len: u8,
    pub adv_at: u16,
    pub adv_len: u8,
}

/// The TIIPredicates a variant class tests
/// ([AArch64SchedPredicates.td](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64SchedPredicates.td)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchedPred {
    /// `AArch64InstrInfo::hasShiftedReg`: a shift amount other than zero on the
    /// shifted register operand.
    HasShiftedReg,
    /// `AArch64InstrInfo::isScaledAddr`: a register offset with the scale bit.
    IsScaledAddr,
}

/// One arm of `resolveVariantSchedClassImpl`: the first whose `pred` holds
/// (`None`: always) names the class.
#[derive(Clone, Copy, Debug)]
pub struct VariantArm {
    pub pred: Option<SchedPred>,
    pub class: u16,
}

#[derive(Clone, Copy, Debug)]
pub enum SchedClass {
    /// The pin's invalid class: the model says nothing about the opcode.
    Unmodeled,
    /// The model marks a write of the opcode `Unsupported`: the CPU does not
    /// run it (LSE atomics).
    Unsupported,
    /// Index in `SchedModel::class_rows`; classes with the same numbers share a row.
    Resolved(u16),
    /// `len` arms from `at` in `SchedModel::variant_arms`.
    Variant { at: u16, len: u8 },
}

/// What the model says about an opcode.
#[derive(Clone, Copy, Debug)]
pub enum Modeled {
    Unmodeled,
    Unsupported,
    Class(ClassData),
}

/// `MCSchedModel`.
#[derive(Debug)]
pub struct SchedModel {
    pub name: &'static str,
    pub issue_width: u32,
    /// 0: in order; -1: not stated.
    pub micro_op_buffer_size: i32,
    pub loop_micro_op_buffer_size: i32,
    pub load_latency: u32,
    pub high_latency: u32,
    pub mispredict_penalty: u32,
    pub post_ra_scheduler: bool,
    pub complete_model: bool,
    /// `(opcode, class)` by opcode.
    pub opcode_class: &'static [(u16, u16)],
    pub write_proc_res: &'static [WriteProcRes],
    pub write_latency: &'static [WriteLatency],
    pub read_advance: &'static [ReadAdvance],
    pub variant_arms: &'static [VariantArm],
    pub class_rows: &'static [ClassRow],
    /// Indexed by resource number; entry 0 is the invalid unit.
    pub resources: &'static [ProcResource],
    /// `(class, row)` by class, the classes the opcodes reach.
    pub classes: &'static [(u16, SchedClass)],
}

impl SchedModel {
    /// The class number of `opcode`; `None` when the model's table does not
    /// cover the opcode.
    pub fn class_of(
        &self,
        opcode: u16,
    ) -> Option<u16> {
        let at = self.opcode_class.binary_search_by_key(&opcode, |&(o, _)| o).ok()?;
        Some(self.opcode_class[at].1)
    }

    pub fn class(
        &self,
        class: u16,
    ) -> Option<&'static SchedClass> {
        let at = self.classes.binary_search_by_key(&class, |&(c, _)| c).ok()?;
        Some(&self.classes[at].1)
    }

    fn data(
        &self,
        row: &ClassRow,
    ) -> ClassData {
        let span = |at: u16, len: u8| at as usize..at as usize + len as usize;
        ClassData {
            num_micro_ops: row.num_micro_ops,
            begin_group: row.flags & BEGIN_GROUP != 0,
            end_group: row.flags & END_GROUP != 0,
            retire_ooo: row.flags & RETIRE_OOO != 0,
            write_proc_res: &self.write_proc_res[span(row.proc_at, row.proc_len)],
            write_latency: &self.write_latency[span(row.lat_at, row.lat_len)],
            read_advance: &self.read_advance[span(row.adv_at, row.adv_len)],
        }
    }

    /// `resolveSchedClass`: the data of `opcode`'s class, a variant followed by
    /// `holds`. `None` when the opcode is not in the table, so a caller
    /// counts what is missing.
    pub fn resolve(
        &self,
        opcode: u16,
        holds: &mut dyn FnMut(SchedPred) -> bool,
    ) -> Option<Modeled> {
        let modeled = |class: &SchedClass| match class {
            SchedClass::Unmodeled => Some(Modeled::Unmodeled),
            SchedClass::Unsupported => Some(Modeled::Unsupported),
            SchedClass::Resolved(row) => Some(Modeled::Class(self.data(&self.class_rows[*row as usize]))),
            SchedClass::Variant { .. } => None,
        };
        match self.class(self.class_of(opcode)?)? {
            SchedClass::Variant { at, len } => {
                let arms = &self.variant_arms[*at as usize..*at as usize + *len as usize];
                let arm = arms.iter().find(|arm| arm.pred.is_none_or(&mut *holds))?;
                modeled(self.class(arm.class)?)
            }
            class => modeled(class),
        }
    }
}
