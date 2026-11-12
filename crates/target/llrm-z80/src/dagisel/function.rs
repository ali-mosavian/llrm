//! A function from IR to scheduled machine nodes: the shared builder, then
//! [`super::pipeline`]'s stages, block by block.
//!
//! DEVIATE: the phi copies of a block that branches to several blocks are per
//! edge (an edge block the emitter makes), not placed by a machine-function phi
//! elimination; a `switch` to a block with phis is refused. Reason: no shared
//! machine function. Exit: the shared machine function.
//!
//! DEVIATE: no static allocas or jump tables: the
//! builder makes the frame objects and the C convention passes what the
//! registers do not hold on the stack, so the slice (arithmetic, branches,
//! calls) needs none. Reason: frame objects wait for the Z80 machine function.
//! Exit: M4.

use llrm_codegen::backend::isel;
use llrm_codegen::backend::registerinfo::RegId;
use llrm_codegen::backend::selectiondag::DagConfig;
use llrm_codegen::backend::selectiondag::SelectionDAG;
use llrm_codegen::backend::selectiondag::builder::{BlockDag, BuilderTarget, TargetNodes, TtiHandle, build_block};
use llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo;
use llrm_codegen::backend::selectiondag::live_out;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::schedule::Unit;
use llrm_mir::context::ConstantKind;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::module::{BlockId, Function, Module, Operand, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_spike_mf::{Variable, VariablePlace};

use super::call::{CALL, RET_GLUE, Z80Calling};
use super::hooks::Cpu;
use super::pipeline::select_block;
use crate::target::Z80;

/// The C library routines the runtime has, and the symbols it names them by.
pub const LIBCALLS: [(&str, &str); 3] = [("memcpy", "__memcpy"), ("memmove", "__memmove"), ("memset", "__memseta")];

/// The builder's view of the Z80: 16-bit pointers, `i8` and `i16` in
/// registers, a byte shift count, and the `CALL`/`RET_GLUE` nodes of
/// `own_ids.txt`.
pub fn builder_target() -> BuilderTarget {
    BuilderTarget {
        shift_amount: EVT::I8,
        legal_ints: vec![8, 16],
        soft_float: true,
        jump_tables: true,
        // An entry is 2 B, a 16-bit compare-and-branch about 6: a table pays
        // from a third of its range filled, at speed as well as size.
        min_jump_table_density: Some(40),
        preselection_blocks: Some(crate::prepare::PHI_EDGE),
        refuse_unknown_intrinsics: true,
        variable_shift_legal: false,
        string_instructions: false,
        max_alloca_align: Some(crate::MAX_ALLOCA_ALIGN),
        // the runtime's own routines (runtime/z80_rt.s). `memcpy` and `memmove`
        // take the C library's registers. The compiler's `memset` has the byte
        // as `i8` (in A) where C's takes an `int`: its own entry, `__memseta`.
        libcalls: Some(LIBCALLS.map(|(name, symbol)| (name.to_owned(), 0, symbol.to_owned())).to_vec()),
        target_nodes: Some(TargetNodes {
            call: CALL,
            ret: RET_GLUE,
            stack_pointer: "sp",
            big_endian: false,
            calling: Some(&Z80Calling),
        }),
        // a branch on `a || b` is two branches (the
        // default `getJumpConditionMergingParams`), not two 0/1 bytes and a
        // logic op
        tti: Some(TtiHandle(std::rc::Rc::new(llrm_mir::tti::Basic))),
        // all addressing is absolute: `&buf[6]` is the symbol and 6, whatever
        // the symbol's linkage (`isOffsetFoldingLegal`)
        dag: DagConfig {
            dso_local_symbols: true,
            sext_bool_add_max_bits: u32::MAX,
            known_bits_of_build_pair: true,
            ..BuilderTarget::for_pointer_bits(16).dag
        },
        // the target's limits (`driver::MEMCPY_STORES`), the same table the mid-end reads
        tuning: llrm_mir::target::Machine::tuning(&*crate::driver::machine()),
        ..BuilderTarget::for_pointer_bits(16)
    }
}

thread_local! {
    /// The pin's block frequencies as they are (`Request::flat_loop_trips`), for
    /// the selections made on this thread.
    pub static FLAT_LOOP_TRIPS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// One block, selected and scheduled.
pub struct Selected {
    pub block: BlockId,
    pub dag: SelectionDAG,
    pub units: Vec<Unit>,
    /// The registers the block reads on arrival, with the virtual register each
    /// is copied to.
    pub live_ins: Vec<(RegId, u32)>,
    /// The phi inputs this block exports, by successor: the register the value
    /// was copied to, and the phi's own. One entry with no successor when
    /// they all go together (a single successor, or a `switch`).
    pub edges: Vec<Edge>,
}

/// The phi copies on the edge to `to` (a parallel copy), or on every edge when
/// `to` is `None`.
pub struct Edge {
    pub to: Option<BlockId>,
    pub copies: Vec<(u32, u32)>,
}

/// A selected function: its blocks and the sizes of its local frame objects, by
/// frame index.
pub struct Selection {
    pub blocks: Vec<Selected>,
    pub locals: Vec<i64>,
    /// The subtarget it was selected for: emission expands to the same CPU.
    pub cpu: Cpu,
    /// The width in bits of the register of every value that has one: a phi
    /// input is read on an edge laid out before any block that names it (a
    /// loop's back edge).
    /// The bit width of each register a value lives in, by register number; 0
    /// where nothing is recorded.
    pub widths: Vec<u32>,
    /// `-g`: the variables the debug info names and where each is (the `.adb`).
    pub variables: Vec<Variable>,
    /// The function calls a routine that returns twice (`setjmp`).
    pub returns_twice: bool,
    /// `internal` or `private` linkage: the label is not exported.
    pub local: bool,
    /// The function carries `optsize` or `minsize`: tuned for size at any level.
    pub sized: bool,
    /// Selected the second time for the module price with the speed routines or
    /// the shift loops (`pipeline::SPEED_ROUTINES`, `INLINE_SHIFTS`): a
    /// function tuned for size chooses between its two by the
    /// smaller relative regret, not by bytes first.
    pub wide_alternative: bool,
    /// The same function selected with its long adds as the generic expansion,
    /// not carry ops (`pipeline::GENERIC_CARRY`); none where it has no
    /// carry ops.
    pub generic: Option<Box<Selection>>,
    /// The blocks IR `unreachable` ends. The pin emits nothing for it (the
    /// machine CFG comes from the IR); here a block with no terminator falls
    /// into the next one, so emission states the end.
    pub unreachable: llrm_mir::dense::IdSet<BlockId>,
    /// The pin's block frequency of each IR block over the entry's
    /// (`BlockFrequencyInfo` over `BranchProbabilityInfo`), by block number: what
    /// the prices that choose between variants of the function weigh by.
    pub frequencies: Vec<(u32, f64)>,
    /// The jump tables: the block of each entry, by table.
    pub tables: Vec<Vec<u32>>,
}

/// Function `name` of `module`, block by block in layout order.
///
/// DEVIATE: static allocas share no frame object (no stack coloring) and none
/// is aligned: the Z80 has no alignment. Reason: nothing here needs the
/// saving. Exit: with the shared frame code.
pub fn select_function(
    module: &Module,
    name: &str,
    cpu: Cpu,
) -> Result<Selection, String> {
    select_function_with(module, name, cpu, true)
}

/// [`select_function`], with `optimize` false at -O0: no register knows
/// anything of what a block put in it.
pub fn select_function_with(
    module: &Module,
    name: &str,
    cpu: Cpu,
    optimize: bool,
) -> Result<Selection, String> {
    let id = module.named(name).ok_or_else(|| format!("no function @{name}"))?;
    let function = module
        .global(id)
        .function()
        .filter(|one| !one.is_declaration())
        .ok_or_else(|| format!("@{name} has no body"))?;
    let text = module.datalayout.as_deref().ok_or("a module with no datalayout")?;
    let layout = DataLayout::parse(text)?;
    let arch = Z80;
    let convention = isel::convention(module, &layout, id, &arch).map_err(|why| why.0)?;
    let target = builder_target();
    let sized = ["optsize", "minsize"].into_iter().any(|name| llrm_mir::memory::has(&function.attrs, name));
    let tuned_for_size = sized;
    let chain_credit = llrm_mir::memory::has(&function.attrs, crate::prepare::WORD_CHAINS);
    let long_multiply = llrm_mir::memory::has(&function.attrs, crate::prepare::LONG_MULTIPLY)
        && !super::pipeline::DECOMPOSE_MULS.with(std::cell::Cell::get);
    let frequencies = crate::frequency::block_frequencies(module, function, &layout, FLAT_LOOP_TRIPS.with(std::cell::Cell::get));
    let mut info = FunctionLoweringInfo::new(function);
    let mut locals = alloca_sizes(module, function, &layout, &info)?;
    let mut selected = Vec::new();
    let mut visited = llrm_support::hash::HashSet::default();
    for &block in function.layout() {
        if optimize {
            // `SelectAllBasicBlocks`: a phi knows what its incoming values do
            // once all its predecessors are selected.
            let ready = function.predecessors(block).iter().all(|pred| visited.contains(pred));
            let phis: Vec<_> = function
                .iter_block(block)
                .take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi)
                .collect();
            for phi in phis {
                if ready {
                    // a value of more than a word is several registers
                    let bits = |bits: u32| (bits != 0 && bits <= 16).then(|| if bits <= 8 { 8 } else { 16 });
                    live_out::compute_phi_live_out_reg_info(module, function, phi, &bits, &mut info);
                } else if let Some(reg) = function.instruction(phi).result.and_then(|r| info.value_regs.get(&r)) {
                    info.live_out.remove(reg);
                }
            }
            visited.insert(block);
        }
        let built = build_block(module, function, &layout, &target, &arch, &convention, &mut info, block)
            .map_err(|why| why.0.to_owned())?;
        select_built(
            Some((function, block)),
            block,
            built,
            cpu,
            (optimize, sized, long_multiply, chain_credit),
            frequencies.iter().any(|&(one, frequency)| one == block.0 && frequency <= 1.0),
            &mut info,
            &mut selected,
        )?;
    }
    // The stack objects the builder made follow the static allocas.
    locals.extend(info.objects.iter().map(|(size, _)| i64::from(*size)));
    let mut sized: Vec<(u32, u32)> = info
        .value_regs
        .iter()
        .map(|(value, reg)| (*reg, 8 * layout.alloc_size(&module.context.types, function.value(value).ty) as u32))
        .collect();
    // a value wider than a word is several registers, each a word: a phi copy
    // names them all
    let wide: Vec<(u32, usize)> = info
        .value_regs
        .iter()
        .map(|(value, reg)| (*reg, layout.alloc_size(&module.context.types, function.value(value).ty) as usize / 2))
        .filter(|(_, parts)| *parts > 1)
        .collect();
    for (reg, parts) in wide {
        sized.extend(info.part_regs(reg, parts).into_iter().skip(1).map(|part| (part, 16)));
    }
    let mut widths = vec![0; sized.iter().map(|(reg, _)| *reg as usize + 1).max().unwrap_or(0)];
    for (reg, width) in sized {
        widths[reg as usize] = width;
    }
    let variables =
        variable_sites(module, function, name, &info, selected.first().map_or(&[][..], |b| &b.live_ins[..]));
    let returns_twice = llrm_mir::memory::calls_returns_twice(module, function);
    let mut unreachable = llrm_mir::dense::IdSet::new();
    for &block in function.layout() {
        if function.terminator(block).is_some_and(|last| function.instruction(last).opcode == Opcode::Unreachable) {
            unreachable.insert(block);
        }
    }
    let local =
        matches!(
            module.global(id).linkage,
            llrm_mir::module::Linkage::Internal | llrm_mir::module::Linkage::Private
        );
    let tables = info.jump_tables.clone();
    Ok(Selection {
        blocks: selected,
        locals,
        cpu,
        widths,
        variables,
        returns_twice,
        local,
        sized: tuned_for_size,
        wide_alternative: super::pipeline::SPEED_ROUTINES.with(std::cell::Cell::get)
            || super::pipeline::INLINE_SHIFTS.with(std::cell::Cell::get),
        generic: None,
        unreachable,
        frequencies,
        tables,
    })
}

/// The variables of `function` (after the debug import): parameters in the
/// register their position arrives in, declared locals in their frame object.
/// A parameter that does not arrive in a register, and a variable the debug
/// info cannot spell, are left out.
fn variable_sites(
    module: &Module,
    function: &Function,
    name: &str,
    info: &FunctionLoweringInfo,
    live_ins: &[(RegId, u32)],
) -> Vec<Variable> {
    let mut sites = Vec::new();
    let Some(described) = llrm_mir::debuginfo::functions(module).into_iter().find(|one| one.function == name) else {
        return sites;
    };
    for (index, parameter, ty) in &described.parameters {
        let Some(&(register, _)) = usize::try_from(*index).ok().and_then(|at| live_ins.get(at)) else { continue };
        sites.push(Variable {
            name: parameter.clone(),
            r#type: *ty,
            expr: Vec::new(),
            parameter: true,
            place: VariablePlace::Register(register),
        });
    }
    for record in function.debug_records() {
        let (llrm_mir::DebugWhat::Declare(Operand::Value(address)), Some(variable)) =
            (record.what, llrm_mir::debuginfo::read_variable(module, record.variable))
        else {
            continue;
        };
        let Some(&index) = info.static_allocas.get(&address) else { continue };
        if variable.scope != name || variable.parameter {
            continue;
        }
        sites.push(Variable {
            name: variable.name,
            r#type: variable.r#type,
            expr: record.expression.and_then(|node| module.expression(node)).unwrap_or_default(),
            parameter: false,
            place: VariablePlace::Frame { index: index as i32, offset: variable.offset },
        });
    }
    sites
}

/// The size of each static alloca, by the frame index `FunctionLoweringInfo`
/// gave it.
fn alloca_sizes(
    module: &Module,
    function: &Function,
    layout: &DataLayout,
    info: &FunctionLoweringInfo,
) -> Result<Vec<i64>, String> {
    let mut sizes = vec![0; info.static_allocas.iter().count()];
    for (value, index) in info.static_allocas.iter() {
        let ValueDef::Instruction(def) = function.value(value).def else {
            return Err("an alloca that is not an instruction".into());
        };
        let instruction = function.instruction(def);
        let Opcode::Alloca { allocated, .. } = instruction.opcode else {
            return Err("an alloca that is not an alloca".into());
        };
        let count = match instruction.operands.first() {
            None => 1,
            Some(Operand::Constant(id)) => match module.context.get(*id).kind {
                ConstantKind::Int(count) => count as i64,
                _ => return Err("an alloca whose count is not a number".into()),
            },
            Some(_) => return Err("an alloca with a count that is not a constant".into()),
        };
        sizes[*index as usize] = layout.alloc_size(&module.context.types, allocated) as i64 * count;
    }
    Ok(sizes)
}

/// One built block selected and scheduled, then the blocks its `switch`
/// lowering made. Every branch is to a label, so their position in the text
/// does not matter.
fn select_built(
    top: Option<(&Function, BlockId)>,
    block: BlockId,
    mut built: BlockDag,
    cpu: Cpu,
    (optimize, sized, long_multiply, chain_credit): (bool, bool, bool, bool),
    cold: bool,
    info: &mut FunctionLoweringInfo,
    selected: &mut Vec<Selected>,
) -> Result<(), String> {
    // the scratch slots the legalizers ask for (a long long result's memory)
    // are frame objects numbered after the function's
    built.dag.first_temporary = info.next_stack_object();
    let units =
        select_block(&mut built.dag, cpu, info, optimize, (sized, long_multiply, cold, chain_credit)).map_err(|why| format!("{why:?}"))?;
    for (size, align) in std::mem::take(&mut built.dag.temporaries) {
        info.new_stack_object(size, align);
    }
    // DEVIATE: refused. A `switch` made of several blocks puts one copy list on
    // all its edges, and a phi read on another edge is lost. Reason: the
    // edges leave blocks the builder made. Exit: split the critical edges
    // of the IR first (`llrm-transforms` `split_critical_edge`).
    if top.is_some() && !built.extra.is_empty() && !built.phi_updates.is_empty() {
        return Err("a switch to a block with phis: split its critical edges first".into());
    }
    // The block each phi is in, to split the copies by successor.
    let mut home: IdMap<ValueId, BlockId> = IdMap::new();
    let mut successors = Vec::new();
    if let Some((function, block)) = top.filter(|_| built.extra.is_empty()) {
        successors = function.successors(block).iter().copied().collect::<Vec<_>>();
        for &successor in &successors {
            for phi in
                function.iter_block(successor).take_while(|inst| function.instruction(*inst).opcode == Opcode::Phi)
            {
                home.extend(function.instruction(phi).result.map(|result| (result, successor)));
            }
        }
    }
    let mut edges: Vec<Edge> = Vec::new();
    for (at, (phi, input)) in built.phi_updates.iter().enumerate() {
        let own = *info.value_regs.get(phi).ok_or("a phi with no register")?;
        // A value of several registers has one entry per part, in order.
        let part = built.phi_updates[..at].iter().filter(|(one, _)| one == phi).count();
        let copy = (*input, if part == 0 { own } else { info.part_regs(own, part + 1)[part] });
        let to = if successors.len() > 1 { home.get(phi).copied() } else { None };
        match edges.iter_mut().find(|edge| edge.to == to) {
            Some(edge) => edge.copies.push(copy),
            None => edges.push(Edge { to, copies: vec![copy] }),
        }
    }
    selected.push(Selected { block, dag: built.dag, units, live_ins: built.live_ins, edges });
    for (extra, made) in built.extra {
        select_built(None, extra, made, cpu, (optimize, sized, long_multiply, chain_credit), cold, info, selected)?;
    }
    Ok(())
}
