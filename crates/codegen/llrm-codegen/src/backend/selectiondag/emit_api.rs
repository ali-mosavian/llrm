//! What the x86 emitter asks of the compile (`emit.rs`) and gives back: the
//! maps it reads (globals, constant cells, conventions, frame), the context
//! it runs in, and what it makes.
//!
//! # How it works
//! Types and traits only. The compile implements the traits and builds an
//! [`EmitContext`]; the emitter returns [`Emitted`]. Kept apart from the
//! emitter so the emitter can sit in the x86 backend crate while these stay
//! with the code that implements them.

use std::sync::Arc;

use llrm_dagisel::addr::AddrMode;
use llrm_support::hash::HashMap;

use super::dag::SDValue;
use super::opcodes::OpcodeMap;
use crate::model::ir::{Addr, Mem};
use crate::model::lir::{Insn, Phi};

/// Why a node was not emitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmitError {
    /// A node or operand this emitter does not translate yet.
    Unsupported(String),
    /// An opcode with no `x86.instr` form (`opcodes_unmapped.txt`).
    Unmapped(String),
}

/// What a direct call names.
#[derive(Clone, Debug)]
pub enum Callee {
    Global(u32),
    Named(String),
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

    /// What a call of `callee` may read and write, as its attributes state it
    /// (`llrm_mir::memory::of`); any memory where they are not known.
    fn effects(
        &self,
        _callee: Option<&Callee>,
    ) -> llrm_mir::memory::Effects {
        llrm_mir::memory::Effects::ANY
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

/// What the x87 forms cost, in clocks per form and in thousandths of a clock
/// per code byte: the prices [`Emitter::sharing_pays`] weighs a pool constant
/// loaded once against a memory operand at each reader.
#[derive(Clone, Copy, Debug)]
pub struct X87Prices {
    pub load: i64,
    pub add: i64,
    pub add_m: i64,
    pub mul: i64,
    pub mul_m: i64,
    pub div: i64,
    pub div_m: i64,
    pub milliclocks_per_byte: i64,
    /// The bytes of `fld m` / `fadd m` on a pool cell (6 flat, 4 real mode);
    /// the register forms take 2.
    pub memory_bytes: i64,
    /// Bytes decide over clocks (`Profile::bytes_strict`: -O1, -Os).
    pub bytes_strict: bool,
}

/// The pin's machine opcode names, by number.
pub struct EmitContext<'a> {
    /// DEVIATE X87-POOL-SHARE (owner x87, exit 2026-11-20): the prices that
    /// decide whether a pool constant read several times is loaded once (the
    /// pin: `IsProfitableToFold` needs `hasOneUse`) or read as a memory
    /// operand at each reader, where that is cheaper (i486: `fmul m` 14 clocks
    /// against `fmul st` 16). None: the pin's rule.
    pub x87_prices: Option<X87Prices>,
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
    /// A near pointer into the stack segment needs no `ss:` override: SS holds
    /// the data segment's value for the whole run
    /// (`Segments::stack_shares_data`).
    pub stack_shares_data: bool,
    /// The multiple a call's argument area is rounded to, so a call finds the
    /// stack aligned (`aligned_calls`); 1 where calls need none.
    pub call_alignment: i64,
    /// The function's convention changes the stack pointer only in the
    /// prologue and epilogue: its calls' areas are frame pseudos, reserved
    /// there, at any width.
    pub reserved_calls: bool,
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
    /// The block's fall-through successor comes before its branch target, as a
    /// custom inserter adds them (`EmitLoweredSelect`: `copy0MBB`, then
    /// `sinkMBB`). Placement breaks a tie between even odds in that order.
    pub fall_first: bool,
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
    /// Whether every value of a `float_bytes` number is exactly an f32 the
    /// program holds: a load of one or a constant. A spill of such a value as
    /// a dword loses no bit, wherever the language keeps wider temporaries.
    pub float_exact: std::collections::BTreeMap<u32, bool>,
}

impl Default for EmitState {
    fn default() -> EmitState {
        EmitState {
            vregs: HashMap::default(),
            next_value: 1,
            at: 1,
            float_bytes: Default::default(),
            float_exact: Default::default(),
        }
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
    /// The LIR value each DAG value was made in.
    pub values: HashMap<SDValue, u32>,
    /// The address of the first instruction emitted for each `IROrder`.
    pub first_at: HashMap<u32, i64>,
}
