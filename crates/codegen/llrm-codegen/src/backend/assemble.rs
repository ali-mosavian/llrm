//! Takes a MIR module to masm, as `llc` takes a module to an object: each
//! defined function selected and put through the machine phases, each defined
//! variable's data, and the symbols both name.
//!
//! # How it works
//! `assembled_by` first places near-callable functions (`nearcode`) and names
//! the globals. It then machines the functions callees first, so what a
//! function that takes part in interprocedural register use writes is known
//! (`calleefacts`) when its callers are selected, and records it after each.
//!
//! `machined` makes one function:
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   staged["<b>staged</b><br/><small>isel, framefree, frame::of<br/>LIR over virtual regs</small>"]:::src
//!   a["<b>machine phases</b><br/><small>the allocator alone</small>"]:::mid
//!   far["<b>far frame cells and a spill</b><br/><small>area: relayout::laid_again<br/>fewer far cells kept</small>"]:::mid
//!   facts["<b>-Omax, a callee's fact used</b><br/><small>again without facts<br/>cheaper kept</small>"]:::mid
//!   pad["<b>landing pad moved last</b><br/><small>(landed_last)</small>"]:::mid
//!   fin["<b>assembled_by: cleaned returns,</b><br/><small>duplicated returns,<br/>shrink-wrap tails<br/>masm::Procedure</small>"]:::out
//!   staged --> a
//!   staged --> far
//!   staged --> facts
//!   a --> pad
//!   far --> pad
//!   facts --> pad
//!   pad --> fin
//! ```
//!
//! *How a staged procedure becomes a masm Procedure*
//!
//! `phased` runs the list of `flow::machine_with` over the staged body
//! (`prologue` is skipped; masm writes the prologue from the frame's reserve).
//! Each phase's output is checked, and `ISEL_DUMP=1` prints the LIR after
//! each.
//!
//! When all functions are done the module's constants are added to the data,
//! stack checks the program proves unneeded are elided, undefined names
//! become externs, and debug info is described.
//!
//! # Example
//! A MIR function and the masm it becomes (`bump`, called twice from `main`):
//!
//! ```mir
//! define internal regparm3cc i16 @bump(i16 %0) addrspace(1) nearcode ... {
//! b1:
//!   %1 = add i16 %0, 1
//!   ret i16 %1
//! }
//! ```
//!
//! ```text
//! _bump proc near
//! L0_0:
//!     inc ax
//!     ret
//! _bump endp
//! ```
//!
//! From `--dump` (`mir/21-homes.ll`) and `-S` of a nib program at -O2 with
//! `-fno-inline-functions -fno-inline-functions-called-once -fno-ipa-cp-clone
//! -march=i386`.
//!
//! # Runs
//! Once per module. Per-function candidates are described above; the gating
//! flags are in [`crate::backend`].

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_lir::registers::{RegId, Regs};
use llrm_mir::facts::Fact;
use llrm_mir::{GlobalId, GlobalKind, Linkage, Module};
use llrm_support::debug::timed;

use crate::abi::runtime::Contract;
use crate::backend::calleefacts;
use crate::backend::classes::RegisterClasses;
use crate::backend::constpool::Pool;
use crate::backend::cpu::{Profile, ProfileOrName};
use crate::backend::isel::{self, Selected};
use crate::backend::shadow::{self, Selector};
use crate::backend::target::Segments;
use crate::backend::{addressvalues, executed, frame, globals, jumps, masm, returnsplit, select};
use crate::flow;
use crate::model::ir::{Addr, AddressRef, Loc, Semantics, Space};
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

/// What only the frontend knows: each call's contract, and the symbol each
/// MIR name links as.
pub trait Abi {
    /// The contract of a call to `callee`: whether it pops its own
    /// arguments, and how many bytes were pushed.
    fn contract(
        &self,
        callee: &str,
        pops: bool,
        pushed: i64,
    ) -> Result<Contract, String>;
    fn linked(
        &self,
        name: &str,
    ) -> String;
    /// Whether a call of a `llrm.qb.` routine has a contract of its own (stack
    /// arguments, flags results, the registers it keeps), which the DAG path
    /// checks against its convention call by call.
    fn runtime_contracts(&self) -> bool {
        false
    }
    /// Where the runtime keeps its stack's lower limit and what overflowing it
    /// calls, where the program checks its stack.
    fn stack_check(&self) -> Option<&masm::StackCheck> {
        None
    }
    /// What a call to `callee` passes and answers in registers rather than
    /// on the stack, where its ABI names them.
    fn registers(
        &self,
        _callee: &str,
    ) -> Option<Registers> {
        None
    }
}

/// A routine's register interface: its last `arguments.len()` arguments,
/// each in its register, and its result's fields, each in its register.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Registers {
    pub arguments: Vec<RegId>,
    pub results: Vec<RegId>,
}

/// The address spaces the i8086 target gives far and huge pointers (clang's
/// `address_space(1)` and `(3)`).
pub(crate) const FAR_SPACE: u32 = 1;
const HUGE_SPACE: u32 = 3;

/// `module` as masm, its code in the segment `code`, selected by the 16-bit x86
/// selector: what the tests of this crate are written for.
#[cfg(test)]
pub fn assembled(
    module: &Module,
    abi: &dyn Abi,
    code: &str,
    cpu: ProfileOrName<'_>,
    segments: &Segments,
) -> Result<masm::Module, String> {
    assembled_by(module, abi, code, cpu, segments, isel::m16(), Selector::Old, &llrm_x86_m16::M16, &[])
}

/// `module` as masm, its code in the segment `code`, selected by `selection`.
pub fn assembled_by(
    module: &Module,
    abi: &dyn Abi,
    code: &str,
    cpu: ProfileOrName<'_>,
    segments: &Segments,
    selection: &'static isel::Compiled,
    selector: Selector,
    arch: &dyn llrm_target::Target,
    libcalls: &[(String, u32, String)],
) -> Result<masm::Module, String> {
    let cpu = crate::backend::cpu::profile(cpu)?;
    // A far object of more bytes than a segment holds is laid down in
    // segments, which a huge pointer steps through.
    let segment_bytes = arch.layout().segment_bytes().map_or(i64::MAX, |bytes| bytes as i64);
    let module = &*timed("mir near code", || crate::backend::nearcode::placed(module, &arch.layout().spaces.roles));
    let mut names =
        timed("global names", || globals::names(module, &arch.layout().spaces.roles, &|name| abi.linked(name)))?;
    names.extend(crate::hir::symbols::symbol_names());
    let mut procedures = Vec::new();
    let mut referenced: IndexMap<String, bool> = IndexMap::default();
    // The i64 divide routines the DAG route calls, once each:
    // ExpandDivRemLibCall's `__divdi3` family, written here as a procedure
    // of the module instead of pasted at every site.
    let mut shared: IndexMap<String, Vec<u8>> = IndexMap::default();
    let mut shared_registers = None;
    let mut data = Vec::new();
    let mut far_data = Vec::new();
    let mut sectioned: IndexMap<String, Vec<masm::Datum>> = IndexMap::default();
    let mut huge_objects: Vec<(String, Vec<masm::Datum>)> = Vec::new();
    let mut data_publics = Vec::new();
    let pool = Rc::new(RefCell::new(Pool::new(module.globals.len() as i64)));
    let classes = Rc::new(RegisterClasses::of(arch));
    let (facts, order) = calleefacts::CalleeFacts::of(module);
    let target = Target {
        facts: &facts,
        cpu,
        segments,
        selection,
        selector,
        arch,
        classes: &classes,
        runtime: "",
        basic: false,
        zeroed: false,
        libcalls,
    };
    // Callees first, so what a function that takes part writes is known when
    // its callers are selected.
    let mut done: IndexMap<GlobalId, Machined> = IndexMap::default();
    for id in order {
        let global = module.global(id);
        if global.function().is_some_and(|one| !one.is_declaration()) {
            let name = global.name.as_deref().unwrap_or_default();
            let made = llrm_support::debug::in_function(name, || machined(module, name, abi, &pool, &target))?;
            facts.record(name, &made.body, &made.registers, &made.calls);
            done.insert(id, made);
        }
    }
    let layout = module.datalayout.as_deref().and_then(|text| llrm_mir::datalayout::DataLayout::parse(text).ok());
    // DEVIATE SD231: a routine one site calls is pasted there, as the old
    // selector does: its procedure, call and return cost more than it saves.
    // The pin has no such routine (a libcall in the library).
    let mut sites: IndexMap<String, usize> = IndexMap::default();
    for made in done.values() {
        for (at, callee) in &made.calls {
            if callee.starts_with(llrm_target::routines::DIVIDE_PREFIX)
                && made.inline.contains_key(at)
                && made.inline_symbols.get(at).is_none_or(Vec::is_empty)
            {
                *sites.entry(callee.clone()).or_default() += 1;
            }
        }
    }
    for (at, global) in module.globals.iter().enumerate() {
        let id = GlobalId(at as u32);
        let name = global.name.as_deref().unwrap_or_default();
        match &global.kind {
            GlobalKind::Variable(_) if globals::is_compiler_only(name) => {}
            GlobalKind::Variable(variable) if variable.initializer.is_some() => {
                // A far or huge object lies outside DGROUP, in a segment of the
                // module's own.
                let mut datums = globals::datums(module, id, &names)?;
                // The constructor table is aligned by `datums`.
                if name != globals::CONSTRUCTORS
                    && let Some(to) = globals::alignment(
                        module,
                        layout.as_ref(),
                        variable,
                        module.sections.contains_key(name),
                        arch.layout().segment_bytes().is_some(),
                    )
                {
                    datums.insert(0, masm::Datum::Align(masm::Align { to }));
                }
                if name == globals::CONSTRUCTORS {
                    sectioned.entry(globals::INIT_SEGMENT.to_owned()).or_default().extend(datums);
                } else if let Some(section) = module.sections.get(name) {
                    // `#pragma data_seg`: a segment of the program's own
                    // naming.
                    sectioned.entry(section.clone()).or_default().extend(datums);
                } else if matches!(global.address_space, FAR_SPACE | HUGE_SPACE)
                    && masm::data_size(&datums, 0) >= segment_bytes
                {
                    // Past 64K: consecutive segments of 64K, which a huge
                    // pointer steps through.
                    huge_objects.push((name.to_owned(), datums));
                } else if matches!(global.address_space, FAR_SPACE | HUGE_SPACE) {
                    far_data.extend(datums);
                } else {
                    data.extend(datums);
                }
                if global.linkage == llrm_mir::Linkage::External {
                    data_publics.push(names[&(globals::space(module, id), at as i64)].clone());
                }
            }
            GlobalKind::Function(function) if !function.is_declaration() => {
                let unselected = |error: isel::Unselected| format!("@{name}: {}", error.0);
                let Machined { body, reserve, calls, inline, inline_symbols, far, pops, popped, registers, .. } =
                    done.shift_remove(&id).expect("every function with a body was machined");
                let body = timed("masm cleaned returns", || {
                    masm::cleaned_returns(
                        &addressvalues::converted(&body),
                        popped,
                        arch.return_address_bytes(isel::far(global).map_err(unselected)?),
                    )
                })?;
                let mut callees = IndexMap::default();
                shared_registers.get_or_insert(registers.clone());
                for (at, callee) in &calls {
                    if let Some(code) = inline.get(at) {
                        if !matches!(selector, Selector::Old)
                            && callee.starts_with(llrm_target::routines::DIVIDE_PREFIX)
                            && inline_symbols.get(at).is_none_or(Vec::is_empty)
                            && sites.get(callee).is_some_and(|&count| count > 1)
                        {
                            let routine = format!("${callee}");
                            shared.entry(routine.clone()).or_insert_with(|| code.clone());
                            callees.insert(*at, masm::Callee::new(routine, false));
                            continue;
                        }
                        // The bytes between the words that name a symbol, and
                        // each of those as a fixup.
                        let mut parts = Vec::new();
                        let mut from = 0;
                        let mut symbols = inline_symbols.get(at).cloned().unwrap_or_default();
                        symbols.sort_by_key(|one| one.at);
                        for symbol in &symbols {
                            if symbol.at > from {
                                parts.push(masm::InlinePart::Bytes(code[from..symbol.at].to_vec()));
                            }
                            let name = names
                                .get(&symbol.global)
                                .cloned()
                                .ok_or_else(|| format!("@{name}: inline code names a symbol with no name"))?;
                            // A function it names is an external of the
                            // distance it is reached at.
                            if module.global(GlobalId(symbol.global.1 as u32)).function().is_some() {
                                referenced.entry(name.clone()).or_insert(symbol.kind != "reloff");
                            }
                            if symbol.kind == "reloff" {
                                // A near call or jump: its opcode goes with its
                                // target.
                                let opcode = match parts.pop() {
                                    Some(masm::InlinePart::Bytes(mut bytes))
                                        if matches!(bytes.last(), Some(0xE8 | 0xE9)) =>
                                    {
                                        let opcode = bytes.pop().expect("an opcode");
                                        if !bytes.is_empty() {
                                            parts.push(masm::InlinePart::Bytes(bytes));
                                        }
                                        opcode
                                    }
                                    _ => {
                                        return Err(format!(
                                            "@{name}: inline code names a symbol relative to a word no call or jump owns"
                                        ));
                                    }
                                };
                                parts.push(masm::InlinePart::Near(opcode, name));
                            } else {
                                parts.push(masm::InlinePart::Fixup(symbol.kind.clone(), name, symbol.addend));
                            }
                            from = symbol.at + 2;
                        }
                        if from < code.len() {
                            parts.push(masm::InlinePart::Bytes(code[from..].to_vec()));
                        }
                        callees.insert(
                            *at,
                            masm::Callee { name: callee.clone(), far: false, pops: 0, code: parts, tail: false },
                        );
                        continue;
                    }
                    // A global of this module is called by the name it is
                    // defined or declared as.
                    let linked = match module.named(callee) {
                        Some(id) => names[&(globals::space(module, id), i64::from(id.0))].clone(),
                        None => abi.linked(callee),
                    };
                    referenced.insert(linked.clone(), far.contains(at));
                    callees.insert(
                        *at,
                        masm::Callee {
                            pops: pops.get(at).copied().unwrap_or(0),
                            ..masm::Callee::new(linked, far.contains(at))
                        },
                    );
                }
                // An interrupt handler loads DGROUP into DS itself.
                let interrupt = (function.calling_convention == llrm_mir::opcode::X86_INTR).then(|| {
                    let (space, index) = crate::hir::symbols::DGROUP;
                    names.insert((space, index), globals::DGROUP.to_owned());
                    Addr { index, ..Addr::new(space, 0) }
                });
                let procedure = masm::Procedure {
                    name: names[&(Space::Segment, at as i64)].clone(),
                    public: global.linkage == Linkage::External,
                    far: isel::far(global).map_err(unselected)?,
                    body,
                    reserve,
                    callees,
                    interrupt,
                    size: cpu.size,
                    entry: 0,
                    stack_check: function
                        .attrs
                        .iter()
                        .any(|one| Fact::of_attribute(one) == Some(Fact::StackCheck))
                        .then(|| abi.stack_check().cloned())
                        .flatten(),
                    probe_stack: None,
                    registers,
                };
                // Copies of returns and split tails only make code better: -O0
                // has neither.
                let procedure = if !cpu.optimized {
                    if let Some(why) = masm::realign_refusal(&procedure) {
                        return Err(why);
                    }
                    masm::realigned(procedure)
                } else {
                    let procedure = timed("lir split returns", || returnsplit::split(procedure));
                    // Asked only by a tail a copy could save a jump for.
                    let (overhead, unprintable) = (std::cell::OnceCell::new(), std::cell::RefCell::new(None));
                    let overhead = || {
                        *overhead.get_or_init(|| {
                            timed("masm return overhead", || masm::return_overhead_bytes(&procedure)).map_or_else(
                                |error| {
                                    *unprintable.borrow_mut() = Some(error.to_string());
                                    0
                                },
                                |bytes| bytes as i64,
                            )
                        })
                    };
                    let body = timed("lir duplicated returns", || {
                        jumps::duplicated_returns(procedure.body.clone(), &overhead)
                    });
                    if let Some(error) = unprintable.take() {
                        return Err(error);
                    }
                    let procedure = masm::Procedure { body, ..procedure };
                    if let Some(why) = masm::realign_refusal(&procedure) {
                        return Err(why);
                    }
                    let procedure = masm::exits_inserted(masm::saved_first(procedure));
                    if procedure.body.frame_exit.is_some() {
                        let body = jumps::tails_merged_after_exits(procedure.body.clone(), cpu.size, Some(&calls))
                            .map_err(|error| error.to_string())?;
                        masm::Procedure { body, ..procedure }
                    } else {
                        procedure
                    }
                };
                procedures.push(procedure);
            }
            _ => {}
        }
    }
    for (name, code) in shared {
        let registers = shared_registers.clone().expect("a routine is shared by a function");
        procedures.push(masm::routine_procedure(&name, code, registers, Regs(arch.registers())));
    }
    for (bytes, id) in crate::backend::constpool::wide_first(pool.borrow().entries()) {
        let name = format!("$K{id}");
        names.insert((Space::Segment, id), name.clone());
        // A double is on an 8-byte boundary: a 486 takes 3 clocks more for one
        // off it (Intel486 data sheet 240440-002, section 10.1 and
        // Table 2.1).
        // A 16-byte one is a bitwise SSE instruction's memory operand, which
        // must be aligned.
        if bytes.len() >= 8 {
            data.push(masm::Datum::Align(masm::Align { to: if bytes.len() >= 16 { 16 } else { 8 } }));
        }
        data.extend([masm::Datum::Label(masm::Label { name }), masm::Datum::Bytes(bytes.to_vec())]);
    }
    data.extend(masm::jump_tables(&procedures, &mut names, module));
    timed("stack checks", || {
        crate::backend::stackusage::elide_checks(&mut procedures, &masm::entered_directly(module, &names), &*arch)
    });
    // A function this module declares and names only in data (a table of
    // function pointers) is an external too: no call says so.
    let declared: BTreeSet<&str> = module
        .globals
        .iter()
        .enumerate()
        .filter(|(_, global)| {
            matches!(
                &global.kind,
                GlobalKind::Function(function) if function.is_declaration()
            )
        })
        .filter_map(|(at, _)| names.get(&(globals::space(module, GlobalId(at as u32)), at as i64)).map(String::as_str))
        .collect();
    for datum in &data {
        if let masm::Datum::Pointer(pointer) = datum
            && declared.contains(pointer.name.as_str())
        {
            referenced.entry(pointer.name.clone()).or_insert(pointer.far);
        }
    }
    // A routine whose address code takes is as much an external as one it
    // calls.
    for (at, global) in module.globals.iter().enumerate() {
        let (GlobalKind::Function(function), Some(name)) =
            (&global.kind, names.get(&(globals::space(module, GlobalId(at as u32)), at as i64)))
        else {
            continue;
        };
        if function.is_declaration() && addressed_in_code(module, GlobalId(at as u32)) {
            referenced.entry(name.clone()).or_insert(isel::far(global).unwrap_or(true));
        }
    }
    let defined: BTreeSet<&str> = procedures.iter().map(|one| one.name.as_str()).collect();
    let mut externs: Vec<(String, String)> = referenced
        .iter()
        .filter(|(name, _)| !defined.contains(name.as_str()))
        .map(|(name, &far)| (name.clone(), if far { "far" } else { "near" }.to_owned()))
        .collect();
    // A variable declared and not defined is data the linker finds; the object
    // lists only those code reaches.
    externs.extend(module.globals.iter().enumerate().filter_map(|(at, global)| {
        let GlobalKind::Variable(variable) = &global.kind else { return None };
        let name = names.get(&(globals::space(module, GlobalId(at as u32)), at as i64))?;
        let kind = if matches!(global.address_space, FAR_SPACE | HUGE_SPACE) { "far-byte" } else { "byte" };
        (variable.initializer.is_none() && !defined.contains(name.as_str())).then(|| (name.clone(), kind.to_owned()))
    }));
    externs.extend(masm::stack_externs(&procedures, &mut names));
    externs.sort();
    externs.dedup();
    let debug = timed("debug info", || {
        crate::backend::debuginfo::described(
            module,
            &names,
            llrm_object::debug::Producer::Native,
            &crate::backend::debuginfo::Machine::of(arch),
        )
    })?;
    let code_segments: std::collections::BTreeMap<String, String> = procedures
        .iter()
        .filter_map(|one| module.sections.get(&one.name).map(|section| (one.name.clone(), section.clone())))
        .collect();
    let has_far = !far_data.is_empty();
    let far_segment = format!("{}_FARDATA", code.strip_suffix("_TEXT").unwrap_or(code));
    let mut huge_segments: Vec<(String, Vec<masm::Datum>)> = Vec::new();
    for (name, datums) in huge_objects {
        let pieces = masm::split_data(datums, segment_bytes).map_err(|why| format!("@{name}: {why}"))?;
        for (at, piece) in pieces.into_iter().enumerate() {
            huge_segments.push((format!("{far_segment}_{}${at}", name.trim_start_matches('_')), piece));
        }
    }
    let huge_names: Vec<String> = huge_segments.iter().map(|(name, _)| name.clone()).collect();
    Ok(masm::Module {
        registers: Regs(arch.registers()),
        code: code.to_owned(),
        names,
        externs,
        publics: procedures.iter().filter(|one| one.public).map(|one| one.name.clone()).chain(data_publics).collect(),
        data: std::iter::once(("_DATA".to_owned(), data))
            .chain(has_far.then(|| (far_segment.clone(), far_data)))
            .chain(huge_segments)
            .chain(sectioned)
            .collect(),
        procedures,
        private: BTreeSet::from_iter(has_far.then(|| far_segment.clone()).into_iter().chain(huge_names)),
        requests: BTreeSet::new(),
        debug,
        far_bss: BTreeSet::new(),
        code_segments,
        stack: 0,
        object: arch.object(),
        padding: masm::Padding { function: arch.pref_function_alignment(), nop: arch.max_nop_bytes() },
    })
}

/// Whether a function body of `module` takes `global`'s address: names it as an
/// operand other than a call's callee.
fn addressed_in_code(
    module: &Module,
    global: GlobalId,
) -> bool {
    use llrm_mir::module::Operand;
    use llrm_mir::{ConstantKind, Opcode};
    module
        .globals
        .iter()
        .filter_map(|one| one.function())
        .filter(|one| !one.is_declaration())
        .any(
            |function| {
                function
                    .layout()
                    .iter()
                    .any(
                        |&block| {
                            function
                                .iter_block(block)
                                .any(
                                    |inst| {
                                        let instruction = function.instruction(inst);
                                        // A call's callee is its last operand
                                        // (an invoke's, before its two
                                        // targets).
                                        let callee_at = match instruction.opcode {
                                            Opcode::Call(_) => instruction.operands.len().checked_sub(1),
                                            Opcode::Invoke(_) => instruction.operands.len().checked_sub(3),
                                            _ => None,
                                        };
                                        instruction
                                            .operands
                                            .iter()
                                            .enumerate()
                                            .any(
                                                |(at, operand)| Some(at) != callee_at
                                                    && matches!(
                                                        operand,
                                                        Operand::Constant(id) if matches!(module.context.get(*id).kind, ConstantKind::Global(g) if g == global)
                                                    ),
                                            )
                                    },
                                )
                        },
                    )
            },
        )
}

/// What the machine a frontend compiles for is: its processor, its segment
/// registers, the runtime family whose frame it calls into, and whether
/// floats keep BASIC's semantics.
#[derive(Clone, Copy)]
pub struct Target<'t> {
    /// What the functions that take part in interprocedural register use leave
    /// different, made as each is machined.
    pub facts: &'t calleefacts::CalleeFacts,
    /// What the target requires of registers: the pins of its forms.
    pub classes: &'t Rc<RegisterClasses>,
    pub cpu: &'t Profile,
    pub segments: &'t Segments,
    /// The instruction selector of the target, which its driver binds.
    pub selection: &'static isel::Compiled,
    /// `--isel=`: which selector makes the LIR.
    pub selector: Selector,
    /// The target the selection is for.
    pub arch: &'t dyn llrm_target::Target,
    pub runtime: &'t str,
    pub basic: bool,
    /// A framed function's locals start zeroed: B$ENRA zero-fills them.
    pub zeroed: bool,
    /// The library routines the runtime has, with the calling convention and
    /// symbol a call of each uses (`options.libcalls`).
    pub libcalls: &'t [(String, u32, String)],
}

/// A function selected and through the machine phases, as llc's
/// per-function pipeline: its LIR, returns not yet cleaned, the bytes its
/// frame reserves below BP, each call's callee by the call's `at` and
/// which are far, and the bytes it pops.
pub struct Machined {
    pub body: LirBody,
    pub reserve: i64,
    pub calls: IndexMap<i64, String>,
    /// The code laid down in place of each call to an inline helper.
    pub inline: IndexMap<i64, Vec<u8>>,
    /// Where the frame places the inline code names were patched in, by site:
    /// (byte, displacement, addend).
    pub inline_places: IndexMap<i64, Vec<(usize, i64, i64)>>,
    pub inline_symbols: IndexMap<i64, Vec<isel::InlineSymbol>>,
    pub far: BTreeSet<i64>,
    /// The argument bytes each direct call's callee pops.
    pub pops: IndexMap<i64, i64>,
    pub popped: i64,
    /// The frame's registers, the saved ones those this function's convention
    /// keeps.
    pub registers: llrm_target::FrameRegisters,
    /// The landing pad's block, laid out last, which the statement table
    /// gives the runtime.
    pub landing: Option<i64>,
}

/// `name` of `module` selected and run through the machine phases, float
/// constants in `pool`.
///
/// Where spill slots end up past a one-byte displacement below allocas,
/// the function is selected again with the allocas below a hole the spill
/// slots fill, and whichever has fewer two-byte displacements is kept.
pub fn machined(
    module: &Module,
    name: &str,
    abi: &dyn Abi,
    pool: &Rc<RefCell<Pool>>,
    target: &Target<'_>,
) -> Result<Machined, String> {
    let first = staged(module, name, abi, pool, target, 0)?;
    let mut kept = machined_once(module, name, abi, pool, target, first.clone())?;
    // With -Omax, a function that calls one whose registers are known is made
    // without that too, and the cheaper kept: what the allocator costs is
    // an estimate, and a freer choice of registers is not always the better
    // code. The facts won in 4% of 947 such functions (0.3% of their cost),
    // all in 6 of them.
    if target.cpu.exhaustive && calls_known(module, name, target.facts) {
        let none = calleefacts::CalleeFacts::none();
        let plain_target = Target { facts: &none, ..*target };
        let again = staged(module, name, abi, pool, &plain_target, 0)?;
        // Selected the same without the facts, it is made the same: only the
        // callee facts reach the selector.
        if !again.same_as(&first) {
            let plain = machined_once(module, name, abi, pool, &plain_target, again)?;
            // Its own cost leaves the pushes and pops of the registers it saves
            // out: they are the frame's, made after.
            let priced = |made: &Machined| route_cost(made, target);
            if let Some((with, without)) = priced(&kept).zip(priced(&plain)) {
                if without < with {
                    kept = plain;
                }
            }
        }
    }
    // What the function's cost in `timefunc` is measured against.
    llrm_support::debug!("size", "{name} {}", kept.body.insns().len());
    // Reported once the choice is made: a rejected candidate is no function's
    // cost.
    if crate::support::debug::enabled("cost") {
        llrm_support::debug!("cost", "{}", executed::summary(&kept.body, target.cpu));
    }
    if crate::support::debug::enabled("isel") {
        llrm_support::debug!("isel", "{}", shadow::row(&kept.body, target.selector));
    }
    if crate::support::debug::enabled("pressure") {
        if let Some(row) = executed::pressure(&kept.body) {
            llrm_support::debug!("pressure", "{row}");
        }
    }
    Ok(kept)
}

/// How many registers `made` pushes and pops for its caller: the ones its
/// convention keeps that its body writes or a call in it disturbs.
fn saves(made: &Machined) -> usize {
    let mut used = masm::_roots(&made.body);
    let regs = made.body.regs();
    for one in made.body.insns() {
        used.extend(one.call.iter().flat_map(|call| call.disturbs.iter().copied().map(|register| regs.root(register))));
    }
    made.registers.saved.iter().filter(|(whole, _)| used.contains(whole)).count()
}

/// Whether `name` makes a direct call of a function whose written registers are
/// known.
fn calls_known(
    module: &Module,
    name: &str,
    facts: &calleefacts::CalleeFacts,
) -> bool {
    let Some(function) = module.named(name).and_then(|id| module.global(id).function()) else { return false };
    function
        .walk()
        .any(
            |(_, inst)| llrm_mir::memory::callee(&module.context, function, inst)
                .and_then(|id| module.global(id).name.as_deref())
                .is_some_and(|callee| facts.written(callee).is_some()),
        )
}

fn machined_once(
    module: &Module,
    name: &str,
    abi: &dyn Abi,
    pool: &Rc<RefCell<Pool>>,
    target: &Target<'_>,
    first_stage: Staged,
) -> Result<Machined, String> {
    MACHINED.with(|count| count.set(count.get() + 1));
    llrm_mir::counts::bump(llrm_mir::counts::Event::Machinings);
    let (first, frame) = timed("candidate first frame", || cheaper(&first_stage, name, pool, target))?;
    // `PrologEpilogInserter` orders the frame objects by use density
    // (`X86FrameLowering::orderFrameObjects`); taken over the layouts below
    // where it brings fewer operands past a one-byte displacement than they do.
    let far = far_frame(&first.body);
    // An over-aligned local lies on its alignment below the aligned top: a
    // layout that moves objects would break it.
    let ordered = if far > 0 && first.body.over_aligned.is_empty() {
        crate::backend::relayout::ordered(&first, &frame, target.basic).map(|(ordered, _)| ordered)
    } else {
        None
    };
    let best = |chosen: Machined| match ordered {
        Some(ordered) if far_frame(&ordered.body) < far_frame(&chosen.body) => ordered,
        _ => chosen,
    };
    let spilled = frame.floor + first.reserve;
    // The allocas move down by the hole: a multiple of the stack alignment, or
    // an 8-aligned array lands at 4 mod 8 (nbody -O2: 18,404 operands). Shared
    // spill slots can end on a word, so a 4-aligned frame needs it too: the
    // backend run again aligns the allocas and the frame moved by the hole
    // did not (reserve 286 against 288).
    let align = frame.align.max(frame.alloca_align);
    let spilled = if align > 1 { (spilled + align - 1).div_euclid(align) * align } else { spilled };
    if target.basic || frame.native.is_some() || spilled <= 0 || frame.floor == 0 || far == 0 {
        return Ok(best(first));
    }
    // The second layout is made from the first run's frame, not by running the
    // backend again; `LLRM_CHECK_FRAME=1` runs it again beside, and the two
    // must be the same function. A frame that cannot be moved keeps its first
    // layout.
    let laid = timed("frame laid again", || crate::backend::relayout::laid_again(&first, &frame, spilled));
    if llrm_support::env_flag!("LLRM_CHECK_FRAME") {
        let (again, _) = cheaper(&staged(module, name, abi, pool, target, spilled)?, name, pool, target)?;
        match &laid {
            Some((laid, _)) => {
                if let Some(difference) = crate::backend::relayout::difference(laid, &again) {
                    panic!(
                        "@{name} (hole {spilled}): the frame laid again differs from the backend run again: {difference}"
                    );
                }
            }
            None => panic!("@{name} (hole {spilled}): the frame could not be laid again"),
        }
    }
    Ok(best(match laid {
        Some((second, _)) if far_frame(&second.body) < far => second,
        _ => first,
    }))
}

/// `phased`, with the spiller; and, where the spiller changed the body, without
/// it too: the one that costs less is kept. The spiller decides on the general
/// registers alone, so where the allocator's pressure is elsewhere (segment
/// registers, x87 and fixed-register glue) its spill code can be on top of what
/// the allocator does anyway.
fn cheaper(
    staged: &Staged,
    name: &str,
    pool: &Rc<RefCell<Pool>>,
    target: &Target<'_>,
) -> Result<(Machined, frame::Frame), String> {
    timed("candidate allocator alone", || phased(staged, name, pool, target))
}

thread_local! {
    static MACHINED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has made a function through the machine phases,
/// one candidate each, for a test of how many a function takes.
pub fn machinings() -> usize {
    MACHINED.with(std::cell::Cell::get)
}

thread_local! {
    static PHASED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

thread_local! {
    static SELECTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has selected a function's instructions, for a
/// test that the routes share one selection.
pub fn selections() -> usize {
    SELECTED.with(std::cell::Cell::get)
}

/// How many routes through the machine phases this thread has run.
pub fn routes() -> usize {
    PHASED.with(std::cell::Cell::get)
}

/// What a route's function costs with the prologue and epilogue its registers
/// cause: `cost` leaves out the pushes and pops of the registers it saves,
/// which are the frame's, made after, and a route that uses more registers pays
/// more of them (nbody_fixed -Os: the spiller's route was smaller by the count
/// and larger by 17 bytes, and 7% slower).
fn route_cost(
    made: &Machined,
    target: &Target<'_>,
) -> Option<f64> {
    cost(made, target).map(|one| one + saves(made) as f64 * 2.0)
}

/// `what` with the stack and frame tokens of its register-addressed cells
/// named at the width of the target's addresses: `[sp]` has no encoding in
/// flat code, and the token is only the stack's name until the frame layout
/// resolves it.
fn stack_as_addressed(
    regs: Regs,
    bits: u32,
    what: &Semantics,
) -> Semantics {
    let bytes = i64::from(bits / 8);
    let widen = |place: &Loc| {
        if place.address().is_none_or(|cell| cell.addr.is_some()) {
            return place.clone();
        }
        place.map_address(|cell| {
            let wide = |register: RegId| {
                if regs.is_stack(register) || regs.is_frame(register) { regs.named(register, bytes) } else { register }
            };
            AddressRef { through: wide(cell.through), index_through: wide(cell.index_through), ..cell }
        })
    };
    Semantics {
        dests: what.dests.iter().map(&widen).collect(),
        sources: what.sources.iter().map(&widen).collect(),
        ..what.clone()
    }
}

/// What a finished function costs: its encoded bytes where the target optimizes
/// for size, else the instructions and memory operands it is expected to
/// execute per call.
fn cost(
    made: &Machined,
    target: &Target<'_>,
) -> Option<f64> {
    if target.cpu.size {
        made.body
            .insns()
            .iter()
            .filter_map(|one| one.what.as_ref())
            .map(|what| {
                let what = &stack_as_addressed(made.body.regs(), made.body.bits, what);
                let priced = select::priced_in(made.body.bits, what, 0, None, false, false, None)
                    .map(|code| code.code.len() as f64);
                // A function with one instruction the encoder cannot price is
                // priced as nothing: its routes compare as
                // unpriced and the first kept (nbody_fixed -Os: `lea esi,
                // [ebx+ebx*2]` had no price).
                if priced.is_none() && llrm_support::debug::verifying() {
                    panic!("@{}: no byte price for {what:?}", made.body.name);
                }
                priced
            })
            .sum()
    } else {
        executed::work(&made.body)
    }
}

/// The frame operands a one-byte displacement does not reach.
fn far_frame(body: &LirBody) -> usize {
    body.insns()
        .iter()
        .filter_map(|one| one.what.as_ref())
        .flat_map(|what| what.dests.iter().chain(&what.sources))
        .filter_map(|place| place.address().and_then(|one| one.addr))
        .filter(|addr| addr.space == Space::Frame && !(-128..128).contains(&addr.disp))
        .count()
}

/// A function selected and its frame made: what every route through the machine
/// phases starts from, as the selector's output does not depend on the route
/// (the spiller, admission) but on the hole and the callee facts alone.
#[derive(Clone)]
struct Staged {
    body: LirBody,
    frame: frame::Frame,
    convention: isel::Convention,
    calls: IndexMap<i64, String>,
    inline: IndexMap<i64, Vec<u8>>,
    inline_places: IndexMap<i64, Vec<(usize, i64, i64)>>,
    inline_symbols: IndexMap<i64, Vec<isel::InlineSymbol>>,
    far: BTreeSet<i64>,
    pops: IndexMap<i64, i64>,
    landing: Option<i64>,
    registers: llrm_target::FrameRegisters,
    /// The register classes the routes allocate from: the target's, with the
    /// frame register a value register where the function needs none.
    classes: Rc<RegisterClasses>,
}

impl Staged {
    /// Whether every route from `other` would make what the routes from this
    /// make.
    fn same_as(
        &self,
        other: &Staged,
    ) -> bool {
        self.body == other.body
            && self.frame == other.frame
            && format!("{:?}", self.convention) == format!("{:?}", other.convention)
            && self.calls == other.calls
            && self.inline == other.inline
            && self.inline_places == other.inline_places
            && self.inline_symbols == other.inline_symbols
            && self.far == other.far
            && self.pops == other.pops
            && self.landing == other.landing
            && format!("{:?}", self.registers) == format!("{:?}", other.registers)
    }
}

/// Whether `name` calls a function `llrm_mir::atomic::hidden` made that the
/// DAG has no node for: all but an exchange and a store.
fn calls_atomic(
    module: &Module,
    name: &str,
) -> bool {
    use llrm_mir::module::Operand;
    use llrm_mir::opcode::Opcode;
    let Some(global) = module.named(name) else { return false };
    let GlobalKind::Function(function) = &module.global(global).kind else { return false };
    function
        .layout()
        .iter()
        .any(
            |&block| function
                .iter_block(block)
                .any(
                    |inst| {
                        let instruction = function.instruction(inst);
                        matches!(instruction.opcode, Opcode::Call(_))
                            && matches!(
                                instruction.operands.last(),
                                Some(&Operand::Constant(callee)) if matches!(module.context.get(callee).kind, llrm_mir::context::ConstantKind::Global(one) if module.global(one).name.owned().and_then(|name| llrm_mir::atomic::decode(&name)).is_some_and(|info| !matches!(info.kind, llrm_mir::opcode::AtomicKind::Rmw(llrm_mir::opcode::RmwOp::Xchg) | llrm_mir::opcode::AtomicKind::Store)))
                            )
                    },
                ),
        )
}

/// `name` selected with `hole` bytes left above the allocas, and its frame
/// made.
fn staged(
    module: &Module,
    name: &str,
    abi: &dyn Abi,
    pool: &Rc<RefCell<Pool>>,
    target: &Target<'_>,
    hole: i64,
) -> Result<Staged, String> {
    SELECTED.with(|count| count.set(count.get() + 1));
    llrm_mir::counts::bump(llrm_mir::counts::Event::Selections);
    let zeroed = target.zeroed && module.named(name).is_some_and(|global| crate::driver::framed(module, global));
    let contracts = Some(crate::backend::selectiondag::dag_function::RuntimeContracts {
        abi,
        segments: target.segments,
        runtime: abi.runtime_contracts(),
        frame_zeroed: zeroed,
        facts: Some(target.facts),
        hole,
    });
    let dag = match target.selector {
        Selector::Old => None,
        // `LLRM_ISEL_ONLY=a,b`: the DAG path for those functions, the old
        // selector for the rest -- to bisect a wrong result to a function
        // DEVIATE SD253: the DAG has `ATOMIC_SWAP` only; a function calling any
        // other hidden atomic (`llrm_mir::atomic::hidden`) is selected by the
        // old selector.
        Selector::Dag | Selector::Shadow if !shadow::only(name) || calls_atomic(module, name) => None,
        Selector::Dag => Some(timed("dag isel", || {
            crate::backend::selectiondag::dag_function::selected_in_pool(
                module,
                name,
                target.arch,
                target.cpu,
                target.libcalls,
                contracts.as_ref(),
                &mut pool.borrow_mut(),
            )
        })),
        Selector::Shadow => {
            match timed("dag isel", || {
                crate::backend::selectiondag::dag_function::selected_in_pool(
                    module,
                    name,
                    target.arch,
                    target.cpu,
                    target.libcalls,
                    contracts.as_ref(),
                    &mut pool.borrow_mut(),
                )
            }) {
                Ok(done) => Some(Ok(done)),
                Err(why) => {
                    llrm_support::debug!("isel", "{name} refused: {}", why.0);
                    None
                }
            }
        }
    };
    let selected = if let Some(done) = dag {
        if done.is_ok() {
            shadow::record(name, Selector::Dag);
        }
        done
    } else {
        shadow::record(name, Selector::Old);
        timed("isel", || {
            isel::selected_with(
                module,
                name,
                abi,
                &mut pool.borrow_mut(),
                target.cpu,
                target.segments,
                target.selection,
                target.arch,
                zeroed,
                hole,
                target.facts,
            )
        })
    };
    let Selected {
        body,
        convention,
        calls,
        inline,
        inline_places,
        inline_symbols,
        far,
        pops,
        depth,
        align: alloca_align,
        extents,
        temporaries,
        over_aligned,
        landing,
    } = selected.map_err(|error| format!("@{name}: {}", error.0))?;
    let mut body = LirBody { over_aligned, ..body };
    // Where no callee pops, a call through a pointer takes nothing off the
    // stack: the frame register is not pinned by not knowing.
    if !target.arch.callee_pops() {
        let open: Vec<i64> = body
            .insns()
            .iter()
            .filter(|one| {
                one.what.as_ref().is_some_and(|what| what.op == crate::model::ir::Operation::Call && what.indirect)
            })
            .map(|one| one.at)
            .filter(|at| !body.call_pops.contains_key(at))
            .collect();
        std::sync::Arc::make_mut(&mut body.call_pops).extend(open.into_iter().map(|at| (at, 0)));
    }
    let mut registers =
        llrm_target::FrameRegisters { saved: convention.saved.clone(), ..target.arch.frame_registers() };
    // The convention's entry functions (`entry_clobbers_all`) keep nothing for
    // a caller that does not exist.
    let entry = module
        .named(name)
        .and_then(|id| module.global(id).function())
        .and_then(|function| target.arch.calling().by_number(function.calling_convention));
    if target.facts.uncalled(name) && entry.is_some_and(|one| one.entry_clobbers_all.iter().any(|entry| entry == name))
    {
        registers.saved.clear();
    }
    registers.align = target.facts.stack_alignment(target.arch, name);
    // LLVM's `hasFP`, before allocation: a function that can do without its
    // frame register has it as a value register.
    // A frame the BASIC runtime builds (`B$ENRA`) is BP's and the runtime walks
    // the chain, so its frame register is never a value register; a frame the
    // compiler builds is like any other (an error landing, which restores BP,
    // keeps it: `landing`). Inline code that is only x87 register operations
    // (`fsin` as bytes) touches no stack cell. Not where BASIC is tuned for
    // size: `[esp+d]` is a byte longer than `[ebp+d]`, and masm's comparison of
    // the two cannot run once the register holds a value (bq32 -Os grew 4-7%).
    let classes: Rc<RegisterClasses> = if !(target.basic && (target.zeroed || target.cpu.size))
        && !crate::backend::framefree::has_fp(
            &body,
            &registers,
            &pops.iter().map(|(at, bytes)| (*at, *bytes)).collect(),
            // Tuned for size any inline code keeps the frame register until
            // masm has compared the two forms; x87 register operations alone
            // do not otherwise.
            if target.cpu.size {
                !inline.is_empty()
            } else {
                inline.values().any(|bytes| !crate::backend::framefree::x87_registers_only(bytes))
            },
            false,
            landing.is_some(),
        ) {
        registers.free = true;
        // Saved as any kept register is: its root, and the register a prologue
        // pushes.
        registers.saved.push((body.regs().root(registers.pointer), registers.pointer));
        Rc::new(target.classes.with_frame_free())
    } else {
        Rc::clone(target.classes)
    };
    if llrm_support::env_flag!("ISEL_DUMP") {
        println!(
            "{}",
            crate::backend::lirtext::lir_stage(
                &format!("ISel {}", target.selector.name()),
                &[(body.name.clone(), body.clone())],
            )
        );
    }
    let body = if llrm_support::debug::verifying() {
        timed("lir verify", || flow::verified(body, "isel", true, None, &target.arch.layout().spaces.roles))
            .map_err(|error| error.0)?
    } else {
        body
    };
    let mut frame =
        timed("lir frame", || frame::of(&body, Some(&calls), target.runtime, None)).map_err(|error| error.0)?;
    frame.floor = frame.floor.min(-depth);
    frame.align = registers.align.max(registers.slot).max(frame.align);
    frame.hole = hole;
    frame.alloca_align = alloca_align;
    frame.extents = extents;
    let body = frame.tagged(&body).map_err(|error| error.0)?;
    // DEVIATE D14: the pin gives each rounding store and reload its own
    // `CreateStackTemporary` and no lifetime marker, so neither StackColoring
    // nor StackSlotColoring shares them; here they are spill slots, which
    // `slots_colored` shares.
    if temporaries > 0 && frame.native.is_none() {
        frame.floor = -depth;
    }
    for (at, (disp, size)) in frame.extents.split_off(frame.extents.len() - temporaries).into_iter().enumerate() {
        frame.slots.insert(frame::SlotKey::Named("temporary".to_owned(), at as i64), disp);
        frame.scratch.insert(disp);
        frame.capacities.insert(disp, size.max(frame::WORD));
    }
    Ok(Staged {
        body,
        frame,
        convention,
        calls,
        inline,
        inline_places,
        inline_symbols,
        far,
        pops,
        landing,
        registers,
        classes,
    })
}

/// `machined` once through the machine phases, and the frame.
fn phased(
    staged: &Staged,
    name: &str,
    pool: &Rc<RefCell<Pool>>,
    target: &Target<'_>,
) -> Result<(Machined, frame::Frame), String> {
    PHASED.with(|count| count.set(count.get() + 1));
    let Staged {
        calls, inline, inline_places, inline_symbols, far, pops, landing, registers, convention, classes, ..
    } = staged.clone();
    let classes = &classes;
    let mut body = staged.body.clone();
    let frame = Rc::new(RefCell::new(staged.frame.clone()));
    let pinned = body.pins.clone();
    let mut in_ssa = true;
    let preserved: std::collections::BTreeSet<RegId> =
        registers.saved.iter().map(|(whole, _)| classes.registers.root(*whole)).collect();
    let mut allocated = false;
    for mut phase in flow::machine_with(
        &pinned,
        Some(Rc::clone(&frame)),
        Some(Rc::clone(pool)),
        Some(&calls),
        target.basic,
        ProfileOrName::Profile(target.cpu),
        target.segments,
        classes,
        target.selection.rules(),
        &registers,
        target.selector,
    )? {
        // masm writes the prologue from the frame's reserve.
        if phase.class_name() == "Prologue" {
            continue;
        }
        if phase.class_name() == "PhiElimination" {
            in_ssa = false;
        }
        let kept = (!body.notes.is_empty()).then(|| body.clone());
        body = flow::checked(
            body,
            phase.as_mut(),
            in_ssa,
            allocated.then_some(&preserved),
            classes,
            target.segments,
            target.arch,
        )
        .map_err(|error| match error {
            flow::Checked::Refused(raised) => format!("@{name}: {}", raised.message),
            flow::Checked::Malformed(malformed) => format!("@{name}: {}", malformed.0),
        })?;
        if phase.class_name() == "RegAlloc" {
            allocated = true;
        }
        if let Some(before) = &kept {
            body = body.with_notes_kept(before).with_defs_kept(before);
        }
        if phase.class_name() == "PhiElimination" {
            body = body.with_phi_copies_noted();
        }
        if llrm_support::env_flag!("ISEL_DUMP") {
            println!(
                "{}",
                crate::backend::lirtext::lir_stage(phase.class_name(), &[(body.name.clone(), body.clone())])
            );
        }
    }
    let frame = frame.borrow().clone();
    // The reserved call area is the lowest bytes below the locals and spills.
    let reserve = -std::cmp::min(frame.slots.values().copied().min().unwrap_or(0), frame.floor) + frame.call_area;
    if llrm_support::env_flag!("LLRM_FRAME_ORACLE") {
        eprintln!("{}", crate::backend::frameobjects::oracle_line(name, &frame, reserve));
    }
    let (body, landing) = match landing {
        Some(marker) => {
            let (body, at) =
                timed("lir landed last", || landed_last(body, marker)).map_err(|error| format!("@{name}: {error}"))?;
            (body, Some(at))
        }
        None => (body, None),
    };
    Ok((
        Machined {
            body,
            reserve,
            calls,
            inline,
            inline_places,
            inline_symbols,
            far,
            pops,
            popped: convention.popped,
            registers,
            landing,
        },
        frame,
    ))
}

/// `body` with its landing pad, the block `marker` starts, laid out last:
/// the runtime resumes at the first statement-table row at or after the
/// faulting call, and the pad is the function's one row.
fn landed_last(
    body: LirBody,
    marker: i64,
) -> Result<(LirBody, i64), String> {
    let starts = |block: &crate::model::lir::Block| {
        block.insns.iter().find(|one| masm::prints(one)).is_some_and(|one| one.at == marker)
    };
    let Some(index) = body.blocks.iter().position(starts) else {
        return Err("the machine phases moved the landing pad's start".to_owned());
    };
    let mut blocks = body.blocks.clone();
    let pad = blocks.remove(index);
    let at = pad.at;
    blocks.push(pad);
    Ok((body.with_blocks(blocks), at))
}
