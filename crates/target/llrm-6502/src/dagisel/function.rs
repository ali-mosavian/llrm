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
//! calls) needs none. Reason: frame objects wait for the 6502 machine function.
//! Exit: M4.

use llrm_codegen::backend::isel;
use llrm_codegen::backend::registerinfo::RegId;
use llrm_codegen::backend::selectiondag::builder::{BlockDag, BuilderTarget, TargetNodes, TtiHandle, build_block};
use llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::schedule::Unit;
use llrm_codegen::backend::selectiondag::{DagConfig, SelectionDAG};
use llrm_mir::context::ConstantKind;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::module::{BlockId, Function, Module, Operand, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_support::hash::HashMap;

use super::call::{CALL, MosCalling, RET_GLUE};
use super::hooks::Cpu;
use super::pipeline::select_block;
use crate::target::Mos;

/// The builder's view of the 6502: 16-bit pointers, `i8` and `i16` in
/// registers, a byte shift count, and the `CALL`/`RET_GLUE` nodes of
/// `own_ids.txt`.
pub fn builder_target() -> BuilderTarget {
    BuilderTarget {
        shift_amount: EVT::I8,
        legal_ints: vec![8, 16],
        soft_float: true,
        // the frame realigns the stack for an over-aligned object
        max_alloca_align: None,
        jump_tables: false,
        string_instructions: false,
        // the runtime's own routines (runtime/mos_rt.s), called by the name the
        // C library gives them
        libcalls: Some(["memcpy", "memmove", "memset"].map(|name| (name.to_owned(), 0, name.to_owned())).to_vec()),
        // each returns dst in A:X (runtime/mos_rt.s)
        mem_libcalls_return_dest: true,
        target_nodes: Some(TargetNodes {
            call: CALL,
            ret: RET_GLUE,
            stack_pointer: "rs0",
            big_endian: false,
            calling: Some(&MosCalling),
        }),
        // a branch on `a || b` is two branches (the
        // default `getJumpConditionMergingParams`), not two 0/1 bytes and a
        // logic op
        tti: Some(TtiHandle(std::rc::Rc::new(llrm_mir::tti::Basic))),
        // DEVIATE 6502-SEXT-BOOL-ADD (owner 6502, exit 2026-12-15): the pin
        // turns `add (sext i1 c), x` into
        // `sub x, (zext i1 c)` (`visitADDLikeCommutative`), which here is
        // an `sbc` chain; a word compared with its bound gained an `ldx #255`
        // and a swapped `cmp` from it
        // (`a_compare_against_a_word_boundary_...`). The add stays up to 16
        // bits; wider, the sign bytes of the add read a cell (`adc rc22`, 3
        // clk) where the pin's `sbc #0` is 2.
        dag: DagConfig { sext_bool_add_max_bits: 16, ..BuilderTarget::for_pointer_bits(16).dag },
        ..BuilderTarget::for_pointer_bits(16)
    }
    .with_narrow_divrem(EVT::I16, true)
    .with_tuning(crate::driver::bytes_first_tuning())
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

/// A local frame object: its size and the alignment the frame gives it.
#[derive(Clone, Copy, Debug)]
pub struct Local {
    pub size: i64,
    pub align: i64,
}

/// A selected function: its blocks and its local frame objects, by frame
/// index.
pub struct Selection {
    /// Which selection this is: unique to the thread, what a cached text of it
    /// is keyed by (a selection is moved, not copied).
    pub id: u64,
    pub blocks: Vec<Selected>,
    pub locals: Vec<Local>,
    /// The subtarget it was selected for: emission expands to the same CPU.
    pub cpu: Cpu,
    /// The width in bits of the register of every value that has one: a phi
    /// input is read on an edge laid out before any block that names it (a
    /// loop's back edge).
    pub widths: HashMap<u32, u32>,
    /// The zero-page bytes the platform gives the imaginary registers: 16 or
    /// more (`Platform::zero_page`).
    pub zero_page: u8,
    /// The level's price of a byte against a clock.
    pub tuning: crate::alloc::Tuning,
    /// The source file `-g` names the lines of, and the line the function
    /// starts at (its scope line); none prints no `.dbg line`.
    pub source: Option<(String, Option<u32>)>,
    /// The multiplies by a constant it expanded in line that a call may beat
    /// in bytes (`mulimage::call_when_it_pays`).
    pub call_candidates: usize,
}

impl Selection {
    /// The calls of `__mulsi3` it makes.
    pub fn mul_calls(&self) -> usize {
        use super::generated::ids::opcode as op;
        self.blocks
            .iter()
            .flat_map(|one| one.units.iter().flat_map(move |unit| unit.nodes.iter().map(move |&n| (one, n))))
            .filter(|(one, node)| {
                matches!(
                    one.dag.nodes()[*node as usize].opcode,
                    ISD::Machine(code) if code as u16 == op::MUL32
                )
            })
            .count()
    }
}

/// Function `name` of `module`, block by block in layout order.
///
/// DEVIATE: static allocas share no frame object (no stack coloring) and none
/// is aligned: the 6502 has no alignment. Reason: nothing here needs the
/// saving. Exit: with the shared frame code.
pub fn select_function(
    module: &Module,
    name: &str,
    cpu: Cpu,
) -> Result<Selection, String> {
    select_function_with(module, name, cpu, false, "")
}

/// Where a product of two words is a widening multiply (two routines that add
/// bytes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Widen {
    /// Nowhere.
    Off,
    /// In functions not tuned for size: a level that prices a clock above a
    /// byte.
    Unsized,
    /// Everywhere: the module-level trial of 6502-WIDEN-IMAGE.
    Always,
}

thread_local! {
    static WIDEN_PRODUCTS: std::cell::Cell<Widen> = const { std::cell::Cell::new(Widen::Off) };
    static COLD_MULTIPLY_CALLS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static LOOP_CALLED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    static CALL_CANDIDATES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static CALL_MULTIPLY_HERE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static TUNED_FOR_SIZE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static LONG_STEPS: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    static STEP_SITES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `run` with a long plus or minus one lowered as the carry chain (`false`) or
/// as the stepping `INC32`/`DEC32` (`true`, the default): the module-level
/// trial of 6502-LONG-STEP-TRIAL.
pub fn with_long_steps<R>(
    on: bool,
    run: impl FnOnce() -> R,
) -> R {
    let before = LONG_STEPS.with(|flag| flag.replace(on));
    let result = run();
    LONG_STEPS.with(|flag| flag.set(before));
    result
}

pub(super) fn long_steps() -> bool {
    LONG_STEPS.with(std::cell::Cell::get)
}

/// A long plus or minus one was lowered as a stepping node.
pub(super) fn note_step_site() {
    STEP_SITES.with(|count| count.set(count.get() + 1));
}

/// The stepping nodes lowered since the last call.
pub fn take_step_sites() -> usize {
    STEP_SITES.with(|count| count.replace(0))
}

/// `run` with the multiplies by a constant of the blocks outside every loop,
/// in a function that is not called from one (`loop_called`), priced as a
/// function tuned for size prices them: a call where that is shorter than the
/// shifts and adds (6502-MUL-CALL-TRIAL, `mulimage`).
pub fn with_cold_multiply_calls<R>(
    loop_called: Vec<String>,
    run: impl FnOnce() -> R,
) -> R {
    let before = COLD_MULTIPLY_CALLS.with(|flag| flag.replace(true));
    let names = LOOP_CALLED.with(|list| list.replace(loop_called));
    let result = run();
    LOOP_CALLED.with(|list| *list.borrow_mut() = names);
    COLD_MULTIPLY_CALLS.with(|flag| flag.set(before));
    result
}

/// A multiply by a constant that was expanded in line and that a call may
/// beat in bytes was met.
pub(super) fn note_call_candidate() {
    CALL_CANDIDATES.with(|count| count.set(count.get() + 1));
}

/// Whether the function being selected is tuned for size (at -O1 and -Os every
/// function is: no level that prices a clock above a byte is running), whatever
/// the module-level re-selection of `mulimage` makes of `sized` (it clears it,
/// to expand multiplies).
pub(super) fn tuned_for_size() -> bool {
    TUNED_FOR_SIZE.with(std::cell::Cell::get) || widen_products() == Widen::Off
}

/// Whether the block being selected prices its multiplies as a call.
pub(super) fn call_multiply_here() -> bool {
    CALL_MULTIPLY_HERE.with(std::cell::Cell::get)
}

/// Set by the driver for each compile.
pub fn set_widen_products(on: Widen) {
    WIDEN_PRODUCTS.with(|f| f.set(on));
}

pub(super) fn widen_products() -> Widen {
    WIDEN_PRODUCTS.with(|f| f.get())
}

/// Whether `function` is tuned for size (`optsize` or `minsize`).
pub fn is_sized(function: &Function) -> bool {
    ["optsize", "minsize"].into_iter().any(|name| llrm_mir::memory::has(&function.attrs, name))
}

/// [`select_function`]; `inline_multiplies` expands every `x * c` that can be
/// shifts and an add, as a function not tuned for size does;
/// `c_symbol_prefix` is the platform's, for the C routines a libcall names.
pub fn select_function_with(
    module: &Module,
    name: &str,
    cpu: Cpu,
    inline_multiplies: bool,
    c_symbol_prefix: &str,
) -> Result<Selection, String> {
    let id = module.named(name).ok_or_else(|| format!("no function @{name}"))?;
    let function = module
        .global(id)
        .function()
        .filter(|one| !one.is_declaration())
        .ok_or_else(|| format!("@{name} has no body"))?;
    let text = module.datalayout.as_deref().ok_or("a module with no datalayout")?;
    let layout = DataLayout::parse(text)?;
    let arch = Mos;
    let convention = isel::convention(module, &layout, id, &arch).map_err(|why| why.0)?;
    let target = builder_target();
    let mut info = FunctionLoweringInfo::new(function);
    let mut locals = alloca_sizes(module, function, &layout, &info)?;
    let mut selected = Vec::new();
    let sized = is_sized(function) && !inline_multiplies;
    TUNED_FOR_SIZE.with(|flag| flag.set(is_sized(function)));
    CALL_CANDIDATES.with(|count| count.set(0));
    // static loop depth, as `driver::cheaper` weighs a block: a place in a loop
    // runs ten times a level
    let depths = (COLD_MULTIPLY_CALLS.with(std::cell::Cell::get)
        && !LOOP_CALLED.with(|list| list.borrow().iter().any(|one| one == name)))
    .then(|| llrm_transforms::memloops::loop_depths(function));
    for &block in function.layout() {
        let built = build_block(module, function, &layout, &target, &arch, &convention, &mut info, block)
            .map_err(|why| why.0.to_owned())?;
        let cold = depths.as_ref().is_some_and(|depths| depths.get(block.0 as usize).copied().unwrap_or(0) == 0);
        CALL_MULTIPLY_HERE.with(|flag| flag.set(cold));
        let selected_block = select_built(
            Some((function, block)),
            block,
            built,
            (cpu, sized, inline_multiplies, c_symbol_prefix),
            &mut info,
            &mut selected,
        );
        CALL_MULTIPLY_HERE.with(|flag| flag.set(false));
        selected_block?;
    }
    // The stack objects the builder made follow the static allocas.
    locals.extend(info.objects.iter().map(|(size, _)| Local { size: i64::from(*size), align: 1 }));
    let mut widths: HashMap<u32, u32> = info
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
        widths.extend(info.part_regs(reg, parts).into_iter().skip(1).map(|part| (part, 16)));
    }
    Ok(Selection {
        id: next_selection_id(),
        blocks: selected,
        locals,
        cpu,
        widths,
        zero_page: 16,
        tuning: crate::alloc::Tuning::SPEED,
        source: None,
        call_candidates: CALL_CANDIDATES.with(std::cell::Cell::get),
    })
}

fn next_selection_id() -> u64 {
    thread_local! {
        static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    }
    NEXT.with(|next| {
        next.set(next.get() + 1);
        next.get()
    })
}

/// The size of each static alloca, by the frame index `FunctionLoweringInfo`
/// gave it.
fn alloca_sizes(
    module: &Module,
    function: &Function,
    layout: &DataLayout,
    info: &FunctionLoweringInfo,
) -> Result<Vec<Local>, String> {
    let mut sizes = vec![Local { size: 0, align: 1 }; info.static_allocas.iter().count()];
    for (value, index) in info.static_allocas.iter() {
        let ValueDef::Instruction(def) = function.value(value).def else {
            return Err("an alloca that is not an instruction".into());
        };
        let instruction = function.instruction(def);
        let Opcode::Alloca { allocated, align, .. } = instruction.opcode else {
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
        // as stated: `constrain_allocas` has clamped what the frame does not
        // keep
        let wanted = align.unwrap_or_else(|| layout.align(&module.context.types, allocated));
        sizes[*index as usize] = Local {
            size: layout.alloc_size(&module.context.types, allocated) as i64 * count,
            align: wanted as i64,
        };
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
    (cpu, sized, synthesize, c_symbol_prefix): (Cpu, bool, bool, &str),
    info: &mut FunctionLoweringInfo,
    selected: &mut Vec<Selected>,
) -> Result<(), String> {
    let units = select_block(&mut built.dag, cpu, sized, synthesize, c_symbol_prefix).map_err(|why| format!("{why:?}"))?;
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
                if let Some(result) = function.instruction(phi).result {
                    home.insert(result, successor);
                }
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
        select_built(None, extra, made, (cpu, sized, synthesize, c_symbol_prefix), info, selected)?;
    }
    Ok(())
}
