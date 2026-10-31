//! `FunctionLoweringInfo`: which values live in virtual registers across
//! blocks, and which allocas are frame indices.

use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;

/// Virtual registers number from here: below it the numbers are the pin's
/// physical registers, which the matcher names as `Register(n)` too (ES is 31,
/// `$noreg` 0). A virtual register numbered 31 was taken for ES and its copy
/// dropped (huge_runs' `copy` read the selector of a far pointer never set).
pub const FIRST_VIRTUAL: u32 = 4096;

/// What `FunctionLoweringInfo::set` decides before any block is selected.
/// `FunctionLoweringInfo::LiveOutInfo`: the sign bits and the known bits of a
/// value.
#[derive(Clone, Copy, Debug)]
pub struct LiveOut {
    pub sign_bits: u32,
    pub zero: u128,
    pub one: u128,
}

pub struct FunctionLoweringInfo {
    /// `ValueMap`: a value used outside its block (or by a phi) and the
    /// virtual register that carries it.
    pub value_regs: IdMap<ValueId, u32>,
    /// `StaticAllocaMap`.
    pub static_allocas: IdMap<ValueId, i32>,
    /// The frame object of the SysV x86-64 register save area a variadic
    /// function has (`RegSaveFrameIndex`), made with the allocas' objects.
    pub variadic_save: Option<i32>,
    /// The entry block's writes of zero the runtime's zero-filled frame
    /// already holds (`prezeroed`): not lowered.
    pub prezeroed: llrm_mir::dense::IdSet<InstId>,
    /// The register the `sret` parameter is copied to, read back to return it.
    pub sret_reg: Option<u32>,
    /// The register a variadic function keeps the pointer to its variadic
    /// arguments in (`WebAssemblyFunctionInfo::VarargBufferVreg`).
    pub variadic_buffer: Option<u32>,
    /// The branch probabilities of the function, made at its first `switch`.
    pub bpi: Option<llrm_analysis::bpi::BranchProbabilityInfo>,
    /// The probability of each machine edge a block's lowering made
    /// (`addSuccessorWithProb`): (from, to, probability in 2^31sts). An edge
    /// the lowering did not state has none.
    pub edge_probs: Vec<(BlockId, BlockId, u32)>,
    /// `LiveOutRegInfo`: what the block that defines a virtual register knew of
    /// the bits of its value.
    pub live_out: std::collections::BTreeMap<u32, LiveOut>,
    next_reg: u32,
    /// The register of the high half of a two-register value, by the low's.
    second: std::collections::BTreeMap<u32, u32>,
    /// The blocks each jump table's entries go to, by the table's index.
    pub jump_tables: Vec<Vec<u32>>,
    /// The first block id a switch lowering has not made one of: ids are
    /// the function's, so two switches never name the same block.
    pub next_block: u32,
    /// The (size, alignment) of each stack object the builder made, in
    /// frame index order after the static allocas.
    pub objects: Vec<(u32, u32)>,
    next_frame_index: i32,
    /// The names and locality of the module's globals and the function's
    /// blocks, made at the first block and shared by the DAGs of the rest
    /// (a table per block was a quadratic share of the compile).
    pub(super) tables: Option<Tables>,
}

/// What every block's DAG reads about the module and the function.
#[derive(Clone)]
pub(super) struct Tables {
    pub global_names: std::rc::Rc<Vec<String>>,
    pub local_symbols: std::rc::Rc<Vec<bool>>,
    pub block_names: std::rc::Rc<Vec<String>>,
}

impl FunctionLoweringInfo {
    /// `AddLiveOutRegInfo`: only information that tells something is kept.
    pub fn add_live_out(
        &mut self,
        reg: u32,
        info: LiveOut,
    ) {
        if info.sign_bits == 1 && info.zero == 0 && info.one == 0 {
            return;
        }
        self.live_out.insert(reg, info);
    }

    /// `FunctionLoweringInfo::set`: one walk in layout order, numbering
    /// registers and frame indices as the pin does.
    pub fn new(function: &Function) -> FunctionLoweringInfo {
        let mut outside: IdSet<ValueId> = IdSet::new();
        for &block in function.layout() {
            for inst in function.iter_block(block) {
                let instruction = function.instruction(inst);
                let is_phi = instruction.opcode == Opcode::Phi;
                for operand in &instruction.operands {
                    let Operand::Value(value) = operand else { continue };
                    let ValueDef::Instruction(def) = function.value(*value).def else { continue };
                    if is_phi || function.parent(def) != Some(block) {
                        outside.insert(*value);
                    }
                }
            }
        }
        let mut info = FunctionLoweringInfo {
            value_regs: IdMap::new(),
            static_allocas: IdMap::new(),
            variadic_save: None,
            prezeroed: Default::default(),
            sret_reg: None,
            variadic_buffer: None,
            bpi: None,
            edge_probs: Vec::new(),
            next_reg: FIRST_VIRTUAL,
            live_out: Default::default(),
            second: Default::default(),
            jump_tables: Vec::new(),
            next_block: 0,
            objects: Vec::new(),
            next_frame_index: 0,
            tables: None,
        };
        let entry = function.entry();
        for &block in function.layout() {
            for inst in function.iter_block(block) {
                let instruction = function.instruction(inst);
                let Some(result) = instruction.result else { continue };
                if let Opcode::Alloca { .. } = instruction.opcode {
                    if Some(block) == entry && instruction.operands.iter().all(|op| matches!(op, Operand::Constant(_)))
                    {
                        info.static_allocas.insert(result, info.next_frame_index);
                        info.next_frame_index += 1;
                        continue;
                    }
                }
                let phi_used = instruction.opcode == Opcode::Phi && !function.users(result).is_empty();
                if phi_used || outside.contains(&result) {
                    info.create_reg(result);
                }
            }
        }
        info
    }

    /// StackColoring's result for the static allocas: each is the frame index
    /// of its group (locals never live together share one), `count` indices
    /// in all.
    pub fn share_allocas(
        &mut self,
        groups: impl IntoIterator<Item = (ValueId, i32)>,
        count: i32,
    ) {
        for (value, index) in groups {
            self.static_allocas.insert(value, index);
        }
        self.next_frame_index = count;
    }

    /// `CreateRegs`: the next virtual register, recorded for `value`.
    pub fn create_reg(
        &mut self,
        value: ValueId,
    ) -> u32 {
        let reg = self.new_reg();
        self.value_regs.insert(value, reg);
        reg
    }

    /// A new stack object (`CreateStackTemporary`): its frame index.
    pub fn new_stack_object(
        &mut self,
        size: u32,
        align: u32,
    ) -> i32 {
        self.objects.push((size, align));
        self.next_frame_index += 1;
        self.next_frame_index - 1
    }

    /// The next jump table index of the function, whose entries go to `blocks`.
    pub fn new_jump_table(
        &mut self,
        blocks: Vec<u32>,
    ) -> u32 {
        self.jump_tables.push(blocks);
        (self.jump_tables.len() - 1) as u32
    }

    /// The registers a value of `parts` registers is in, the first being
    /// `first`.
    pub fn part_regs(
        &mut self,
        first: u32,
        parts: usize,
    ) -> Vec<u32> {
        let mut regs = vec![first];
        for _ in 1..parts {
            let last = *regs.last().expect("a register");
            let next = match self.second.get(&last) {
                Some(next) => *next,
                None => {
                    let next = self.new_reg();
                    self.second.insert(last, next);
                    next
                }
            };
            regs.push(next);
        }
        regs
    }

    /// A register with no value of its own (a constant a phi receives).
    pub fn new_reg(&mut self) -> u32 {
        let reg = self.next_reg;
        self.next_reg += 1;
        reg
    }
}
