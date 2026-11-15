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
    /// `MFI.hasVarSizedObjects()`: a non-static alloca was lowered.
    pub has_var_sized_objects: bool,
    /// The entry block's writes of zero the runtime's zero-filled frame
    /// already holds (`prezeroed`): not lowered.
    pub prezeroed: llrm_mir::dense::IdSet<InstId>,
    /// `ElidedArgCopyInstrs`: the stores that copy an argument into a local the
    /// argument's own stack slot now stands for; not lowered.
    pub elided_stores: llrm_mir::dense::IdSet<InstId>,
    /// The arguments whose copy is elided: their slot is no longer immutable.
    pub elided_args: llrm_mir::dense::IdSet<ValueId>,
    /// The register the `sret` parameter is copied to, read back to return it.
    pub sret_reg: Option<u32>,
    /// The register holding the hidden result pointer of a function whose
    /// result does not fit in registers (`DemoteRegister`).
    pub demote_reg: Option<u32>,
    /// The register a variadic function keeps the pointer to its variadic
    /// arguments in (`WebAssemblyFunctionInfo::VarargBufferVreg`).
    pub variadic_buffer: Option<u32>,
    /// The fixed object of the first variadic argument on the stack
    /// (`VarArgsFrameIndex`), for `llrm.vararg.address` in any block.
    pub vararg_frame_index: Option<i32>,
    /// The words a target's `va_start` writes (`VarArgsFrameIndex` and the save
    /// areas' tops).
    pub variadic_area: Option<super::target_calling::VariadicArea>,
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
    pub tables: Option<Tables>,
    /// The module's part of the tables, when an earlier function made it.
    pub module_tables: Option<crate::backend::constpool::Symbols>,
}

/// How many times [`module_tables`] ran: a test hook.
pub static MODULE_TABLES_BUILT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The module's part of the tables: the names of its globals, and which are
/// local. `isOffsetFoldingLegal`: `shouldAssumeDSOLocal` holds for every symbol
/// under the static relocation model, which DOS objects (no PIC) always use.
pub fn module_tables(
    module: &llrm_mir::module::Module,
    dso_local_symbols: bool,
    defined_dso_local: bool,
) -> crate::backend::constpool::Symbols {
    MODULE_TABLES_BUILT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    (
        std::rc::Rc::new(
            module.globals.iter().map(|global| global.name.as_deref().unwrap_or("").to_string()).collect(),
        ),
        std::rc::Rc::new(if dso_local_symbols {
            vec![true; module.globals.len()]
        } else {
            module
                .globals
                .iter()
                .enumerate()
                .map(|(at, global)| {
                    // `GlobalValue::isImplicitDSOLocal`: local linkage, or a
                    // visibility other than default on a symbol that is not
                    // `extern_weak`. clang -fPIE marks what it defines
                    // `dso_local`, not what it declares.
                    matches!(
                        global.linkage,
                        llrm_mir::module::Linkage::Internal | llrm_mir::module::Linkage::Private
                    )
                        || (global.visibility != llrm_mir::module::Visibility::Default
                            && global.linkage != llrm_mir::module::Linkage::ExternWeak)
                        || (defined_dso_local
                            && crate::backend::globals::space(module, llrm_mir::GlobalId(at as u32))
                                == crate::model::ir::Space::Segment)
                })
                .collect()
        }),
    )
}

/// What every block's DAG reads about the module and the function.
#[derive(Clone)]
pub struct Tables {
    pub global_names: std::rc::Rc<Vec<String>>,
    pub local_symbols: std::rc::Rc<Vec<bool>>,
    pub block_names: std::rc::Rc<Vec<String>>,
}

impl FunctionLoweringInfo {
    /// The module's tables, made once by the caller ([`module_tables`]) for
    /// every function of the module.
    pub fn with_module_tables(
        &mut self,
        tables: crate::backend::constpool::Symbols,
    ) {
        self.module_tables = Some(tables);
    }

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
            has_var_sized_objects: false,
            prezeroed: Default::default(),
            elided_stores: Default::default(),
            elided_args: Default::default(),
            sret_reg: None,
            demote_reg: None,
            variadic_buffer: None,
            vararg_frame_index: None,
            variadic_area: None,
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
            module_tables: None,
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

    /// The frame index the next stack object gets.
    pub fn next_stack_object(&self) -> i32 {
        self.next_frame_index
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

/// An argument the entry block stores whole into a static alloca
/// (`ArgCopyElisionCandidates`).
#[derive(Clone, Copy, Debug)]
pub struct ArgumentCopy {
    /// The argument's position.
    pub position: usize,
    pub arg: ValueId,
    pub alloca: ValueId,
    pub store: InstId,
}

/// `findArgumentCopyElisionCandidates`
/// ([SelectionDAGBuilder.cpp:11860](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L11860)):
/// the arguments the entry block stores to a static alloca that nothing
/// touched before and that the store fills; any other use of an alloca in the
/// entry block escapes it.
pub fn argument_copy_candidates(
    module: &llrm_mir::module::Module,
    function: &Function,
    layout: &llrm_mir::datalayout::DataLayout,
    is_static_alloca: impl Fn(ValueId) -> bool,
) -> Vec<ArgumentCopy> {
    use llrm_mir::types::Type;
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Unknown,
        Clobbered,
        Elidable,
    }
    let Some(entry) = function.entry() else { return Vec::new() };
    let types = &module.context.types;
    let parameters = function.parameters();
    let mut states: IdMap<ValueId, State> = IdMap::new();
    let mut found: Vec<ArgumentCopy> = Vec::new();
    let static_alloca = |operand: &Operand| match operand {
        Operand::Value(value) if is_static_alloca(*value) => Some(*value),
        _ => None,
    };
    for inst in function.iter_block(entry) {
        let instruction = function.instruction(inst);
        let Opcode::Store { .. } = instruction.opcode else {
            if matches!(instruction.opcode, Opcode::Cast(_)) {
                continue;
            }
            for operand in &instruction.operands {
                if let Some(alloca) = static_alloca(operand) {
                    states.insert(alloca, State::Clobbered);
                }
            }
            continue;
        };
        let (stored, destination) = (instruction.operands[0], instruction.operands[1]);
        if let Some(alloca) = static_alloca(&stored) {
            states.insert(alloca, State::Clobbered);
        }
        let Some(alloca) = static_alloca(&destination) else { continue };
        if *states.get_or_insert_with(alloca, || State::Unknown) != State::Unknown {
            continue;
        }
        let ValueDef::Instruction(def) = function.value(alloca).def else { continue };
        let Opcode::Alloca { allocated, .. } = function.instruction(def).opcode else { continue };
        let count = match function.instruction(def).operands.first() {
            None => Some(1),
            Some(Operand::Constant(id)) => match module.context.get(*id).kind {
                llrm_mir::context::ConstantKind::Int(count) => u64::try_from(count).ok(),
                _ => None,
            },
            Some(_) => None,
        };
        let argument = match stored {
            Operand::Value(value) => match function.value(value).def {
                ValueDef::Argument(at) => Some((value, at as usize)),
                _ => None,
            },
            _ => None,
        };
        // an argument that is a scalar the store fills whole, once
        let elidable = argument.filter(|&(value, at)| {
            let ty = function.value(value).ty;
            let scalar = match types.get(ty) {
                Type::Int(bits) => bits % 8 == 0,
                Type::Float(_) | Type::Pointer(_) => true,
                _ => false,
            };
            let by_copy = function
                .parameter_attrs
                .get(at)
                .is_some_and(
                    |attrs| attrs
                        .iter()
                        .any(
                            |attr| matches!(
                                attr,
                                llrm_mir::opcode::Attribute::Type(name, _) if ["byval", "inalloca", "preallocated"].contains(&name.as_str())
                            ),
                        ),
                );
            scalar
                && !by_copy
                && count
                    .is_some_and(|count| layout.store_size(types, ty) == layout.alloc_size(types, allocated) * count)
                && layout.size_bits(types, ty) == layout.store_size(types, ty) * 8
                && !found.iter().any(|one| one.arg == value)
        });
        match (elidable, argument) {
            (Some(_), Some((arg, position))) => {
                states.insert(alloca, State::Elidable);
                found.push(ArgumentCopy { position, arg, alloca, store: inst });
                // all of them seen: the scan stops (early at -O0, where the
                // entry block is large)
                if found.len() == parameters.len() {
                    break;
                }
            }
            _ => {
                states.insert(alloca, State::Clobbered);
            }
        }
    }
    found
}
