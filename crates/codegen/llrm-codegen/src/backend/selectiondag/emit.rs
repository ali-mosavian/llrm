//! `InstrEmitter`: scheduled units to LIR.
//!
//! # How it works
//! [`emit`] walks the units [`super::schedule::schedule`] returns, a unit's
//! glued nodes top first, and writes one LIR instruction per machine node and
//! per register copy. A node's results are virtual values, numbered in the
//! order they are emitted (`VRBaseMap`); an operand is that value, an
//! immediate, or a memory cell built from the pin's five address operands
//! (base, scale, index, displacement, segment).
//!
//! ```text
//! ADD32rm  r1, [r2 + r3*4 + 8]   (node: src1, base, scale, index, disp, seg)
//!   v4 := add v1, dword [v2 + v3*4 + 8]
//! CopyToReg %5, v4        ->  v5 := mov v4
//! ```
//!
//! A tied source is a source of the LIR form (`^0` in `x86.instr`), so the
//! pin's two-address instruction is three-address here and `twoaddr.rs`
//! ties it again. A memory destination with no result (`ADD32mr`) is the
//! form's destination and its tied source.
//!
//! # Example
//! `x = a + b` with `a`, `b` in virtual registers 1 and 2 and the result
//! copied to register 3: `v10 := add v1, v2` then `v3 := mov v10`.
//!
//! DEVIATE SD400: `EmitLoweredSelect` expands one `CMOV_GR*` at a time: a run
//! of selects on one condition (case 1) and the cascaded form (case 2) each
//! get their own diamond, the same values with more branches. Reason: the
//! pin looks ahead in the block it is rewriting; this emitter is a single
//! pass. Exit: D1c, with the block-level pass. Owner: sdag-d. Review
//! 2026-12-03.
//!
//! # Runs
//! Nothing runs it yet: the shadow harness calls it once the matcher's output
//! reaches a whole function (D1c). The old selector stays the default.
//!
//! DEVIATE SD39: not translated yet: frame indices of locals (fixed objects are
//! cells), globals, segment
//! overrides, inline assembly and the custom inserters but `CMOV_GR*`.
//! Each is an [`EmitError::Unsupported`], never a skipped operand. Reason:
//! the builder (A2) and matcher output do not produce them yet. Exit: with
//! A2 and the custom inserter piece. Owner: sdag-d. Review 2026-12-03.
//!
//! # See also
//! `InstrEmitter::EmitMachineNode`
//! ([L1003](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/InstrEmitter.cpp#L1003)),
//! `EmitCopyToRegClassNode`, `EmitCopyFromReg`.

use std::sync::Arc;

use llrm_dagisel::instr::{self, InstrDesc, operand_type};
use llrm_dagisel::x86_addr::AddrMode;
use llrm_dagisel::x86_classes::{Role, class_tag};
use llrm_support::hash::HashMap;

use super::dag::{NodeData, SDValue, SelectionDAG};
use super::isd::ISD;
use super::mvt::{EVT, MVT};
use super::opcodes::OpcodeMap;
use super::schedule::Unit;
use crate::backend::isel::{_read, _written};
use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Reg, Semantics};
use crate::model::lir::{Insn, Phi};

/// Why a node was not emitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmitError {
    /// A node or operand this emitter does not translate yet.
    Unsupported(String),
    /// An opcode with no `x86.instr` form (`opcodes_unmapped.txt`).
    Unmapped(String),
}

/// Where the module's globals are.
pub trait GlobalMap {
    /// The address of global `symbol` (`GlobalId`) plus `offset`: its symbol's
    /// space (defined here, or external) and index.
    fn address(
        &self,
        symbol: u32,
        offset: i64,
    ) -> Addr;

    /// The address whose value is the segment `symbol` lives in (`seg g`), or
    /// DGROUP's where it names none or a near one.
    fn segment(
        &self,
        symbol: Option<u32>,
    ) -> Addr;
}

/// The readonly cell that holds a constant's bytes (`constpool`), one for each
/// distinct datum of the module.
pub trait ConstantCells {
    fn cell(
        &self,
        bytes: Vec<u8>,
    ) -> Mem;
}

/// No pool: the DAG makes no constant cell here.
pub struct NoConstants;

impl ConstantCells for NoConstants {
    fn cell(
        &self,
        bytes: Vec<u8>,
    ) -> Mem {
        Mem::new(None, bytes.len() as u32)
    }
}

/// What a call destroys, from the convention of what it calls.
pub trait Conventions {
    /// The registers a call of `callee` destroys beside the ones it passes
    /// arguments in and returns results in; `None` where the convention is
    /// not known (the target-wide set stands).
    fn clobbers(
        &self,
        callee: Option<&Callee>,
    ) -> Option<std::collections::BTreeSet<llrm_lir::registers::RegId>>;

    /// What a call of `callee`, which destroys `clobbers`, makes a function
    /// that keeps registers for its caller save: all of them, but for a callee
    /// that never returns (nothing is left to restore).
    fn disturbs(
        &self,
        _callee: Option<&Callee>,
        clobbers: &std::collections::BTreeSet<llrm_lir::registers::RegId>,
    ) -> std::collections::BTreeSet<llrm_lir::registers::RegId> {
        clobbers.clone()
    }

    /// What a call of `callee` wrote, where it took part in interprocedural
    /// register use and was machined first: the registers it destroys whole,
    /// and those it keeps only the pushed half of. These replace the
    /// convention's.
    fn written(
        &self,
        _callee: Option<&Callee>,
    ) -> Option<(
        std::collections::BTreeSet<llrm_lir::registers::RegId>,
        std::collections::BTreeSet<llrm_lir::registers::RegId>,
    )> {
        None
    }

    /// The registers a call of `callee` keeps only the pushed part of (a
    /// 16-bit push of a 32-bit register): their upper half is destroyed.
    fn clobbers_high(
        &self,
        _callee: Option<&Callee>,
    ) -> std::collections::BTreeSet<llrm_lir::registers::RegId> {
        Default::default()
    }
}

/// No convention is known: every call destroys `EmitContext::caller_saved`.
pub struct NoConventions;

impl Conventions for NoConventions {
    fn clobbers(
        &self,
        _callee: Option<&Callee>,
    ) -> Option<std::collections::BTreeSet<llrm_lir::registers::RegId>> {
        None
    }
}

/// Where the frame's fixed objects are.
pub trait FrameMap {
    /// The cell of fixed object `index` (`FrameIndex<-1>` is the first stack
    /// argument), `width` bits wide, or `None` if there is no such object.
    fn cell(
        &self,
        index: i32,
        width: u32,
    ) -> Option<Mem>;

    /// (displacement, size) of local object `index`; `None` for a parameter
    /// or where the frame does not say.
    fn extent(
        &self,
        _index: i32,
    ) -> Option<(i64, i64)> {
        None
    }
}

/// The pin's machine opcode names, by number.
pub struct EmitContext<'a> {
    /// The registers a call destroys (EAX, ECX, EDX at m32).
    pub caller_saved: &'a std::collections::BTreeSet<llrm_lir::registers::RegId>,
    /// The stack pointer: a `CopyFromReg` of it is the base `SP`, not a value.
    pub stack_pointer: llrm_lir::registers::RegId,
    /// The registers a string operation names, where the target fixes them.
    pub strings: Option<llrm_target::StringRegisters>,
    /// The target's registers: which one a narrower view of another is.
    pub registers: &'static llrm_lir::registers::Info,
    pub conventions: &'a dyn Conventions,
    pub constants: &'a dyn ConstantCells,
    pub globals: &'a dyn GlobalMap,
    /// The LIR address of each MIR block, by `BlockId`: what a branch names.
    pub block_at: &'a HashMap<u32, i64>,
    pub frame: &'a dyn FrameMap,
    /// How addresses are formed: real mode constrains a base and an index to
    /// their register classes (SD4).
    pub address_mode: AddrMode,
    /// Code size is wanted over speed (`-Os`, `-Oz`).
    pub opt_size: bool,
    /// The multiple a call's argument area is rounded to, so a call finds the
    /// stack aligned (`aligned_calls`); 1 where calls need none.
    pub call_alignment: i64,
    pub map: &'a OpcodeMap,
    pub names: &'a HashMap<u16, &'a str>,
    /// The jump tables of the function, by `JumpTable` index.
    pub tables: &'a [JumpTable],
}

/// A jump table: the data object that holds it and the block each entry goes
/// to.
#[derive(Clone, Debug)]
pub struct JumpTable {
    pub id: i64,
    pub targets: Vec<i64>,
}

/// One block of the emitted code: `phis` first, then `insns`. A custom
/// inserter (`EmitLoweredSelect`) splits the block it is in.
#[derive(Clone, Debug, Default)]
pub struct EmitBlock {
    /// The block's address, as a branch names it.
    pub at: i64,
    pub phis: Vec<Phi>,
    pub insns: Vec<Arc<Insn>>,
}

struct Emitter<'a, 'd> {
    dag: &'d SelectionDAG,
    ctx: &'a EmitContext<'a>,
    /// `VRBaseMap`: the value each emitted result is.
    values: HashMap<SDValue, u32>,
    /// The LIR value a virtual register number of the DAG is.
    state: &'a mut EmitState,
    /// The blocks so far; the last is where the next instruction goes.
    blocks: Vec<EmitBlock>,
    /// The register tag each address component's value is constrained to
    /// (`constrainRegClass`).
    classes: Vec<(u32, &'static str)>,
    /// `CopyFromReg` of the stack pointer: addresses through `SP`.
    sp_values: llrm_support::hash::HashSet<SDValue>,
    /// The value each of EAX/EDX (AX/DX) holds for the division or multiply
    /// that reads it, set by the `CopyToReg` the matcher glues to it or by the
    /// instruction before.
    accumulator: HashMap<llrm_lir::registers::RegId, Held>,
    /// The segments of the last string move: a move glued after it goes on
    /// through them.
    string_segments: Option<(Loc, Loc)>,
    /// The arguments passed in registers to the call being built.
    arguments: Vec<(Held, llrm_lir::registers::RegId)>,
    /// The values copied to the return registers, which the `RET` requires.
    returns: Vec<(Held, llrm_lir::registers::RegId)>,
    /// (call address, callee symbol) of each direct call.
    calls: Vec<(i64, Callee)>,
    pops: Vec<(i64, i64)>,
    indirect_pops: Vec<(i64, i64)>,
    inline_places: Vec<(i64, Vec<(usize, i64, i64)>)>,
    /// The call whose result registers have not been named yet: (block, insn).
    last_call: Option<(usize, usize)>,
    /// The nodes that read each value of a glue chain, from their operands (the
    /// matcher's nodes are not in the DAG's use lists).
    glue_users: HashMap<SDValue, Vec<u32>>,
    /// The `fcom` operands of each float compare, by the `SAHF` that reads its
    /// status word: the `fcom` is pushed there, next to the flag reader.
    pending_fcom: HashMap<u32, (Held, Held)>,
    /// The floating constants loaded where they are read: (mnemonic, cell).
    lazy: HashMap<SDValue, (&'static str, Option<Mem>)>,
    /// The `fld1`/`fldz` already loaded for a constant, valid until the next
    /// call or barrier (the x87 stack is empty across one) or block.
    loaded: HashMap<SDValue, (Held, usize, usize)>,
    /// Calls and barriers pushed so far.
    stack_breaks: usize,
    /// The selector each `LES16rm` loaded beside its offset, by node: a cell
    /// based on that offset takes it as its segment.
    selectors: HashMap<u32, Held>,
    /// The node each accumulator register's value came from, for the segment
    /// a string operation's pointer is in.
    origins: HashMap<llrm_lir::registers::RegId, SDValue>,
    /// The value the last `CopyToReg ES` put there: the selector of the next
    /// cell through ES that is not based on a loaded far pointer.
    es: Option<Held>,
    /// Whether that selector is SS: the cells through ES are then the frame's.
    es_is_ss: bool,
    /// The `CopyToReg ES` nodes that copy SS.
    ss_copies: llrm_support::hash::HashSet<u32>,
    /// What each `CopyToReg ES` put there, by node: a cell takes the one of
    /// the copy it is chained after, whatever was set last.
    es_copies: HashMap<u32, Held>,
    /// The node whose flags EFLAGS holds now.
    flags_now: Option<u32>,
    /// The select a following one may join.
    last_select: Option<Select>,

    /// The compares that made flags, to make them again.
    flag_insns: HashMap<u32, Arc<Insn>>,
    /// The carry a flag producer made, saved in a byte before another unit
    /// wrote the flags, for the `adc`/`sbb` that still reads it.
    saved_carry: HashMap<u32, Held>,
    /// The machine nodes emitted: the scheduler's clone of a unit emits its
    /// node again.
    machine_emitted: llrm_support::hash::HashSet<u32>,
    /// The `CopyToReg EFLAGS` nodes emitted.
    flag_copies_done: llrm_support::hash::HashSet<u32>,
}

/// The last lowered select: the block that branches, the one it skips, the
/// one it joins, and what it reads.
#[derive(Clone, Copy)]
struct Select {
    producer: u32,
    cc: usize,
    this: i64,
    false_at: i64,
    sink: i64,
    result: u32,
}

/// What outlives one MIR block: the LIR value of each virtual register of the
/// DAG, the next value number and the next address (blocks, instructions).
#[derive(Debug)]
pub struct EmitState {
    pub vregs: HashMap<u32, u32>,
    pub next_value: u32,
    pub at: i64,
    /// The bytes of the float type (4, 8 or 10) of each LIR value that is a DAG
    /// value of a float type: `LirBody::float_bytes`, which a spill rounds to.
    pub float_bytes: std::collections::BTreeMap<u32, u32>,
}

impl Default for EmitState {
    fn default() -> EmitState {
        EmitState { vregs: HashMap::default(), next_value: 1, at: 1, float_bytes: Default::default() }
    }
}

impl EmitState {
    /// The LIR value a virtual register of the DAG is, made on first use.
    pub fn vreg(
        &mut self,
        reg: u32,
    ) -> u32 {
        if let Some(&v) = self.vregs.get(&reg) {
            return v;
        }
        let v = self.next_value;
        self.next_value += 1;
        self.vregs.insert(reg, v);
        v
    }

    pub fn next_address(&mut self) -> i64 {
        let at = self.at;
        self.at += 1;
        at
    }
}

/// What [`emit`] makes: the blocks, and the register tags (`registers.regs`)
/// that address components are confined to, by value.
#[derive(Clone, Debug, Default)]
pub struct Emitted {
    pub blocks: Vec<EmitBlock>,
    pub classes: Vec<(u32, &'static str)>,
    /// (address, global) of each direct call.
    pub calls: Vec<(i64, Callee)>,
    /// (address, bytes) of each call whose callee pops arguments it was pushed.
    pub pops: Vec<(i64, i64)>,
    /// The same for every indirect call, none included.
    pub indirect_pops: Vec<(i64, i64)>,
    /// The frame places each inline code site patches in, by site: (byte of
    /// the code, displacement, addend).
    pub inline_places: Vec<(i64, Vec<(usize, i64, i64)>)>,
}

/// The LIR blocks for `units` of `dag`, in order: one, plus the three a
/// `select` without `cmov` makes.
pub fn emit(
    dag: &SelectionDAG,
    units: &[Unit],
    ctx: &EmitContext<'_>,
) -> Result<Emitted, EmitError> {
    emit_in(dag, units, ctx, &mut EmitState::default(), 0)
}

/// [`emit`] for one MIR block of a function: values and addresses continue
/// from `state`, and the first block made is at `first_at`.
pub fn emit_in(
    dag: &SelectionDAG,
    units: &[Unit],
    ctx: &EmitContext<'_>,
    state: &mut EmitState,
    first_at: i64,
) -> Result<Emitted, EmitError> {
    let mut emitter = Emitter {
        dag,
        ctx,
        values: HashMap::default(),
        es_copies: HashMap::default(),
        state,
        blocks: vec![EmitBlock { at: first_at, ..EmitBlock::default() }],
        classes: Vec::new(),
        sp_values: Default::default(),
        accumulator: HashMap::default(),
        string_segments: None,
        arguments: Vec::new(),
        returns: Vec::new(),
        calls: Vec::new(),
        pops: Vec::new(),
        indirect_pops: Vec::new(),
        inline_places: Vec::new(),
        last_call: None,
        flags_now: None,
        origins: HashMap::default(),
        es: None,
        es_is_ss: false,
        ss_copies: llrm_support::hash::HashSet::default(),
        selectors: HashMap::default(),
        lazy: HashMap::default(),
        loaded: HashMap::default(),
        stack_breaks: 0,
        last_select: None,
        flag_insns: HashMap::default(),
        saved_carry: HashMap::default(),
        machine_emitted: Default::default(),
        flag_copies_done: Default::default(),
        pending_fcom: HashMap::default(),
        glue_users: {
            let mut users: HashMap<SDValue, Vec<u32>> = HashMap::default();
            for (at, one) in dag.nodes().iter().enumerate() {
                for op in &one.ops {
                    users.entry(*op).or_default().push(at as u32);
                }
            }
            users
        },
    };
    for unit in units {
        for &node in unit.nodes.iter().rev() {
            let before = emitter.blocks.last().map_or(0, |block| block.insns.len());
            let (first_block, order) =
                (emitter.blocks.len().saturating_sub(1), dag.node(SDValue { node, res: 0 }).order);
            emitter.node(node)?;
            // `SDLoc` to `DebugLoc`: what the node emitted is of its IR
            // instruction's line.
            if let Some(&Some(line)) = dag.lines.get(order as usize) {
                for (at, block) in emitter.blocks.iter_mut().enumerate().skip(first_block) {
                    let from = if at == first_block { before } else { 0 };
                    for one in &mut block.insns[from..] {
                        if one.line.is_none() {
                            *one = Arc::new(Insn { line: Some(line), ..(**one).clone() });
                        }
                    }
                }
            }
            if dag.invariant_loads.contains(&node) {
                emitter.mark_invariant_loads(before);
            }
            if dag.volatile_accesses.contains(&node)
                || matches!(
                    dag.node(SDValue { node, res: 0 }).data,
                    NodeData::Mem(info) if info.volatile
                )
            {
                emitter.mark_volatile(before);
            }
        }
    }
    for (value, number) in &emitter.values {
        let bytes = match dag.value_type(*value) {
            EVT::Simple(MVT::F32) => 4,
            EVT::Simple(MVT::F64) => 8,
            EVT::Simple(MVT::F80) => 10,
            _ => continue,
        };
        let widest = emitter.state.float_bytes.entry(*number).or_insert(0);
        *widest = bytes.max(*widest);
    }
    let mut blocks = emitter.blocks;
    if ctx.address_mode == AddrMode::Real16 {
        sunk_areas(&mut blocks, ctx.stack_pointer);
        based_areas(&mut blocks, ctx.stack_pointer, emitter.state, &mut emitter.classes);
        let mut reads: HashMap<u32, usize> = HashMap::default();
        for insn in blocks.iter().flat_map(|block| &block.insns) {
            for value in &insn.uses {
                *reads.entry(*value).or_default() += 1;
            }
        }
        for block in &mut blocks {
            pushed_arguments(
                &mut block.insns,
                ctx.stack_pointer,
                emitter.state,
                &mut emitter.classes,
                &reads,
                (ctx.opt_size && ctx.address_mode == AddrMode::Real16)
                    .then(|| llrm_lir::registers::Regs(ctx.registers).width_of(ctx.stack_pointer))
                    .flatten(),
            )?;
        }
    }
    Ok(Emitted {
        blocks,
        classes: emitter.classes,
        calls: emitter.calls,
        pops: emitter.pops,
        indirect_pops: emitter.indirect_pops,
        inline_places: emitter.inline_places,
    })
}

/// The high and low words (`word` bytes each) of a constant of two words when
/// pushing them one at a time is shorter than a push of the whole: a half fits
/// a sign-extended byte.
fn word_halves(
    imm: &Imm,
    word: i64,
) -> Option<(i64, i64)> {
    let bits = word * 8;
    let mask = (1_i64 << bits) - 1;
    let small = |half: i64| i8::try_from(((half << (64 - bits)) >> (64 - bits)) as i64).is_ok();
    (imm.address.is_none() && imm.width == 2 * word as u32)
        .then(|| ((imm.value >> bits) & mask, imm.value & mask))
        .filter(|&(high, low)| small(high) || small(low))
}

/// A push of a register whose only use it is, and which a `mov` of an
/// immediate made (a constant, or the segment of a global), pushes the
/// immediate: `X86CallFrameOptimization::canFoldIntoRegPush`'s
/// `MOV32ri` with one use. `reads` counts the uses of each value over the
/// whole DAG; a virtual register of the function is read by other DAGs too.
fn fold_immediate_pushes(
    insns: &mut Vec<Arc<Insn>>,
    reads: &HashMap<u32, usize>,
    state: &mut EmitState,
    split_wide: Option<i64>,
) {
    let mut at = 0;
    while at < insns.len() {
        let pushed = insns[at]
            .what
            .as_ref()
            .filter(|what| what.op == Operation::Push)
            .and_then(
                |what| match what.sources.first() {
                    Some(Loc::Held(held)) => Some(held.value),
                    _ => None,
                },
            );
        let Some(value) = pushed else {
            at += 1;
            continue;
        };
        let reads = if state.vregs.values().any(|v| *v == value) { 0 } else { reads.get(&value).copied().unwrap_or(0) };
        let made = insns.iter().position(|insn| insn.defines == [value]);
        let immediate = made.and_then(|place| {
            let what = insns[place].what.as_ref()?;
            match (&what.op, &what.dests[..], &what.sources[..]) {
                (Operation::Move, [Loc::Held(_)], [Loc::Imm(imm)]) => Some(imm.clone()),
                _ => None,
            }
        });
        let (Some(place), Some(imm), 1) = (made, immediate, reads) else {
            at += 1;
            continue;
        };
        let old = insns[at].clone();
        let mut what = old.what.clone().expect("a push");
        // DEVIATE SD243 (real mode): tuned for size, a dword constant with a
        // byte-sized half is two `pushw` (high, then low), 4 or 5 bytes
        // against `pushd imm32`'s 6; the pin has no 16-bit push. The old
        // selector's pair. Owner w12. Exit: a cost hook on the target.
        let halves = split_wide.and_then(|word| word_halves(&imm, word).map(|halves| (halves, word)));
        let split_at = place < at;
        let Some(((high, low), word)) = halves else {
            what.sources[0] = Loc::Imm(imm);
            insns[at] = Arc::new(Insn::new(old.at, old.covers, Some(what), old.defines.clone(), Vec::new()));
            insns.remove(place);
            if split_at {
                at -= 1;
            }
            at += 1;
            continue;
        };
        let word = |half: i64| Loc::Imm(Imm { value: half, width: word as u32, address: None });
        let mut first = what.clone();
        first.sources[0] = word(high);
        let mut second = what;
        second.sources[0] = word(low);
        let made = state.next_address();
        insns[at] = Arc::new(Insn::new(old.at, old.covers, Some(first), old.defines.clone(), Vec::new()));
        insns.insert(at + 1, Arc::new(Insn::new(made, Some((made, made)), Some(second), vec![], Vec::new())));
        insns.remove(place);
        if split_at {
            at -= 1;
        }
        at += 2;
    }
}

/// Real-mode addressing has no `[sp+k]`: the arguments a call takes on the
/// stack are pushed, last first, where the lowering stored them into the area
/// it made (`sub sp, n` and a store to each cell before the call): the
/// pin's `X86CallFrameOptimization`, which also turns those stores into
/// pushes. The stores must cover the area; one left over is refused, as an
/// address no instruction encodes (`mov [sp+2], ax` stopped 32 QCport modules
/// from assembling).
///
/// A floating argument cannot be pushed (`fstp` stores to memory): an area with
/// one is stored through a register that holds the stack pointer, `mov bx, sp`
/// and `fstp [bx+k]`, as the old selector does.
fn pushed_arguments(
    insns: &mut Vec<Arc<Insn>>,
    sp: llrm_lir::registers::RegId,
    state: &mut EmitState,
    classes: &mut Vec<(u32, &'static str)>,
    reads: &HashMap<u32, usize>,
    split_wide: Option<i64>,
) -> Result<(), EmitError> {
    let through_sp =
        |cell: &Mem| cell.through == sp && cell.base.is_none() && cell.index.is_none() && cell.addr.is_none();
    let is_store = |insn: &Insn| match insn.what.as_ref() {
        Some(what) => matches!(&what.dests[..], [Loc::Mem(cell)] if through_sp(cell)),
        None => false,
    };
    let is_area = |insn: &Insn| {
        insn.what
            .as_ref()
            .is_some_and(
                |what| what.name.as_deref() == Some("sub")
                    && matches!(&what.dests[..], [Loc::Reg(reg)] if reg.register == sp)
                    && matches!(&what.sources[..], [_, Loc::Imm(_)]),
            )
    };
    let mut at = 0;
    let mut refused = false;
    // The areas stored through a register already.
    let mut based: Vec<i64> = Vec::new();
    while at < insns.len() {
        if !is_area(&insns[at]) || based.contains(&insns[at].at) {
            at += 1;
            continue;
        }
        let Some(Loc::Imm(size)) = insns[at].what.as_ref().and_then(|what| what.sources.get(1)).cloned() else {
            at += 1;
            continue;
        };
        let end = insns[at..]
            .iter()
            .position(|insn| insn.what.as_ref().is_some_and(|what| what.op == Operation::Call))
            .map(|found| at + found);
        let Some(end) = end else {
            at += 1;
            continue;
        };
        let mut stores: Vec<(usize, i64, u32)> = (at + 1..end)
            .filter(|&place| is_store(&insns[place]))
            .inspect(|&place| {
                // Only a move of a word or a dword can be a push.
                if insns[place].what.as_ref().is_some_and(|what| what.op != Operation::Move) {
                    refused = true;
                }
            })
            .map(|place| {
                let Some(Loc::Mem(cell)) = insns[place].what.as_ref().and_then(|what| what.dests.first()) else {
                    unreachable!("a store has its cell")
                };
                (place, cell.offset, cell.width)
            })
            .collect();
        // Pushes stand for the stores only when they tile the whole area: a gap
        // or a tail the stores leave (the padding of a 41-byte byval struct)
        // keeps the stores in place, each based on a copy of SP.
        let tiled = {
            let mut sorted: Vec<(i64, u32)> = stores.iter().map(|&(_, offset, width)| (offset, width)).collect();
            sorted.sort_unstable();
            let mut next = 0;
            sorted.iter().all(|&(offset, width)| {
                let here = offset == next;
                next += i64::from(width);
                here
            }) && next == size.value
        };
        if refused || !tiled {
            // The stores keep their places; each is based on a copy of SP.
            let value = state.next_value;
            state.next_value += 1;
            let base = Held { value, width: 2 };
            if let Some(tag) = class_tag(AddrMode::Real16, Role::Base) {
                classes.push((value, tag));
            }
            let places: Vec<usize> = stores.iter().map(|&(place, _, _)| place).collect();
            for place in places {
                let old = insns[place].clone();
                let mut what = old.what.clone().expect("a store");
                if let [Loc::Mem(cell)] = &mut what.dests[..] {
                    cell.through = llrm_lir::registers::RegId::None;
                    cell.base = Some(base);
                }
                let mut uses = old.uses.clone();
                uses.push(value);
                insns[place] = Arc::new(Insn::new(old.at, old.covers, Some(what), old.defines.clone(), uses));
            }
            let from = Semantics {
                name: Some("mov".into()),
                dests: vec![Loc::Held(base)],
                sources: vec![Loc::Reg(Reg { register: sp, width: 2 })],
                ..Semantics::new(Operation::Move)
            };
            let made = state.next_address();
            insns.insert(at + 1, Arc::new(Insn::new(made, Some((made, made)), Some(from), vec![value], vec![])));
            refused = false;
            based.push(insns[at].at);
            // Past the area; its stores are not pushes.
            at += 2;
            continue;
        }
        stores.sort_by_key(|&(_, offset, _)| offset);
        // The pushes go where the last store was; the area and the stores go.
        let last = stores.iter().map(|&(place, _, _)| place).max().unwrap_or(end - 1);
        let mut pushes = Vec::new();
        for &(place, _, _) in stores.iter().rev() {
            let old = &insns[place];
            let what = old.what.as_ref().expect("a store");
            let source = what.sources[0].clone();
            let uses = old.uses.clone();
            let halves = match (&source, split_wide) {
                (Loc::Imm(imm), Some(word)) => word_halves(imm, word).map(|halves| (halves, word)),
                _ => None,
            };
            let pusher = |source| Semantics {
                name: Some("push".into()),
                sources: vec![source],
                ..Semantics::new(Operation::Push)
            };
            let Some(((high, low), width)) = halves else {
                pushes.push(Arc::new(Insn::new(old.at, old.covers, Some(pusher(source)), vec![], uses)));
                continue;
            };
            // DEVIATE SD243: see `fold_immediate_pushes`
            let word = |half: i64| Loc::Imm(Imm { value: half, width: width as u32, address: None });
            let made = state.next_address();
            pushes.push(Arc::new(Insn::new(old.at, old.covers, Some(pusher(word(high))), vec![], uses)));
            pushes.push(Arc::new(Insn::new(made, Some((made, made)), Some(pusher(word(low))), vec![], vec![])));
        }
        let gone: std::collections::BTreeSet<usize> = stores.iter().map(|&(place, _, _)| place).chain([at]).collect();
        let mut rebuilt = Vec::with_capacity(insns.len());
        for (place, insn) in insns.drain(..).enumerate() {
            if place == last {
                rebuilt.append(&mut pushes);
            } else if !gone.contains(&place) {
                rebuilt.push(insn);
            }
        }
        *insns = rebuilt;
        fold_immediate_pushes(insns, reads, state, split_wide);
        // Back to the start: indices moved.
        at = 0;
        while at < insns.len() && !insns[at].what.as_ref().is_some_and(|w| w.op == Operation::Call) {
            at += 1;
        }
        at += 1;
    }
    if insns.iter().any(|insn| is_store(insn)) {
        return Err(EmitError::Unsupported("a store through the stack pointer at m16".to_owned()));
    }
    Ok(())
}

/// DEVIATE SD228: an outgoing argument area whose call is in a later block (a
/// select split the block between the area and the call) is made again at the
/// call, when what lies between neither calls nor moves the stack pointer: the
/// pushes `pushed_arguments` makes stay pushes, where `based_areas` would store
/// each argument through a copy of SP (`sub sp, 2; mov bx, sp; mov [bx], x`,
/// four bytes over a push in `jumps` and `range_power_digits`). The pin's
/// scheduler keeps such a select out of the call sequence; ours does not
/// always.
fn sunk_areas(
    blocks: &mut [EmitBlock],
    sp: llrm_lir::registers::RegId,
) {
    let through_sp =
        |cell: &Mem| cell.through == sp && cell.base.is_none() && cell.index.is_none() && cell.addr.is_none();
    let is_store = |insn: &Insn| match insn.what.as_ref() {
        Some(what) => matches!(&what.dests[..], [Loc::Mem(cell)] if through_sp(cell)),
        None => false,
    };
    let is_area = |insn: &Insn| {
        insn.what
            .as_ref()
            .is_some_and(
                |what| what.name.as_deref() == Some("sub")
                    && matches!(&what.dests[..], [Loc::Reg(reg)] if reg.register == sp)
                    && matches!(&what.sources[..], [_, Loc::Imm(_)]),
            )
    };
    let is_call = |insn: &Insn| insn.what.as_ref().is_some_and(|what| what.op == Operation::Call);
    let touches_sp = |insn: &Insn| {
        insn.what
            .as_ref()
            .is_some_and(
                |what| matches!(what.op, Operation::Push | Operation::Pop | Operation::Call)
                    || what.dests.iter().any(|dest| matches!(dest, Loc::Reg(reg) if reg.register == sp))
                    || what.sources.iter().any(|source| matches!(source, Loc::Reg(reg) if reg.register == sp)),
            )
    };
    for first in 0..blocks.len() {
        let mut at = 0;
        while at < blocks[first].insns.len() {
            if !is_area(&blocks[first].insns[at]) || blocks[first].insns[at..].iter().any(|one| is_call(one)) {
                at += 1;
                continue;
            }
            let Some(last) = (first + 1..blocks.len()).find(|&one| blocks[one].insns.iter().any(|insn| is_call(insn)))
            else {
                at += 1;
                continue;
            };
            let call = blocks[last].insns.iter().position(|insn| is_call(insn)).expect("a call");
            // Only stores and what leaves SP alone lie between.
            let clear = blocks[first].insns[at + 1..].iter().all(|insn| is_store(insn) || !touches_sp(insn))
                && (first + 1..last).all(|one| blocks[one].insns.iter().all(|insn| !touches_sp(insn)))
                && blocks[last].insns[..call].iter().all(|insn| is_store(insn) || !touches_sp(insn));
            if !clear {
                at += 1;
                continue;
            }
            let mut moved = vec![blocks[first].insns.remove(at)];
            let mut kept = Vec::new();
            for insn in blocks[first].insns.drain(at..) {
                if is_store(&insn) { moved.push(insn) } else { kept.push(insn) }
            }
            blocks[first].insns.extend(kept);
            for one in first + 1..last {
                let (stores, rest): (Vec<_>, Vec<_>) = blocks[one].insns.drain(..).partition(|insn| is_store(insn));
                moved.extend(stores);
                blocks[one].insns = rest;
            }
            let call = blocks[last].insns.iter().position(|insn| is_call(insn)).expect("a call");
            let tail = blocks[last].insns.split_off(call);
            let (stores, rest): (Vec<_>, Vec<_>) = blocks[last].insns.drain(..).partition(|insn| is_store(insn));
            moved.extend(stores);
            blocks[last].insns = rest;
            blocks[last].insns.extend(moved);
            blocks[last].insns.extend(tail);
        }
    }
}

/// An outgoing argument area whose call is in a later block (a select split the
/// block between the area and the call): its stores go through a register that
/// holds SP, from the area to the call, whichever block they are in.
fn based_areas(
    blocks: &mut [EmitBlock],
    sp: llrm_lir::registers::RegId,
    state: &mut EmitState,
    classes: &mut Vec<(u32, &'static str)>,
) {
    let through_sp =
        |cell: &Mem| cell.through == sp && cell.base.is_none() && cell.index.is_none() && cell.addr.is_none();
    let is_area = |insn: &Insn| {
        insn.what
            .as_ref()
            .is_some_and(
                |what| what.name.as_deref() == Some("sub")
                    && matches!(&what.dests[..], [Loc::Reg(reg)] if reg.register == sp)
                    && matches!(&what.sources[..], [_, Loc::Imm(_)]),
            )
    };
    let is_call = |insn: &Insn| insn.what.as_ref().is_some_and(|what| what.op == Operation::Call);
    let mut open: Option<Held> = None;
    for block in blocks {
        let mut at = 0;
        while at < block.insns.len() {
            let insn = block.insns[at].clone();
            if let Some(base) = open {
                if is_call(&insn) {
                    open = None;
                } else if let Some(mut what) = insn.what.clone()
                    && let [Loc::Mem(cell)] = &mut what.dests[..]
                    && through_sp(cell)
                {
                    cell.through = llrm_lir::registers::RegId::None;
                    cell.base = Some(base);
                    let mut uses = insn.uses.clone();
                    uses.push(base.value);
                    block.insns[at] = Arc::new(Insn::new(insn.at, insn.covers, Some(what), insn.defines.clone(), uses));
                }
            } else if is_area(&insn) && !block.insns[at..].iter().any(|one| is_call(one)) {
                let value = state.next_value;
                state.next_value += 1;
                let base = Held { value, width: 2 };
                if let Some(tag) = class_tag(AddrMode::Real16, Role::Base) {
                    classes.push((value, tag));
                }
                let from = Semantics {
                    name: Some("mov".into()),
                    dests: vec![Loc::Held(base)],
                    sources: vec![Loc::Reg(Reg { register: sp, width: 2 })],
                    ..Semantics::new(Operation::Move)
                };
                let made = state.next_address();
                block
                    .insns
                    .insert(at + 1, Arc::new(Insn::new(made, Some((made, made)), Some(from), vec![value], vec![])));
                open = Some(base);
                at += 1;
            }
            at += 1;
        }
    }
}

/// `X86::CondCode` order, the jump on each.
const CONDITIONS: [&str; 16] =
    ["jo", "jno", "jb", "jae", "je", "jne", "jbe", "ja", "js", "jns", "jp", "jnp", "jl", "jge", "jle", "jg"];

/// A LIR operand's width: bytes, as the old selector writes it.
fn width(
    sse: bool,
    vt: EVT,
) -> u32 {
    match vt {
        // With scalar SSE a floating value is the 4 or 8 bytes of an xmm
        // register.
        EVT::Simple(MVT::F32) if sse => 4,
        EVT::Simple(MVT::F64) if sse => 8,
        EVT::Simple(MVT::I1 | MVT::I8) => 1,
        EVT::Simple(MVT::I16) => 2,
        EVT::Simple(MVT::I32) => 4,
        EVT::Simple(MVT::I64) => 8,
        EVT::Int(bits) => bits.div_ceil(8),
        // A floating value is one 10-byte register value of the x87 stack.
        EVT::Simple(MVT::F32 | MVT::F64 | MVT::F80) => 10,
        _ => 4,
    }
}

/// What a direct call names.
#[derive(Clone, Debug)]
pub enum Callee {
    Global(u32),
    Named(String),
}

impl Emitter<'_, '_> {
    /// The loads of memory the node's operand says never changes, among the
    /// instructions made since `before` in the last block.
    /// The instructions emitted since `before` for a volatile access: the ones
    /// that touch memory keep the mark (`MachineMemOperand::isVolatile`).
    fn mark_volatile(
        &mut self,
        before: usize,
    ) {
        let Some(block) = self.blocks.last_mut() else { return };
        for one in block.insns.iter_mut().skip(before) {
            let touches = one
                .what
                .as_ref()
                .is_some_and(
                    |what| what.dests.iter().chain(&what.sources).any(|loc| matches!(loc, Loc::Mem(_)))
                        && matches!(
                            what.op,
                            Operation::Move | Operation::FloatLoad | Operation::FloatStore | Operation::Binary
                        ),
                );
            if touches {
                *one = Arc::new(Insn { volatile: true, ..(**one).clone() });
            }
        }
    }

    fn mark_invariant_loads(
        &mut self,
        before: usize,
    ) {
        let Some(block) = self.blocks.last_mut() else { return };
        for one in block.insns.iter_mut().skip(before) {
            let plain = one
                .what
                .as_ref()
                .is_some_and(
                    |what| matches!(what.op, Operation::Move | Operation::FloatLoad)
                        && matches!(what.sources.as_slice(), [Loc::Mem(_)]),
                );
            if plain && !one.volatile() {
                *one = Arc::new(Insn { invariant: true, ..(**one).clone() });
            }
        }
    }

    fn node(
        &mut self,
        n: u32,
    ) -> Result<(), EmitError> {
        let node = self.dag.node(SDValue { node: n, res: 0 });
        match node.opcode {
            ISD::CopyToReg if self.writes_flags(node.ops[1]) => {
                let made = self.ensure_flags(n, node.ops[2].node);
                self.flag_copies_done.insert(n);
                made
            }
            ISD::CopyToReg if self.is_flags_result(node.ops[2]) => Ok(()),
            // A selector put in ES for the cells that follow: a value, taken
            // by the next cell through ES.
            ISD::CopyToReg if self.is_es(node.ops[1]) => {
                let selector = self.held_operand(node.ops[2])?;
                self.es = Some(selector);
                self.es_copies.insert(n, selector);
                // SS itself, or a `CopyFromReg` of it.
                let value = self.dag.node(node.ops[2]);
                let is_ss = |v: SDValue| self.dag.node(v).data == NodeData::PhysReg(llrm_lir::registers::RegId::SS);
                if is_ss(node.ops[2])
                    || (value.opcode == ISD::CopyFromReg && value.ops.get(1).is_some_and(|r| is_ss(*r)))
                {
                    self.ss_copies.insert(n);
                }
                self.es_is_ss = self.ss_copies.contains(&n);
                Ok(())
            }
            ISD::CopyToReg
                if self.dag.node(node.ops[1]).data == NodeData::PhysReg(llrm_lir::registers::RegId::AH)
                    && self.glue_reaches(n, |name| name == "SAHF") =>
            {
                Ok(())
            }
            ISD::CopyToReg if self.feeds_call(n) => {
                let NodeData::PhysReg(register) = self.dag.node(node.ops[1]).data else {
                    return Err(EmitError::Unsupported("a virtual register passed to a call".to_owned()));
                };
                // An any-extended word (selected as an `INSERT_SUBREG` into an
                // undefined dword) is passed as the word it is.
                let value = node.ops[2];
                let source = if self.is_machine_named(value, "INSERT_SUBREG")
                    && self.is_implicit_def(self.dag.operand(value, 0))
                    && self.dag.value_type(self.dag.operand(value, 1)) == EVT::I16
                {
                    self.dag.operand(value, 1)
                } else {
                    value
                };
                let held = self.held_operand(source)?;
                self.arguments.push((held, self.argument_register(register, held.width)));
                Ok(())
            }
            // A value returned in a register is required there by the `RET`,
            // as a call's arguments are by the call: a plain copy to EAX is
            // no claim on EAX, and another return value held in it is lost.
            ISD::CopyToReg if self.feeds_return(n) => {
                let NodeData::PhysReg(register) = self.dag.node(node.ops[1]).data else {
                    return Err(EmitError::Unsupported("a virtual register returned".to_owned()));
                };
                let held = self.held_operand(node.ops[2])?;
                self.returns.push((held, register));
                Ok(())
            }
            ISD::CopyToReg if self.feeds_implicit_operand(n) => {
                let NodeData::PhysReg(register) = self.dag.node(node.ops[1]).data else {
                    return Err(EmitError::Unsupported("a virtual register in a division sequence".to_owned()));
                };
                let value = self.held_operand(node.ops[2])?;
                self.accumulator.insert(register, value);
                self.origins.insert(register, node.ops[2]);
                Ok(())
            }
            ISD::CopyToReg => {
                let dst = self.register(node.ops[1])?;
                let src = self.operand(node.ops[2])?;
                // The definition already wrote the register
                // (`exported_register`).
                if let (Loc::Held(to), Loc::Held(from)) = (&dst, &src)
                    && to.value == from.value
                {
                    return Ok(());
                }
                self.push(Operation::Move, "mov", vec![dst], vec![src]);
                Ok(())
            }
            // A runtime routine's answer in the flags (`LLRM_CALL_FLAGS`): the
            // call left them, the branch that reads them follows.
            ISD::CopyFromReg if self.writes_flags(node.ops[1]) => {
                self.flags_now = Some(n);
                Ok(())
            }
            ISD::CopyFromReg => {
                let result = SDValue { node: n, res: 0 };
                // The selector `LES16rm` left in ES, read on the chain it made.
                if self.is_es(node.ops[1])
                    && let Some(selector) = self.selectors.get(&node.ops[0].node).copied()
                {
                    self.values.insert(result, selector.value);
                    return Ok(());
                }
                match self.dag.node(node.ops[1]).data {
                    // `EmitCopyFromReg`: a virtual source is the value itself.
                    NodeData::Register(reg) => {
                        let value = self.vreg(reg);
                        self.values.insert(result, value);
                    }
                    NodeData::PhysReg(register) if register == self.ctx.stack_pointer => {
                        self.sp_values.insert(result);
                    }
                    // The x87 result of a call: the stack's top (`FP0`), which
                    // the stackifier reads as an unnamed load of no cell
                    // (`handleCall`'s `STReturns`).
                    NodeData::PhysReg(register)
                        if register == llrm_lir::registers::RegId::MM0
                            && self.dag.value_type(result).is_floating_point() =>
                    {
                        let held = self.fresh(10);
                        self.values.insert(result, held.value);
                        self.push(Operation::FloatLoad, "", vec![Loc::Held(held)], Vec::new());
                    }
                    NodeData::PhysReg(register) => {
                        // A quotient, remainder or product is the value the
                        // instruction made.
                        if self.reads_implicit_result(node.ops.get(2).copied()) {
                            let made = self.accumulated(register)?;
                            self.values.insert(result, made.value);
                            return Ok(());
                        }
                        let phys = Reg { register, width: width_of(self.ctx.map.scalar_sse(), self.dag, result) };
                        let held = self.fresh(width_of(self.ctx.map.scalar_sse(), self.dag, result));
                        // The result of a call is what the call delivers in
                        // that register.
                        let glued = self.reads_call_result(node.ops.get(2).copied());
                        if self.deliver(held, register, glued) {
                            self.values.insert(result, held.value);
                            return Ok(());
                        }
                        self.push(Operation::Move, "mov", vec![Loc::Held(held)], vec![Loc::Reg(phys)]);
                        self.values.insert(result, held.value);
                    }
                    _ => return Err(EmitError::Unsupported("CopyFromReg without a register".to_owned())),
                }
                Ok(())
            }
            ISD::Machine(opcode) => self.machine(n, opcode),
            // Chains, entry token, constants and registers are operands.
            _ => Ok(()),
        }
    }

    fn machine(
        &mut self,
        n: u32,
        opcode: u32,
    ) -> Result<(), EmitError> {
        let before = (self.blocks.len() - 1, self.blocks.last().map_or(0, |block| block.insns.len()));
        // `MOV32r0` is `xor` in the pin and writes the flags there; it is a
        // `mov` here, which a later pass makes an `xor` only where they are
        // dead.
        let writes_flags = self.ctx.names.get(&(opcode as u16)).is_none_or(|name| *name != "MOV32r0")
            && instr::desc(opcode as u16)
                .is_some_and(|desc| desc.implicit_defs.contains(&llrm_dagisel::generated::ids::x86::EFLAGS));
        let previous = self.flags_now.filter(|&p| p != n);
        // The scheduler may put two `CopyToReg ES` ahead of the cells that read
        // them: each cell reads its own.
        let node = self.dag.node(SDValue { node: n, res: 0 });
        if let Some((copy, selector)) =
            node.ops.iter().find_map(|op| self.es_copies.get(&op.node).map(|s| (op.node, *s)))
        {
            self.es = Some(selector);
            self.es_is_ss = self.ss_copies.contains(&copy);
        }
        self.machine_in(n, opcode)?;
        if writes_flags && let Some(previous) = previous {
            self.save_carry(previous, before);
        }
        // What it left in EFLAGS: a compare with no result is cheap to make
        // again where a consumer needs it (`ensure_flags`).
        if writes_flags {
            self.flags_now = Some(n);
            if let Some(last) = self.blocks.last().and_then(|block| block.insns.last()) {
                let again = last.what.as_ref().and_then(|what| match (what.op, what.name.as_deref()) {
                    (Operation::Compare, _) if what.dests.is_empty() => Some(Arc::clone(last)),
                    // A `sub` leaves the flags a `cmp` of its operands does, an
                    // `and` those of a `test`: made again without its result.
                    (Operation::Binary, Some(kind @ ("sub" | "and"))) if what.sources.len() == 2 => {
                        let compare = Semantics {
                            name: Some(if kind == "sub" { "cmp" } else { "test" }.into()),
                            sources: what.sources.clone(),
                            ..Semantics::new(Operation::Compare)
                        };
                        Some(Arc::new(Insn::new(last.at, last.covers, Some(compare), vec![], last.uses.clone())))
                    }
                    // An `add` has no cheaper form: made again with a result of
                    // its own, as the pin's scheduler clones the unit.
                    (Operation::Binary, Some("add")) if what.sources.len() == 2 => Some(Arc::clone(last)),
                    _ => None,
                });
                if let Some(again) = again {
                    self.flag_insns.insert(n, again);
                }
            }
        }
        Ok(())
    }

    /// The last instruction that defines `value` defines it `width` bytes wide.
    fn narrow_definition(
        &mut self,
        value: u32,
        width: u32,
    ) {
        let Some(block) = self.blocks.last_mut() else { return };
        for insn in block.insns.iter_mut().rev().take(8) {
            let Some(what) = insn.what.as_ref() else { continue };
            if !matches!(
                what.dests.as_slice(),
                [Loc::Held(held)] if held.value == value
            ) {
                continue;
            }
            let dests = vec![Loc::Held(Held { value, width })];
            // A zero's immediate narrows with it: the `xor` pass wants both
            // alike.
            let sources = match what.sources.as_slice() {
                [Loc::Imm(imm)] if imm.value == 0 => vec![Loc::Imm(Imm { width, ..imm.clone() })],
                _ => what.sources.clone(),
            };
            let narrowed = Semantics { dests, sources, ..what.clone() };
            *insn = Arc::new(Insn::new(insn.at, insn.covers, Some(narrowed), insn.defines.clone(), insn.uses.clone()));
            return;
        }
    }

    /// Whether the units glued after the flags copy `copy` are `adc`s and
    /// `sbb`s: they read the carry flag and no other.
    fn reads_only_carry(
        &self,
        copy: u32,
    ) -> bool {
        let users = self.glue_users.get(&SDValue { node: copy, res: 1 });
        users.is_some_and(|users| {
            !users.is_empty()
                && users.iter().all(|&user| {
                    let ISD::Machine(op) = self.dag.opcode(SDValue { node: user, res: 0 }) else { return false };
                    let name = self.ctx.names.get(&(op as u16)).copied().unwrap_or("");
                    let node = self.dag.node(SDValue { node: user, res: 0 });
                    // The condition codes B and AE test the carry alone.
                    let condition = if name.starts_with("CMOV_") {
                        node.ops.get(2).and_then(|op| self.dag.constant(*op))
                    } else if name == "SETCCr" || name == "JCC_1" {
                        node.ops.iter().find_map(|op| self.dag.constant(*op))
                    } else {
                        None
                    };
                    name.starts_with("ADC") || name.starts_with("SBB") || matches!(condition, Some(2 | 3))
                })
        })
    }

    /// The carry `previous` made, put in a byte before the insns node `before`
    /// pushed when a flags copy of it is still to come: the unit being emitted
    /// writes the flags first.
    fn save_carry(
        &mut self,
        previous: u32,
        before: (usize, usize),
    ) {
        let name = if let ISD::Machine(op) = self.dag.opcode(SDValue { node: previous, res: 0 }) {
            self.ctx.names.get(&(op as u16)).copied().unwrap_or("")
        } else {
            ""
        };
        let defines_carry = ["ADD", "ADC", "SUB", "SBB"].iter().any(|stem| name.starts_with(stem));
        let pending = (0..self.dag.node(SDValue { node: previous, res: 0 }).values.len() as u8).any(|res| {
            self.glue_users
                .get(&SDValue { node: previous, res })
                .is_some_and(
                    |users| users
                        .iter()
                        .any(
                            |&user| self.dag.opcode(SDValue { node: user, res: 0 }) == ISD::CopyToReg
                                && self.writes_flags(self.dag.node(SDValue { node: user, res: 0 }).ops[1])
                                && !self.flag_copies_done.contains(&user),
                        ),
                )
        });
        if !defines_carry || !pending || self.saved_carry.contains_key(&previous) {
            return;
        }
        let held = self.fresh(1);
        let what =
            Semantics { name: Some("setb".into()), dests: vec![Loc::Held(held)], ..Semantics::new(Operation::Unary) };
        let at = self.next_address();
        let insn = Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![held.value], vec![]));
        let (block, place) = before;
        if let Some(target) = self.blocks.get_mut(block) {
            target.insns.insert(place.min(target.insns.len()), insn);
            self.saved_carry.insert(previous, held);
        }
    }

    /// The flags `producer` made are what EFLAGS holds for the consumer glued
    /// after this: made again if something else wrote them since. The pin's
    /// scheduler may place a flag-writing unit between a compare and the second
    /// of two selects on it (the sink block of the first select reads EFLAGS
    /// live-in), and a pass here may place one anywhere: shift_int64_variable
    /// at -O2 branched on the flags of an unrelated `sub`.
    fn ensure_flags(
        &mut self,
        copy: u32,
        producer: u32,
    ) -> Result<(), EmitError> {
        if self.flags_now == Some(producer) {
            return Ok(());
        }
        // The carry of an `add` another unit's flags came between: restored
        // from its byte (`add byte, 0xFF` carries when the byte is 1)
        // for the `adc`/`sbb` that reads it, which reads nothing else.
        if let Some(&saved) = self.saved_carry.get(&producer)
            && self.reads_only_carry(copy)
        {
            let restored = self.fresh(1);
            let all_ones = Imm { value: 0xFF, width: 1, address: None };
            self.push(Operation::Binary, "add", vec![Loc::Held(restored)], vec![Loc::Held(saved), Loc::Imm(all_ones)]);
            self.flags_now = Some(producer);
            return Ok(());
        }
        let Some(compare) = self.flag_insns.get(&producer).cloned() else {
            return Err(EmitError::Unsupported("flags overwritten between their producer and a consumer".to_owned()));
        };
        let at = self.next_address();
        let mut again = Insn { at, covers: Some((at, at)), ..(*compare).clone() };
        // A `sub` whose value nothing reads, made again for its flags, writes a
        // value of its own: LIR defines each value once.
        if let Some(what) = again.what.as_mut() {
            for dest in &mut what.dests {
                if let Loc::Held(held) = dest {
                    let fresh = self.fresh(held.width);
                    *held = fresh;
                    again.defines = vec![fresh.value];
                }
            }
        }
        self.blocks.last_mut().expect("a block").insns.push(Arc::new(again));
        self.flags_now = Some(producer);
        Ok(())
    }

    fn machine_in(
        &mut self,
        n: u32,
        opcode: u32,
    ) -> Result<(), EmitError> {
        // `SEGMENT_OF` (sdag-c's pseudo, SD305): the segment word of a global.
        if opcode == u32::from(llrm_dagisel::x86_llrm::SEGMENT_OF_OPCODE) {
            let node = self.dag.node(SDValue { node: n, res: 0 });
            // The global rides as the operand (morph clears the node's data).
            let operand_data = node.ops.first().map(|op| self.dag.node(*op).data);
            let symbol = match (operand_data, node.data) {
                (Some(NodeData::Global { symbol, .. }), _) | (_, NodeData::Global { symbol, .. }) => Some(symbol),
                _ => None,
            };
            // Naming no global it is DGROUP's.
            let held = self.fresh(2);
            self.values.insert(SDValue { node: n, res: 0 }, held.value);
            let word = Imm { value: 0, width: 2, address: Some(self.ctx.globals.segment(symbol)) };
            self.push(Operation::Move, "mov", vec![Loc::Held(held)], vec![Loc::Imm(word)]);
            return Ok(());
        }
        let name =
            *self.ctx.names.get(&(opcode as u16)).ok_or_else(|| EmitError::Unsupported(format!("opcode {opcode}")))?;
        if name == "COPY" {
            return self.copy(n);
        }
        if let Some(done) = self.special(n, name)? {
            return Ok(done);
        }
        let desc =
            instr::desc(opcode as u16).ok_or_else(|| EmitError::Unsupported(format!("{name}: no descriptor")))?;
        if name == "JMP_1" || name == "JCC_1" {
            return self.jump(n, name == "JCC_1");
        }
        if name == "RET" {
            // A floating result is the stack's top at the return: the
            // unnamed store the stackifier reads as `handleReturn`.
            let node = self.dag.node(SDValue { node: n, res: 0 });
            for &op in &node.ops {
                if self.dag.value_type(op).is_floating_point() {
                    match self.operand(op)? {
                        Loc::Held(value) => self.push(Operation::FloatStore, "", Vec::new(), vec![Loc::Held(value)]),
                        // An xmm result: its `CopyToReg` already put it there.
                        Loc::Reg(_) => {}
                        _ => return Err(EmitError::Unsupported("a floating result that is no register".to_owned())),
                    }
                }
            }
            // The value is already in its register (`CopyToReg`); the frame
            // pass adds the epilogue before it.
            // Unnamed: the printer says `retf` in a far procedure and `ret` in
            // a near one.
            let requires = std::mem::take(&mut self.returns);
            let uses = requires.iter().map(|(held, _)| held.value).collect();
            let what = Semantics { name: Some("".into()), ..Semantics::new(Operation::Return) };
            let at = self.next_address();
            let insn =
                Insn { requires, reads_complete: true, ..Insn::new(at, Some((at, at)), Some(what), vec![], uses) };
            self.blocks.last_mut().expect("a block").insns.push(Arc::new(insn));
            return Ok(());
        }
        if matches!(
            name,
            "CMOV_GR8"
                | "CMOV_GR16"
                | "CMOV_GR32"
                | "CMOV_FR32"
                | "CMOV_FR64"
                | "CMOV_RFP32"
                | "CMOV_RFP64"
                | "CMOV_RFP80"
        ) {
            return self.lowered_select(n);
        }
        if name == "VASTART_SAVE_XMM_REGS" {
            return self.lowered_vastart_save_xmm(n);
        }
        // `FsFLD0SD`: a zero double, `xorps` of the register with itself.
        if matches!(name, "FsFLD0SD" | "FsFLD0SS") {
            let node = self.dag.node(SDValue { node: n, res: 0 });
            let zero = self.fresh(width(self.ctx.map.scalar_sse(), node.values[0]));
            self.values.insert(SDValue { node: n, res: 0 }, zero.value);
            // The idiom reads nothing: the instruction uses no value.
            let what = Semantics {
                name: Some("xorps".into()),
                dests: vec![Loc::Held(zero)],
                sources: vec![Loc::Held(zero), Loc::Held(zero)],
                ..Semantics::new(Operation::Binary)
            };
            let at = self.next_address();
            let insn = Insn::new(at, Some((at, at)), Some(what), vec![zero.value], vec![]);
            self.blocks.last_mut().expect("a block").insns.push(Arc::new(insn));
            return Ok(());
        }
        // `MOVZX32rr8_NOREX` is `movzx` of a byte register that needs no REX
        // prefix to name (AH): the same form at this target.
        let form = self
            .ctx
            .map
            .form(name)
            .or_else(|_| self.ctx.map.form(name.strip_suffix("_NOREX").unwrap_or(name)))
            .map_err(|_| EmitError::Unmapped(name.to_owned()))?;
        let form = &self.ctx.map.forms()[form];
        let node = self.dag.node(SDValue { node: n, res: 0 });
        // The results: the first `num_defs` values that are not chain or glue.
        // A clone of the unit (`CopyAndMoveSuccessors`) is there for the flags
        // it leaves: its results are values of their own, and the
        // readers keep the first one's.
        let cloned = !self.machine_emitted.insert(n);
        let mut dests = Vec::new();
        for (i, vt) in node.values.iter().enumerate().take(usize::from(desc.num_defs)) {
            let held = if cloned {
                self.fresh(width(self.ctx.map.scalar_sse(), *vt))
            } else {
                self.exported_register(SDValue { node: n, res: i as u8 })
                    .unwrap_or_else(|| self.fresh(width(self.ctx.map.scalar_sse(), *vt)))
            };
            if !cloned {
                self.values.insert(SDValue { node: n, res: i as u8 }, held.value);
            }
            dests.push(Loc::Held(held));
        }
        let cell_width =
            self.ctx.map.memory_bytes(name).unwrap_or_else(|| width_of_node(self.ctx.map.scalar_sse(), self.dag, n));
        let mut operands = self.operands(node.ops.as_slice(), desc, cell_width)?;
        // A memory destination (`ADD32mr`, no result): the cell is the
        // destination and its tied source.
        let memory_dest = dests.is_empty() && form.dests.first().is_some_and(|d| d.allows('m'));
        if memory_dest {
            let at = operands
                .iter()
                .position(|o| matches!(o, Loc::Mem(_)))
                .ok_or_else(|| EmitError::Unsupported(format!("{name}: no memory operand")))?;
            dests.push(operands.remove(at));
        }
        let mut sources = Vec::new();
        let mut rest = operands.into_iter();
        for source in &form.sources {
            let taken = match source.tied {
                Some(k) if memory_dest => dests.get(k).cloned(),
                _ => rest.next(),
            };
            // a shift by `cl` has the amount as the value the `CopyToReg` glued
            // to it wrote to `CL`; the form pins that source to `cx`
            let implicit = || {
                (name.ends_with("rCL") || name.ends_with("mCL"))
                    .then(|| self.accumulator.get(&llrm_lir::registers::RegId::CL).map(|held| Loc::Held(*held)))
                    .flatten()
            };
            sources.push(
                taken.or_else(implicit).ok_or_else(|| EmitError::Unsupported(format!("{name}: missing source")))?,
            );
        }
        if rest.next().is_some() {
            return Err(EmitError::Unsupported(format!("{name}: more operands than the form's sources")));
        }
        let op = Operation::named(&form.operation)
            .ok_or_else(|| EmitError::Unsupported(format!("{name}: operation {}", form.operation)))?;
        self.push(op, &form.name, dests, sources);
        Ok(())
    }

    /// `EmitLoweredSelect` for one `CMOV_GR*` pseudo (the pin's custom
    /// inserter): branch on the condition around a block that holds the false
    /// value, and join with a phi.
    ///
    /// ```text
    /// this:   jCC sink              this -> false, sink
    /// false:  (falls into sink)     false -> sink
    /// sink:   r := phi [false: src1] [this: src2]
    /// ```
    ///
    /// The operands are `src1` (the value when the condition is false),
    /// `src2`, and the condition code.
    fn lowered_select(
        &mut self,
        n: u32,
    ) -> Result<(), EmitError> {
        let node = self.dag.node(SDValue { node: n, res: 0 });
        let held = |loc: Loc| match loc {
            Loc::Held(held) => Ok(held),
            other => Err(EmitError::Unsupported(format!("a select of {other:?}"))),
        };
        let when_false = held(self.operand(node.ops[0])?)?;
        let when_true = held(self.operand(node.ops[1])?)?;
        let cc = self.constant(node.ops[2])? as usize;
        let jump = *CONDITIONS.get(cc).ok_or_else(|| EmitError::Unsupported(format!("condition code {cc}")))?;
        // `CreateVirtualRegisters`: a result only exported by a `CopyToReg` is
        // made in that register, so the export emits no copy between this
        // select and the next.
        let result = self
            .exported_register(SDValue { node: n, res: 0 })
            .unwrap_or_else(|| self.fresh(width(self.ctx.map.scalar_sse(), node.values[0])));
        self.values.insert(SDValue { node: n, res: 0 }, result.value);
        // The flags this select reads: the producer of the `CopyToReg EFLAGS`
        // glued to it.
        let producer =
            node.ops.iter().find(|op| self.dag.opcode(**op) == ISD::CopyToReg).map(|op| self.dag.node(*op).ops[2].node);
        // `EmitLoweredSelect`'s cascade: a select right after another on the
        // same flags and condition, reading neither's result, is one more phi
        // of the first's sink block (one branch, not two).
        if let Some(last) = self.last_select
            && Some(last.producer) == producer
            && last.cc == cc
            && self.blocks.last().is_some_and(|block| block.at == last.sink && block.insns.is_empty())
            && when_false.value != last.result
            && when_true.value != last.result
        {
            let phi = Phi {
                result: result.value,
                incoming: vec![(last.false_at, when_false.value), (last.this, when_true.value)],
            };
            self.blocks.last_mut().expect("a block").phis.push(phi);
            return Ok(());
        }
        let this = self.blocks.last().expect("a block").at;
        let (false_at, sink_at) = (self.next_address(), self.next_address());
        let target = Some(sink_at);
        let what = Semantics { target, name: Some(jump.into()), ..Semantics::new(Operation::Branch) };
        self.push_semantics(what);
        let phi = Phi { result: result.value, incoming: vec![(false_at, when_false.value), (this, when_true.value)] };
        self.blocks.push(EmitBlock { at: false_at, ..EmitBlock::default() });
        self.blocks.push(EmitBlock { at: sink_at, phis: vec![phi], ..EmitBlock::default() });
        self.last_select =
            producer.map(|producer| Select { producer, cc, this, false_at, sink: sink_at, result: result.value });
        Ok(())
    }

    /// `EmitVAStartSaveXMMRegsWithCustomInserter` (the pin's custom inserter):
    /// the vector argument registers the named parameters left are stored in
    /// the register save area unless `al` is zero.
    ///
    /// ```text
    /// this:  test al, al; je sink
    /// save:  movaps [area + offset + 16 i], xmm(first + i)   (falls into sink)
    /// sink:
    /// ```
    ///
    /// The operands are `al`, the save area's frame index and the offset of
    /// the first register's slot; the registers are read where the call left
    /// them, as nothing runs between the entry and this node.
    fn lowered_vastart_save_xmm(
        &mut self,
        n: u32,
    ) -> Result<(), EmitError> {
        use iced_x86::Register;
        const XMM: [Register; 8] = [
            Register::XMM0,
            Register::XMM1,
            Register::XMM2,
            Register::XMM3,
            Register::XMM4,
            Register::XMM5,
            Register::XMM6,
            Register::XMM7,
        ];
        let node = self.dag.node(SDValue { node: n, res: 0 });
        // The matcher's operands: `al`, the address the pattern names (frame
        // index, scale, index, displacement, segment), then the offset of the
        // first register's slot, then the chain.
        let operands: Vec<SDValue> =
            node.ops.iter().copied().filter(|op| self.dag.value_type(*op) != EVT::OTHER).collect();
        let [al, frame, _scale, _index, _disp, _segment, offset] = operands[..] else {
            return Err(EmitError::Unsupported(format!(
                "VASTART_SAVE_XMM_REGS with {} operands, not al, an address and an offset",
                operands.len()
            )));
        };
        let Loc::Held(al) = self.operand(al)? else {
            return Err(EmitError::Unsupported("VASTART_SAVE_XMM_REGS: al is no register".to_owned()));
        };
        let index = self.constant(frame)? as i32;
        let first = self.constant(offset)?;
        let area =
            self.ctx.frame.cell(index, 16).ok_or_else(|| EmitError::Unsupported(format!("frame index {index}")))?;
        let sink_at = self.next_address();
        self.push(Operation::Compare, "test", vec![], vec![Loc::Held(al), Loc::Held(al)]);
        let what = Semantics { target: Some(sink_at), name: Some("je".into()), ..Semantics::new(Operation::Branch) };
        self.push_semantics(what);
        let save_at = self.next_address();
        self.blocks.push(EmitBlock { at: save_at, ..EmitBlock::default() });
        let skipped = ((first - 48) / 16) as usize;
        for (at, register) in XMM.iter().enumerate().skip(skipped) {
            let disp = first + 16 * (at - skipped) as i64;
            let addr = area.addr.map(|addr| Addr { disp: addr.disp + disp, ..addr });
            let cell = Mem { addr, ..area.clone() };
            let source = Reg { register: llrm_lir::registers::RegId::new(*register), width: 16 };
            self.push(Operation::Move, "movaps", vec![Loc::Mem(cell)], vec![Loc::Reg(source)]);
        }
        self.blocks.push(EmitBlock { at: sink_at, ..EmitBlock::default() });
        Ok(())
    }

    fn next_address(&mut self) -> i64 {
        self.state.next_address()
    }

    /// Whether `register` is the flags register: LIR has no flags value, the
    /// branch that reads them follows the instruction that sets them, so the
    /// `CopyToReg EFLAGS` the matcher adds writes nothing.
    fn writes_flags(
        &self,
        register: SDValue,
    ) -> bool {
        self.dag.node(register).data == NodeData::PhysReg(super::dagisel_adapter::EFLAGS_ID)
    }

    /// Whether `value` is an implicit result of a machine node (EFLAGS from
    /// `SUB32rr`, say) rather than one of its defs: the same flags write
    /// when the register is not named.
    fn is_flags_result(
        &self,
        value: SDValue,
    ) -> bool {
        let ISD::Machine(op) = self.dag.opcode(value) else { return false };
        // The implicit results follow the explicit ones, in the order the
        // description lists them: `MUL8r` has AX before EFLAGS, and AX is not
        // flags.
        instr::desc(op as u16).is_some_and(|d| {
            usize::from(value.res)
                .checked_sub(usize::from(d.num_defs))
                .and_then(|implicit| d.implicit_defs.get(implicit))
                .is_some_and(|def| *def == 28)
        })
    }

    /// A word argument is required in the word register: the DAG names the
    /// convention's 32-bit register (`$eax` for an any-extended `i16`), and a
    /// word value required in `EAX` is a dword to the allocator, which spilled
    /// every pointer live across a call as a dword (QCport -Os: +16%).
    fn argument_register(
        &self,
        register: llrm_lir::registers::RegId,
        width: u32,
    ) -> llrm_lir::registers::RegId {
        let registers = self.ctx.registers;
        if width == 2 && register.is_gpr32() {
            return registers.view(registers.root(register), 16).unwrap_or(register);
        }
        register
    }

    /// The call instruction: it destroys the caller-saved registers and may
    /// touch any memory. Its address.
    fn push_call(
        &mut self,
        what: Semantics,
        callee: Option<&Callee>,
    ) -> i64 {
        let at = self.next_address();
        // The convention's destroyed registers and the ones the arguments
        // went in (the callee's to use); a result joins them in `deliver`.
        let written = self.ctx.conventions.written(callee);
        let clobbers = match &written {
            Some((whole, _)) => whole.clone(),
            None => {
                let mut clobbers =
                    self.ctx.conventions.clobbers(callee).unwrap_or_else(|| self.ctx.caller_saved.clone());
                clobbers.extend(self.arguments.iter().map(|(_, register)| *register));
                clobbers
            }
        };
        let memory = Arc::new(crate::model::lir::CallMemory {
            effects: llrm_mir::memory::Effects::ANY,
            private: Vec::new(),
            // A callee that pops its own arguments lists SP among what it
            // disturbs; the pushes and the call account for that, and a
            // disturbed SP makes the epilogue restore it (`moves_stack`).
            disturbs: self
                .ctx
                .conventions
                .disturbs(callee, &clobbers)
                .into_iter()
                .filter(|register| *register != self.ctx.stack_pointer)
                .collect(),
        });
        let mut uses = _read(&what);
        let requires = std::mem::take(&mut self.arguments);
        uses.extend(requires.iter().map(|(held, _)| held.value));
        let clobbers_high = match written {
            Some((_, high)) => high,
            None => self.ctx.conventions.clobbers_high(callee),
        };
        let insn = Insn {
            call: Some(memory),
            clobbers,
            clobbers_high,
            requires,
            ..Insn::new(at, Some((at, at)), Some(what), vec![], uses)
        };
        let block = self.blocks.last_mut().expect("a block");
        block.insns.push(Arc::new(insn));
        self.last_call = Some((self.blocks.len() - 1, self.blocks.last().expect("a block").insns.len() - 1));
        at
    }

    /// Names `held` as what the last call delivers in `register`; false if no
    /// call is waiting or the register is not one it destroys.
    fn deliver(
        &mut self,
        held: Held,
        register: llrm_lir::registers::RegId,
        glued: bool,
    ) -> bool {
        let Some((block, at)) = self.last_call else { return false };
        let call = Arc::make_mut(&mut self.blocks[block].insns[at]);
        // A result the node reads through the call's glue is delivered whatever
        // the convention's lists say: a callee may return in a register
        // (EDX of an i64) that its clobber list and the target's do not name.
        if !glued && !call.clobbers.contains(&register) && !self.ctx.caller_saved.contains(&register) {
            return false;
        }
        call.clobbers.insert(register);
        call.delivers.push((held, register));
        call.defines.push(held.value);
        true
    }

    /// The function a call's target operand names, if it names one.
    fn direct_callee(
        &self,
        target: SDValue,
    ) -> Option<Callee> {
        match self.dag.node(target).data {
            NodeData::Global { symbol, .. } => Some(Callee::Global(symbol)),
            // A runtime routine the lowering named (`memcpy_`).
            NodeData::Symbol(id) => Some(Callee::Named(self.dag.symbol_names[id as usize].clone())),
            _ => None,
        }
    }

    /// The readonly cell of constant pool entry `entry`.
    fn pool_cell(
        &self,
        entry: u32,
    ) -> Result<Mem, EmitError> {
        let (bits, vt) = self.dag.constant_pool[entry as usize];
        let size = match vt {
            EVT::Simple(MVT::F32) => 4,
            EVT::Simple(MVT::F64) => 8,
            EVT::Simple(MVT::F80) => 10,
            // The bits of an integer the lowering keeps in the pool (the
            // fudge of an unsigned conversion).
            EVT::Simple(MVT::I64) => 8,
            EVT::Simple(MVT::I32) => 4,
            // The 16-byte mask of `fabs`, `fneg` and `copysign` (the pin's
            // vector constant): the bitwise SSE instruction reads all of it.
            EVT::Simple(MVT::I128) => 16,
            _ => return Err(EmitError::Unsupported(format!("a constant pool entry of {vt:?}"))),
        };
        Ok(self.ctx.constants.cell(bits.to_le_bytes()[..size].to_vec()))
    }

    /// `value` as a held value, copied into one if it is a constant or a cell.
    fn held_operand(
        &mut self,
        value: SDValue,
    ) -> Result<Held, EmitError> {
        match self.operand(value)? {
            Loc::Held(held) => Ok(held),
            source => {
                let held = self.fresh(width_of(self.ctx.map.scalar_sse(), self.dag, value));
                self.push(Operation::Move, "mov", vec![Loc::Held(held)], vec![source]);
                Ok(held)
            }
        }
    }

    /// The value the accumulator register `register` holds: what a glued
    /// `CopyToReg` set, or an earlier division left.
    fn accumulated(
        &self,
        register: llrm_lir::registers::RegId,
    ) -> Result<Held, EmitError> {
        self.accumulator
            .get(&register)
            .copied()
            .ok_or_else(
                || EmitError::Unsupported(format!("a division operand ({register:?}) not set by a glued CopyToReg")),
            )
    }

    /// The `SAHF` that copies the status word `fnstsw` took from the compare
    /// `n` into the flags.
    fn status_reader(
        &self,
        n: u32,
    ) -> Option<u32> {
        let users = |value: SDValue| self.glue_users.get(&value).into_iter().flatten().copied();
        let status = users(SDValue { node: n, res: 0 })
            .find(|&user| self.is_machine_named(SDValue { node: user, res: 0 }, "FNSTSW16r"))?;
        users(SDValue { node: status, res: 0 })
            .flat_map(|byte| users(SDValue { node: byte, res: 0 }))
            .find_map(|copy| self.glue_reaching(copy, |name| name == "SAHF"))
    }

    /// What the instruction at the end of the glue chain from the `CopyToReg`
    /// node `n` is, if `wanted` accepts its name.
    fn glue_reaches(
        &self,
        n: u32,
        wanted: impl Fn(&str) -> bool,
    ) -> bool {
        self.glue_reaching(n, wanted).is_some()
    }

    /// The node `glue_reaches` found.
    fn glue_reaching(
        &self,
        n: u32,
        wanted: impl Fn(&str) -> bool,
    ) -> Option<u32> {
        let mut work = vec![SDValue { node: n, res: 1 }];
        while let Some(glue) = work.pop() {
            if usize::from(glue.res) >= self.dag.node(glue).values.len() {
                continue;
            }
            for &user in self.glue_users.get(&glue).into_iter().flatten() {
                match self.dag.opcode(SDValue { node: user, res: 0 }) {
                    ISD::CopyToReg => work.push(SDValue { node: user, res: 1 }),
                    ISD::Machine(op) => {
                        let name =
                            self.ctx.names.get(&(op as u16)).map_or("", |name| name.split('|').next().unwrap_or(name));
                        if wanted(name) {
                            return Some(user);
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    /// Whether the `CopyToReg` node `n` sets an implicit operand of the
    /// division, multiply or sign extension that its glue leads to.
    fn feeds_implicit_operand(
        &self,
        n: u32,
    ) -> bool {
        self.glue_reaches(n, |name| {
            let name = name.split('|').next().unwrap_or(name);
            matches!(name, "CDQ" | "CWD" | "CQO")
                || is_div_or_mul(name)
                || name.starts_with("REP_")
                || name.ends_with("rCL")
                || name.ends_with("mCL")
        })
    }

    /// Whether the `CopyToReg` node `n` passes an argument in a register to
    /// the call its glue leads to.
    fn feeds_return(
        &self,
        n: u32,
    ) -> bool {
        self.glue_reaches(n, |name| name.starts_with("RET"))
    }

    fn feeds_call(
        &self,
        n: u32,
    ) -> bool {
        self.glue_reaches(n, |name| name.starts_with("CALL") || name.starts_with("TCRETURN"))
    }

    /// Whether `glue` (the third operand of a `CopyFromReg`) comes from a call,
    /// through the glue of the reads of its other results.
    fn reads_call_result(
        &self,
        glue: Option<SDValue>,
    ) -> bool {
        let Some(mut glue) = glue else { return false };
        while self.dag.opcode(glue) == ISD::CopyFromReg
            || self.is_machine_named(glue, "MOVZX32rr8_NOREX")
            || self.is_machine_named(glue, "MOVSX32rr8_NOREX")
        {
            let Some(&next) = self.dag.node(glue).ops.last() else { return false };
            glue = next;
        }
        let ISD::Machine(op) = self.dag.opcode(glue) else { return false };
        // The results are read after the call sequence ends.
        self.ctx
            .names
            .get(&(op as u16))
            .is_some_and(
                |name| {
                    let name = name.split('|').next().unwrap_or(name);
                    name.starts_with("CALL") || name.starts_with("ADJCALLSTACKUP")
                },
            )
    }

    /// Whether `glue` (the third operand of a `CopyFromReg`) comes from a
    /// division, multiply or sign extension.
    fn reads_implicit_result(
        &self,
        glue: Option<SDValue>,
    ) -> bool {
        let Some(mut glue) = glue else { return false };
        // A second result is read through the glue of the first read.
        // The byte divide's remainder copy (`MOV[SZ]X32rr8_NOREX` of AH) sits
        // between the divide and the read of AL.
        while self.dag.opcode(glue) == ISD::CopyFromReg
            || self.is_machine_named(glue, "MOVZX32rr8_NOREX")
            || self.is_machine_named(glue, "MOVSX32rr8_NOREX")
        {
            let Some(&next) = self.dag.node(glue).ops.last() else { return false };
            glue = next;
        }
        let ISD::Machine(op) = self.dag.opcode(glue) else { return false };
        let name = self.ctx.names.get(&(op as u16)).map_or("", |name| name.split('|').next().unwrap_or(name));
        matches!(name, "CDQ" | "CWD" | "CQO") || is_div_or_mul(name)
    }

    /// (low, high, bytes): AX and DX, EAX and EDX, or RAX and RDX.
    fn accumulator_pair(
        &self,
        bytes: u32,
    ) -> Result<(llrm_lir::registers::RegId, llrm_lir::registers::RegId, u32), EmitError> {
        use llrm_lir::registers::RegId;
        Ok(match bytes {
            8 => (RegId::RAX, RegId::RDX, 8),
            4 => (RegId::EAX, RegId::EDX, 4),
            _ => (RegId::AX, RegId::DX, 2),
        })
    }

    /// Whether `register` is ES, named as the builder's physical register or as
    /// the pin's register number (the matcher's copies of the selector).
    fn is_es(
        &self,
        register: SDValue,
    ) -> bool {
        matches!(
            self.dag.node(register).data,
            NodeData::PhysReg(llrm_lir::registers::RegId::ES) | NodeData::Register(31)
        )
    }

    /// The offset word of the pointer in `register` for a real-mode string
    /// operation: the word a far pointer was joined from (`offset | selector <<
    /// 16`), so the join itself is not made (`shl; movzx; add; mov edi`);
    /// otherwise what the register was copied from.
    fn offset_of_pointer(
        &mut self,
        register: llrm_lir::registers::RegId,
    ) -> Result<Held, EmitError> {
        let held = self.accumulated(register)?;
        let Some(&origin) = self.origins.get(&register) else { return Ok(held) };
        let joined = ["OR32rr", "ADD32rr", "ADD32rr_DB"].iter().any(|name| self.is_machine_named(origin, name));
        if !joined || (self.huge_selector(origin).is_none() && self.high_half_of(origin).is_none()) {
            return Ok(held);
        }
        let low = self.dag.node(origin).ops.iter().copied().find(|&op| self.is_machine_named(op, "MOVZX32rr16"));
        let Some(low) = low else { return Ok(held) };
        let offset = self.dag.node(low).ops[0];
        let mut offset = self.held_operand(offset)?;
        offset.width = 2;
        Ok(offset)
    }

    /// Whether the pointer in `register` is a near one into DGROUP: the address
    /// of a global that lives there, or any near pointer the builder took
    /// for one (`segment_of_pointer` loads the same selector for these).
    fn is_dgroup_pointer(
        &self,
        register: llrm_lir::registers::RegId,
    ) -> bool {
        let Some(&origin) = self.origins.get(&register) else { return false };
        if self.selectors.contains_key(&origin.node) {
            return false;
        }
        if self.sp_values.contains(&origin) {
            return false;
        }
        let dgroup = self.ctx.globals.segment(None);
        let ISD::Machine(op) = self.dag.opcode(origin) else {
            return width_of(self.ctx.map.scalar_sse(), self.dag, origin) == 2;
        };
        let name = self.ctx.names.get(&(op as u16)).map_or("", |name| name.split('|').next().unwrap_or(name));
        let first = self.dag.node(origin).ops.first().copied();
        match (name, first.map(|op| self.dag.node(op).data.clone())) {
            ("MOV16ri", Some(NodeData::Global { symbol, .. })) => self.ctx.globals.segment(Some(symbol)) == dgroup,
            ("LEA16r", Some(NodeData::FrameIndex(_))) => false,
            ("LEA16r", _) if self.global_displacement(origin).is_some() => {
                self.ctx.globals.segment(self.global_displacement(origin)) == dgroup
            }
            ("OR32rr" | "ADD32rr" | "SHL32ri" | "ADD32rr_DB", _) => false,
            _ => width_of(self.ctx.map.scalar_sse(), self.dag, origin) == 2,
        }
    }

    /// Whether the pointer in `register` is the address of a frame object or
    /// the stack pointer itself: into SS.
    fn is_frame_pointer(
        &self,
        register: llrm_lir::registers::RegId,
    ) -> bool {
        let Some(&origin) = self.origins.get(&register) else { return false };
        if self.sp_values.contains(&origin) {
            return true;
        }
        let ISD::Machine(op) = self.dag.opcode(origin) else { return false };
        let name = self.ctx.names.get(&(op as u16)).map_or("", |name| name.split('|').next().unwrap_or(name));
        let first = self.dag.node(origin).ops.first().copied();
        name == "LEA16r" && first.is_some_and(|op| matches!(self.dag.node(op).data, NodeData::FrameIndex(_)))
    }

    /// The selector of the segment the pointer in `register` is in, as a value:
    /// DGROUP for the address of a global, SS for one of the frame, the
    /// selector loaded with a far pointer.
    fn segment_of_pointer(
        &mut self,
        register: llrm_lir::registers::RegId,
    ) -> Result<Held, EmitError> {
        let origin = *self
            .origins
            .get(&register)
            .ok_or_else(|| EmitError::Unsupported("a string operation on a pointer of no known origin".to_owned()))?;
        if let Some(&selector) = self.selectors.get(&origin.node) {
            return Ok(selector);
        }
        // A pointer held whole in a dword (a far pointer a call returned, a
        // huge one loaded): its selector is the high word, `x >> 16`. Only the
        // shapes below are known to be a word pointer into DGROUP, a frame or a
        // global.
        let whole = width_of(self.ctx.map.scalar_sse(), self.dag, origin) == 4;
        let known_shape = matches!(
            self.dag.opcode(origin),
            ISD::Machine(op) if self.ctx.names.get(&(op as u16)).is_some_and(|name| { matches!(name.split('|').next(), Some("OR32rr" | "ADD32rr" | "SHL32ri" | "ADD32rr_DB")) })
        );
        if whole && !known_shape {
            let value = self.held_operand(origin)?;
            let shifted = self.fresh(4);
            let sixteen = Imm { value: 16, width: 1, address: None };
            self.push(Operation::Binary, "shr", vec![Loc::Held(shifted)], vec![Loc::Held(value), Loc::Imm(sixteen)]);
            return Ok(Held { width: 2, ..shifted });
        }
        // The stack pointer itself (a byval argument's address) is in SS.
        if self.sp_values.contains(&origin) {
            let held = self.fresh(2);
            let ss = Loc::Reg(Reg { register: self.ctx.registers.stack_segment_register(), width: 2 });
            self.push(Operation::Move, "mov", vec![Loc::Held(held)], vec![ss]);
            return Ok(held);
        }
        let held = self.fresh(2);
        let source = if let ISD::Machine(op) = self.dag.opcode(origin) {
            let name = self.ctx.names.get(&(op as u16)).map_or("", |name| name.split('|').next().unwrap_or(name));
            let first = self.dag.node(origin).ops.first().copied();
            match (name, first.map(|op| self.dag.node(op).data.clone())) {
                ("MOV16ri", Some(NodeData::Global { symbol, .. })) => {
                    Loc::Imm(Imm { value: 0, width: 2, address: Some(self.ctx.globals.segment(Some(symbol))) })
                }
                ("LEA16r", Some(NodeData::FrameIndex(_))) => {
                    Loc::Reg(Reg { register: llrm_lir::registers::RegId::SS, width: 2 })
                }
                // `lea r, [index*scale + global]`: an element of a global
                ("LEA16r", _) if self.global_displacement(origin).is_some() => Loc::Imm(Imm {
                    value: 0,
                    width: 2,
                    address: Some(self.ctx.globals.segment(self.global_displacement(origin))),
                }),
                // `SHL32ri`: a huge pointer whose offset is zero, the `or`
                // folded away
                ("OR32rr" | "ADD32rr" | "SHL32ri" | "ADD32rr_DB", _) => {
                    // A huge pointer built as `offset | (selector << 16)`, or
                    // as the same sum (the halves are disjoint, so the combiner
                    // may turn the or into an add).
                    let Some(selector) = self.huge_selector(origin) else {
                        // `offset + (x & 0xFFFF0000)`: the segment is in the
                        // high word of the dword `x`.
                        let whole = self.high_half_of(origin).ok_or_else(|| {
                            EmitError::Unsupported("a string operation on a pointer of no known segment".to_owned())
                        })?;
                        let value = self.held_operand(whole)?;
                        let shifted = self.fresh(4);
                        let sixteen = Imm { value: 16, width: 1, address: None };
                        self.push(
                            Operation::Binary,
                            "shr",
                            vec![Loc::Held(shifted)],
                            vec![Loc::Held(value), Loc::Imm(sixteen)],
                        );
                        return Ok(Held { width: 2, ..shifted });
                    };
                    // The selector is a word: the low word of a dword whose
                    // high half it is (`x >> 16`), or the word itself.
                    let mut held = self.held_operand(selector)?;
                    held.width = 2;
                    return Ok(held);
                }
                // any other near pointer is into DGROUP, as the builder takes
                // it
                _ => Loc::Imm(Imm { value: 0, width: 2, address: Some(self.ctx.globals.segment(None)) }),
            }
        } else {
            Loc::Imm(Imm { value: 0, width: 2, address: Some(self.ctx.globals.segment(None)) })
        };
        self.push(Operation::Move, "mov", vec![Loc::Held(held)], vec![source]);
        Ok(held)
    }

    /// The global a `lea`'s displacement names.
    fn global_displacement(
        &self,
        lea: SDValue,
    ) -> Option<u32> {
        match self.dag.node(*self.dag.node(lea).ops.get(3)?).data {
            NodeData::Global { symbol, .. } => Some(symbol),
            _ => None,
        }
    }

    /// The dword `x` of a pointer `origin` = `offset + (x & 0xFFFF0000)`: a far
    /// pointer a call returned, with its offset replaced.
    fn high_half_of(
        &self,
        origin: SDValue,
    ) -> Option<SDValue> {
        let masked = *self.dag.node(origin).ops.iter().find(|op| self.is_machine_named(**op, "AND32ri"))?;
        let [whole, mask] = self.dag.node(masked).ops[..] else { return None };
        (self.constant(mask).ok()? as u32 == 0xFFFF_0000).then_some(whole)
    }

    /// The 16-bit selector of a huge pointer `origin` = `or(offset,
    /// shl(insert(undef, selector), 16))`.
    fn huge_selector(
        &self,
        origin: SDValue,
    ) -> Option<SDValue> {
        let shifted = if self.is_machine_named(origin, "SHL32ri") {
            origin
        } else {
            *self.dag.node(origin).ops.iter().find(|op| self.is_machine_named(**op, "SHL32ri"))?
        };
        let [inserted, amount] = self.dag.node(shifted).ops[..] else { return None };
        let NodeData::Const { bits: 16, .. } = self.dag.node(amount).data else { return None };
        // A pointer made of a dword (`inttoptr` of a call's result): the
        // combiner leaves `(x >> 16) << 16`, whose low word is the selector.
        if self.is_machine_named(inserted, "SHR32ri") {
            let [_, by] = self.dag.node(inserted).ops[..] else { return None };
            return matches!(self.dag.node(by).data, NodeData::Const { bits: 16, .. }).then_some(inserted);
        }
        if !self.is_machine_named(inserted, "INSERT_SUBREG") {
            return None;
        }
        let [undef, selector, _] = self.dag.node(inserted).ops[..] else { return None };
        (self.is_implicit_def(undef) && width_of(self.ctx.map.scalar_sse(), self.dag, selector) == 2)
            .then_some(selector)
    }

    /// Whether `value` is a machine node of the pin's opcode `name`.
    fn is_machine_named(
        &self,
        value: SDValue,
        name: &str,
    ) -> bool {
        let ISD::Machine(op) = self.dag.opcode(value) else { return false };
        self.ctx.names.get(&(op as u16)).is_some_and(|one| one.split('|').next() == Some(name))
    }

    /// Whether `value` is an `IMPLICIT_DEF` machine node.
    fn is_implicit_def(
        &self,
        value: SDValue,
    ) -> bool {
        let ISD::Machine(op) = self.dag.opcode(value) else { return false };
        self.ctx.names.get(&(op as u16)).is_some_and(|name| name.split('|').next() == Some("IMPLICIT_DEF"))
    }

    /// Opcodes `x86.instr` has no row for but LIR has a form of: the constant
    /// loads (`MOV32r0` is `mov r, 0`, which a later pass makes `xor r, r`),
    /// `MOV32ImmSExti8`, and `LEA32r`/`LEA16r` (`lea r, [address]`). `None`:
    /// not one of them.
    fn special(
        &mut self,
        n: u32,
        name: &str,
    ) -> Result<Option<()>, EmitError> {
        let node = self.dag.node(SDValue { node: n, res: 0 });
        let width = width(self.ctx.map.scalar_sse(), node.values[0]);
        let result = SDValue { node: n, res: 0 };
        match name.split('|').next().unwrap_or(name) {
            "ADJCALLSTACKDOWN32" | "ADJCALLSTACKUP32" | "ADJCALLSTACKDOWN64" | "ADJCALLSTACKUP64" => {
                let bytes = self.constant(node.ops[0])?;
                let popped = if name.starts_with("ADJCALLSTACKUP") { self.constant(node.ops[1])? } else { 0 };
                // What the call just before this took back is what the code
                // that reads the frame back (`cfi.rs`) must not count.
                if name.starts_with("ADJCALLSTACKUP")
                    && let Some((block, index)) = self.last_call
                {
                    let call = &self.blocks[block].insns[index];
                    if popped != 0 {
                        self.pops.push((call.at, popped));
                    }
                    // An indirect call's bytes are said apart: the frame
                    // readers refuse one that does not say,
                    // even for none (`LowerCallFrame` says
                    // it only where a frame was set up).
                    if call.what.as_ref().is_some_and(|what| what.indirect) {
                        self.indirect_pops.push((call.at, popped));
                    }
                }
                // The area is a multiple of the call alignment; the callee pops
                // `popped` of it, the caller the rest.
                let bytes = (bytes + self.ctx.call_alignment - 1) / self.ctx.call_alignment * self.ctx.call_alignment;
                // Flat 32-bit: the pin's pseudos, which `callframe.rs` turns
                // into pushes or `sub`/`add` (DEVIATE CF4).
                if self.ctx.address_mode == AddrMode::Flat32 {
                    if bytes != 0 {
                        let imm = |value: i64| Loc::Imm(Imm { value, width: 4, address: None });
                        let (pseudo, sources) = if name.starts_with("ADJCALLSTACKDOWN") {
                            ("call_frame_setup", vec![imm(bytes), imm(0), imm(0)])
                        } else {
                            ("call_frame_destroy", vec![imm(bytes), imm(popped)])
                        };
                        self.push(Operation::Barrier, pseudo, Vec::new(), sources);
                    }
                    return Ok(Some(()));
                }
                let (verb, count) =
                    if name.starts_with("ADJCALLSTACKDOWN") { ("sub", bytes) } else { ("add", bytes - popped) };
                if count != 0 {
                    // The stack pointer is a word at real mode, a quadword at
                    // 64 bits.
                    let word = match self.ctx.address_mode {
                        AddrMode::Real16 => 2,
                        AddrMode::Flat64 => 8,
                        AddrMode::Flat32 => 4,
                    };
                    let sp = Loc::Reg(Reg { register: self.ctx.stack_pointer, width: word });
                    let count = Loc::Imm(Imm { value: count, width: word, address: None });
                    self.push(Operation::Binary, verb, vec![sp.clone()], vec![sp, count]);
                }
                return Ok(Some(()));
            }
            // A sibling call: the frame pass puts the epilogue before this
            // return, which is a jump to the callee (`masm`).
            "TCRETURNdi" | "TCRETURNdi64" | "TCRETURN_16" => {
                // the pseudo keeps the node's operands: the chain, then the
                // callee
                let target = if name == "TCRETURN_16" { node.ops[1] } else { node.ops[0] };
                let callee = self
                    .direct_callee(target)
                    .ok_or_else(|| EmitError::Unsupported("a sibling call of a non-global".to_owned()))?;
                let at = self.next_address();
                let requires = std::mem::take(&mut self.arguments);
                let uses = requires.iter().map(|(held, _)| held.value).collect();
                let what = Semantics { name: Some("jmp".into()), ..Semantics::new(Operation::Return) };
                let insn = Insn { requires, ..Insn::new(at, Some((at, at)), Some(what), vec![], uses) };
                self.blocks.last_mut().expect("a block").insns.push(Arc::new(insn));
                self.calls.push((at, callee));
                return Ok(Some(()));
            }
            // An indirect sibling call: the callee's address is pinned to one
            // of EAX, ECX, EDX (`GR32_TC`) the arguments do not take, as the
            // epilogue restores every other register before the jump.
            "TCRETURNri" => {
                let Loc::Held(callee) = self.operand(node.ops[0])? else {
                    return Err(EmitError::Unsupported("a sibling call through a non-register".to_owned()));
                };
                use llrm_lir::registers::RegId;
                let mut requires = std::mem::take(&mut self.arguments);
                let free = [RegId::EAX, RegId::ECX, RegId::EDX]
                    .into_iter()
                    .find(|one| {
                        self.ctx.caller_saved.contains(one) && requires.iter().all(|(_, taken)| taken.root() != *one)
                    })
                    .ok_or_else(|| EmitError::Unsupported("a sibling call with no free register".to_owned()))?;
                requires.push((callee, free));
                let at = self.next_address();
                let mut uses: Vec<u32> = requires.iter().map(|(held, _)| held.value).collect();
                uses.dedup();
                let what = Semantics {
                    indirect: true,
                    name: Some("jmp".into()),
                    sources: vec![Loc::Held(callee)],
                    ..Semantics::new(Operation::Return)
                };
                let insn = Insn { requires, ..Insn::new(at, Some((at, at)), Some(what), vec![], uses) };
                self.blocks.last_mut().expect("a block").insns.push(Arc::new(insn));
                return Ok(Some(()));
            }
            "CALLpcrel32" | "CALL64pcrel32" => {
                // A call of a number (`inttoptr (i32 N to ptr)`): the address
                // in a register, called through it.
                if self.direct_callee(node.ops[0]).is_none()
                    && let NodeData::Const { bits, .. } = self.dag.node(node.ops[0]).data
                {
                    let width = if name == "CALL64pcrel32" { 8 } else { 4 };
                    let address = self.fresh(width);
                    self.push(
                        Operation::Move,
                        "mov",
                        vec![Loc::Held(address)],
                        vec![Loc::Imm(Imm { value: bits as i64, width, address: None })],
                    );
                    let what = Semantics {
                        indirect: true,
                        name: Some("call".into()),
                        sources: vec![Loc::Held(address)],
                        ..Semantics::new(Operation::Call)
                    };
                    self.push_call(what, None);
                    return Ok(Some(()));
                }
                let callee = self
                    .direct_callee(node.ops[0])
                    .ok_or_else(|| EmitError::Unsupported("a call of a non-global".to_owned()))?;
                let at = self.push_call(
                    Semantics { name: Some("call".into()), ..Semantics::new(Operation::Call) },
                    Some(&callee),
                );
                self.calls.push((at, callee));
                return Ok(Some(()));
            }
            // `llrm.ia16.code`: raw bytes laid down at the site, each frame
            // place its name patches in a displacement from BP; the site is a
            // call of the block's name to the passes, whose bytes the function
            // adds (`dag_function`).
            "CALL_CODE" => {
                let NodeData::Symbol(id) = self.dag.node(node.ops[1]).data else {
                    return Err(EmitError::Unsupported("inline code without its name".to_owned()));
                };
                let name = self.dag.symbol_names[id as usize].clone();
                let (_, places) = llrm_mir::intrinsics::code(&name)
                    .ok_or_else(|| EmitError::Unsupported("inline code that does not parse".to_owned()))?;
                let mut patched = Vec::new();
                for (op, (offset, addend)) in node.ops[2..].iter().zip(places) {
                    let NodeData::FrameIndex(index) = self.dag.node(*op).data else {
                        return Err(EmitError::Unsupported("inline code naming other than a frame place".to_owned()));
                    };
                    let cell = self
                        .ctx
                        .frame
                        .cell(index, 2)
                        .ok_or_else(|| EmitError::Unsupported(format!("frame index {index}")))?;
                    let Some(addr) = cell.addr else {
                        return Err(EmitError::Unsupported("inline code naming a place of no frame".to_owned()));
                    };
                    patched.push((offset, addr.disp, addend));
                }
                let callee = Callee::Named(name);
                let at = self.push_call(
                    Semantics { name: Some("call".into()), ..Semantics::new(Operation::Call) },
                    Some(&callee),
                );
                self.calls.push((at, callee));
                self.inline_places.push((at, patched));
                return Ok(Some(()));
            }
            // `llrm.ia16.in`/`out`: a barrier the later passes give `in`/`out`
            // forms; the port is an immediate byte or `dx`. A node with a
            // value result reads.
            "PORT_IO" => {
                let reading = node.values.first().is_some_and(|vt| *vt != EVT::OTHER);
                let port = match self.dag.node(node.ops[1]).data {
                    NodeData::Const { bits, .. } if (0..256).contains(&(bits as i64)) => {
                        Loc::Imm(Imm { value: bits as i64, width: 1, address: None })
                    }
                    _ => self.operand(node.ops[1])?,
                };
                if reading {
                    let held = self.fresh(width_of(self.ctx.map.scalar_sse(), self.dag, SDValue { node: n, res: 0 }));
                    self.values.insert(SDValue { node: n, res: 0 }, held.value);
                    self.push(Operation::Barrier, "in", vec![Loc::Held(held)], vec![port]);
                } else {
                    let value = self.operand(node.ops[2])?;
                    self.push(Operation::Barrier, "out", Vec::new(), vec![port, value]);
                }
                return Ok(Some(()));
            }
            // `llrm.ia16.asm`: a block of bytes laid down at the site, its
            // inputs in the registers the name states, as a call of its name
            // that clobbers what the name says. A block that names symbols is
            // not translated.
            "CALL_ASM" => {
                let NodeData::Symbol(id) = self.dag.node(node.ops[1]).data else {
                    return Err(EmitError::Unsupported("inline assembly without its name".to_owned()));
                };
                let name = self.dag.symbol_names[id as usize].clone();
                let block = llrm_mir::intrinsics::asm(&name)
                    .ok_or_else(|| EmitError::Unsupported("inline assembly that does not parse".to_owned()))?;
                if !block.fixups.is_empty() {
                    return Err(EmitError::Unsupported("inline assembly naming symbols".to_owned()));
                }
                let callee = Callee::Named(name);
                let at = self.push_call(
                    Semantics { name: Some("call".into()), ..Semantics::new(Operation::Call) },
                    Some(&callee),
                );
                self.calls.push((at, callee));
                return Ok(Some(()));
            }
            // A call through a far pointer: the offset and the selector sit in
            // a four-byte cell, which the call reads (`call dword ptr [cell]`).
            "CALL_FAR" => {
                let NodeData::FrameIndex(index) = self.dag.node(node.ops[1]).data else {
                    return Err(EmitError::Unsupported("a far call without a cell".to_owned()));
                };
                let cell = self
                    .ctx
                    .frame
                    .cell(index, 4)
                    .ok_or_else(|| EmitError::Unsupported(format!("frame index {index}")))?;
                let what = Semantics {
                    indirect: true,
                    name: Some("call".into()),
                    sources: vec![Loc::Mem(cell)],
                    ..Semantics::new(Operation::Call)
                };
                self.push_call(what, None);
                return Ok(Some(()));
            }
            // A call through a register or a cell of memory.
            "CALL32r" | "CALL32m" | "CALL32r_NT" | "CALL32m_NT" | "CALL16r" | "CALL16m" | "CALL16r_NT"
            | "CALL16m_NT" | "CALL64r" | "CALL64m" | "CALL64r_NT" | "CALL64m_NT" => {
                // At real mode the matcher spells a call of a symbol `CALL16r`
                // of its address: a direct call, not one through an immediate
                // (`call offset _report`; 79 programs of the m16 run).
                if name[6..].starts_with('r')
                    && let Some(callee) = self.direct_callee(node.ops[0])
                {
                    let at = self.push_call(
                        Semantics { name: Some("call".into()), ..Semantics::new(Operation::Call) },
                        Some(&callee),
                    );
                    self.calls.push((at, callee));
                    return Ok(Some(()));
                }
                let bytes = match &name[4..6] {
                    "16" => 2,
                    "64" => 8,
                    _ => 4,
                };
                let target = if name[6..].starts_with('m') {
                    let group = node.ops.get(..5).ok_or_else(|| EmitError::Unsupported("a short call".to_owned()))?;
                    Loc::Mem(self.cell(group, bytes)?)
                } else {
                    self.operand(node.ops[0])?
                };
                let what = Semantics {
                    indirect: true,
                    name: Some("call".into()),
                    sources: vec![target],
                    ..Semantics::new(Operation::Call)
                };
                self.push_call(what, None);
                return Ok(Some(()));
            }
            // A string move or fill repeated ECX times: the matcher names ESI,
            // EDI, ECX (and the stored AL, AX or EAX) with `CopyToReg` around
            // it, which the form pins.
            // The real-mode string operations: ES:DI (and DS:SI) are the
            // segments the pointers live in, values of their own.
            "REP_STOSB_16" | "REP_STOSW_16" | "REP_STOSD_16" => {
                use llrm_lir::registers::RegId;
                let stored = match name.as_bytes()[8] {
                    b'B' => RegId::AL,
                    b'W' => RegId::AX,
                    _ => RegId::EAX,
                };
                let (value, count, destination) =
                    (self.accumulated(stored)?, self.accumulated(RegId::CX)?, self.offset_of_pointer(RegId::DI)?);
                let segment = self.segment_of_pointer(RegId::DI)?;
                let (destination_after, emptied) = (self.fresh(2), self.fresh(2));
                self.push(
                    Operation::Fill,
                    &name[4..9].to_ascii_lowercase(),
                    vec![Loc::Mem(Mem::new(None, 0)), Loc::Held(destination_after), Loc::Held(emptied)],
                    vec![Loc::Held(value), Loc::Held(count), Loc::Held(destination), Loc::Held(segment)],
                );
                self.values.insert(SDValue { node: n, res: 0 }, destination_after.value);
                return Ok(Some(()));
            }
            "REP_MOVSB_16" | "REP_MOVSW_16" | "REP_MOVSD_16" => {
                let Some(strings) = self.ctx.strings else {
                    return Err(EmitError::Unsupported(
                        "a segmented string operation at a target without segments".to_owned(),
                    ));
                };
                let (counter, to_offset, from_offset, from_segment) =
                    (strings.count, strings.destination, strings.source, strings.source_segment);
                // DEVIATE SD224 (real mode): a move glued straight after
                // another moves one block from where it
                // stopped, with no count: `movsw` or `movsb`
                // for the bytes past the last dword.
                let continued = node
                    .ops
                    .last()
                    .is_some_and(
                        |&glue| matches!(
                            self.dag.opcode(glue),
                            ISD::Machine(op) if self.ctx.names.get(&(op as u16)).is_some_and(|name| name.starts_with("REP_MOVS"))
                        ),
                    );
                if continued {
                    let Some((from, to)) = self.string_segments.clone() else {
                        return Err(EmitError::Unsupported("a string move glued to none".to_owned()));
                    };
                    let (source, destination) = (self.accumulated(from_offset)?, self.accumulated(to_offset)?);
                    let (source_after, destination_after) = (self.fresh(2), self.fresh(2));
                    self.push(
                        Operation::Copy,
                        &name[4..9].to_ascii_lowercase(),
                        vec![Loc::Mem(Mem::new(None, 0)), Loc::Held(source_after), Loc::Held(destination_after)],
                        vec![Loc::Held(source), Loc::Held(destination), from, to],
                    );
                    self.accumulator.insert(from_offset, source_after);
                    self.accumulator.insert(to_offset, destination_after);
                    return Ok(Some(()));
                }
                let (count, source, destination) = (
                    self.accumulated(counter)?,
                    self.offset_of_pointer(from_offset)?,
                    self.offset_of_pointer(to_offset)?,
                );
                // a source in DGROUP is read through DS, which holds it: no
                // segment to load and no override
                let from = if self.is_dgroup_pointer(from_offset) {
                    Loc::Reg(Reg { register: from_segment, width: 2 })
                } else if self.is_frame_pointer(from_offset) {
                    // a frame object or the argument area: SS, read with an
                    // override and no selector copied
                    Loc::Reg(Reg { register: self.ctx.registers.stack_segment_register(), width: 2 })
                } else {
                    Loc::Held(self.segment_of_pointer(from_offset)?)
                };
                let to = self.segment_of_pointer(to_offset)?;
                let (source_after, destination_after, emptied) = (self.fresh(2), self.fresh(2), self.fresh(2));
                self.string_segments = Some((from.clone(), Loc::Held(to)));
                self.push(
                    Operation::Copy,
                    &name[4..9].to_ascii_lowercase(),
                    vec![
                        Loc::Mem(Mem::new(None, 0)),
                        Loc::Held(source_after),
                        Loc::Held(destination_after),
                        Loc::Held(emptied),
                    ],
                    vec![Loc::Held(count), Loc::Held(source), Loc::Held(destination), from, Loc::Held(to)],
                );
                // a `rep` glued after this one goes on from where it stopped
                self.accumulator.insert(counter, emptied);
                self.accumulator.insert(from_offset, source_after);
                self.accumulator.insert(to_offset, destination_after);
                return Ok(Some(()));
            }
            "REP_MOVSB_32" | "REP_MOVSW_32" | "REP_MOVSD_32" | "REP_MOVSB_64" | "REP_MOVSW_64" | "REP_MOVSD_64"
            | "REP_MOVSQ_64" => {
                use llrm_lir::registers::RegId;
                let wide = name.ends_with("_64");
                let (counter, from, to) =
                    if wide { (RegId::RCX, RegId::RSI, RegId::RDI) } else { (RegId::ECX, RegId::ESI, RegId::EDI) };
                let size = if wide { 8 } else { 4 };
                let (count, source, destination) =
                    (self.accumulated(counter)?, self.accumulated(from)?, self.accumulated(to)?);
                let (source_after, destination_after, emptied) = (self.fresh(size), self.fresh(size), self.fresh(size));
                self.push(
                    Operation::Copy,
                    &name[4..9].to_ascii_lowercase(),
                    vec![
                        Loc::Mem(Mem::new(None, 0)),
                        Loc::Held(source_after),
                        Loc::Held(destination_after),
                        Loc::Held(emptied),
                    ],
                    vec![Loc::Held(count), Loc::Held(source), Loc::Held(destination)],
                );
                return Ok(Some(()));
            }
            "REP_STOSB_32" | "REP_STOSW_32" | "REP_STOSD_32" | "REP_STOSB_64" | "REP_STOSW_64" | "REP_STOSD_64"
            | "REP_STOSQ_64" => {
                use llrm_lir::registers::RegId;
                let wide = name.ends_with("_64");
                let stored = match (name.as_bytes()[8], wide) {
                    (b'B', _) => RegId::AL,
                    (b'W', _) => RegId::AX,
                    (b'D', _) => RegId::EAX,
                    _ => RegId::RAX,
                };
                let (counter, to) = if wide { (RegId::RCX, RegId::RDI) } else { (RegId::ECX, RegId::EDI) };
                let size = if wide { 8 } else { 4 };
                let (value, count, destination) =
                    (self.accumulated(stored)?, self.accumulated(counter)?, self.accumulated(to)?);
                let (destination_after, emptied) = (self.fresh(size), self.fresh(size));
                self.push(
                    Operation::Fill,
                    &name[4..9].to_ascii_lowercase(),
                    vec![Loc::Mem(Mem::new(None, 0)), Loc::Held(destination_after), Loc::Held(emptied)],
                    vec![Loc::Held(value), Loc::Held(count), Loc::Held(destination)],
                );
                return Ok(Some(()));
            }
            // A far pointer loaded from memory: the offset and, beside it, the
            // selector, as the two word loads `farload` joins into a `les`.
            // The pin leaves the selector in ES, which the cells that use it
            // name; here it is a value of its own, taken by the cells based on
            // this offset.
            "LES16rm" => {
                let group = node.ops.get(..5).ok_or_else(|| EmitError::Unsupported("a short les".to_owned()))?;
                let low = self.cell(group, 2)?;
                let high = Mem {
                    offset: low.offset + 2,
                    addr: low.addr.map(|addr| Addr { disp: addr.disp + 2, ..addr }),
                    ..low.clone()
                };
                let (offset, selector) = (self.fresh(2), self.fresh(2));
                self.values.insert(SDValue { node: n, res: 0 }, offset.value);
                self.selectors.insert(n, selector);
                self.push(Operation::Move, "mov", vec![Loc::Held(offset)], vec![Loc::Mem(low)]);
                self.push(Operation::Move, "mov", vec![Loc::Held(selector)], vec![Loc::Mem(high)]);
                return Ok(Some(()));
            }
            // A float compare: the stackifier follows the `fcom` with the
            // `fnstsw ax; sahf` that put its result in EFLAGS, so those pin
            // pseudos (`FNSTSW16r`, the copy of AH, `SAHF`) emit nothing and
            // `SAHF` is the producer of the flags.
            // `UCOM_FpIr*` (`fucomip`) sets EFLAGS itself: the same compare,
            // with no status word between it and its user.
            _ if name.starts_with("UCOM_Fpr")
                || name.starts_with("COM_Fpr")
                || name.starts_with("UCOM_FpIr")
                || name.starts_with("COM_FpIr") =>
            {
                let (Loc::Held(left), Loc::Held(right)) = (self.operand(node.ops[0])?, self.operand(node.ops[1])?)
                else {
                    return Err(EmitError::Unsupported(format!("{name}: an operand that is no register")));
                };
                match self.status_reader(n) {
                    Some(sahf) => {
                        self.pending_fcom.insert(sahf, (left, right));
                    }
                    None => self.push(Operation::Compare, "fcom", Vec::new(), vec![Loc::Held(left), Loc::Held(right)]),
                }
                return Ok(Some(()));
            }
            "FNSTSW16r" => {
                let held = self.fresh(2);
                self.values.insert(SDValue { node: n, res: 0 }, held.value);
                return Ok(Some(()));
            }
            "SAHF" => {
                if let Some((left, right)) = self.pending_fcom.remove(&n) {
                    self.push(Operation::Compare, "fcom", Vec::new(), vec![Loc::Held(left), Loc::Held(right)]);
                }
                return Ok(Some(()));
            }
            // Division and the one-operand multiply work in AX:DX or EAX:EDX,
            // which the matcher names with `CopyToReg`/`CopyFromReg` around
            // them: the LIR form says the pair in its operands.
            "CDQ" | "CWD" | "CQO" => {
                let (low, high, bytes) = self.accumulator_pair(match name {
                    "CQO" => 8,
                    "CDQ" => 4,
                    _ => 2,
                })?;
                let source = self.accumulated(low)?;
                let sign = self.fresh(bytes);
                self.accumulator.insert(high, sign);
                self.push(
                    Operation::Extend,
                    match name {
                        "CQO" => "cqo",
                        "CDQ" => "cdq",
                        _ => "cwd",
                    },
                    vec![Loc::Held(sign)],
                    vec![Loc::Held(source)],
                );
                return Ok(Some(()));
            }
            // The remainder of the byte divide: AH of the word AX, shifted down
            // and extended (the pin's `movzx` of AH).
            "MOVZX32rr8_NOREX" | "MOVSX32rr8_NOREX"
                if matches!(
                    self.dag.node(node.ops[0]).data,
                    NodeData::PhysReg(llrm_lir::registers::RegId::AH)
                ) =>
            {
                let word = self.accumulated(llrm_lir::registers::RegId::AX)?;
                let high = self.fresh(2);
                let eight = Imm { value: 8, width: 1, address: None };
                self.push(Operation::Binary, "shr", vec![Loc::Held(high)], vec![Loc::Held(word), Loc::Imm(eight)]);
                let wide = self.fresh(4);
                let mnemonic = if name == "MOVSX32rr8_NOREX" { "movsx" } else { "movzx" };
                self.push(
                    Operation::Extend,
                    mnemonic,
                    vec![Loc::Held(wide)],
                    vec![Loc::Held(Held { width: 1, ..high })],
                );
                self.values.insert(SDValue { node: n, res: 0 }, wide.value);
                return Ok(Some(()));
            }
            _ if is_div_or_mul(name) => {
                let (signed, divide, bytes, memory) = div_or_mul(name);
                if bytes == 1 && divide {
                    // AX divided by the byte: AL the quotient, AH the
                    // remainder, both in the word AX the form defines.
                    let dividend = self.accumulated(llrm_lir::registers::RegId::AX)?;
                    let divisor = if memory {
                        let group =
                            node.ops.get(..5).ok_or_else(|| EmitError::Unsupported("a short div".to_owned()))?;
                        Loc::Mem(self.cell(group, 1)?)
                    } else {
                        self.operand(node.ops[0])?
                    };
                    let word = self.fresh(2);
                    self.push(
                        Operation::Divide,
                        if signed { "idiv" } else { "div" },
                        vec![Loc::Held(word)],
                        vec![Loc::Held(dividend), divisor],
                    );
                    self.values.insert(SDValue { node: n, res: 0 }, word.value);
                    self.accumulator.insert(llrm_lir::registers::RegId::AX, word);
                    self.accumulator.insert(llrm_lir::registers::RegId::AL, Held { width: 1, ..word });
                    return Ok(Some(()));
                }
                // The byte multiply is AL times the operand into AX: the form
                // pins AX and DX like the wider ones, the product is AX.
                let (low, high, wide) = if bytes == 1 {
                    (llrm_lir::registers::RegId::AL, llrm_lir::registers::RegId::DX, 2)
                } else {
                    self.accumulator_pair(bytes)?
                };
                let divisor = if memory {
                    let group = node.ops.get(..5).ok_or_else(|| EmitError::Unsupported("a short div".to_owned()))?;
                    Loc::Mem(self.cell(group, bytes)?)
                } else {
                    self.operand(node.ops[0])?
                };
                let mnemonic = match (divide, signed) {
                    (true, true) => "idiv",
                    (true, false) => "div",
                    (false, true) => "imul",
                    (false, false) => "mul",
                };
                // The form pins the quotient and the low product to AX, the
                // remainder and the high product to DX (`x86.instr`'s fixed
                // column), so the operands are values, not registers.
                let (first, second) = (self.fresh(wide), self.fresh(wide));
                if divide {
                    let (hi, lo) = (self.accumulated(high)?, self.accumulated(low)?);
                    self.push(
                        Operation::Divide,
                        mnemonic,
                        vec![Loc::Held(first), Loc::Held(second)],
                        vec![Loc::Held(hi), Loc::Held(lo), divisor],
                    );
                } else {
                    let lo = self.accumulated(low)?;
                    self.push(
                        Operation::Multiply,
                        mnemonic,
                        vec![Loc::Held(first), Loc::Held(second)],
                        vec![Loc::Held(lo), divisor],
                    );
                }
                // The product of the byte form is the word AX; the node's own
                // result is that value too, for a reader that does not go
                // through the register.
                self.values.insert(SDValue { node: n, res: 0 }, first.value);
                self.accumulator.insert(if bytes == 1 { llrm_lir::registers::RegId::AX } else { low }, first);
                self.accumulator.insert(high, second);
                return Ok(Some(()));
            }
            // The low byte, word or dword of a register is the same value
            // viewed narrower (`Held { width, ..held }`); the high byte
            // (AH) is a different register.
            "EXTRACT_SUBREG" => {
                let index = self.constant(node.ops[1])?;
                // AH of the status word (`FNSTSW16r`) is read by `SAHF`
                // alone, which the compare's `fcom` stands for.
                let status = self.is_machine_named(node.ops[0], "FNSTSW16r");
                if !matches!(index, 1 | 2 | 4 | 6) {
                    return Err(EmitError::Unsupported(format!("EXTRACT_SUBREG index {index}")));
                }
                let Loc::Held(source) = self.operand(node.ops[0])? else {
                    return Err(EmitError::Unsupported("EXTRACT_SUBREG of a non-value".to_owned()));
                };
                // The high byte of a word is the word shifted right by eight,
                // viewed as its low byte.
                if index == 2 && !status {
                    let shifted = self.fresh(source.width);
                    let eight = Imm { value: 8, width: 1, address: None };
                    self.push(
                        Operation::Binary,
                        "shr",
                        vec![Loc::Held(shifted)],
                        vec![Loc::Held(source), Loc::Imm(eight)],
                    );
                    self.values.insert(result, shifted.value);
                    return Ok(Some(()));
                }
                // The word cut from a `movzx` of a byte, when the dword has no
                // other reader, is that `movzx` made at word
                // width (`movzx ax, al`): held as the dword it was
                // zero-extended to, a word that lives across a call is spilled
                // and reloaded as a dword.
                let only_user = self.glue_users.get(&node.ops[0]).is_some_and(|users| users.len() == 1);
                if index == 4 && !status && only_user && self.is_machine_named(node.ops[0], "MOVZX32rr8") {
                    self.narrow_definition(source.value, 2);
                }
                // DEVIATE SD237 (m16): the pin's `(i8 0)` and `(i16 0)` are the
                // low part of a `MOV32r0`; a dword `xor` is 3 bytes here (the
                // 66 prefix), the word's or byte's 2. A zero read only narrow
                // is made narrow.
                if matches!(index, 1 | 4) && !status && only_user && self.is_machine_named(node.ops[0], "MOV32r0") {
                    self.narrow_definition(source.value, if index == 1 { 1 } else { 2 });
                }
                self.values.insert(result, source.value);
                return Ok(Some(()));
            }
            // `SUBREG_TO_REG 0, x, sub_32bit`: x in the low dword of a
            // quadword whose high half is zero. A dword write zeroes it, so
            // the copy is that write.
            "SUBREG_TO_REG" => {
                // The pin's operands are (0, x, index); the node keeps (x,
                // index), and a folded load's chain after them.
                let is_constant = |op: SDValue| matches!(self.dag.node(op).data, NodeData::Const { .. });
                let (source, index) = match node.ops[..] {
                    [source, index, ..] if is_constant(index) => (source, self.constant(index)?),
                    [_, source, index, ..] => (source, self.constant(index)?),
                    _ => return Err(EmitError::Unsupported("SUBREG_TO_REG operands".to_owned())),
                };
                if index != 6 {
                    return Err(EmitError::Unsupported(format!("SUBREG_TO_REG index {index}")));
                }
                let Loc::Held(source) = self.operand(source)? else {
                    return Err(EmitError::Unsupported("SUBREG_TO_REG of a non-value".to_owned()));
                };
                let held = self.fresh(source.width);
                self.push(Operation::Move, "mov", vec![Loc::Held(held)], vec![Loc::Held(source)]);
                self.values.insert(result, held.value);
                return Ok(Some(()));
            }
            // A register-class constraint is the allocator's business.
            "COPY_TO_REGCLASS" => {
                let Loc::Held(source) = self.operand(node.ops[0])? else {
                    return Err(EmitError::Unsupported("COPY_TO_REGCLASS of a non-value".to_owned()));
                };
                self.values.insert(result, source.value);
                return Ok(Some(()));
            }
            // `ProcessImplicitDefs` turns the reads into undef reads and
            // erases it, as for the old selector's undef.
            "IMPLICIT_DEF" => {
                let held = self.fresh(width);
                self.values.insert(result, held.value);
                self.push(Operation::Nothing, crate::processimpdefs::IMPLICIT_DEF, vec![Loc::Held(held)], vec![]);
                return Ok(Some(()));
            }
            // `INSERT_SUBREG undef, x, idx`: x in the low part of a wider
            // value.
            "INSERT_SUBREG" => {
                if !self.is_implicit_def(node.ops[0]) {
                    return Err(EmitError::Unsupported("INSERT_SUBREG into a defined register".to_owned()));
                }
                let Loc::Held(source) = self.operand(node.ops[1])? else {
                    return Err(EmitError::Unsupported("INSERT_SUBREG of a non-value".to_owned()));
                };
                self.values.insert(result, source.value);
                return Ok(Some(()));
            }
            _ => {}
        }
        let constant = match name {
            "MOV32r0" => Some(0),
            "MOV32r1" => Some(1),
            "MOV32r_1" => Some(-1),
            _ => None,
        };
        if let Some(value) = constant {
            let held = self.result_register(SDValue { node: n, res: 0 }, width);
            self.values.insert(SDValue { node: n, res: 0 }, held.value);
            self.push(
                Operation::Move,
                "mov",
                vec![Loc::Held(held)],
                vec![Loc::Imm(Imm { value, width, address: None })],
            );
            return Ok(Some(()));
        }
        match name {
            // A jump through a table: the entry at the table plus the index.
            "JMP32m" | "JMP16m" => {
                let group = node.ops.get(..5).ok_or_else(|| EmitError::Unsupported("a short jump".to_owned()))?;
                // The table is the displacement, or the base is its address.
                let base = self.dag.node(group[0]).ops.first().map(|op| self.dag.node(*op).data.clone());
                let table = match (self.dag.node(group[3]).data.clone(), base) {
                    (NodeData::JumpTable(table), _) | (_, Some(NodeData::JumpTable(table))) => table,
                    _ => return Err(EmitError::Unsupported("an indirect jump through memory".to_owned())),
                };
                let jump = self.jump_table(table)?;
                // The cell is `$J + index * entry` whichever way the table's
                // address came: the masm names the table from this address.
                let index = self.address_register(group[2], Role::Index)?;
                let scale = self.constant(group[1])?;
                let mut address = Addr::new(crate::model::ir::Space::Segment, 0);
                address.index = jump.id;
                let entry = if name == "JMP16m" { 2 } else { 4 };
                // At real mode a lone register is the address's base, as the
                // old selector makes it (16-bit addressing has no scale).
                let (base, index) =
                    if self.ctx.address_mode == AddrMode::Real16 { (index, None) } else { (None, index) };
                let cell = Mem { base, index, scale, disp_width: 2, ..Mem::new(Some(address), entry) };
                let targets = jump.targets;
                let what = Semantics { indirect: true, table: targets, ..Semantics::new(Operation::Jump) };
                self.push_semantics(Semantics { name: Some("jmp".into()), sources: vec![Loc::Mem(cell)], ..what });
                Ok(Some(()))
            }
            "MOV32ImmSExti8" => {
                let held = self.result_register(SDValue { node: n, res: 0 }, width);
                self.values.insert(SDValue { node: n, res: 0 }, held.value);
                let source = self.operand(node.ops[0])?;
                self.push(Operation::Move, "mov", vec![Loc::Held(held)], vec![source]);
                Ok(Some(()))
            }
            // The conditional move (P6 and later, every x86-64): the second
            // source replaces the first, tied to the result, when the
            // condition holds.
            "CMOV16rr" | "CMOV32rr" | "CMOV64rr" => {
                let cc = self.constant(node.ops[2])? as usize;
                let jump = CONDITIONS.get(cc).ok_or_else(|| EmitError::Unsupported(format!("condition code {cc}")))?;
                let (Loc::Held(when_false), Loc::Held(when_true)) =
                    (self.operand(node.ops[0])?, self.operand(node.ops[1])?)
                else {
                    return Err(EmitError::Unsupported("a conditional move of a non-register".to_owned()));
                };
                let held = self.result_register(SDValue { node: n, res: 0 }, width);
                self.values.insert(result, held.value);
                let name = format!("cmov{}", &jump[1..]);
                self.push(
                    Operation::Select,
                    &name,
                    vec![Loc::Held(held)],
                    vec![Loc::Held(when_false), Loc::Held(when_true)],
                );
                Ok(Some(()))
            }
            // The same from memory: the second source is the cell, read only
            // when the condition holds (`CMOV64rm src1, [mem], cc`).
            "CMOV16rm" | "CMOV32rm" | "CMOV64rm" => {
                let cc = self.constant(node.ops[6])? as usize;
                let jump = CONDITIONS.get(cc).ok_or_else(|| EmitError::Unsupported(format!("condition code {cc}")))?;
                let Loc::Held(when_false) = self.operand(node.ops[0])? else {
                    return Err(EmitError::Unsupported("a conditional move of a non-register".to_owned()));
                };
                let group = node.ops.get(1..6).ok_or_else(|| EmitError::Unsupported("a short cmov".to_owned()))?;
                let cell = self.cell(group, u32::from(width))?;
                let held = self.result_register(SDValue { node: n, res: 0 }, width);
                self.values.insert(result, held.value);
                let name = format!("cmov{}", &jump[1..]);
                self.push(Operation::Select, &name, vec![Loc::Held(held)], vec![Loc::Held(when_false), Loc::Mem(cell)]);
                Ok(Some(()))
            }
            "SETCCr" => {
                let cc = node
                    .ops
                    .iter()
                    .find_map(|op| self.dag.constant(*op))
                    .ok_or_else(|| EmitError::Unsupported("a setcc without a condition".to_owned()))?
                    as usize;
                let jump = CONDITIONS.get(cc).ok_or_else(|| EmitError::Unsupported(format!("condition code {cc}")))?;
                let held = self.result_register(SDValue { node: n, res: 0 }, 1);
                self.values.insert(SDValue { node: n, res: 0 }, held.value);
                let name = format!("set{}", &jump[1..]);
                self.push(Operation::Unary, &name, vec![Loc::Held(held)], Vec::new());
                Ok(Some(()))
            }
            // A condition stored straight to a byte of memory.
            "SETCCm" => {
                let group = node.ops.get(..5).ok_or_else(|| EmitError::Unsupported("a short setcc".to_owned()))?;
                let cc = node
                    .ops
                    .get(5)
                    .and_then(|op| self.dag.constant(*op))
                    .ok_or_else(|| EmitError::Unsupported("a setcc without a condition".to_owned()))?
                    as usize;
                let jump = CONDITIONS.get(cc).ok_or_else(|| EmitError::Unsupported(format!("condition code {cc}")))?;
                let cell = self.cell(group, 1)?;
                let name = format!("set{}", &jump[1..]);
                self.push(Operation::Unary, &name, vec![Loc::Mem(cell)], Vec::new());
                Ok(Some(()))
            }
            "LEA32r" | "LEA16r" | "LEA64r" | "LEA64_32r" => {
                // a `lea` only computes the address: a scaled index is fine
                let cell = self.cell_unchecked(
                    node.ops.get(..5).ok_or_else(|| EmitError::Unsupported("a short lea".to_owned()))?,
                    width,
                )?;
                // 16-bit addressing has no scale and the encoder no field for a
                // symbol's displacement beside a lone index: the unscaled
                // index is the address's base.
                let mut cell = cell;
                //
                // A frame cell (`through` is `bp`) is not the symbol's case:
                // its index is already beside a base, and
                // moving it made `bp` its own index.
                if self.ctx.address_mode == AddrMode::Real16
                    && cell.addr.is_some()
                    && cell.through == llrm_lir::registers::RegId::default()
                    && cell.base.is_none()
                    && cell.index.is_some()
                {
                    self.unscale_index(&mut cell)?;
                    cell.base = cell.index.take();
                }
                // A 16-bit index beside a scale has no encoding (the scale
                // was dropped, the address lost its doubling): shifted first.
                if self.ctx.address_mode == AddrMode::Real16 && cell.index.is_some_and(|index| index.width == 2) {
                    self.unscale_index(&mut cell)?;
                }
                // A scaled index beside a base is the 386's 32-bit form: its
                // literal place adds nothing and has no encoding with a scale.
                if self.ctx.address_mode == AddrMode::Real16
                    && cell.base.is_some()
                    && cell.index.is_some()
                    && cell.scale != 1
                    && cell.addr.is_some_and(|addr| addr.space == crate::model::ir::Space::Literal)
                {
                    cell.offset = cell.addr.map_or(cell.offset, |addr| addr.disp);
                    cell.addr = None;
                }
                let held = self.fresh(width);
                self.values.insert(SDValue { node: n, res: 0 }, held.value);
                // The address of a symbol alone is its offset as an immediate
                // (`mov r, offset sym`, which the encoder makes `lea r,
                // [rip+sym]`), not an address value: the passes fold a `lea`
                // into the operands that read it, and `sym[index]` is no
                // address at 64 bits.
                if self.ctx.address_mode == AddrMode::Flat64
                    && cell.base.is_none()
                    && cell.index.is_none()
                    && let Some(address) = cell.addr
                    && matches!(
                        address.space,
                        crate::model::ir::Space::Segment
                            | crate::model::ir::Space::External
                            | crate::model::ir::Space::Group
                    )
                {
                    let offset = Imm { value: 0, width, address: Some(address) };
                    self.push(Operation::Move, "mov", vec![Loc::Held(held)], vec![Loc::Imm(offset)]);
                    return Ok(Some(()));
                }
                self.push(
                    Operation::Address,
                    "lea",
                    vec![Loc::Held(held)],
                    // An `AddressRef` names registers, not the values a base or
                    // an index is: a symbol plus a value (`lea [sym + 12 - x]`)
                    // stays a cell.
                    vec![if cell.addr.is_some() && cell.base.is_none() && cell.index.is_none() {
                        Loc::Address(cell.address())
                    } else {
                        Loc::Mem(cell)
                    }],
                );
                Ok(Some(()))
            }
            _ => self.x87(n, name),
        }
    }

    /// The register an x87 result is defined in: the one a `CopyToReg` of it
    /// names (`CreateVirtualRegisters`), else a fresh value. Made apart, the
    /// result and its export were two values with a copy between them, and the
    /// coalescer's terminal rule kept a loop's copy of the old value
    /// (`recfloat` -O2: 10 instructions a trip against the pin's 9).
    fn x87_result(
        &mut self,
        result: SDValue,
    ) -> Held {
        let exported = self
            .dag
            .users(result)
            .find_map(
                |user| {
                    let node = self.dag.node(SDValue { node: user, res: 0 });
                    if self.dag.opcode(SDValue { node: user, res: 0 }) != ISD::CopyToReg || node.ops[2] != result {
                        return None;
                    }
                    match self.dag.node(node.ops[1]).data {
                        NodeData::Register(reg) => Some(reg),
                        _ => None,
                    }
                },
            );
        match exported {
            Some(reg) => Held { value: self.vreg(reg), width: 10 },
            None => self.fresh(10),
        }
    }

    /// The pin's x87 pseudos (`LD_Fp64m`, `ST_Fp64m32`, `ADD_Fp80m64`,
    /// `ILD_Fp32m64`, `CHS_Fp32` ...) as the forms the stackifier reads:
    /// `FloatLoad` `fld` of a cell, `fldz`, `fld1`, `fild`; `FloatStore`
    /// `fst`, `fist`; `FloatArith` `fadd` ... on two values or a value and
    /// a cell (`fiadd` for an integer cell); `FloatUnary` `fchs`, `fabs`,
    /// `fsqrt`. A floating value is one 10-byte `Held` whatever its
    /// register class; the cell says the memory size. `None`: not one of
    /// them.
    fn x87(
        &mut self,
        n: u32,
        name: &str,
    ) -> Result<Option<()>, EmitError> {
        // `FP64_TO_INT32_IN_MEM`: the truncating store of a float as an
        // integer. It is `fistt` (the pin's custom inserter writes the
        // control-word dance around a `fist`); `floatalloc` makes `fistt` what
        // a 387 has, once for a run of them.
        if let Some(bits) = name
            .strip_prefix("FP")
            .and_then(|rest| rest.split_once("_TO_INT"))
            .and_then(|(_, rest)| rest.strip_suffix("_IN_MEM"))
            .and_then(|bits| bits.parse::<u32>().ok())
        {
            let node = self.dag.node(SDValue { node: n, res: 0 });
            let group = node.ops.get(..5).ok_or_else(|| EmitError::Unsupported(format!("a short {name}")))?;
            let cell = self.cell(group, bits / 8)?;
            let Loc::Held(source) = self.operand(node.ops[5])? else {
                return Err(EmitError::Unsupported(format!("{name}: a stored value that is no register")));
            };
            self.push(Operation::FloatStore, "fisttp", vec![Loc::Mem(cell)], vec![Loc::Held(source)]);
            return Ok(Some(()));
        }
        let Some((prefix, rest)) = name.split_once("_Fp") else { return Ok(None) };
        let digits = |text: &str| -> (u32, String) {
            let end = text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len());
            (text[..end].parse().unwrap_or(0), text[end..].to_owned())
        };
        let node = self.dag.node(SDValue { node: n, res: 0 });
        let result = SDValue { node: n, res: 0 };
        let group = |from: usize| -> Result<Vec<SDValue>, EmitError> {
            node.ops
                .get(from..from + 5)
                .map(<[SDValue]>::to_vec)
                .ok_or_else(|| EmitError::Unsupported(format!("a short {name}")))
        };
        match prefix {
            // A constant is loaded where it is read: a value held on the x87
            // stack across a call cannot stay there (see `operand`).
            "LD" if rest.len() == 3 && rest.starts_with(['0', '1']) => {
                let mnemonic = if rest.starts_with('0') { "fldz" } else { "fld1" };
                self.lazy.insert(result, (mnemonic, None));
            }
            "LD" if matches!(self.dag.node(group(0)?[3]).data, NodeData::PoolIndex(..)) => {
                let (first, _) = digits(rest);
                let cell = self.cell(&group(0)?, first / 8)?;
                self.lazy.insert(result, ("fld", Some(cell)));
            }
            "LD" | "ILD" => {
                let (first, _) = digits(rest);
                let cell = self.cell(&group(0)?, first / 8)?;
                let held = self.x87_result(result);
                self.values.insert(result, held.value);
                let mnemonic = if prefix == "LD" { "fld" } else { "fild" };
                self.push(Operation::FloatLoad, mnemonic, vec![Loc::Held(held)], vec![Loc::Mem(cell)]);
            }
            "ST" | "IST" => {
                let rest = rest.strip_prefix('P').unwrap_or(rest);
                let (first, after) = digits(rest);
                let size = match prefix {
                    "IST" => first,
                    _ => after
                        .strip_prefix('m')
                        .map(digits)
                        .filter(|(second, _)| *second != 0)
                        .map_or(first, |(second, _)| second),
                };
                let cell = self.cell(&group(0)?, size / 8)?;
                let Loc::Held(source) = self.operand(node.ops[5])? else {
                    return Err(EmitError::Unsupported(format!("{name}: a stored value that is no register")));
                };
                let mnemonic = if prefix == "ST" { "fst" } else { "fistp" };
                self.push(Operation::FloatStore, mnemonic, vec![Loc::Mem(cell)], vec![Loc::Held(source)]);
            }
            "CHS" | "ABS" | "SQRT" | "LN" | "EXP" | "LOG2" | "EXP2" | "SIN" | "COS" | "ATAN" | "RNDINT" => {
                let Loc::Held(source) = self.operand(node.ops[0])? else {
                    return Err(EmitError::Unsupported(format!("{name}: an operand that is no register")));
                };
                let held = self.x87_result(result);
                self.values.insert(result, held.value);
                let mnemonic = format!("f{}", prefix.to_ascii_lowercase());
                self.push(Operation::FloatUnary, &mnemonic, vec![Loc::Held(held)], vec![Loc::Held(source)]);
                // The sequence pushes above its operand: the stack's top slots
                // are not free for a value live across it (`fexp2` overflowed
                // the stack with six doubles live: pow_stack_depth).
                let scratch = llrm_x86::select::x87_scratch(&mnemonic);
                if scratch > 0 {
                    let last = self.blocks.last_mut().and_then(|block| block.insns.last_mut()).expect("the insn");
                    Arc::get_mut(last)
                        .expect("a fresh instruction")
                        .clobbers
                        .extend(self.ctx.registers.rfp_registers().iter().rev().take(scratch).copied());
                }
            }
            "ADD" | "SUB" | "SUBR" | "MUL" | "DIV" | "DIVR" => {
                let Loc::Held(left) = self.operand(node.ops[0])? else {
                    return Err(EmitError::Unsupported(format!("{name}: an operand that is no register")));
                };
                let held = self.x87_result(result);
                self.values.insert(result, held.value);
                let op = prefix.to_ascii_lowercase();
                let (mnemonic, right) = if let Some(integer) = rest.strip_prefix('I') {
                    let (bits, _) = digits(integer);
                    (format!("fi{op}"), Loc::Mem(self.cell(&group(1)?, bits / 8)?))
                } else {
                    let (first, after) = digits(rest);
                    match after.strip_prefix('m') {
                        Some(tail) => {
                            let (second, _) = digits(tail);
                            let size = if second == 0 { first } else { second };
                            (format!("f{op}"), Loc::Mem(self.cell(&group(1)?, size / 8)?))
                        }
                        None => {
                            let Loc::Held(right) = self.operand(node.ops[1])? else {
                                return Err(EmitError::Unsupported(format!("{name}: an operand that is no register")));
                            };
                            (format!("f{op}"), Loc::Held(right))
                        }
                    }
                };
                self.push(Operation::FloatArith, &mnemonic, vec![Loc::Held(held)], vec![Loc::Held(left), right]);
            }
            _ => return Ok(None),
        }
        Ok(Some(()))
    }

    /// `JMP_1 dest` and `JCC_1 dest, cc`: a jump or a conditional branch to a
    /// MIR block.
    fn jump(
        &mut self,
        n: u32,
        conditional: bool,
    ) -> Result<(), EmitError> {
        let node = self.dag.node(SDValue { node: n, res: 0 });
        let block = node
            .ops
            .iter()
            .find_map(|op| match self.dag.node(*op).data {
                NodeData::Block(id) => Some(id),
                _ => None,
            })
            .ok_or_else(|| EmitError::Unsupported("a jump without a block".to_owned()))?;
        let target = *self.ctx.block_at.get(&block).ok_or_else(|| EmitError::Unsupported(format!("block {block}")))?;
        let (op, name) = if conditional {
            let cc = node
                .ops
                .iter()
                .find_map(|op| self.dag.constant(*op))
                .ok_or_else(|| EmitError::Unsupported("a branch without a condition".to_owned()))?
                as usize;
            (
                Operation::Branch,
                *CONDITIONS.get(cc).ok_or_else(|| EmitError::Unsupported(format!("condition code {cc}")))?,
            )
        } else {
            (Operation::Jump, "jmp")
        };
        self.push_semantics(Semantics { target: Some(target), name: Some(name.into()), ..Semantics::new(op) });
        Ok(())
    }

    /// `COPY dst <- src`, a register-to-register move.
    fn copy(
        &mut self,
        n: u32,
    ) -> Result<(), EmitError> {
        let node = self.dag.node(SDValue { node: n, res: 0 });
        let src = self.operand(node.ops[0])?;
        let held = self.fresh(width(self.ctx.map.scalar_sse(), node.values[0]));
        self.values.insert(SDValue { node: n, res: 0 }, held.value);
        self.push(Operation::Move, "mov", vec![Loc::Held(held)], vec![src]);
        Ok(())
    }

    /// The machine node's input operands, a run of five memory operands
    /// becoming one cell. `desc.operands` lists the defs first.
    fn operands(
        &mut self,
        ops: &[SDValue],
        desc: &InstrDesc,
        cell_width: u32,
    ) -> Result<Vec<Loc>, EmitError> {
        let ins = &desc.operands[usize::from(desc.num_defs)..];
        let mut out = Vec::new();
        let mut i = 0;
        while i < ins.len() && i < ops.len() {
            if ins[i].kind == operand_type::MEMORY {
                let group =
                    ops.get(i..i + 5).ok_or_else(|| EmitError::Unsupported("a memory operand cut short".to_owned()))?;
                out.push(Loc::Mem(self.cell(group, cell_width)?));
                i += 5;
            } else {
                out.push(self.operand(ops[i])?);
                i += 1;
            }
        }
        Ok(out)
    }

    /// `cell` with its index scaled by a shift: 16-bit addressing has no
    /// scale.
    fn unscale_index(
        &mut self,
        cell: &mut Mem,
    ) -> Result<(), EmitError> {
        if cell.scale == 1 {
            return Ok(());
        }
        let shift = match cell.scale {
            2 => 1,
            4 => 2,
            8 => 3,
            other => {
                return Err(EmitError::Unsupported(format!("a lea of an index scaled by {other} at m16")));
            }
        };
        let index = cell.index.take().expect("an index");
        let scaled = self.fresh(index.width);
        let amount = Loc::Imm(Imm { value: shift, width: 1, address: None });
        self.push(Operation::Binary, "shl", vec![Loc::Held(scaled)], vec![Loc::Held(index), amount]);
        cell.index = Some(scaled);
        cell.scale = 1;
        Ok(())
    }

    /// base, scale, index, displacement, segment.
    fn cell(
        &mut self,
        group: &[SDValue],
        cell_width: u32,
    ) -> Result<Mem, EmitError> {
        let cell = self.cell_unchecked(group, cell_width)?;
        // 16-bit addressing has no scale (SD3): a 16-bit index beside one is
        // a cell no instruction encodes.
        if self.ctx.address_mode == AddrMode::Real16
            && cell.scale != 1
            && cell.index.is_some_and(|index| index.width == 2)
        {
            return Err(EmitError::Unsupported("a scaled 16-bit index at m16".to_owned()));
        }
        Ok(cell)
    }

    fn cell_unchecked(
        &mut self,
        group: &[SDValue],
        cell_width: u32,
    ) -> Result<Mem, EmitError> {
        if matches!(
            self.dag.opcode(group[0]),
            ISD::FrameIndex | ISD::TargetFrameIndex
        ) {
            return self.frame_cell(group, cell_width);
        }
        if self.sp_values.contains(&group[0]) {
            let mut cell = self.cell_through(group, cell_width, self.ctx.stack_pointer)?;
            cell.base = None;
            return Ok(cell);
        }
        // A base made of the address of a pool entry (`MOV32ri` of the
        // constant) reads that entry: as wide as its constant, and no wider.
        if let ISD::Machine(op) = self.dag.opcode(group[0])
            && self.ctx.names.get(&(op as u16)).is_some_and(|name| name.split('|').next() == Some("MOV32ri"))
            && let NodeData::PoolIndex(entry, _) = self.dag.node(self.dag.node(group[0]).ops[0]).data
        {
            let size = self.pool_cell(entry)?.width;
            if cell_width > size {
                return Err(EmitError::Unsupported(format!("a pool entry of {size} bytes read as {cell_width}")));
            }
        }
        let base = self.address_register(group[0], Role::Base)?;
        let scale = self.constant(group[1])?;
        let index = self.address_register(group[2], Role::Index)?;
        // A cell through ES is a far cell: the selector is the one loaded with
        // the offset it is based on.
        if self.is_es(group[4]) {
            let selector = [group[0], group[2]]
                .into_iter()
                .find_map(|part| self.selectors.get(&part.node).copied())
                .or(self.es)
                .ok_or_else(|| EmitError::Unsupported("a cell through ES that nothing loaded".to_owned()))?;
            // The displacement is a number, or a far global's symbol and
            // offset.
            let (disp, address) = match self.dag.node(group[3]).data {
                NodeData::Global { symbol, offset } => (
                    offset,
                    Addr { segment: llrm_lir::registers::RegId::ES.iced(), ..self.ctx.globals.address(symbol, offset) },
                ),
                _ => {
                    let disp = self.constant(group[3])?;
                    // a bare offset in a 16-bit segment is unsigned:
                    // `es:[36766]`, not `es:[-28770]`
                    let disp = match llrm_lir::registers::Regs(self.ctx.registers).width_of(self.ctx.stack_pointer) {
                        Some(word)
                            if base.is_none() && index.is_none() && self.ctx.address_mode == AddrMode::Real16 =>
                        {
                            disp & ((1_i64 << (word * 8)) - 1)
                        }
                        _ => disp,
                    };
                    (
                        disp,
                        Addr {
                            segment: llrm_lir::registers::RegId::ES.iced(),
                            ..Addr::new(crate::model::ir::Space::Far, disp)
                        },
                    )
                }
            };
            return Ok(Mem {
                offset: disp,
                scale,
                disp_width: 2,
                base,
                index,
                selector: Some(selector),
                ..Mem::new(Some(address), cell_width)
            });
        }
        if let NodeData::Global { symbol, offset } = self.dag.node(group[3]).data {
            let addr = self.ctx.globals.address(symbol, offset);
            if self.address_register(group[4], Role::Segment)?.is_some() {
                return Err(EmitError::Unsupported("a segment override".to_owned()));
            }
            // A symbol takes a displacement field to carry its relocation, as
            // the old selector's global cells do.
            return Ok(Mem { base, index, scale, disp_width: 2, ..Mem::new(Some(addr), cell_width) });
        }
        if let NodeData::JumpTable(table) = self.dag.node(group[3]).data {
            let table = self.jump_table(table)?;
            let mut address = Addr::new(crate::model::ir::Space::Segment, 0);
            address.index = table.id;
            return Ok(Mem { base, index, scale, disp_width: 2, ..Mem::new(Some(address), cell_width) });
        }
        if let NodeData::PoolIndex(entry, shift) = self.dag.node(group[3]).data {
            if self.address_register(group[4], Role::Segment)?.is_some() {
                return Err(EmitError::Unsupported("a segment override".to_owned()));
            }
            let pooled = self.pool_cell(entry)?;
            // The entry is as wide as its constant; read wider it is the
            // neighbouring data (a read inside it, the fudge's float at +0 or
            // +4, is the lowering's choice) (a double that is exactly a float
            // is a float entry the legalizer loads without saying
            // it extends).
            if cell_width > pooled.width {
                return Err(EmitError::Unsupported(format!(
                    "a pool entry of {} bytes read as {cell_width}",
                    pooled.width
                )));
            }
            // as wide as the read, which may be inside the entry
            let addr = pooled
                .addr
                .map(
                    |mut addr| {
                        addr.disp += i64::from(shift);
                        addr
                    },
                );
            return Ok(Mem { base, index, scale, width: cell_width, addr, ..pooled });
        }
        let disp = self.displacement(group[3])?;
        if self.address_register(group[4], Role::Segment)?.is_some() {
            return Err(EmitError::Unsupported("a segment override".to_owned()));
        }
        // An absolute address (`[0xB8000]`) is a literal address of four bytes.
        if base.is_none() && index.is_none() {
            let address = Addr::new(crate::model::ir::Space::Literal, disp);
            return Ok(Mem { disp_width: 4, ..Mem::new(Some(address), cell_width) });
        }
        // An indexed cell's displacement is a literal address, as the old
        // selector makes it: without one later passes read `[base]` alone and
        // the index is dropped (strlen_idiom looped forever).
        if index.is_some() {
            let address = Addr::new(crate::model::ir::Space::Literal, disp);
            // Dword address registers at real mode (the 67h form) take a
            // dword displacement.
            let wide = [base, index].into_iter().flatten().any(|held| held.width == 4);
            if self.ctx.address_mode == AddrMode::Real16
                && base.zip(index).is_some_and(|(base, index)| base.width != index.width)
            {
                return Err(EmitError::Unsupported("an address of registers of two widths at m16".to_owned()));
            }
            let disp_width = if wide && self.ctx.address_mode == AddrMode::Real16 { 4 } else { 2 };
            return Ok(Mem { base, index, scale, offset: disp, disp_width, ..Mem::new(Some(address), cell_width) });
        }
        Ok(Mem { base, index, scale, offset: disp, ..Mem::new(None, cell_width) })
    }

    /// A cell addressed through the register `through` (the stack pointer):
    /// Whether the segment operand of a cell moves it off its own segment: an
    /// override, except ES holding SS (a pointer to the frame made far), which
    /// reads what the frame register's default does.
    fn segment_overrides(
        &mut self,
        segment: SDValue,
    ) -> Result<bool, EmitError> {
        if self.is_es(segment) && self.es_is_ss {
            return Ok(false);
        }
        Ok(self.address_register(segment, Role::Segment)?.is_some())
    }

    /// `[through + index*scale + disp]`.
    fn cell_through(
        &mut self,
        group: &[SDValue],
        cell_width: u32,
        through: llrm_lir::registers::RegId,
    ) -> Result<Mem, EmitError> {
        let scale = self.constant(group[1])?;
        let index = self.address_register(group[2], Role::Index)?;
        let offset = self.constant(group[3])?;
        if self.segment_overrides(group[4])? {
            return Err(EmitError::Unsupported("a segment override".to_owned()));
        }
        Ok(Mem { through, index, scale, offset, disp_width: 0, ..Mem::new(None, cell_width) })
    }

    /// A cell of the frame: the fixed object plus the displacement operand.
    fn frame_cell(
        &mut self,
        group: &[SDValue],
        cell_width: u32,
    ) -> Result<Mem, EmitError> {
        let NodeData::FrameIndex(index_of) = self.dag.node(group[0]).data else {
            return Err(EmitError::Unsupported("a frame index without an index".to_owned()));
        };
        let index = self.address_register(group[2], Role::Index)?;
        if self.segment_overrides(group[4])? {
            return Err(EmitError::Unsupported("a segment-overridden frame cell".to_owned()));
        }
        let scale = self.constant(group[1])?;
        let disp = self.constant(group[3])?;
        let cell = self
            .ctx
            .frame
            .cell(index_of, cell_width)
            .ok_or_else(|| EmitError::Unsupported(format!("frame index {index_of}")))?;
        // A displacement past the object (an index that makes up the rest,
        // `[ebp+ecx+126]` for 8192 bytes below) is a position of its own in the
        // frame, where `home_of` would name the nearest slot; so is one exactly
        // at its end, which is the next object's first byte: the cell says
        // which object it was made from, and the layout moves it with that.
        let made_from = self.ctx.frame.extent(index_of).map(|(start, _)| start);
        // The object's place is its `addr`; the displacement moves that, as
        // `offset` of a cell with an `addr` is only how it is encoded.
        let addr = cell
            .addr
            .map(
                |addr| {
                    let moved = Addr { disp: addr.disp + disp, ..addr };
                    made_from.map_or(moved, |home| moved.in_slot(home))
                },
            );
        // With an index the frame register is the base of the address and,
        // for a dword index, EBP (the 67h form's base).
        let through = match &index {
            Some(held) if held.width == 4 => llrm_lir::registers::RegId::EBP,
            _ => cell.through,
        };
        let addr = addr.map(|addr| if index.is_some() { Addr { base: through.iced(), ..addr } } else { addr });
        Ok(Mem { addr, through, index, scale, ..cell })
    }

    fn jump_table(
        &self,
        table: u32,
    ) -> Result<JumpTable, EmitError> {
        self.ctx.tables.get(table as usize).cloned().ok_or_else(|| EmitError::Unsupported("a jump table".to_owned()))
    }

    /// An address component: a value, or the `$noreg` register.
    fn address_register(
        &mut self,
        value: SDValue,
        role: Role,
    ) -> Result<Option<Held>, EmitError> {
        // `$noreg` is virtual register 0.
        if self.dag.node(value).data == NodeData::Register(0) {
            return Ok(None);
        }
        // `%rip` as a base is how the pin spells a symbol's address; the
        // operand is the symbol alone, which the encoder makes `%rip`-relative.
        if self.dag.node(value).data == NodeData::PhysReg(llrm_lir::registers::RegId::RIP) {
            return Ok(None);
        }
        match self.operand(value)? {
            Loc::Held(held) => {
                if let Some(tag) = class_tag(self.ctx.address_mode, role) {
                    self.classes.push((held.value, tag));
                }
                Ok(Some(held))
            }
            other => Err(EmitError::Unsupported(format!("address component {other:?}"))),
        }
    }

    /// An address's displacement: the constant's low 32 bits, signed.
    fn displacement(
        &self,
        value: SDValue,
    ) -> Result<i64, EmitError> {
        self.constant(value).map(|bits| i64::from(bits as i32))
    }

    fn constant(
        &self,
        value: SDValue,
    ) -> Result<i64, EmitError> {
        match self.dag.node(value).data {
            NodeData::Const { bits, .. } => {
                Ok(sign_extend(bits, width_of(self.ctx.map.scalar_sse(), self.dag, value) * 8))
            }
            _ => Err(EmitError::Unsupported("a non-constant address part".to_owned())),
        }
    }

    fn operand(
        &mut self,
        value: SDValue,
    ) -> Result<Loc, EmitError> {
        if let NodeData::Const { bits, .. } = self.dag.node(value).data {
            let w = width_of(self.ctx.map.scalar_sse(), self.dag, value);
            return Ok(Loc::Imm(Imm { value: sign_extend(bits, w * 8), width: w, address: None }));
        }
        if let Some((mnemonic, cell)) = self.lazy.get(&value).cloned() {
            // DEVIATE SD234: the pin loads `LD_F1` once for all its readers; a
            // call or block end is the only place the stack cannot hold it
            let at = (self.stack_breaks, self.blocks.len());
            let constant = cell.is_none();
            if constant {
                if let Some(&(held, breaks, blocks)) = self.loaded.get(&value) {
                    if (breaks, blocks) == at {
                        return Ok(Loc::Held(held));
                    }
                }
            }
            let held = self.fresh(10);
            self.push(Operation::FloatLoad, mnemonic, vec![Loc::Held(held)], cell.map(Loc::Mem).into_iter().collect());
            if constant {
                self.loaded.insert(value, (held, at.0, at.1));
            }
            return Ok(Loc::Held(held));
        }
        if self.dag.opcode(value) == ISD::Register {
            return self.register(value);
        }
        if let NodeData::Global { symbol, offset } = self.dag.node(value).data {
            let addr = self.ctx.globals.address(symbol, offset);
            return Ok(Loc::Imm(Imm {
                value: 0,
                width: width_of(self.ctx.map.scalar_sse(), self.dag, value),
                address: Some(addr),
            }));
        }
        // The address of a jump table.
        if let NodeData::JumpTable(table) = self.dag.node(value).data {
            let table = self.jump_table(table)?;
            let mut address = Addr::new(crate::model::ir::Space::Segment, 0);
            address.index = table.id;
            return Ok(Loc::Imm(Imm {
                value: 0,
                width: width_of(self.ctx.map.scalar_sse(), self.dag, value),
                address: Some(address),
            }));
        }
        // The address of a pool constant.
        if let NodeData::PoolIndex(entry, shift) = self.dag.node(value).data {
            let mut addr = self.pool_cell(entry)?.addr;
            if let Some(addr) = addr.as_mut() {
                addr.disp += i64::from(shift);
            }
            return Ok(Loc::Imm(Imm {
                value: 0,
                width: width_of(self.ctx.map.scalar_sse(), self.dag, value),
                address: addr,
            }));
        }
        // The stack pointer itself as a value (`mov edi, esp`, the first byval
        // argument's address): a copy of the register.
        if self.sp_values.contains(&value) {
            let width = width_of(self.ctx.map.scalar_sse(), self.dag, value);
            let held = self.fresh(width);
            let sp = Reg { register: self.ctx.stack_pointer, width };
            self.push(Operation::Move, "mov", vec![Loc::Held(held)], vec![Loc::Reg(sp)]);
            return Ok(Loc::Held(held));
        }
        match self.values.get(&value) {
            Some(&v) => Ok(Loc::Held(Held { value: v, width: width_of(self.ctx.map.scalar_sse(), self.dag, value) })),
            None => Err(EmitError::Unsupported(format!("operand {:?} before its definition", self.dag.opcode(value)))),
        }
    }

    /// A `Register` node: a physical register, or a virtual one's value.
    fn register(
        &mut self,
        value: SDValue,
    ) -> Result<Loc, EmitError> {
        let width = width_of(self.ctx.map.scalar_sse(), self.dag, value);
        match self.dag.node(value).data {
            NodeData::PhysReg(register) => Ok(Loc::Reg(Reg { register, width })),
            NodeData::Register(reg) => Ok(Loc::Held(Held { value: self.vreg(reg), width })),
            _ => Err(EmitError::Unsupported("a register operand without a register".to_owned())),
        }
    }

    fn vreg(
        &mut self,
        reg: u32,
    ) -> u32 {
        self.state.vreg(reg)
    }

    /// `InstrEmitter::CreateVirtualRegisters`: a result copied by a
    /// `CopyToReg` to a virtual register of its width is defined in that
    /// register, and the copy emits nothing.
    fn exported_register(
        &mut self,
        result: SDValue,
    ) -> Option<Held> {
        let width = width_of(self.ctx.map.scalar_sse(), self.dag, result);
        let reg = self
            .dag
            .users(result)
            .find_map(
                |user| {
                    if self.dag.opcode(SDValue { node: user, res: 0 }) != ISD::CopyToReg {
                        return None;
                    }
                    let node = self.dag.node(SDValue { node: user, res: 0 });
                    let NodeData::Register(reg) = self.dag.node(node.ops[1]).data else { return None };
                    // ES is the selector the next cell takes, not a value of
                    // its own: made in two blocks it would be defined twice.
                    if self.is_es(node.ops[1]) {
                        return None;
                    }
                    (node.ops[2] == result && width_of(self.ctx.map.scalar_sse(), self.dag, node.ops[1]) == width)
                        .then_some(reg)
                },
            )?;
        Some(Held { value: self.vreg(reg), width })
    }

    /// The register a result is defined in: the one a `CopyToReg` of it names
    /// (`exported_register`), else a fresh value.
    fn result_register(
        &mut self,
        result: SDValue,
        width: u32,
    ) -> Held {
        self.exported_register(result).unwrap_or_else(|| self.fresh(width))
    }

    fn new_value(&mut self) -> u32 {
        let v = self.state.next_value;
        self.state.next_value += 1;
        v
    }

    fn fresh(
        &mut self,
        width: u32,
    ) -> Held {
        Held { value: self.new_value(), width }
    }

    fn push(
        &mut self,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) {
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        self.push_semantics(what);
    }

    fn push_semantics(
        &mut self,
        what: Semantics,
    ) {
        if matches!(what.op, Operation::Call | Operation::Barrier) {
            self.stack_breaks += 1;
        }
        let (defines, uses) = (_written(&what.dests), _read(&what));
        let at = self.next_address();
        let insn = Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses));
        self.blocks.last_mut().expect("a block").insns.push(insn);
    }
}

/// `DIV32r`, `IDIV16m`, `MUL32r`, `IMUL32m`: the one-operand forms.
fn is_div_or_mul(name: &str) -> bool {
    let rest = name.strip_prefix('I').unwrap_or(name);
    (rest.starts_with("DIV") || rest.starts_with("MUL"))
        && rest[3..].len() >= 2
        && rest[3..rest.len() - 1].chars().all(|c| c.is_ascii_digit())
        && matches!(rest.chars().last(), Some('r' | 'm'))
}

/// (signed, divide, bytes, memory) of a name `is_div_or_mul` accepts.
fn div_or_mul(name: &str) -> (bool, bool, u32, bool) {
    let signed = name.starts_with('I');
    let rest = name.strip_prefix('I').unwrap_or(name);
    let bytes = rest[3..rest.len() - 1].parse::<u32>().unwrap_or(32) / 8;
    (signed, rest.starts_with("DIV"), bytes, rest.ends_with('m'))
}

fn width_of(
    sse: bool,
    dag: &SelectionDAG,
    value: SDValue,
) -> u32 {
    width(sse, dag.value_type(value))
}

/// The width of the node's memory access: its first result, else its first
/// operand that is a value.
fn width_of_node(
    sse: bool,
    dag: &SelectionDAG,
    n: u32,
) -> u32 {
    let node = dag.node(SDValue { node: n, res: 0 });
    node.values.first().filter(|vt| **vt != EVT::OTHER).map_or(4, |vt| width(sse, *vt))
}

fn sign_extend(
    bits: u128,
    width: u32,
) -> i64 {
    let shift = 128 - width.clamp(1, 128);
    (((bits << shift) as i128) >> shift) as i64
}
