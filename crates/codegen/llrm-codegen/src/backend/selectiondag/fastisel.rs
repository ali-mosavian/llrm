//! `FastISel` for x86 at -O0: the pin's `-fast-isel` path, which selects an
//! IR block straight to machine nodes without the combiner, the legalizers or
//! the matcher.
//!
//! Off unless `LLRM_FAST_ISEL=1`, and then only at -O0 (`!cpu.optimized`); -O1
//! and up never see it. A function it cannot take is selected by the DAG path,
//! whole (DEVIATE F2).
//!
//! # See also
//! `docs`: `/home/alim/scratch/handoff/o0-fastisel-poc-plan-2026-11-06.md`.

use std::cell::Cell;

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{BlockId, Function, Module, Operand, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;

use super::builder::{BlockDag, BuilderTarget, fresh_dag};
use super::dag::{NodeData, SDValue, SelectionDAG};
use super::isd::ISD;
use super::mvt::EVT;
use super::function_lowering::FunctionLoweringInfo;
use crate::backend::isel::Convention;

thread_local! {
    static TOOK: Cell<bool> = const { Cell::new(false) };
}

/// Whether `LLRM_FAST_ISEL=1` asks for the fast path.
pub fn enabled() -> bool {
    llrm_support::env_var("LLRM_FAST_ISEL").is_some_and(|value| value == "1")
}

/// Records whether the function being selected goes through FastISel.
pub fn note(fast: bool) {
    TOOK.with(|took| took.set(fast));
}

/// The function just selected went through FastISel: read once by the caller
/// that records the selector.
pub fn take_took() -> bool {
    TOOK.with(|took| took.replace(false))
}

/// Logs why FastISel left a whole function to the DAG (`LLRM_DEBUG=isel`).
fn declined_all(why: &str) -> Vec<Option<BlockDag>> {
    llrm_support::debug!("isel", "fast declined: {why}");
    Vec::new()
}

/// A 32-bit integer, or at m32 a pointer in the default space (the same register).
fn word(types: &llrm_mir::types::Types, ty: llrm_mir::types::TypeId, m32: bool) -> bool {
    types.int_bits(ty) == Some(32) || (m32 && matches!(types.get(ty), llrm_mir::types::Type::Pointer(0)))
}

/// Whether a value of `ty` lives in a 64-bit register (`Some(true)`), a 32-bit one
/// (`Some(false)`), or is not a type FastISel holds: `i32`; at m64 also `i64` and a pointer.
fn wide_kind(types: &llrm_mir::types::Types, ty: llrm_mir::types::TypeId, m32: bool) -> Option<bool> {
    match (types.int_bits(ty), types.get(ty)) {
        (Some(32), _) => Some(false),
        (Some(64), _) if !m32 => Some(true),
        (_, llrm_mir::types::Type::Pointer(0)) => Some(!m32),
        _ => None,
    }
}

/// FastISel's block for each block of `function` in layout order: a block it cannot take is
/// none, and the DAG path builds it. A function it cannot take at all is an empty list.
// DEVIATE F1: hand-written subset; exit 2026-11-27 (a generator over X86GenFastISel.inc).
// DEVIATE F2: fallback by block, not by instruction (the pin sends the failing instruction and
// the ones before it); exit 2026-11-30 if blocks that decline stay over 10% of the blocks.
// DEVIATE F3: no FP, vector, atomics or intrinsics; exit after parity.
#[allow(clippy::too_many_arguments)]
pub fn build_all(
    module: &Module,
    function: &Function,
    target: &BuilderTarget,
    opcode: &dyn Fn(&str) -> Option<u16>,
    data_layout: &llrm_mir::datalayout::DataLayout,
    convention: &Convention,
    arch: &dyn llrm_target::Target,
    info: &mut FunctionLoweringInfo,
) -> Vec<Option<BlockDag>> {
    let m32 = arch.object().bitness == 32;
    if !(m32 || arch.object().bitness == 64) || module.signature(function.ty).2 {
        return declined_all("a variadic function or another width");
    }
    // every parameter is a 4-byte stack cell: fixed object -(n + 1)
    let int32 = |ty| wide_kind(&module.context.types, ty, m32).is_some();
    let (returns, parameters, _) = module.signature(function.ty);
    // m32: a 4-byte stack cell; m64: one register
    let arrives = |one: &crate::backend::isel::Parameter| match one {
        crate::backend::isel::Parameter::Cell(_) => m32,
        crate::backend::isel::Parameter::Registers(regs) => !m32 && regs.len() == 1,
        _ => false,
    };
    if parameters.iter().any(|&ty| !int32(ty))
        || convention.parameters.len() != parameters.len()
        || !convention.parameters.iter().all(arrives)
    {
        return declined_all("a parameter that is not a 4-byte stack cell or one register");
    }
    let arg_regs: Vec<(llrm_lir::registers::RegId, u32)> = match m32 {
        true => Vec::new(),
        false => convention
            .parameters
            .iter()
            .filter_map(|one| match one {
                crate::backend::isel::Parameter::Registers(regs) => Some((regs[0], info.new_reg())),
                _ => None,
            })
            .collect(),
    };
    let returns_int = int32(returns);
    if !returns_int && !module.context.types.is_void(returns) {
        return declined_all("a result that is not an int, a pointer or void");
    }
    let return_bytes = if wide_kind(&module.context.types, returns, m32) == Some(true) { 8 } else { 4 };
    let eax = (returns_int && convention.returns.len() == 1).then(|| arch.registers().named(convention.returns[0], return_bytes));
    let view = |name: &str, bytes| arch.registers().named(arch.register_named(name), bytes);
    let sysv: Vec<[llrm_lir::registers::RegId; 2]> =
        [("edi", "rdi"), ("esi", "rsi"), ("edx", "rdx"), ("ecx", "rcx"), ("r8d", "r8"), ("r9d", "r9")]
            .iter()
            .map(|&(narrow, wide)| [view(narrow, 4), view(wide, 8)])
            .collect();
    let al = view("al", 1);
    let mut uses: llrm_mir::dense::IdMap<ValueId, u32> = Default::default();
    for block in function.layout() {
        for id in function.iter_block(*block) {
            for operand in &function.instruction(id).operands {
                if let Operand::Value(value) = operand {
                    let seen = uses.get(value).copied().unwrap_or(0);
                    uses.insert(*value, seen + 1);
                }
            }
        }
    }
    let Some(names) = Names::of(opcode, if m32 { llrm_dagisel::fastisel::Table::I486 } else { llrm_dagisel::fastisel::Table::X64 }) else { return Vec::new() };
    let mut blocks = Vec::new();
    let layout = function.layout();
    for (at, &block) in layout.iter().enumerate() {
        let next = layout.get(at + 1).copied();
        let dag = fresh_dag(module, function, target, info, block, None);
        let chain = dag.entry_node();
        let mut declined = String::new();
        let dag = Fast { layout: data_layout, module, function, info: &mut *info, names: &names, eax, any_eax: arch.registers().named(arch.register_named("eax"), 4), esp: arch.stack_pointer(), arg_regs: &arg_regs, entry: layout[0] == block, sysv: &sysv, al, edx: arch.registers().named(arch.register_named("edx"), 4), rax: view("rax", 8), rdx: view("rdx", 8), m32, dag, chain, values: Default::default(), next, uses: &uses, folded: None, exports: Vec::new(), phi_updates: Vec::new() }
            .block(block, &mut declined);
        let Some((dag, phi_updates)) = dag else {
            llrm_support::debug!("isel", "fast declined at {declined}");
            blocks.push(None);
            continue;
        };
        blocks.push(Some(BlockDag {
            dag,
            phi_updates,
            live_ins: if at == 0 { arg_regs.clone() } else { Vec::new() },
            extra: Vec::new(),
            tail_calls: Vec::new(),
            values: Vec::new(),
        }));
    }
    // A fast entry block does not lower the arguments the way the DAG's does (no `value_regs` for
    // them): a block left to the DAG that reads one needs the DAG's entry.
    let reads_argument = |block: &BlockId| {
        function.iter_block(*block).any(|id| {
            function.instruction(id).operands.iter().any(|operand| match operand {
                Operand::Value(value) => matches!(function.value(*value).def, ValueDef::Argument(_)),
                _ => false,
            })
        })
    };
    if blocks.first().is_some_and(Option::is_some)
        && blocks.iter().zip(layout).skip(1).any(|(built, block)| built.is_none() && reads_argument(block))
    {
        blocks[0] = None;
    }
    blocks
}

/// The forms that read their second operand from memory (a folded load): `add`, `sub`,
/// `and`, `or`, `xor`, then `cmp` and `imul`.
struct Mem {
    alu: [u16; 5],
    cmp: u16,
    imul: u16,
}

/// The 64-bit forms of the loads, stores, ALU, compare and multiply (m64 only).
struct Wide {
    mov_rm: u16,
    mov_mr: u16,
    mov_mi: u16,
    mov_ri: u16,
    /// `movabs r64, imm64`, for a constant that is not a sign-extended `imm32`.
    movabs: u16,
    alu: [(u16, u16); 5],
    cmp_rr: u16,
    cmp_ri: u16,
    imul: u16,
    shl_ri: u16,
    shr_ri: u16,
    sar_ri: u16,
    imul_ri: u16,
    mem: Mem,
}

/// The machine opcodes FastISel emits.
struct Names {
    wide: Option<Wide>,
    mem: Mem,
    /// m64: `movsxd r64, m32`.
    movsx64_rm: Option<u16>,
    mov_rm: u16,
    mov_mr: u16,
    mov_mi: u16,
    mov_ri: u16,
    ret: u16,
    lea: Option<u16>,
    jmp: u16,
    jcc: u16,
    cmp_rr: u16,
    cmp_ri: u16,
    imul: u16,
    shl_ri: u16,
    shr_ri: u16,
    sar_ri: u16,
    imul_ri: u16,
    /// `setcc` and the `i1` to `i32` widening: `and r8, 1`, `movzx`.
    setcc: u16,
    and8: u16,
    movzx8: u16,
    /// m64 only: `movsxd`, and the 64-bit `lea` that adds a scaled index to a base.
    movsx64: Option<u16>,
    lea64: Option<u16>,
    /// `add`, `sub`, `and`, `or`, `xor` with a register and with an immediate.
    alu: [(u16, u16); 5],
    /// m32 only so far: calls and the signed divide.
    calls: Option<Calls>,
    divide: Option<Divide>,
    /// `SUBREG_TO_REG` and `EXTRACT_SUBREG`: widening and narrowing between `r32` and `r64`.
    subreg: Option<(u16, u16)>,
    /// m64: `ADJCALLSTACKDOWN64`, `ADJCALLSTACKUP64`, `CALL64pcrel32`, `MOV8ri`.
    calls64: Option<(u16, u16, u16, u16)>,
}

struct Calls {
    call_down: u16,
    call_up: u16,
    call: u16,
}

/// `cdq`, `idiv r`, `div r`.
struct Divide {
    cdq: u16,
    idiv: u16,
    div: u16,
    /// m64: `cqo`, `idiv r64`, `div r64`.
    wide: Option<(u16, u16, u16)>,
}

impl Names {
    /// The opcode set of `table`, looked up once per process: the by-name search costs about 4 M
    /// instructions, which on every function made a FastISel compile dearer than the DAG's.
    fn of(opcode: &dyn Fn(&str) -> Option<u16>, table: llrm_dagisel::fastisel::Table) -> Option<&'static Names> {
        static M32: std::sync::OnceLock<Option<Names>> = std::sync::OnceLock::new();
        static M64: std::sync::OnceLock<Option<Names>> = std::sync::OnceLock::new();
        let cell = if matches!(table, llrm_dagisel::fastisel::Table::I486) { &M32 } else { &M64 };
        cell.get_or_init(|| Names::new(opcode, table)).as_ref()
    }

    fn new(opcode: &dyn Fn(&str) -> Option<u16>, table: llrm_dagisel::fastisel::Table) -> Option<Names> {
        llrm_support::debug!("isel", "fast names built");
        // The pin's own rows (`fastEmit_ISD_<OP>_MVT_i32_rr` / `_ri`) name the opcodes.
        let pair = |one: &str| {
            let row = |shape| llrm_dagisel::fastisel::lookup(table, "ISD", one, "i32", "", shape).map(|(name, _)| name);
            Some((opcode(row(1)?)?, opcode(row(2)?)?))
        };
        let mem = |width: &str| {
            let one = |name: &str| opcode(&format!("{name}{width}rm"));
            Some(Mem { alu: [one("ADD")?, one("SUB")?, one("AND")?, one("OR")?, one("XOR")?], cmp: one("CMP")?, imul: one("IMUL")? })
        };
        let wide = (|| {
            let both = |one: &str| Some((opcode(&format!("{one}64rr"))?, opcode(&format!("{one}64ri32"))?));
            Some(Wide {
                mov_rm: opcode("MOV64rm")?,
                mov_mr: opcode("MOV64mr")?,
                mov_mi: opcode("MOV64mi32")?,
                mov_ri: opcode("MOV64ri32")?,
                movabs: opcode("MOV64ri")?,
                alu: [both("ADD")?, both("SUB")?, both("AND")?, both("OR")?, both("XOR")?],
                cmp_rr: opcode("CMP64rr")?,
                cmp_ri: opcode("CMP64ri32")?,
                imul: opcode("IMUL64rr")?,
                shl_ri: opcode("SHL64ri")?,
                shr_ri: opcode("SHR64ri")?,
                sar_ri: opcode("SAR64ri")?,
                imul_ri: opcode("IMUL64rri32")?,
                mem: mem("64")?,
            })
        })();
        Some(Names {
            wide,
            mem: mem("32")?,
            movsx64_rm: opcode("MOVSX64rm32"),
            mov_rm: opcode("MOV32rm")?,
            mov_mr: opcode("MOV32mr")?,
            mov_mi: opcode("MOV32mi")?,
            mov_ri: opcode("MOV32ri")?,
            ret: opcode("RET")?,
            lea: opcode("LEA32r"),
            jmp: opcode("JMP_1")?,
            jcc: opcode("JCC_1")?,
            cmp_rr: opcode("CMP32rr")?,
            cmp_ri: opcode("CMP32ri")?,
            imul: opcode("IMUL32rr")?,
            shl_ri: opcode("SHL32ri")?,
            shr_ri: opcode("SHR32ri")?,
            sar_ri: opcode("SAR32ri")?,
            imul_ri: opcode("IMUL32rri")?,
            setcc: opcode("SETCCr")?,
            and8: opcode("AND8ri")?,
            movzx8: opcode("MOVZX32rr8")?,
            movsx64: opcode("MOVSX64rr32"),
            lea64: opcode("LEA64r"),
            alu: [pair("ADD")?, pair("SUB")?, pair("AND")?, pair("OR")?, pair("XOR")?],
            calls: (|| {
                Some(Calls {
                    call_down: opcode("ADJCALLSTACKDOWN32")?,
                    call_up: opcode("ADJCALLSTACKUP32")?,
                    call: opcode("CALLpcrel32")?,
                })
            })(),
            calls64: (|| Some((opcode("ADJCALLSTACKDOWN64")?, opcode("ADJCALLSTACKUP64")?, opcode("CALL64pcrel32")?, opcode("MOV8ri")?)))(),
            divide: (|| {
                Some(Divide {
                    cdq: opcode("CDQ")?,
                    idiv: opcode("IDIV32r")?,
                    div: opcode("DIV32r")?,
                    wide: (|| Some((opcode("CQO")?, opcode("IDIV64r")?, opcode("DIV64r")?)))(),
                })
            })(),
            subreg: (|| Some((opcode("SUBREG_TO_REG")?, opcode("EXTRACT_SUBREG")?)))(),
        })
    }
}

/// What a call calls.
enum Callee {
    Global(llrm_mir::context::GlobalId),
    /// A C library function a FastISel intrinsic stands for.
    Library(&'static str),
}

struct Fast<'a> {
    layout: &'a llrm_mir::datalayout::DataLayout,
    module: &'a Module,
    function: &'a Function,
    info: &'a mut FunctionLoweringInfo,
    names: &'a Names,
    eax: Option<llrm_lir::registers::RegId>,
    any_eax: llrm_lir::registers::RegId,
    esp: llrm_lir::registers::RegId,
    /// m64: the register each parameter arrives in and the virtual register it is copied to.
    arg_regs: &'a [(llrm_lir::registers::RegId, u32)],
    entry: bool,
    /// m64: the argument registers (`edi`.. as 4-byte, `rdi`.. as 8-byte views), and `al`.
    sysv: &'a [[llrm_lir::registers::RegId; 2]],
    al: llrm_lir::registers::RegId,
    edx: llrm_lir::registers::RegId,
    rax: llrm_lir::registers::RegId,
    rdx: llrm_lir::registers::RegId,
    /// Frame objects are addressed at m32 only so far (F3).
    m32: bool,
    dag: SelectionDAG,
    /// The memory chain: every load and store follows the one before.
    chain: SDValue,
    values: llrm_mir::dense::IdMap<ValueId, SDValue>,
    next: Option<BlockId>,
    /// How many times each value is used in the function.
    uses: &'a llrm_mir::dense::IdMap<ValueId, u32>,
    /// A load not emitted because the instruction right after it reads it from memory.
    folded: Option<(ValueId, Vec<SDValue>)>,
    /// The values of this block another block reads, whose virtual register is not yet written.
    exports: Vec<ValueId>,
    /// The phis of the blocks this one branches to and the register each is given its value in.
    phi_updates: Vec<(ValueId, u32)>,
}

impl Fast<'_> {
    fn node(&mut self, op: u16, vts: &[EVT], ops: &[SDValue]) -> SDValue {
        self.dag.intern(ISD::Machine(u32::from(op)), vts, ops, Default::default(), NodeData::None)
    }

    /// Whether `operand` is 64-bit (see `wide_kind`).
    fn is_wide(&self, operand: Operand) -> Option<bool> {
        let ty = self.function.operand_type(&self.module.context, operand)?;
        wide_kind(&self.module.context.types, ty, self.m32)
    }

    /// The opcode set and register type for 32-bit (`false`) or 64-bit values.
    fn set(&self, wide: bool) -> Option<(u16, u16, u16, u16, [(u16, u16); 5], u16, u16, u16, EVT)> {
        let n = self.names;
        Some(match wide {
            false => (n.mov_rm, n.mov_mr, n.mov_mi, n.mov_ri, n.alu, n.cmp_rr, n.cmp_ri, n.imul, EVT::I32),
            true => {
                let w = n.wide.as_ref()?;
                (w.mov_rm, w.mov_mr, w.mov_mi, w.mov_ri, w.alu, w.cmp_rr, w.cmp_ri, w.imul, EVT::I64)
            }
        })
    }

    fn int(&self, operand: Operand) -> Option<i128> {
        let Operand::Constant(id) = operand else { return None };
        match self.module.context.get(id).kind {
            ConstantKind::Int(value) => i32::try_from(value).ok().map(i128::from).or_else(|| u32::try_from(value).ok().map(|v| i128::from(v as i32))),
            _ => None,
        }
    }

    /// A constant that fits a sign-extended 32-bit immediate (what the 64-bit forms take).
    fn int_signed(&self, operand: Operand) -> Option<i128> {
        let Operand::Constant(id) = operand else { return None };
        match self.module.context.get(id).kind {
            ConstantKind::Int(value) => i32::try_from(value).ok().map(i128::from),
            _ => None,
        }
    }

    /// `[base, scale, index, disp, segment]` of a frame object, or of the pointer in a register.
    fn address(&mut self, pointer: Operand) -> Option<Vec<SDValue>> {
        if let Operand::Value(value) = pointer
            && let Some(&object) = self.info.static_allocas.get(&value)
        {
            let ValueDef::Instruction(def) = self.function.value(value).def else { return None };
            let Opcode::Alloca { allocated, .. } = self.function.instruction(def).opcode else { return None };
            if wide_kind(&self.module.context.types, allocated, self.m32).is_none() {
                return None;
            }
            return Some(self.frame(object));
        }
        let base = self.value(pointer)?;
        let pointer = if self.m32 { EVT::I32 } else { EVT::I64 };
        Some(vec![
            base,
            self.dag.get_constant_with(1, EVT::I8, true, false),
            self.dag.get_register(pointer, 0),
            self.dag.get_constant_with(0, EVT::I32, true, false),
            self.dag.get_register(EVT::I16, 0),
        ])
    }

    /// The byte register an `i1` lives in: only a `setcc` computed in this block.
    fn value_byte(&self, operand: Operand) -> Option<SDValue> {
        let Operand::Value(value) = operand else { return None };
        self.values.get(&value).copied()
    }

    /// Whether `user` reads the load `result` (of width `wide`) from memory, as the pin's
    /// `tryToFoldLoad` folds a one-use load into the instruction right after it.
    fn folds(&self, user: &llrm_mir::module::Instruction, result: ValueId, wide: bool) -> bool {
        use llrm_mir::opcode::BinaryOp::*;
        if self.uses.get(&result).copied() != Some(1) {
            return false;
        }
        let second = |this: &Self| user.operands.get(1) == Some(&Operand::Value(result)) && matches!(user.operands[0], Operand::Value(_)) && this.is_wide(user.operands[0]) == Some(wide);
        match &user.opcode {
            Opcode::Binary(Add | Sub | And | Or | Xor | Mul) => second(self) && wide_kind(&self.module.context.types, user.ty, self.m32) == Some(wide),
            Opcode::ICmp(_) => second(self),
            Opcode::Cast(llrm_mir::opcode::CastOp::SExt) => {
                !self.m32 && !wide && user.operands[0] == Operand::Value(result) && self.module.context.types.int_bits(user.ty) == Some(64)
            }
            _ => false,
        }
    }

    /// Whether `global` names an `llvm.*` intrinsic: the DAG expands it, a call to it would name a symbol that does not exist.
    /// A call to an intrinsic `FastISel` selects to nothing (permalink: FastISel.cpp `selectIntrinsicCall`).
    fn is_noop_intrinsic(&self, instruction: &llrm_mir::module::Instruction) -> bool {
        const NOOPS: [&str; 6] =
            ["llvm.lifetime.start", "llvm.lifetime.end", "llvm.donothing", "llvm.sideeffect", "llvm.assume", "llvm.experimental.noalias.scope.decl"];
        let Some(Operand::Constant(callee)) = instruction.operands.last() else { return false };
        let ConstantKind::Global(global) = self.module.context.get(*callee).kind else { return false };
        instruction.result.is_none()
            && self.module.global(global).name.as_deref().is_some_and(|name| NOOPS.iter().any(|noop| name.starts_with(noop)))
    }

    /// Whether conventions `a` and `b` pass integer arguments alike. At m32 `cdecl` and `sysvcc` do: on the
    /// stack, the caller popping. The C library routines the `memcpy` expansion calls are `sysvcc`, their declarations `cdecl`.
    fn alike(&self, a: u32, b: u32) -> bool {
        use llrm_mir::opcode::SYSV;
        a == b || (self.m32 && matches!(a, 0 | SYSV) && matches!(b, 0 | SYSV))
    }

    /// A `memcpy` or `memset` intrinsic call as the library call FastISel makes of it, with the
    /// operands passed: not volatile (permalink: X86FastISel.cpp `fastLowerIntrinsicCall`, `lowerCallTo(II, "memcpy", ...)`).
    /// `memmove` is not here: the pin hands it to the DAG.
    fn mem_libcall(&self, instruction: &llrm_mir::module::Instruction) -> Option<(&'static str, Vec<Operand>)> {
        let (&Operand::Constant(callee), arguments) = instruction.operands.split_last()? else { return None };
        let ConstantKind::Global(global) = self.module.context.get(callee).kind else { return None };
        let name = self.module.global(global).name.as_deref()?;
        let libcall = if name.starts_with("llvm.memcpy.p") {
            "memcpy"
        } else if name.starts_with("llvm.memset.p") {
            "memset"
        } else {
            return None;
        };
        let [first, second, length, volatile] = arguments[..] else { return None };
        (self.int(volatile) == Some(0)).then(|| (libcall, vec![first, second, length]))
    }

    /// An `i8` constant as the 32-bit register a C library call takes it in (`memset`'s fill byte).
    fn narrow_constant(&mut self, operand: Operand) -> Option<SDValue> {
        let ty = self.function.operand_type(&self.module.context, operand)?;
        let Operand::Constant(id) = operand else { return None };
        let ConstantKind::Int(value) = self.module.context.get(id).kind else { return None };
        (self.module.context.types.int_bits(ty) == Some(8)).then_some(())?;
        let imm = self.dag.get_constant_with(value as u8 as u128, EVT::I32, true, false);
        Some(self.node(self.names.mov_ri, &[EVT::I32], &[imm]))
    }

    /// What a call calls, the operands passed, its result type (none: void) and whether it is
    /// variadic: a function in this module with the caller's convention, or a library call
    /// standing for an intrinsic. At most `limit` operands.
    fn callee(&self, instruction: &llrm_mir::module::Instruction, call: &llrm_mir::opcode::CallInfo, limit: usize) -> Option<(Vec<Operand>, Callee, Option<llrm_mir::types::TypeId>, bool)> {
        if let Some((name, arguments)) = self.mem_libcall(instruction) {
            return (arguments.len() <= limit).then_some((arguments, Callee::Library(name), None, false));
        }
        let (callee, arguments) = instruction.operands.split_last()?;
        let Operand::Constant(callee) = *callee else { return None };
        let ConstantKind::Global(global) = self.module.context.get(callee).kind else { return None };
        let (returns, parameters, variadic) = self.module.signature(call.function_type);
        // an argument that changes how it is passed (a struct by value is a copy on the stack, not the pointer)
        let by_abi = |attribute: &llrm_mir::opcode::Attribute| match attribute {
            llrm_mir::opcode::Attribute::Type(name, _) => ["sret", "byval", "byref", "inalloca", "preallocated"].iter().any(|one| name == *one),
            llrm_mir::opcode::Attribute::Flag(name) => ["inreg", "nest", "returned", "swiftself"].iter().any(|one| name == *one),
            _ => false,
        };
        if call.argument_attrs.iter().flatten().any(by_abi) || call.return_attrs.iter().any(by_abi) {
            return None;
        }
        if arguments.len() < parameters.len()
            || arguments.len() > limit
            || (!variadic && arguments.len() != parameters.len())
            || self.is_intrinsic(global)
            || self.module.global(global).function().is_none_or(|callee| !self.alike(callee.calling_convention, call.calling_convention))
        {
            return None;
        }
        Some((arguments.to_vec(), Callee::Global(global), Some(returns), variadic))
    }

    fn is_intrinsic(&self, global: llrm_mir::context::GlobalId) -> bool {
        self.module.global(global).name.intrinsic_id().is_some()
            || self.module.global(global).name.as_deref().is_some_and(|name| name.starts_with("llvm."))
    }

    /// The address of the folded load that `operand` is, if it is one.
    fn take_fold(&mut self, operand: Operand) -> Option<Vec<SDValue>> {
        let Operand::Value(value) = operand else { return None };
        if self.folded.as_ref().is_some_and(|(held, _)| *held == value) { self.folded.take().map(|(_, ops)| ops) } else { None }
    }

    fn eax_reg(&self) -> Option<llrm_lir::registers::RegId> {
        Some(self.any_eax)
    }

    fn frame(&mut self, object: i32) -> Vec<SDValue> {
        let pointer = if self.m32 { EVT::I32 } else { EVT::I64 };
        vec![
            self.dag.get_target_frame_index(pointer, object),
            self.dag.get_constant_with(1, EVT::I8, true, false),
            self.dag.get_register(pointer, 0),
            self.dag.get_constant_with(0, EVT::I32, true, false),
            self.dag.get_register(EVT::I16, 0),
        ]
    }

    /// The value of `operand` in a register: a computed one, a constant, or a parameter loaded from its cell.
    fn value(&mut self, operand: Operand) -> Option<SDValue> {
        match operand {
            Operand::Constant(id) if !self.m32 && matches!(self.module.context.get(id).kind, ConstantKind::Global(_)) => {
                // the address of a global: `lea r, [rip + g]`
                let ConstantKind::Global(global) = self.module.context.get(id).kind else { return None };
                let target = self.dag.get_target_global_address(EVT::I64, global.0, 0);
                let rip = self.dag.get_phys_register(EVT::I64, llrm_lir::registers::RegId::RIP);
                let one = self.dag.get_constant_with(1, EVT::I8, true, false);
                let noreg = self.dag.get_register(EVT::I64, 0);
                let segment = self.dag.get_register(EVT::I16, 0);
                Some(self.node(self.names.lea64?, &[EVT::I64], &[rip, one, noreg, target, segment]))
            }
            Operand::Constant(_) => {
                // DEVIATE F4: the pin (X86MaterializeInt) makes 0 with `MOV32r0` (xor) and a 64-bit
                // constant with `getMOVriOpcode`; here every constant is `MOV32ri`. Exit 2026-11-27 with F1.
                let wide = self.is_wide(operand)?;
                let (_, _, _, mov_ri, _, _, _, _, vt) = self.set(wide)?;
                if wide && self.int_signed(operand).is_none() {
                    // `X86MaterializeInt`: a 64-bit constant past `imm32` is `movabs`
                    let Operand::Constant(id) = operand else { return None };
                    let ConstantKind::Int(value) = self.module.context.get(id).kind else { return None };
                    let imm = self.dag.get_constant_with(value as u64 as u128, EVT::I64, true, false);
                    return Some(self.node(self.names.wide.as_ref()?.movabs, &[EVT::I64], &[imm]));
                }
                let value = if wide { self.int_signed(operand)? } else { self.int(operand)? };
                let imm = self.dag.get_constant_with(value as u128, vt, true, false);
                Some(self.node(mov_ri, &[vt], &[imm]))
            }
            Operand::Value(value) => {
                if let Some(&done) = self.values.get(&value) {
                    return Some(done);
                }
                if let Some(&reg) = self.info.value_regs.get(&value)
                    && matches!(self.function.value(value).def, ValueDef::Instruction(_))
                {
                    // defined in another block: read back from its virtual register
                    let vt = if self.is_wide(Operand::Value(value))? { EVT::I64 } else { EVT::I32 };
                    let copy = self.dag.get_copy_from_reg(self.chain, reg, vt);
                    self.chain = SDValue { node: copy.node, res: 1 };
                    let copy = SDValue { node: copy.node, res: 0 };
                    self.values.insert(value, copy);
                    return Some(copy);
                }
                if let Some(&object) = self.info.static_allocas.get(&value) {
                    // the address of a frame object: `lea r, [frame + 0]`
                    let (lea, vt) = if self.m32 { (self.names.lea?, EVT::I32) } else { (self.names.lea64?, EVT::I64) };
                    let ops = self.frame(object);
                    let address = self.node(lea, &[vt], &ops);
                    self.values.insert(value, address);
                    return Some(address);
                }
                let ValueDef::Argument(n) = self.function.value(value).def else { return None };
                if !self.m32 {
                    if !self.entry {
                        return None;
                    }
                    let (_, vreg) = *self.arg_regs.get(n as usize)?;
                    let vt = if self.is_wide(Operand::Value(value))? { EVT::I64 } else { EVT::I32 };
                    let copy = self.dag.get_copy_from_reg(self.chain, vreg, vt);
                    self.chain = SDValue { node: copy.node, res: 1 };
                    let copy = SDValue { node: copy.node, res: 0 };
                    self.values.insert(value, copy);
                    return Some(copy);
                }
                let mut ops = self.frame(-(n as i32) - 1);
                ops.push(self.chain);
                let load = self.node(self.names.mov_rm, &[EVT::I32, EVT::OTHER], &ops);
                self.chain = SDValue { node: load.node, res: 1 };
                self.values.insert(value, load);
                Some(load)
            }
            Operand::Block(_) => None,
        }
    }

    /// Writes the virtual register of each value of this block another block reads, once it is computed.
    fn export_ready(&mut self) -> Option<()> {
        let mut at = 0;
        while at < self.exports.len() {
            let value = self.exports[at];
            let Some(&computed) = self.values.get(&value) else {
                at += 1;
                continue;
            };
            let reg = *self.info.value_regs.get(&value)?;
            if !matches!(self.dag.value_type(computed), EVT::I32 | EVT::I64) {
                return None;
            }
            self.chain = self.dag.get_copy_to_reg(self.chain, reg, computed);
            self.exports.swap_remove(at);
        }
        Some(())
    }

    /// `HandlePHINodesInSuccessorBlocks`: the value each phi of a successor takes from this block,
    /// copied to a register before the branch.
    fn phi_copies(&mut self, block: BlockId) -> Option<()> {
        for successor in self.function.successors(block).to_vec() {
            let phis: Vec<_> = self
                .function
                .iter_block(successor)
                .take_while(|&id| self.function.instruction(id).opcode == Opcode::Phi)
                .collect();
            for phi in phis {
                let phi = self.function.instruction(phi).clone();
                let result = phi.result?;
                if self.function.users(result).is_empty() {
                    continue;
                }
                wide_kind(&self.module.context.types, phi.ty, self.m32)?;
                let incoming = phi.operands.chunks(2).find(|pair| pair[1] == Operand::Block(block)).map(|pair| pair[0])?;
                let reg = match incoming {
                    Operand::Value(value) if self.info.value_regs.contains_key(&value) => self.info.value_regs[&value],
                    _ => {
                        let value = self.value(incoming)?;
                        let reg = self.info.new_reg();
                        self.chain = self.dag.get_copy_to_reg(self.chain, reg, value);
                        reg
                    }
                };
                self.phi_updates.push((result, reg));
            }
        }
        Some(())
    }

    fn block(mut self, block: BlockId, declined: &mut String) -> Option<(SelectionDAG, Vec<(ValueId, u32)>)> {
        self.chain = self.dag.entry_node();
        let insts: Vec<_> = self.function.iter_block(block).collect();
        self.exports = insts
            .iter()
            .filter_map(|&id| self.function.instruction(id).result)
            .filter(|result| self.info.value_regs.contains_key(result))
            .collect();
        let mut at = 0;
        while at < insts.len() {
            let instruction = self.function.instruction(insts[at]).clone();
            at += 1;
            *declined = format!("{:?}", instruction.opcode);
            self.export_ready()?;
            match &instruction.opcode {
                Opcode::Phi => {
                    // read back from its register where used; the predecessors write it
                    wide_kind(&self.module.context.types, instruction.ty, self.m32)?;
                }
                Opcode::Alloca { .. } => {
                    // a static alloca is its frame object
                    self.info.static_allocas.get(&instruction.result?)?;
                }
                Opcode::Load { volatile, .. } => {
                    let wide = wide_kind(&self.module.context.types, instruction.ty, self.m32)?;
                    let (mov_rm, _, _, _, _, _, _, _, vt) = self.set(wide)?;
                    let mut ops = self.address(instruction.operands[0])?;
                    let result = instruction.result?;
                    // a volatile load is never folded (`tryToFoldLoad`)
                    if !volatile
                        && let Some(&user) = insts.get(at)
                        && self.folds(&self.function.instruction(user).clone(), result, wide)
                    {
                        self.folded = Some((result, ops));
                        continue;
                    }
                    ops.push(self.chain);
                    let load = self.node(mov_rm, &[vt, EVT::OTHER], &ops);
                    self.chain = SDValue { node: load.node, res: 1 };
                    self.values.insert(instruction.result?, load);
                }
                Opcode::Store { .. } => {
                    let stored = self.function.operand_type(&self.module.context, instruction.operands[0])?;
                    let wide = wide_kind(&self.module.context.types, stored, self.m32)?;
                    let (_, mov_mr, mov_mi, _, _, _, _, _, vt) = self.set(wide)?;
                    let mut ops = self.address(instruction.operands[1])?;
                    let immediate = if wide { self.int_signed(instruction.operands[0]) } else { self.int(instruction.operands[0]) };
                    let op = if let Some(imm) = immediate {
                        ops.push(self.dag.get_constant_with(imm as u128, vt, true, false));
                        mov_mi
                    } else {
                        let value = self.value(instruction.operands[0])?;
                        ops.push(value);
                        mov_mr
                    };
                    ops.push(self.chain);
                    let store = self.node(op, &[EVT::OTHER], &ops);
                    self.chain = store;
                }
                Opcode::GetElementPtr { source } if !self.m32 => {
                    // `lea base + index * size` with the index already 64-bit (a `sext`):
                    // `gep T, p, i` or `gep [n x T], p, 0, i`
                    let (base, index, element) = match instruction.operands[..] {
                        [base, index] => (base, index, *source),
                        [base, zero, index] if self.int(zero) == Some(0) => {
                            let llrm_mir::types::Type::Array { element, .. } = self.module.context.types.get(*source) else { return None };
                            (base, index, *element)
                        }
                        _ => return None,
                    };
                    let size = self.layout.alloc_size(&self.module.context.types, element) as u128;
                    if !matches!(size, 1 | 2 | 4 | 8) || self.int(index).is_some() {
                        return None;
                    }
                    let Operand::Value(index) = index else { return None };
                    let index = *self.values.get(&index)?;
                    if self.dag.value_type(index) != EVT::I64 {
                        return None;
                    }
                    let base = self.value(base)?;
                    let scale = self.dag.get_constant_with(size, EVT::I8, true, false);
                    let zero = self.dag.get_constant_with(0, EVT::I32, true, false);
                    let segment = self.dag.get_register(EVT::I16, 0);
                    let done = self.node(self.names.lea64?, &[EVT::I64], &[base, scale, index, zero, segment]);
                    self.values.insert(instruction.result?, done);
                }
                Opcode::Cast(llrm_mir::opcode::CastOp::SExt) if !self.m32 => {
                    // i32 -> i64: `movsxd`
                    let from = self.function.operand_type(&self.module.context, instruction.operands[0])?;
                    if self.module.context.types.int_bits(from) != Some(32) || self.module.context.types.int_bits(instruction.ty) != Some(64) {
                        return None;
                    }
                    let done = if let Some(mut address) = self.take_fold(instruction.operands[0]) {
                        address.push(self.chain);
                        let done = self.node(self.names.movsx64_rm?, &[EVT::I64, EVT::OTHER], &address);
                        self.chain = SDValue { node: done.node, res: 1 };
                        SDValue { node: done.node, res: 0 }
                    } else {
                        let value = self.value(instruction.operands[0])?;
                        self.node(self.names.movsx64?, &[EVT::I64], &[value])
                    };
                    self.values.insert(instruction.result?, done);
                }
                Opcode::GetElementPtr { source } if self.m32 => {
                    // one index: base + index * size(source), `lea`
                    let [base, index] = instruction.operands[..] else { return None };
                    let size = self.layout.alloc_size(&self.module.context.types, *source) as i64;
                    let base = self.value(base)?;
                    let noreg = self.dag.get_register(EVT::I32, 0);
                    let segment = self.dag.get_register(EVT::I16, 0);
                    let ops = if let Some(imm) = self.int(index) {
                        let disp = i32::try_from(i64::try_from(imm).ok()?.checked_mul(size)?).ok()?;
                        let one = self.dag.get_constant_with(1, EVT::I8, true, false);
                        let disp = self.dag.get_constant_with(disp as u32 as u128, EVT::I32, true, false);
                        vec![base, one, noreg, disp, segment]
                    } else {
                        if !matches!(size, 1 | 2 | 4 | 8)
                            || self.function.operand_type(&self.module.context, index).and_then(|ty| self.module.context.types.int_bits(ty)) != Some(32)
                        {
                            return None;
                        }
                        let index = self.value(index)?;
                        let scale = self.dag.get_constant_with(size as u128, EVT::I8, true, false);
                        let zero = self.dag.get_constant_with(0, EVT::I32, true, false);
                        vec![base, scale, index, zero, segment]
                    };
                    let lea = self.names.lea?;
                    let done = self.node(lea, &[EVT::I32], &ops);
                    self.values.insert(instruction.result?, done);
                }
                Opcode::Binary(op @ (llrm_mir::opcode::BinaryOp::SDiv | llrm_mir::opcode::BinaryOp::SRem | llrm_mir::opcode::BinaryOp::UDiv | llrm_mir::opcode::BinaryOp::URem)) => {
                    // signed `cdq; idiv r` (`cqo` at 64 bits), unsigned `mov edx,0; div r`: quotient in eax, remainder in edx
                    use llrm_mir::opcode::BinaryOp::*;
                    let calls = self.names.divide.as_ref()?;
                    let wide = match self.module.context.types.int_bits(instruction.ty) {
                        Some(32) => false,
                        Some(64) if !self.m32 => true,
                        _ => return None,
                    };
                    let (sign, idiv, div, vt, accumulator, remainder) = match (wide, calls.wide) {
                        (false, _) => (calls.cdq, calls.idiv, calls.div, EVT::I32, self.eax_reg()?, self.edx),
                        (true, Some((cqo, idiv, div))) => (cqo, idiv, div, EVT::I64, self.rax, self.rdx),
                        (true, None) => return None,
                    };
                    let (left, right) = (self.value(instruction.operands[0])?, self.value(instruction.operands[1])?);
                    let copy = self.dag.get_copy_to_phys_reg(self.dag.entry_node(), accumulator, left, None);
                    let glue = SDValue { node: copy.node, res: 1 };
                    let divided = if matches!(op, SDiv | SRem) {
                        let sign = self.node(sign, &[EVT::GLUE], &[glue]);
                        self.node(idiv, &[EVT::GLUE], &[right, sign])
                    } else {
                        let zero = self.dag.get_constant_with(0, vt, true, false);
                        let clear = self.dag.get_copy_to_phys_reg(self.dag.entry_node(), remainder, zero, Some(glue));
                        self.node(div, &[EVT::GLUE], &[right, SDValue { node: clear.node, res: 1 }])
                    };
                    let register = if matches!(op, SDiv | UDiv) { accumulator } else { remainder };
                    let result = self.dag.get_copy_from_phys_reg(self.dag.entry_node(), register, vt, divided);
                    self.values.insert(instruction.result?, SDValue { node: result.node, res: 0 });
                }
                // `FastISel::selectIntrinsicCall`: "at -O0 we don't care about the lifetime intrinsics", and
                // `donothing`, `sideeffect`, `assume` and `noalias.scope.decl` emit nothing either.
                Opcode::Call(_) if self.is_noop_intrinsic(&instruction) => {}
                Opcode::Call(call) if !self.m32 && call.calling_convention == self.function.calling_convention => {
                    // SysV: up to six integer or pointer arguments in registers; `al` = 0 for a variadic callee
                    let (down, up, call_op, mov8) = self.names.calls64?;
                    let (arguments, callee, returns, variadic) = self.callee(&instruction, call, self.sysv.len())?;
                    let types = &self.module.context.types;
                    let void = returns.is_none_or(|ty| types.is_void(ty));
                    let wide_result = match returns.and_then(|ty| types.int_bits(ty)) {
                        _ if void => false,
                        Some(32) => false,
                        Some(64) => true,
                        None if returns.is_some_and(|ty| matches!(types.get(ty), llrm_mir::types::Type::Pointer(0))) => true,
                        _ => return None,
                    };
                    let mut values = Vec::new();
                    for &one in &arguments {
                        let ty = self.function.operand_type(&self.module.context, one)?;
                        let wide = match types.int_bits(ty) {
                            Some(32) => false,
                            Some(64) => true,
                            Some(8) if matches!(callee, Callee::Library(_)) => {
                                values.push((self.narrow_constant(one)?, false));
                                continue;
                            }
                            _ if matches!(types.get(ty), llrm_mir::types::Type::Pointer(0)) => true,
                            _ => return None,
                        };
                        values.push((self.value(one)?, wide));
                    }
                    let zero = self.dag.get_constant_with(0, EVT::I32, true, false);
                    let bytes = zero;
                    let start = self.node(down, &[EVT::OTHER, EVT::GLUE], &[bytes, zero, self.chain]);
                    let (mut chain, mut glue) = (SDValue { node: start.node, res: 0 }, SDValue { node: start.node, res: 1 });
                    let mut regs = Vec::new();
                    for (at, (value, wide)) in values.into_iter().enumerate() {
                        let register = self.sysv[at][usize::from(wide)];
                        let copy = self.dag.get_copy_to_phys_reg(chain, register, value, Some(glue));
                        (chain, glue) = (SDValue { node: copy.node, res: 0 }, SDValue { node: copy.node, res: 1 });
                        regs.push(self.dag.get_phys_register(if wide { EVT::I64 } else { EVT::I32 }, register));
                    }
                    if variadic {
                        let none = self.dag.get_constant_with(0, EVT::I8, true, false);
                        let set = self.node(mov8, &[EVT::I8], &[none]);
                        let copy = self.dag.get_copy_to_phys_reg(chain, self.al, set, Some(glue));
                        (chain, glue) = (SDValue { node: copy.node, res: 0 }, SDValue { node: copy.node, res: 1 });
                        regs.push(self.dag.get_phys_register(EVT::I8, self.al));
                    }
                    let target = match callee {
                        Callee::Global(global) => self.dag.get_target_global_address(EVT::I64, global.0, 0),
                        Callee::Library(name) => self.dag.get_target_external_symbol(EVT::I64, name),
                    };
                    let mask = self.dag.get_register_mask_clobbering(&super::dagisel_adapter::call_clobbered_flags());
                    let mut ops = vec![target];
                    ops.extend(regs);
                    ops.extend([mask, chain, glue]);
                    let called = self.node(call_op, &[EVT::OTHER, EVT::GLUE], &ops);
                    let ended = self.node(up, &[EVT::OTHER, EVT::GLUE], &[zero, zero, SDValue { node: called.node, res: 0 }, SDValue { node: called.node, res: 1 }]);
                    let mut chain = SDValue { node: ended.node, res: 0 };
                    if !void {
                        let (register, vt) = if wide_result { (self.rax, EVT::I64) } else { (self.eax_reg()?, EVT::I32) };
                        let result = self.dag.get_copy_from_phys_reg(chain, register, vt, SDValue { node: ended.node, res: 1 });
                        chain = SDValue { node: result.node, res: 1 };
                        self.values.insert(instruction.result?, SDValue { node: result.node, res: 0 });
                    }
                    self.chain = chain;
                }
                Opcode::Call(call) if self.m32 && self.alike(call.calling_convention, self.function.calling_convention) => {
                    let calls = self.names.calls.as_ref()?;
                    let (arguments, callee, returns, variadic) = self.callee(&instruction, call, usize::MAX)?;
                    let int32 = |ty| word(&self.module.context.types, ty, true);
                    let void = returns.is_none_or(|ty| self.module.context.types.is_void(ty));
                    if variadic || !(void || returns.is_some_and(int32)) {
                        return None;
                    }
                    let mut values = Vec::new();
                    for &one in &arguments {
                        let ty = self.function.operand_type(&self.module.context, one)?;
                        values.push(match int32(ty) {
                            true => self.value(one)?,
                            false if matches!(callee, Callee::Library(_)) => self.narrow_constant(one)?,
                            false => return None,
                        });
                    }
                    let bytes = self.dag.get_constant_with(4 * values.len() as u128, EVT::I32, true, false);
                    let zero = self.dag.get_constant_with(0, EVT::I32, true, false);
                    let down = self.node(calls.call_down, &[EVT::I32, EVT::OTHER, EVT::GLUE], &[bytes, zero, zero, self.chain]);
                    let after_down = SDValue { node: down.node, res: 1 };
                    let mut stores = Vec::new();
                    for (at, value) in values.into_iter().enumerate() {
                        let esp_register = self.dag.get_phys_register(EVT::I32, self.esp);
                        let esp = self.dag.get_node_multi(ISD::CopyFromReg, &[EVT::I32, EVT::OTHER], &[after_down, esp_register]);
                        let scale = self.dag.get_constant_with(1, EVT::I8, true, false);
                        let noreg = self.dag.get_register(EVT::I32, 0);
                        let disp = self.dag.get_constant_with(4 * at as u128, EVT::I32, true, false);
                        let segment = self.dag.get_register(EVT::I16, 0);
                        stores.push(self.node(
                            self.names.mov_mr,
                            &[EVT::OTHER],
                            &[esp, scale, noreg, disp, segment, value, after_down],
                        ));
                    }
                    let chain = if stores.is_empty() { after_down } else { self.dag.get_token_factor(&stores) };
                    let target = match callee {
                        Callee::Global(global) => self.dag.get_target_global_address(EVT::I32, global.0, 0),
                        Callee::Library(name) => self.dag.get_target_external_symbol(EVT::I32, name),
                    };
                    let mask = self.dag.get_register_mask_clobbering(&super::dagisel_adapter::call_clobbered_flags());
                    let called = self.node(calls.call, &[EVT::OTHER, EVT::GLUE], &[target, mask, chain]);
                    let up = self.node(
                        calls.call_up,
                        &[EVT::I32, EVT::OTHER, EVT::GLUE],
                        &[bytes, zero, SDValue { node: called.node, res: 0 }, SDValue { node: called.node, res: 1 }],
                    );
                    let mut chain = SDValue { node: up.node, res: 1 };
                    if !void {
                        let eax = self.eax_reg()?;
                        let result = self.dag.get_copy_from_phys_reg(chain, eax, EVT::I32, SDValue { node: up.node, res: 2 });
                        chain = SDValue { node: result.node, res: 1 };
                        self.values.insert(instruction.result?, SDValue { node: result.node, res: 0 });
                    }
                    self.chain = chain;
                }
                Opcode::Binary(op) => {
                    use llrm_mir::opcode::BinaryOp::*;
                    let wide = wide_kind(&self.module.context.types, instruction.ty, self.m32)?;
                    let (_, _, _, _, alu, _, _, imul, vt) = self.set(wide)?;
                    if matches!(op, Shl | LShr | AShr) {
                        // a constant count: `shl`/`shr`/`sar r, imm8` (a variable count is `cl`, not here yet)
                        let bits: i128 = if wide { 64 } else { 32 };
                        let count = self.int(instruction.operands[1]).filter(|count| (0..bits).contains(count))?;
                        let (shl, shr, sar) = if wide {
                            let w = self.names.wide.as_ref()?;
                            (w.shl_ri, w.shr_ri, w.sar_ri)
                        } else {
                            (self.names.shl_ri, self.names.shr_ri, self.names.sar_ri)
                        };
                        let left = self.value(instruction.operands[0])?;
                        let count = self.dag.get_constant_with(count as u128, EVT::I8, true, false);
                        let opcode = match op {
                            Shl => shl,
                            LShr => shr,
                            _ => sar,
                        };
                        let done = self.node(opcode, &[vt, EVT::I32], &[left, count]);
                        self.values.insert(instruction.result?, done);
                        continue;
                    }
                    if matches!(op, Mul) {
                        // the pin's `fastEmit_ri_`: a power of two is `shl r, k`, any other constant
                        // `imul r, r, imm`; a constant on the left is swapped to the right
                        let constant = |this: &Self, one| if wide { this.int_signed(one) } else { this.int(one) };
                        let (a, b) = (instruction.operands[0], instruction.operands[1]);
                        let (register, imm) = match (constant(&self, a), constant(&self, b)) {
                            (_, Some(imm)) => (a, Some(imm)),
                            (Some(imm), None) => (b, Some(imm)),
                            (None, None) => (a, None),
                        };
                        let done = if let Some(imm) = imm {
                            let (shl_ri, imul_ri) = if wide {
                                let w = self.names.wide.as_ref()?;
                                (w.shl_ri, w.imul_ri)
                            } else {
                                (self.names.shl_ri, self.names.imul_ri)
                            };
                            let left = self.value(register)?;
                            if imm > 0 && (imm & (imm - 1)) == 0 {
                                let count = self.dag.get_constant_with(imm.trailing_zeros().into(), EVT::I8, true, false);
                                self.node(shl_ri, &[vt, EVT::I32], &[left, count])
                            } else {
                                let imm = self.dag.get_constant_with(imm as u128, vt, true, false);
                                self.node(imul_ri, &[vt, EVT::I32], &[left, imm])
                            }
                        } else if let Some(mut address) = self.take_fold(b) {
                            let left = self.value(a)?;
                            let rm = if wide { self.names.wide.as_ref()?.mem.imul } else { self.names.mem.imul };
                            let mut ops = vec![left];
                            ops.append(&mut address);
                            ops.push(self.chain);
                            let done = self.node(rm, &[vt, EVT::I32, EVT::OTHER], &ops);
                            self.chain = SDValue { node: done.node, res: 2 };
                            SDValue { node: done.node, res: 0 }
                        } else {
                            let (left, right) = (self.value(a)?, self.value(b)?);
                            self.node(imul, &[vt, EVT::I32], &[left, right])
                        };
                        self.values.insert(instruction.result?, done);
                        continue;
                    }
                    let which = match op {
                        Add => 0,
                        Sub => 1,
                        And => 2,
                        Or => 3,
                        Xor => 4,
                        _ => return None,
                    };
                    let (rr, ri) = alu[which];
                    let left = self.value(instruction.operands[0])?;
                    if let Some(mut address) = self.take_fold(instruction.operands[1]) {
                        let rm = if wide { self.names.wide.as_ref()?.mem.alu[which] } else { self.names.mem.alu[which] };
                        let mut ops = vec![left];
                        ops.append(&mut address);
                        ops.push(self.chain);
                        let done = self.node(rm, &[vt, EVT::I32, EVT::OTHER], &ops);
                        self.chain = SDValue { node: done.node, res: 2 };
                        self.values.insert(instruction.result?, SDValue { node: done.node, res: 0 });
                        continue;
                    }
                    let immediate = if wide { self.int_signed(instruction.operands[1]) } else { self.int(instruction.operands[1]) };
                    let done = if let Some(imm) = immediate {
                        let imm = self.dag.get_constant_with(imm as u128, vt, true, false);
                        self.node(ri, &[vt, EVT::I32], &[left, imm])
                    } else {
                        let right = self.value(instruction.operands[1])?;
                        self.node(rr, &[vt, EVT::I32], &[left, right])
                    };
                    self.values.insert(instruction.result?, done);
                }
                Opcode::ICmp(predicate) => {
                    // the condition of the branch that closes the block, or else a `setcc` value
                    let branch = insts.get(at).and_then(|&next| {
                        let next = self.function.instruction(next).clone();
                        match (&next.opcode, next.operands.as_slice()) {
                            (Opcode::Br, [Operand::Value(cond), Operand::Block(on_true), Operand::Block(on_false)])
                                if Some(*cond) == instruction.result && at + 1 == insts.len() =>
                            {
                                Some((*on_true, *on_false))
                            }
                            _ => None,
                        }
                    });
                    let wide = self.is_wide(instruction.operands[0])?;
                    let (_, _, _, _, _, cmp_rr, cmp_ri, _, vt) = self.set(wide)?;
                    let left = self.value(instruction.operands[0])?;
                    let immediate = if wide { self.int_signed(instruction.operands[1]) } else { self.int(instruction.operands[1]) };
                    let compared = if let Some(imm) = immediate {
                        let imm = self.dag.get_constant_with(imm as u128, vt, true, false);
                        self.node(cmp_ri, &[EVT::I32], &[left, imm])
                    } else if let Some(mut address) = self.take_fold(instruction.operands[1]) {
                        let rm = if wide { self.names.wide.as_ref()?.mem.cmp } else { self.names.mem.cmp };
                        let mut ops = vec![left];
                        ops.append(&mut address);
                        ops.push(self.chain);
                        let done = self.node(rm, &[EVT::I32, EVT::OTHER], &ops);
                        self.chain = SDValue { node: done.node, res: 1 };
                        SDValue { node: done.node, res: 0 }
                    } else {
                        let right = self.value(instruction.operands[1])?;
                        self.node(cmp_rr, &[EVT::I32], &[left, right])
                    };
                    use llrm_mir::opcode::IntPredicate::*;
                    let mut code: u8 = match predicate {
                        Eq => 4,
                        Ne => 5,
                        Ult => 2,
                        Uge => 3,
                        Ule => 6,
                        Ugt => 7,
                        Slt => 12,
                        Sge => 13,
                        Sle => 14,
                        Sgt => 15,
                    };
                    let Some((mut on_true, mut on_false)) = branch else {
                        // `setcc r8`: an i1 held in a byte register
                        if self.module.context.types.int_bits(instruction.ty) != Some(1) {
                            return None;
                        }
                        let flags = self.dag.get_copy_to_phys_reg(self.chain, super::dagisel_adapter::EFLAGS_ID, compared, None);
                        let glue = SDValue { node: flags.node, res: 1 };
                        let cc = self.dag.get_constant_with(u128::from(code), EVT::I8, true, false);
                        let set = self.node(self.names.setcc, &[EVT::I8], &[cc, glue]);
                        self.chain = SDValue { node: flags.node, res: 0 };
                        self.values.insert(instruction.result?, set);
                        continue;
                    };
                    if Some(on_true) == self.next {
                        std::mem::swap(&mut on_true, &mut on_false);
                        code ^= 1;
                    }
                    self.phi_copies(block)?;
                    let flags = self.dag.get_copy_to_phys_reg(self.chain, super::dagisel_adapter::EFLAGS_ID, compared, None);
                    let glue = SDValue { node: flags.node, res: 1 };
                    let dest = self.dag.get_basic_block(on_true.0);
                    let cc = self.dag.get_constant_with(u128::from(code), EVT::I8, true, false);
                    let jcc = self.node(self.names.jcc, &[EVT::OTHER], &[dest, cc, flags, glue]);
                    let fall = self.dag.get_basic_block(on_false.0);
                    let jmp = self.node(self.names.jmp, &[EVT::OTHER], &[fall, jcc]);
                    self.chain = jmp;
                    at += 1;
                }
                Opcode::Cast(llrm_mir::opcode::CastOp::ZExt) if !self.m32 && self.is_wide(instruction.operands[0]) == Some(false) && self.module.context.types.int_bits(instruction.ty) == Some(64) => {
                    // i32 -> i64: `SUBREG_TO_REG` (a dword write zeroes the high half)
                    let (to_reg, _) = self.names.subreg?;
                    let value = self.value(instruction.operands[0])?;
                    let index = self.dag.get_constant_with(6, EVT::I32, true, false);
                    let done = self.node(to_reg, &[EVT::I64], &[value, index]);
                    self.values.insert(instruction.result?, done);
                }
                Opcode::Cast(llrm_mir::opcode::CastOp::Trunc) if !self.m32 => {
                    // i64 -> i32: the low dword, `EXTRACT_SUBREG`
                    let (_, extract) = self.names.subreg?;
                    if self.is_wide(instruction.operands[0]) != Some(true) || self.module.context.types.int_bits(instruction.ty) != Some(32) {
                        return None;
                    }
                    let value = self.value(instruction.operands[0])?;
                    let index = self.dag.get_constant_with(6, EVT::I32, true, false);
                    let done = self.node(extract, &[EVT::I32], &[value, index]);
                    self.values.insert(instruction.result?, done);
                }
                Opcode::Cast(llrm_mir::opcode::CastOp::ZExt) => {
                    // i1 -> i32: `and r8, 1; movzx`
                    let from = self.function.operand_type(&self.module.context, instruction.operands[0])?;
                    if self.module.context.types.int_bits(from) != Some(1) || self.module.context.types.int_bits(instruction.ty) != Some(32) {
                        return None;
                    }
                    let byte = self.value_byte(instruction.operands[0])?;
                    let one = self.dag.get_constant_with(1, EVT::I8, true, false);
                    let masked = self.node(self.names.and8, &[EVT::I8, EVT::I32], &[byte, one]);
                    let done = self.node(self.names.movzx8, &[EVT::I32], &[masked]);
                    self.values.insert(instruction.result?, done);
                }
                Opcode::Br if instruction.operands.len() == 1 => {
                    let Operand::Block(target) = instruction.operands[0] else { return None };
                    self.phi_copies(block)?;
                    if Some(target) != self.next {
                        let dest = self.dag.get_basic_block(target.0);
                        self.chain = self.node(self.names.jmp, &[EVT::OTHER], &[dest, self.chain]);
                    }
                }
                Opcode::Ret => {
                    let zero = self.dag.get_constant_with(0, EVT::I32, true, false);
                    let ret = match (instruction.operands.first().copied(), self.eax) {
                        (None, _) => self.node(self.names.ret, &[EVT::OTHER], &[zero, self.chain]),
                        (Some(operand), Some(eax)) => {
                            let value = self.value(operand)?;
                            let vt = self.dag.value_type(value);
                            let copy = self.dag.get_copy_to_phys_reg(self.chain, eax, value, None);
                            let register = self.dag.get_phys_register(vt, eax);
                            let glue = SDValue { node: copy.node, res: 1 };
                            self.node(self.names.ret, &[EVT::OTHER], &[zero, register, copy, glue])
                        }
                        _ => return None,
                    };
                    self.chain = ret;
                }
                _ => return None,
            }
        }
        if !self.exports.is_empty() {
            *declined = "a value another block reads that is not a register value".to_owned();
            return None;
        }
        self.dag.set_root(self.chain);
        Some((self.dag, self.phi_updates))
    }
}
