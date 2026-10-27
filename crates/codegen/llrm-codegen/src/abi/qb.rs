//! The QB-family runtime's call ABI, as selection asks it: each callee's
//! contract (stack cleanup, clobbers, register arguments), its linked name,
//! and the CPU target the backend lowers to. MIR calls carry typed
//! arguments only; no QB convention reaches MIR.

use std::collections::BTreeSet;
use std::fmt;

use llrm_lir::registers::RegId;

use crate::abi::runtime::{self, Contract, Control};
use crate::backend::assemble::Registers;
use crate::hir::model;
use crate::support::hash::IndexMap;
use crate::support::pyrepr::{self};

/// A typed call lacks enough measured ABI information to emit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbiError(pub String);

impl fmt::Display for AbiError {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AbiError {}

/// An inline block as a call: its declared registers are all it reads and
/// changes.
fn _inline_contract(
    asm: &model::Asm
) -> Result<(Contract, Vec<(runtime::Reg, u32)>, Vec<(runtime::Reg, u32)>), AbiError> {
    let registers = |names: &[String]| {
        names
            .iter()
            .map(|name| {
                crate::backend::inline_asm::view(name)
                    .ok_or_else(|| AbiError(format!("inline assembly names no register {}", pyrepr::string(name))))
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let (inputs, outputs, clobbers) = (registers(&asm.inputs)?, registers(&asm.outputs)?, registers(&asm.clobbers)?);
    // A register is its root whatever the view: a 32-bit one clobbers the
    // whole.
    let roots = |named: &[(runtime::Reg, u32)]| named.iter().map(|one| one.0).collect::<Vec<_>>();
    let (input_roots, output_roots, clobber_roots) = (roots(&inputs), roots(&outputs), roots(&clobbers));
    let memory = if asm.memory { runtime::Memory::Any } else { runtime::Memory::None };
    let contract = Contract {
        name: crate::hir::symbols::ASM.to_owned(),
        cleanup: Some(0),
        control: Control::Returns,
        enters_user_code: false,
        raises_error: false,
        error_handling: false,
        writes: memory,
        reads: memory,
        clobbers: clobber_roots.iter().chain(&output_roots).copied().collect(),
        established: true,
        evidence: "inline assembly: the inputs, outputs and clobbers it declares".to_owned(),
        documented: None,
        inputs: Some(input_roots.iter().copied().collect()),
        direct_inputs: None,
        clobbers_reached: true,
        caller_cleanup: 0,
        // It names no 32-bit register, so it keeps every high half.
        i386: false,
        direct_writes: None,
        flags_result: false,
        direct_reads: None,
    };
    Ok((contract, inputs, outputs))
}

/// The contract and registers of the inline block an intrinsic name spells,
/// as `llrm_mir::intrinsics::asm` reads it; none for another name.
pub fn asm_call(name: &str) -> Option<Result<(Contract, Registers), AbiError>> {
    let block = llrm_mir::intrinsics::asm(name)?;
    let asm = model::Asm {
        code: block.code.iter().map(|&byte| i64::from(byte)).collect(),
        inputs: block.inputs,
        outputs: block.outputs,
        clobbers: block.clobbers,
        memory: block.memory,
        fixups: Vec::new(),
    };
    Some(_inline_contract(&asm).and_then(|(contract, inputs, outputs)| {
        let machine = |names: Vec<(runtime::Reg, u32)>| {
            names
                .into_iter()
                .map(|(one, bits)| match one {
                    runtime::Reg::Ax
                    | runtime::Reg::Bx
                    | runtime::Reg::Cx
                    | runtime::Reg::Dx
                    | runtime::Reg::Si
                    | runtime::Reg::Di
                    | runtime::Reg::Sp
                    | runtime::Reg::Es => crate::backend::inline_asm::machine(one, bits)
                        .ok_or_else(|| AbiError(format!("inline assembly passes a value in {}", one.name()))),
                    other => Err(AbiError(format!("inline assembly passes a value in {}", other.name()))),
                })
                .collect::<Result<Vec<_>, _>>()
        };
        Ok((contract, Registers { arguments: machine(inputs)?, results: machine(outputs)? }))
    }))
}

pub fn _contract(
    name: &str,
    cleanup: model::StackCleanup,
    pushed: i64,
    family: model::RuntimeProfile,
) -> Result<Contract, AbiError> {
    _contract_keeping(name, true, cleanup, pushed, family, &BTreeSet::new())
}

/// `_contract`, a call no runtime contract describes keeping `preserved`.
fn _contract_keeping(
    name: &str,
    runtime_call: bool,
    cleanup: model::StackCleanup,
    pushed: i64,
    family: model::RuntimeProfile,
    preserved: &BTreeSet<runtime::Reg>,
) -> Result<Contract, AbiError> {
    // A function the program names is not a runtime entry whatever its
    // spelling: only the runtime's own calls (`RUNTIME`-prefixed) have a
    // contract, which the runtime tables and the entries' declarations give.
    if !runtime_call {
        return Ok(ordinary(name, cleanup, pushed, preserved));
    }
    let resume_label = name.starts_with("$QB$RESA:");
    let restore_label = name.starts_with("$QB$RSTB:");
    let physical_name = if resume_label {
        "B$RESA"
    } else if restore_label {
        "B$RSTB"
    } else {
        name
    };
    let found =
        runtime::per_call(&IndexMap::from_iter([(0, physical_name.to_owned())]), family.tables(), &BTreeSet::new())
            .swap_remove(&0)
            .expect("one call in, one contract out");
    let evidence = &found.evidence;
    // `replace(found, cleanup=..., control=..., enters_user_code=...,
    // established=True, inputs=frozenset(), i386=True, evidence=...)`.
    let refined =
        |found: &Contract, cleanup: i64, control: Control, enters_user_code: bool, evidence: String| Contract {
            cleanup: Some(cleanup),
            control,
            enters_user_code,
            established: true,
            inputs: Some(BTreeSet::new()),
            i386: true,
            evidence,
            ..found.clone()
        };
    let returns = |cleanup: i64, text: String| refined(&found, cleanup, Control::Returns, false, text);
    if runtime::audited_pop("statement", physical_name) == Some(pushed) {
        return Ok(returns(
            pushed,
            format!(
                "{evidence} Actual QB45 /A output supplies {pushed} stack bytes \
                 to {name}; the shipped runtime returns past that exact fixed block. \
                 All non-stack effects remain conservative."
            ),
        ));
    }
    if runtime::audited_pop("graphics", name) == Some(pushed) {
        return Ok(returns(
            pushed,
            format!(
                "{evidence} QB45 /A listings show {name} consuming exactly \
                 {pushed} typed stack bytes; the shipped runtime returns past the same block. \
                 All graphics-state, memory, error, and clobber effects remain conservative."
            ),
        ));
    }
    if runtime::audited_pop("string", name) == Some(pushed) {
        return Ok(returns(
            pushed,
            format!(
                "{evidence} Typed source supplies {pushed} stack bytes. \
                 BCOM45.LIB, BCL71ENR.LIB and VBDCL10E.LIB expose the same \
                 Pascal {name} normal-return cleanup; tools/libdump.py shows \
                 RETF {pushed} in each member. All non-stack effects remain conservative."
            ),
        ));
    }
    if resume_label && pushed == 0 {
        // QB45 rt/error.asm documents B$RESA's target in AX and its exit as a
        // transfer to that address after B$RES_SETUP/B$EXSA. VBDOS LOCERR.OBJ
        // and PDS71 PDLOCAL.OBJ independently spell `mov ax,offset target`
        // immediately before a far B$RESA call. The frontend inserts that AX
        // move only after final code labels exist, so it is not a semantic
        // stack argument here.
        return Ok(Contract {
            error_handling: true,
            ..refined(
                &found,
                0,
                Control::Never,
                true,
                "QB45 rt/error.asm B$RESA takes the resume offset in AX, calls \
                 B$RES_SETUP, unwinds with B$EXSA, and transfers through RESRET; \
                 measured VBDOS LOCERR.OBJ and PDS71 PDLOCAL.OBJ use the same \
                 MOV AX,offset / far-call shape."
                    .to_owned(),
            )
        });
    }
    if name == "B$RES0" && pushed == 0 {
        return Ok(Contract {
            error_handling: true,
            ..refined(
                &found,
                0,
                Control::Never,
                true,
                "QB45 rt/error.asm B$RES0 calls B$RES_SETUP, reloads b$erradr, \
                 unwinds with B$EXSA, and transfers through RESRET to the failing statement."
                    .to_owned(),
            )
        });
    }
    if name == "B$RDIM" || name == "B$RDMP" || name == "B$DDIM" {
        // B$ExitDim removes three header words and two words per dimension;
        // this is exactly the byte count represented by the typed operands.
        return Ok(returns(
            pushed,
            format!(
                "rt/dynamic.asm {name} reaches DIM_COMMON with stack parameters; \
                 B$ExitDim removes 6 + 4*rank bytes, represented exactly by this \
                 call site's typed operands. Other effects stay conservative."
            ),
        ));
    }
    if (name == "B$COLR" || name == "B$LOCT") && pushed >= 2 && pushed % 2 == 0 {
        return Ok(returns(
            pushed,
            format!(
                "QB45 runtime/rt/gwscr.asm documents a presence word and optional value for each \
                 positional argument, followed by their word count; this site supplies {pushed} \
                 bytes. B$ScCleanUpParms removes that complete count-led block; later runtimes retain it."
            ),
        ));
    }
    if name == "B$HARY" && pushed >= 4 && pushed % 2 == 0 {
        // The rank word plus exactly that many INTEGER subscripts are the
        // complete variable-sized stack block. The descriptor itself is a
        // fixed BX input and the result is ES:BX, established independently
        // by the PDS /Ah object probe and the shipped runtime listing.
        return Ok(Contract {
            inputs: Some(BTreeSet::from([runtime::Reg::Bx])),
            ..returns(
                pushed,
                format!(
                    "{evidence} Typed /Ah access supplies {} \
                     subscripts plus rank ({pushed} stack bytes) and a separate BX descriptor; \
                     PDS71 PDHUGE.OBJ 0047..0055 shows this exact shape and consumes the \
                     returned ES:BX immediately as the element address.",
                    pushed / 2 - 1
                ),
            )
        });
    }
    if name == "B$CLOS" {
        // CLOS walks a count followed by that many file words and restores SP
        // from the resulting cursor. The typed source site makes the otherwise
        // variable cleanup exact; all device/error/memory effects remain those
        // of the conservative measured contract.
        return Ok(returns(
            pushed,
            format!(
                "{evidence} Typed CLOSE site supplies its count and \
                 {} bytes of file numbers, so normal cleanup is {pushed}. \
                 All other effects remain conservative.",
                pushed - 2
            ),
        ));
    }
    if family == model::RuntimeProfile::Vbdos && (name == "B$STR4" || name == "B$STR8") {
        // The shared object raiser kept these conservative because it does not
        // model the mathpack's transitive stack paths. At a typed source site,
        // however, the wrapper's own RETF 4/8 and exact argument width
        // establish normal cleanup without making any stronger effect
        // claim.
        return Ok(returns(
            pushed,
            format!(
                "{evidence} VBDOS stringfp.asm wrapper ends RETF {pushed}; \
                 typed source supplies that exact stored floating argument. \
                 All memory, clobber and error effects remain conservative."
            ),
        ));
    }
    if let Some(text) = runtime::audited_pop_note(name, pushed) {
        return Ok(returns(pushed, format!("{evidence} {text}")));
    }
    if family == model::RuntimeProfile::Qb45 && name == "B$POKE" && pushed == 4 {
        return Ok(returns(
            4,
            format!(
                "{evidence} QB45 rt/peek.asm declares addr then val as two \
                 Pascal words; B$POKE ends through cEnd and normally removes all \
                 4 bytes. Memory and error effects remain conservative."
            ),
        ));
    }
    if name == "B$SERR" && pushed == 2 {
        return Ok(refined(
            &found,
            2,
            Control::Never,
            true,
            format!(
                "{evidence} QB45 rt/erproc.asm declares one word errnum, \
                 and measured VBDOS LOCERR.OBJ uses the identical one-word far \
                 call; B$SERR validates it and jumps to B$RUNERR. Its documented \
                 exit is either the installed handler or fatal termination, never \
                 the instruction after the call."
            ),
        ));
    }
    let print_width = |suffix: &str| match suffix {
        "I2" => Some(2),
        "I4" => Some(4),
        "SD" => Some(2),
        "R4" => Some(4),
        "R8" => Some(8),
        _ => None,
    };
    if name.len() == 6 && name.starts_with("B$P") && "CSE".contains(&name[3..4]) {
        let expected = print_width(&name[4..]);
        if expected == Some(pushed) {
            return Ok(returns(
                pushed,
                format!(
                    "{evidence} QB45 rt/prnval.asm and rt/prnvalfp.asm dispatch {name} \
                     to B$PRINT, whose typed epilogue removes the {pushed}-byte value."
                ),
            ));
        }
    }
    if (name == "B$CHOU" && pushed == 2) || (name == "B$PEOS" && pushed == 0) {
        return Ok(returns(
            pushed,
            format!(
                "{evidence} QB45 rt/pr0a.asm/prnval.asm establishes the typed {name} \
                 entry with {pushed} stack bytes; output and error effects remain conservative."
            ),
        ));
    }
    if ["B$RDI2", "B$RDI4", "B$RDR4", "B$RDR8", "B$RDCY"].contains(&name) && pushed == 4 {
        return Ok(returns(
            4,
            format!(
                "{evidence} QB45 rt/read.asm sends {name} through CommRead with one \
                 far destination pointer; its shared cEnd removes 4 bytes."
            ),
        ));
    }
    if matches!(name, "B$PCCY" | "B$PSCY" | "B$PECY" | "B$STCY") && pushed == 8 {
        return Ok(returns(
            8,
            format!(
                "{evidence} PDS 7.1 BC pushes a CURRENCY item as four words, [bx+6] first, then calls the entry; \
                 the entry is B$PER8's shape (type byte, terminator, B$PRINT), which pops the whole value. \
                 Device dispatch and error effects remain conservative."
            ),
        ));
    }
    if name == "B$CSCN" && (2..=18).contains(&pushed) && pushed % 2 == 0 {
        return Ok(returns(
            pushed,
            format!(
                "{evidence} SCREEN emits a count-led block like COLOR: a presence word (and the value when given) per \
                 argument, then their count; this site supplies {pushed} bytes. SYS.OBJ 0787..0792 shows the \
                 one-mode form as six bytes; the shared cleanup removes the whole block. Display, alias and error \
                 effects remain conservative."
            ),
        ));
    }
    if matches!(name, "B$FLOF" | "B$FLOC" | "B$FSEK") && pushed == 2 {
        return Ok(returns(
            2,
            format!(
                "{evidence} Typed LOF supplies the single file-number word; \
                 MAIN.OBJ 0ec4..0ecc shows that exact call shape and the runtime \
                 wrapper's normal return consumes it. Device/error effects remain conservative."
            ),
        ));
    }
    if found.established && found.cleanup.is_some() {
        return Ok(found);
    }
    if found.cleanup == Some(pushed) {
        // The object raiser may know only a conservative machine interface
        // for this runtime family even though it measured the normal RETF
        // cleanup. A typed source call establishes that all arguments are on
        // the stack, so refine only those two facts. Keep unknown control,
        // memory, callbacks, errors, and clobbers exactly as conservative as
        // the measured contract supplied them.
        return Ok(Contract {
            established: true,
            inputs: Some(BTreeSet::new()),
            i386: true,
            evidence: format!(
                "{evidence} QB typed source call supplies {pushed} stack bytes; \
                 no register inputs are used. All other effects are unchanged."
            ),
            ..found.clone()
        });
    }
    if runtime_call && name.starts_with("B$") {
        return Err(AbiError(format!("runtime call {name} has no complete stack-cleanup contract")));
    }
    Ok(ordinary(name, cleanup, pushed, preserved))
}

/// What a call of a function no runtime table describes is: stack-only, the
/// cleanup its verified call site has, the registers the calling convention
/// gives, memory conservative.
pub(crate) fn ordinary(
    name: &str,
    cleanup: model::StackCleanup,
    pushed: i64,
    preserved: &BTreeSet<runtime::Reg>,
) -> Contract {
    let caller = cleanup == model::StackCleanup::Caller;
    Contract {
        cleanup: Some(if caller { 0 } else { pushed }),
        caller_cleanup: if caller { pushed } else { 0 },
        control: Control::Returns,
        enters_user_code: false,
        established: true,
        inputs: Some(BTreeSet::new()),
        i386: true,
        evidence: "QB source ABI: stack-only far call; cleanup and argument widths \
                   come from verified HIR, clobbers from the calling convention, while \
                   memory remains conservative"
            .to_owned(),
        clobbers: runtime::EVERY.difference(preserved).copied().collect(),
        ..runtime::worst(name)
    }
}

/// The ABI of the MIR a HIR program emits: a runtime routine linked by its
/// own name and called by its contract, any other function by its
/// frontend's object name.
pub struct HirAbi {
    pub runtime: model::RuntimeProfile,
    /// Each function's object name, where it is not its HIR name.
    pub objects: std::collections::BTreeMap<String, String>,
    /// The registers a call no runtime contract describes keeps.
    pub preserved: BTreeSet<runtime::Reg>,
    /// The runtime's stack limit and overflow handler, where the program checks
    /// its stack.
    pub stack_check: Option<model::StackCheck>,
    /// The runtime is a host's (`RuntimePromises::hosted`).
    pub hosted: bool,
}

impl HirAbi {
    /// `program`'s calls: its runtime's, and its own by their symbols.
    pub fn of(program: &model::Program) -> Result<Self, String> {
        let functions = program.modules.iter().flat_map(|module| &module.functions);
        Ok(Self {
            runtime: program.runtime,
            objects: functions.filter_map(|one| Some((one.name.clone(), one.symbol.clone()?))).collect(),
            preserved: program.preserved.iter().map(|one| runtime::Reg::from_value(one)).collect::<Result<_, _>>()?,
            stack_check: program.stack_check.clone(),
            hosted: program.promises.hosted,
        })
    }
}

/// The stack-block routines a host's runtime defines as C varargs
/// (runtime/qb/posix/blockentries.c): called as BASIC pushes them, the caller
/// removes what it pushed.
pub const HOST_STACK_BLOCKS: [&str; 7] = ["B$DDIM", "B$RDIM", "B$RDMP", "B$CLOS", "B$COLR", "B$LOCT", "B$CSCN"];

/// A routine of llrm's runtime, called as the target calls any function: the
/// table's effects (what it reads and writes, whether it raises an error or
/// never returns), but not its Microsoft stack block or registers, which the
/// convention states. What the callee pops is `pops`; the stack bytes pushed
/// are `pushed`, past the registers.
fn native_contract(
    name: &str,
    pops: bool,
    pushed: i64,
    family: model::RuntimeProfile,
    hosted: bool,
) -> Contract {
    // A routine that keeps BASIC's pushes pops its whole block, however many
    // words the call pushes, so the words beyond its declaration are not
    // the caller's to remove.
    let pops = pops || (runtime::semantics::keeps_stack_abi(name) && !(hosted && HOST_STACK_BLOCKS.contains(&name)));
    let found = runtime::per_call(&IndexMap::from_iter([(0, name.to_owned())]), family.tables(), &BTreeSet::new())
        .swap_remove(&0)
        .expect("one call in, one contract out");
    Contract {
        cleanup: Some(if pops { pushed } else { 0 }),
        caller_cleanup: if pops { 0 } else { pushed },
        established: true,
        inputs: Some(BTreeSet::new()),
        flags_result: false,
        i386: true,
        evidence: format!("{} Called as the target's own convention calls a function.", found.evidence),
        ..found
    }
}

/// A runtime routine's register interface beside its stack block. B$HARY
/// takes the subscripts and their count pushed and the descriptor in BX,
/// and answers the element's address in ES:BX: PDS71 PDHUGE.OBJ 0047..0055
/// sets BX last and uses ES:BX at once.
pub fn registers(name: &str) -> Option<Registers> {
    match name {
        "B$HARY" => Some(Registers { arguments: vec![RegId::BX], results: vec![RegId::BX, RegId::ES] }),
        _ => None,
    }
}

impl crate::backend::assemble::Abi for HirAbi {
    fn stack_check(&self) -> Option<&model::StackCheck> {
        self.stack_check.as_ref()
    }

    fn registers(
        &self,
        callee: &str,
    ) -> Option<Registers> {
        if let Some(block) = asm_call(callee) {
            return block.ok().map(|(_, registers)| registers);
        }
        registers(callee.strip_prefix(crate::hir::mir::RUNTIME).unwrap_or(callee))
    }

    fn contract(
        &self,
        callee: &str,
        pops: bool,
        pushed: i64,
    ) -> Result<Contract, String> {
        if let Some(block) = asm_call(callee) {
            return block.map(|(contract, _)| contract).map_err(|error| error.0);
        }
        let cleanup = if pops { model::StackCleanup::Callee } else { model::StackCleanup::Caller };
        let name = callee.strip_prefix(crate::hir::mir::RUNTIME).unwrap_or(callee);
        let runtime_call = callee.starts_with(crate::hir::mir::RUNTIME);
        if self.runtime.calls_natively() && runtime_call {
            return Ok(native_contract(name, pops, pushed, self.runtime, self.hosted));
        }
        _contract_keeping(name, runtime_call, cleanup, pushed, self.runtime, &self.preserved).map_err(|error| error.0)
    }

    fn runtime_contracts(&self) -> bool {
        true
    }

    fn linked(
        &self,
        name: &str,
    ) -> String {
        match name.strip_prefix(crate::hir::mir::RUNTIME) {
            Some(routine) => routine.to_owned(),
            None => self.objects.get(name).cloned().unwrap_or_else(|| name.to_owned()),
        }
    }
}

/// A CPU's target as the backend lowers to it: a call keeps the registers
/// its callee's contract leaves, and a multiply by a constant costs its
/// cheapest chain, as `abi` and `arithmetic` lower them.
pub struct LoweredTarget {
    machine: std::rc::Rc<dyn llrm_mir::target::Machine>,
    abi: HirAbi,
    cpu: &'static crate::backend::cpu::Profile,
}

impl LoweredTarget {
    pub fn of(
        cpu: &'static crate::backend::cpu::Profile,
        abi: HirAbi,
    ) -> Self {
        Self { machine: cpu.target(), abi, cpu }
    }
}

impl llrm_mir::target::Machine for LoweredTarget {
    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        self.machine.spaces()
    }

    fn foreign_span(
        &self,
        selectors: (i64, i64),
        offsets: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)> {
        self.machine.foreign_span(selectors, offsets, width)
    }

    fn costs(&self) -> llrm_mir::target::OperationCosts {
        self.machine.costs()
    }

    fn size_costs(&self) -> llrm_mir::target::OperationCosts {
        self.machine.size_costs()
    }

    fn keeps_division(
        &self,
        bits: u32,
        size: bool,
    ) -> bool {
        self.machine.keeps_division(bits, size)
    }

    fn tti(&self) -> Option<std::rc::Rc<dyn llrm_mir::tti::Tti>> {
        self.machine.tti()
    }

    fn lowering(&self) -> Option<std::rc::Rc<dyn llrm_mir::lowering::Lowering>> {
        self.machine.lowering()
    }

    fn library(&self) -> Option<std::rc::Rc<dyn llrm_mir::libfunc::Library>> {
        self.machine.library()
    }

    fn registers(&self) -> i64 {
        self.machine.registers()
    }

    fn call_registers(&self) -> i64 {
        self.machine.call_registers()
    }

    fn float_registers(&self) -> i64 {
        self.machine.float_registers()
    }

    fn far_access_registers(&self) -> i64 {
        self.machine.far_access_registers()
    }

    fn segment_registers(&self) -> i64 {
        self.machine.segment_registers()
    }

    fn two_address(&self) -> bool {
        self.machine.two_address()
    }

    fn private_convention(&self) -> Option<llrm_mir::target::PrivateConvention> {
        self.machine.private_convention()
    }

    fn callee_pop(
        &self,
        convention: u32,
    ) -> Option<u32> {
        self.machine.callee_pop(convention)
    }

    fn stack_argument_bytes(
        &self,
        convention: u32,
        arguments: &[llrm_mir::target::Argument],
    ) -> Option<i64> {
        self.machine.stack_argument_bytes(convention, arguments)
    }

    fn address_registers(&self) -> i64 {
        self.machine.address_registers()
    }

    fn kept_across(
        &self,
        callee: Option<&str>,
    ) -> i64 {
        use crate::backend::assemble::Abi;
        match callee.map(|name| self.abi.contract(name, false, 0)) {
            Some(Ok(contract)) => crate::backend::callregs::call_keeps(&contract, &self.cpu.general).len() as i64,
            _ => self.machine.call_registers(),
        }
    }

    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        self.machine.address_forms()
    }

    fn huge_window(&self) -> Option<(u32, i64)> {
        self.machine.huge_window()
    }

    fn multiply_by(
        &self,
        factor: i64,
    ) -> i64 {
        use crate::backend::arithmetic;
        let multiply =
            arithmetic::immediate_multiply(self.cpu, factor).unwrap_or_else(|_| self.machine.costs().multiply);
        match arithmetic::cheapest_chain(factor, self.cpu) {
            Ok(Some((_, clocks))) => clocks.min(multiply),
            _ => multiply,
        }
    }

    fn load_may_trap(
        &self,
        width: u64,
        align: u64,
    ) -> bool {
        self.machine.load_may_trap(width, align)
    }

    fn port_touches_memory(
        &self,
        ports: (i64, i64),
    ) -> bool {
        self.machine.port_touches_memory(ports)
    }
}
