//! One body's machine code as one MIR function.
//!
//! Registers, ES and the flags C, Z, S and O become values. A block reads
//! one it has not written through a placeholder; once every block and
//! terminator exists, `SsaUpdater` answers each placeholder with what
//! reaches it. Every variable starts as a sentinel: one still read once SSA
//! is built is a read of something the machine left undefined, and refuses
//! the function with its reason.
//!
//! A flag is computed where something reads it, from the operation that
//! wrote it; one live out of its block (`analysis::flags`) is computed at
//! the block's end. A branch on a comparison still in its block is that
//! comparison's `icmp`.
//!
//! The stack below the function's own is one alloca, addressed by how many
//! bytes are pushed; a BASIC frame is an alloca for its locals and one for
//! the arguments its caller pushed.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use iced_x86::Register;
use llrm_analysis::ssa::{SsaUpdater, provider};
use llrm_bcmachine::analysis::flags::{self as flagged, Flag};
use llrm_bcmachine::frontends::bc::blocks::{self, Block, Ends};
use llrm_bcmachine::frontends::bc::declen::Insn;
use llrm_bcmachine::frontends::bc::extent::BodyKind;
use llrm_bcmachine::model::ir::nodes::{Node, span};
use llrm_bcmachine::model::ir::{Effects, Loc, Operation, Reg, Semantics};
use llrm_bcmachine::objectfile::module::{Addr, Space};
use llrm_bcmachine::support::hash::IndexMap;
use llrm_mir::build::Builder;
use llrm_mir::{
    BinaryOp, BlockId, CastOp, Constant, ConstantId, ConstantKind, FloatKind, Flags, InstId, IntPredicate, Opcode, Operand, Position, Type, TypeId, ValueId,
};

use crate::machine::{Answer, BodyFacts, FRAME_ENTRY, FRAME_EXIT, Facts, Interface, TRACKED, never_returns, restore_pair, tracked};
use crate::objects::Objects;
use crate::runtime::Callees;
use crate::sites;
use crate::{FAR, SEGMENT};

pub type Emit<T> = Result<T, String>;

/// What a frame's zeroing calls, as HIR's emitter names it.
pub const MEMSET: &str = "llvm.memset.p0.i16";

/// What every function's emission shares.
pub struct Unit<'u> {
    pub facts: &'u Facts<'u>,
    pub objects: &'u Objects,
    pub callees: &'u Callees,
    /// Each procedure's function, its type and interface, by name.
    pub procedures: BTreeMap<String, Result<(ConstantId, TypeId, Interface), String>>,
    /// `llvm.{u,s}{add,sub}.with.overflow`, by name.
    pub intrinsics: BTreeMap<String, (ConstantId, TypeId)>,
    /// The main body's frame: where its locals start below BP, and how many.
    pub main_frame: Option<(i64, i64)>,
    /// The bytes below BP the runtime's frame header takes.
    pub header: Option<i64>,
}

/// A value the machine keeps.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Var {
    /// A tracked root's low word, or its high word under /G3.
    Reg(usize, Half),
    Es,
    Bit(Bit),
    /// An x87 slot, counted from the bottom: st(i) is `St(depth - 1 - i)`.
    St(u8),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Half {
    Low,
    High,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Bit {
    C,
    Z,
    S,
    O,
}

const BITS: [(Bit, Flag); 4] = [(Bit::C, Flag::CF), (Bit::Z, Flag::ZF), (Bit::S, Flag::SF), (Bit::O, Flag::OF)];

/// What one operation computed, for the flags it wrote.
#[derive(Clone, Debug)]
pub enum Kind {
    Add,
    Sub,
    /// With a carry in.
    Adc(Operand),
    Sbb(Operand),
    Logic,
    Inc,
    Dec,
    Neg,
    Shl(u32),
    Shr(u32),
    Sar(u32),
    /// C and O are this.
    Overflow(Operand),
}

#[derive(Clone, Debug)]
pub struct Desc {
    pub kind: Kind,
    pub a: Operand,
    pub b: Operand,
    pub r: Operand,
    pub bits: u32,
}

#[derive(Clone, Debug)]
pub enum BitState {
    Value(Operand),
    Lazy(Rc<Desc>),
    Unknown(String),
}

/// Whether the BASIC frame is set up yet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Frame {
    Before,
    Active,
    After,
}

/// A frame's storage: its locals below BP, and its arguments above.
#[derive(Clone, Copy, Debug)]
struct Layout {
    /// The first byte of the locals, from BP, and the pointer to it.
    low: i64,
    high: i64,
    locals: Operand,
    /// Where the arguments start above BP, their bytes, and the pointer.
    arguments: Option<(i64, i64, Operand)>,
}

/// A machine block's state on entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Entry {
    depth: i64,
    frame: Frame,
    floats: u8,
}

/// A value nothing may observe: why observing it refuses the function.
struct Sentinel {
    why: String,
}

pub struct Emitter<'b, 'm, 'u> {
    pub b: &'b mut Builder<'m>,
    pub unit: &'b Unit<'u>,
    body: &'b BodyFacts,
    blocks: BTreeMap<usize, BlockId>,
    pub block: BlockId,
    current: HashMap<Var, Operand>,
    bits: HashMap<Bit, BitState>,
    depth: i64,
    frame: Frame,
    /// Values on the x87 stack.
    pub floats: u8,
    entries: HashMap<usize, Entry>,
    placeholders: Vec<(BlockId, Var, ValueId)>,
    ends: HashMap<BlockId, (HashMap<Var, Operand>, HashMap<Bit, BitState>)>,
    sentinels: HashMap<ValueId, Sentinel>,
    /// What this emitter made that computes only a value, for sweeping.
    pure: Vec<InstId>,
    top: Operand,
    deepest: i64,
    layout: Option<Layout>,
    /// What each push still standing in this block stored, by its depth
    /// and width: a pop or an argument reads it back as that value.
    pushes: HashMap<i64, (Operand, i64)>,
    live_in: IndexMap<usize, Flag>,
    /// The instruction being emitted.
    pub insn: Option<Insn>,
    /// This block's nodes before its transfer, the one being emitted, and
    /// how many after it a recognizer consumed.
    run: Vec<&'b Node>,
    cursor: usize,
    consumed: usize,
}

fn poison(b: &mut Builder, ty: TypeId) -> Operand {
    Operand::Constant(b.context.constant(Constant { ty, kind: ConstantKind::Poison }))
}

/// Emits `body` into the function `b` builds.
pub fn function(b: &mut Builder, unit: &Unit, body: &BodyFacts) -> Emit<()> {
    match body.body.kind {
        BodyKind::Main | BodyKind::Procedure => {}
        other => return Err(format!("a {} is entered by the runtime's own protocol", other.value())),
    }
    if body.blocks.is_empty() {
        return Err("no blocks".to_owned());
    }
    let entry = b.block("entry");
    b.position(entry);
    let ptr = b.context.types.ptr(0);
    let placeholder = poison(b, ptr);
    let top = {
        let inst = b.function.create_instruction(Opcode::Freeze, ptr, vec![placeholder], Flags::default(), Some("top"));
        b.function.insert(inst, Position::End(entry)).expect("the entry");
        Operand::Value(b.function.instruction(inst).result.expect("a value"))
    };
    let mut emitter = Emitter {
        b,
        unit,
        body,
        blocks: BTreeMap::new(),
        block: entry,
        current: HashMap::new(),
        bits: HashMap::new(),
        depth: 0,
        frame: Frame::Before,
        floats: 0,
        entries: HashMap::new(),
        placeholders: Vec::new(),
        ends: HashMap::new(),
        sentinels: HashMap::new(),
        pure: Vec::new(),
        top,
        deepest: 0,
        layout: None,
        pushes: HashMap::new(),
        live_in: flagged::live_in(&body.blocks),
        insn: None,
        run: Vec::new(),
        cursor: 0,
        consumed: 0,
    };
    emitter.run()
}

impl<'b, 'm, 'u> Emitter<'b, 'm, 'u> {
    fn run(&mut self) -> Emit<()> {
        let entry = self.block;
        self.prologue()?;
        for block in &self.body.blocks {
            let id = self.b.block(&format!("b{:04x}", block.at));
            self.blocks.insert(block.at, id);
        }
        let seed = self.body.blocks[0].at;
        self.b.position(entry);
        self.ends.insert(entry, (self.current.clone(), self.bits.clone()));
        self.b.br(self.blocks[&seed]);
        self.entries.insert(seed, Entry { depth: 0, frame: self.frame, floats: 0 });
        for block in self.order() {
            self.emit_block(block)?;
        }
        self.resolve()?;
        self.finish()
    }

    /// Blocks in reverse postorder from the seed: each one's state on entry
    /// is known before it.
    fn order(&self) -> Vec<&'b Block> {
        let by_at: BTreeMap<usize, &'b Block> = self.body.blocks.iter().map(|one| (one.at, one)).collect();
        let mut seen = BTreeSet::new();
        let mut post = Vec::new();
        let mut stack = vec![(self.body.blocks[0].at, 0usize)];
        seen.insert(self.body.blocks[0].at);
        while let Some((at, next)) = stack.pop() {
            let succ = &by_at[&at].succ;
            if next < succ.len() {
                stack.push((at, next + 1));
                let one = succ[next];
                if by_at.contains_key(&one) && seen.insert(one) {
                    stack.push((one, 0));
                }
            } else {
                post.push(by_at[&at]);
            }
        }
        post.reverse();
        post
    }

    /// The entry block: every variable a sentinel, the frame of a main
    /// body, a procedure's arguments stored where its code reads them.
    fn prologue(&mut self) -> Emit<()> {
        for index in 0..TRACKED.len() {
            for half in [Half::Low, Half::High] {
                let ty = self.var_type(Var::Reg(index, half));
                let why = format!("reads {} before anything writes it", half_name(index, half));
                let one = self.sentinel(ty, why);
                self.current.insert(Var::Reg(index, half), one);
            }
        }
        let es = self.var_type(Var::Es);
        let one = self.sentinel(es, "reads es before anything writes it".to_owned());
        self.current.insert(Var::Es, one);
        for (bit, _) in BITS {
            self.bits.insert(bit, BitState::Unknown("reads the flags on entry".to_owned()));
        }
        match self.body.body.kind {
            BodyKind::Main => {
                if let (Some((low, size)), Some(header)) = (self.unit.main_frame, self.unit.header) {
                    self.layout = Some(self.frame_layout(low, low + size, header)?);
                    self.frame = Frame::Active;
                }
            }
            BodyKind::Procedure => {
                let Some(Ok(interface)) = &self.body.interface else {
                    return Err(match &self.body.interface {
                        Some(Err(why)) => why.clone(),
                        _ => "no interface".to_owned(),
                    });
                };
                if interface.popped > 0 {
                    let byte = self.b.context.types.int(8);
                    let ty = self.b.context.types.intern(Type::Array { element: byte, count: interface.popped as u64 });
                    let arguments = self.b.alloca(ty, "arguments");
                    // BASIC pushes the first argument first: it is the highest.
                    let count = self.b.function.parameters().len();
                    for index in 0..count {
                        let offset = interface.popped - 2 * (index as i64 + 1);
                        let value = self.b.parameter(index);
                        let slot = self.offset(arguments, offset);
                        self.b.store(value, slot, false);
                    }
                    self.layout = Some(Layout { low: 0, high: 0, locals: arguments, arguments: Some((6, interface.popped, arguments)) });
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// A frame of locals `[low, high)` below BP, the header above them left
    /// out, zeroed where it is set up, as B$ENSA and B$ENRA zero-fill it:
    /// QB 4.5's `runtime/inc/stack.inc` keeps FR_GOSUB "last on the frame to
    /// optimize recursive zero-fill".
    fn frame_layout(&mut self, low: i64, high: i64, header: i64) -> Emit<Layout> {
        if high > -header {
            return Err("locals overlap the runtime's frame header".to_owned());
        }
        let arguments = self.layout.and_then(|one| one.arguments);
        let byte = self.b.context.types.int(8);
        let ty = self.b.context.types.intern(Type::Array { element: byte, count: (high - low) as u64 });
        let locals = self.b.alloca(ty, "frame");
        let &(memset, memset_ty) = self.unit.intrinsics.get(MEMSET).ok_or("@llvm.memset undeclared")?;
        let (zero, size, volatile) = (self.b.int(8, 0), self.b.int(16, i128::from(high - low)), self.b.int(1, 0));
        self.b.call(memset_ty, Operand::Constant(memset), &[locals, zero, size, volatile], "");
        Ok(Layout { low, high, locals, arguments })
    }

    pub fn var_type(&mut self, var: Var) -> TypeId {
        match var {
            Var::Reg(..) => self.b.context.types.int(16),
            Var::Es => self.b.context.types.ptr(SEGMENT),
            Var::Bit(_) => self.b.context.types.int(1),
            Var::St(_) => self.b.context.types.intern(Type::Float(FloatKind::Double)),
        }
    }

    /// A value that refuses the function if anything still reads it.
    fn sentinel(&mut self, ty: TypeId, why: String) -> Operand {
        let undefined = poison(self.b, ty);
        let inst = self.b.function.create_instruction(Opcode::Freeze, ty, vec![undefined], Flags::default(), None);
        let first = self.b.function.block(self.block).instructions().first().copied();
        self.b.function.insert(inst, first.map_or(Position::End(self.block), Position::Before)).expect("a placed block");
        let value = self.b.function.instruction(inst).result.expect("a value");
        self.sentinels.insert(value, Sentinel { why });
        Operand::Value(value)
    }

    /// Everything a call may change and does not answer.
    pub fn clobber(&mut self, registers: &[Register], why: &str) {
        for &register in registers {
            let Some(index) = tracked(register) else { continue };
            for half in [Half::Low, Half::High] {
                let ty = self.var_type(Var::Reg(index, half));
                let one = self.sentinel(ty, format!("reads {} after {why}", half_name(index, half)));
                self.current.insert(Var::Reg(index, half), one);
            }
        }
        for (bit, _) in BITS {
            self.bits.insert(bit, BitState::Unknown(format!("reads the flags {why} leaves")));
        }
    }

    fn emit_block(&mut self, block: &'b Block) -> Emit<()> {
        let id = self.blocks[&block.at];
        self.block = id;
        self.b.position(id);
        // Only an edge from a call that never returns reaches it.
        let Some(&entry) = self.entries.get(&block.at) else {
            self.ends.insert(id, (HashMap::new(), HashMap::new()));
            self.b.unreachable();
            return Ok(());
        };
        self.current.clear();
        self.bits.clear();
        self.pushes.clear();
        self.depth = entry.depth;
        self.frame = entry.frame;
        self.floats = entry.floats;
        let nodes: Vec<&Node> = self.body.nodes_of(block).map(|one| &**one).collect();
        let (last, rest) = match nodes.split_last() {
            Some((last, rest)) if is_transfer(last) || block.ends == Ends::Table => (Some(*last), rest),
            _ => (None, &nodes[..]),
        };
        self.run = rest.to_vec();
        let mut index = 0;
        while let Some(&node) = self.run.get(index) {
            (self.cursor, self.consumed) = (index, 0);
            self.node(node)?;
            index += 1 + self.consumed;
            if let Node::Call(call) = node {
                if self.unit.facts.contract(call.insn.at).is_some_and(never_returns) {
                    self.ends.insert(self.block, (self.current.clone(), self.bits.clone()));
                    self.b.unreachable();
                    return Ok(());
                }
            }
        }
        // A branch reads its flags before the block's end forgets them.
        let condition = match (block.ends, last) {
            (Ends::Conditional, Some(node)) => Some(self.condition(node.semantics().name.as_deref().unwrap_or(""))?),
            _ => None,
        };
        self.live_out(block);
        self.terminate(block, last, condition)
    }

    /// Computes each flag live out of this block from what wrote it.
    fn live_out(&mut self, block: &Block) {
        let mut out = Flag::NONE;
        for successor in &block.succ {
            out |= self.live_in.get(successor).copied().unwrap_or(Flag::NONE);
        }
        for (bit, flag) in BITS {
            let state = self.bits.get(&bit).cloned();
            let live = !(out & flag).is_empty();
            let made = match state {
                Some(BitState::Lazy(desc)) if live => match self.compute(&desc, bit) {
                    Ok(value) => BitState::Value(value),
                    Err(why) => BitState::Unknown(why),
                },
                Some(BitState::Lazy(_)) => BitState::Unknown(format!("reads a flag its writer's block did not keep {bit:?} {:#x} out {:?} succ {:?}", block.at, out, block.succ)),
                Some(other) => other,
                None => continue,
            };
            self.bits.insert(bit, made);
        }
    }

    /// Records the state a successor starts in, which every edge into it must agree on.
    fn enter(&mut self, at: usize) -> Emit<BlockId> {
        let target = *self.blocks.get(&at).ok_or_else(|| format!("a jump out of the body, to {at:#06x}"))?;
        let state = Entry { depth: self.depth, frame: self.frame, floats: self.floats };
        match self.entries.get(&at) {
            Some(known) if *known != state => Err(format!("paths into {at:#06x} disagree on the stack, the x87 stack or the frame")),
            _ => {
                self.entries.insert(at, state);
                Ok(target)
            }
        }
    }

    fn terminate(&mut self, block: &Block, last: Option<&Node>, condition: Option<Operand>) -> Emit<()> {
        let end = (self.current.clone(), self.bits.clone());
        match (block.ends, last) {
            (Ends::Return, Some(node)) => {
                self.ret(node)?;
            }
            (Ends::Conditional, Some(node)) => {
                let what = node.semantics();
                let taken = what.target.ok_or("a branch without a target")? as usize;
                let condition = condition.expect("computed before the flags were forgotten");
                let otherwise = *block.succ.iter().find(|&&one| one != taken).unwrap_or(&taken);
                let (taken, otherwise) = (self.enter(taken)?, self.enter(otherwise)?);
                self.b.position(self.block);
                self.b.cond_br(condition, taken, otherwise);
            }
            (Ends::Jump | Ends::FallsThrough, _) => {
                if let Some(node) = last {
                    if node.semantics().op == Operation::Escape {
                        return Err("a far jump".to_owned());
                    }
                }
                let next = *block.succ.first().ok_or("a block that goes nowhere")?;
                let target = self.enter(next)?;
                self.b.position(self.block);
                self.b.br(target);
            }
            (Ends::Table, Some(Node::Call(call))) if blocks::INLINE_TABLE.contains(call.name.as_str()) => return self.dispatch(&call.insn, &call.name),
            (Ends::Table, _) => return Err("a jump through a table".to_owned()),
            (Ends::Indirect, _) => return Err("an indirect jump".to_owned()),
            (other, _) => return Err(format!("a block that ends {}", other.value())),
        }
        self.ends.insert(self.block, end);
        Ok(())
    }

    /// ON GOTO: B$OGTA reads a count byte and that many code offsets past
    /// its call, and goes to the BX'th; 0 or past the count goes on past the
    /// table, and past 255 is Illegal function call.
    fn dispatch(&mut self, insn: &Insn, name: &str) -> Emit<()> {
        let found = self.unit.facts.found;
        let targets = blocks::dispatch_targets(found, insn).ok_or_else(|| format!("{name}'s table names code outside this segment"))?;
        let (_, past, _) = blocks::inline_table(found, insn).ok_or_else(|| format!("{name} without a table"))?;
        let index = self.register(Register::BX)?;
        let contract = self.unit.facts.contract(insn.at).ok_or_else(|| format!("{name} has no contract"))?.clone();
        let disturbed: Vec<Register> =
            llrm_bcmachine::abi::runtime::disturbs(&contract).into_iter().filter_map(crate::machine::from_contract).filter(|&one| one != crate::machine::FLAGS).collect();
        let why = format!("{name} clobbers it");
        self.clobber(&disturbed, &why);
        let error = match self.unit.callees.named.get(crate::runtime::ERROR) {
            Some(Ok(spec)) => spec.clone(),
            Some(Err(why)) => return Err(why.clone()),
            None => return Err(format!("{} is undeclared", crate::runtime::ERROR)),
        };
        let end = (self.current.clone(), self.bits.clone());
        let (raise, within) = (self.b.block("dispatch.error"), self.b.block("dispatch"));
        let limit = self.b.int(16, 255);
        let above = self.b.icmp(IntPredicate::Ugt, index, limit, "");
        self.b.cond_br(above, raise, within);
        self.ends.insert(self.block, end.clone());
        self.b.position(raise);
        let code = self.b.int(16, crate::runtime::ILLEGAL_FUNCTION_CALL);
        self.b.call_as(error.convention, error.ty, Operand::Constant(error.reference), &[code], "");
        self.b.unreachable();
        self.ends.insert(raise, end.clone());
        let otherwise = self.enter(past)?;
        let mut cases = Vec::new();
        for (number, &target) in targets.iter().enumerate() {
            let target = self.enter(target as usize)?;
            cases.push((self.b.int(16, number as i128 + 1), target));
        }
        self.b.position(within);
        self.b.switch(index, otherwise, &cases);
        self.ends.insert(within, end);
        Ok(())
    }

    fn ret(&mut self, node: &Node) -> Emit<()> {
        if self.body.body.kind != BodyKind::Procedure {
            return Err("a return from the main body".to_owned());
        }
        if self.frame == Frame::Active {
            return Err("returns inside its frame".to_owned());
        }
        if node.semantics().name.as_deref() != Some("retf") {
            return Err("a near return (RETURN from a GOSUB)".to_owned());
        }
        if self.depth != 0 {
            return Err("returns with bytes still pushed".to_owned());
        }
        if self.floats != 0 {
            return Err("returns with values on the x87 stack".to_owned());
        }
        let Some(Ok(interface)) = &self.body.interface else { return Err("no interface".to_owned()) };
        let value = match &interface.answer {
            Answer::None => None,
            Answer::Registers(registers) => Some(self.answer_value(registers)?),
            Answer::Flags => return Err("a procedure answering in the flags".to_owned()),
        };
        self.b.ret(value);
        Ok(())
    }

    /// The value registers answer: one word, or DX:AX joined.
    fn answer_value(&mut self, registers: &[Register]) -> Emit<Operand> {
        match registers {
            [one] => self.register(word(*one)),
            [Register::EAX, Register::EDX] => {
                let (low, high) = (self.register(Register::AX)?, self.register(Register::DX)?);
                Ok(self.join(low, high))
            }
            _ => Err("answers in more than DX:AX".to_owned()),
        }
    }

    /// `high:low` as one value twice as wide.
    pub fn join(&mut self, low: Operand, high: Operand) -> Operand {
        if let Some(whole) = crate::longs::whole(self, low, high) {
            return whole;
        }
        let bits = self.bits_of(low);
        let wide = self.b.context.types.int(bits * 2);
        let low = self.cast(CastOp::ZExt, low, wide);
        let high = self.cast(CastOp::ZExt, high, wide);
        let shift = self.b.int(bits * 2, i128::from(bits));
        let high = self.binary(BinaryOp::Shl, high, shift);
        self.binary(BinaryOp::Or, high, low)
    }

    /// The body being emitted.
    pub fn body(&self) -> &'b BodyFacts {
        self.body
    }

    /// The `n`th node after the one being emitted, in its block.
    pub fn ahead(&self, n: usize) -> Option<&'b Node> {
        self.run.get(self.cursor + n).copied()
    }

    /// Owns the next `n` nodes too: the core raise does not see them.
    pub fn consume(&mut self, n: usize) {
        self.consumed = n;
    }

    pub fn bits_of(&self, value: Operand) -> u32 {
        self.b.context.types.int_bits(self.b.type_of(value)).expect("an integer")
    }

    // ---------------------------------------------------------------- values

    /// A variable's value here, a placeholder where this block has not written it.
    pub fn get(&mut self, var: Var) -> Operand {
        if let Some(&value) = self.current.get(&var) {
            return value;
        }
        let ty = self.var_type(var);
        let undefined = poison(self.b, ty);
        let inst = self.b.function.create_instruction(Opcode::Freeze, ty, vec![undefined], Flags::default(), None);
        let first = self.b.function.block(self.block).instructions().first().copied();
        self.b.function.insert(inst, first.map_or(Position::End(self.block), Position::Before)).expect("a placed block");
        let value = self.b.function.instruction(inst).result.expect("a value");
        self.placeholders.push((self.block, var, value));
        self.current.insert(var, Operand::Value(value));
        Operand::Value(value)
    }

    pub fn set(&mut self, var: Var, value: Operand) {
        self.current.insert(var, value);
    }

    /// A general register's value at its own width.
    pub fn register(&mut self, register: Register) -> Emit<Operand> {
        let index = tracked(register).ok_or_else(|| format!("reads {} as a value", name(register)))?;
        let low = self.get(Var::Reg(index, Half::Low));
        match register.size() {
            4 => {
                let high = self.get(Var::Reg(index, Half::High));
                Ok(self.join(low, high))
            }
            2 => Ok(low),
            _ => {
                let shifted = if is_high_byte(register) {
                    let by = self.b.int(16, 8);
                    self.binary(BinaryOp::LShr, low, by)
                } else {
                    low
                };
                let byte = self.b.context.types.int(8);
                Ok(self.cast(CastOp::Trunc, shifted, byte))
            }
        }
    }

    /// Writes a general register; a byte merges into the rest of its word.
    pub fn set_register(&mut self, register: Register, value: Operand) -> Emit<()> {
        let index = tracked(register).ok_or_else(|| format!("writes {}", name(register)))?;
        let word = self.b.context.types.int(16);
        match register.size() {
            4 => {
                let low = self.cast(CastOp::Trunc, value, word);
                let sixteen = self.b.int(32, 16);
                let high = self.binary(BinaryOp::LShr, value, sixteen);
                let high = self.cast(CastOp::Trunc, high, word);
                self.set(Var::Reg(index, Half::Low), low);
                self.set(Var::Reg(index, Half::High), high);
            }
            2 => self.set(Var::Reg(index, Half::Low), value),
            _ => {
                let shift = if is_high_byte(register) { 8 } else { 0 };
                let old = self.get(Var::Reg(index, Half::Low));
                let mask = self.b.int(16, !(0xffi128 << shift));
                let kept = self.binary(BinaryOp::And, old, mask);
                let widened = self.cast(CastOp::ZExt, value, word);
                let placed = if shift > 0 {
                    let by = self.b.int(16, shift);
                    self.binary(BinaryOp::Shl, widened, by)
                } else {
                    widened
                };
                let merged = self.binary(BinaryOp::Or, kept, placed);
                self.set(Var::Reg(index, Half::Low), merged);
            }
        }
        Ok(())
    }

    /// Sets a whole 16-bit register from a call's answer.
    fn set_word(&mut self, root: Register, value: Operand) -> Emit<()> {
        self.set_register(word(root), value)
    }

    // ----------------------------------------------------------- instructions

    pub fn binary(&mut self, op: BinaryOp, a: Operand, b: Operand) -> Operand {
        let made = self.b.binary(op, a, b, Flags::default(), "");
        if !matches!(op, BinaryOp::SDiv | BinaryOp::SRem | BinaryOp::UDiv | BinaryOp::URem) {
            self.note_pure();
        }
        made
    }

    pub fn cast(&mut self, op: CastOp, a: Operand, to: TypeId) -> Operand {
        if self.b.type_of(a) == to {
            return a;
        }
        let made = self.b.cast(op, a, to, "");
        self.note_pure();
        made
    }

    pub fn icmp(&mut self, predicate: IntPredicate, a: Operand, b: Operand) -> Operand {
        let made = self.b.icmp(predicate, a, b, "");
        self.note_pure();
        made
    }

    pub fn note_pure(&mut self) {
        let last = *self.b.function.block(self.block).instructions().last().expect("just emitted");
        self.pure.push(last);
    }

    fn truth(&mut self, value: bool) -> Operand {
        self.b.int(1, i128::from(value))
    }

    /// `pointer` advanced by `offset` bytes.
    pub fn offset(&mut self, pointer: Operand, offset: i64) -> Operand {
        if offset == 0 {
            return pointer;
        }
        let byte = self.b.context.types.int(8);
        let index = self.b.int(16, i128::from(offset));
        let made = self.b.gep(byte, pointer, &[index], Flags::default(), "");
        self.note_pure();
        made
    }

    /// `pointer` advanced by the 16-bit value `index`.
    fn indexed(&mut self, pointer: Operand, index: Operand) -> Operand {
        let byte = self.b.context.types.int(8);
        let made = self.b.gep(byte, pointer, &[index], Flags::default(), "");
        self.note_pure();
        made
    }

    fn node(&mut self, node: &Node) -> Emit<()> {
        self.insn = insn_of(node).cloned();
        if self.unit.facts.event_poll(node) {
            return self.call(crate::runtime::EVENT_POLL, span(node).0);
        }
        for recognizer in sites::RECOGNIZERS {
            if let Some(done) = recognizer.node(self, node) {
                return done;
            }
        }
        match node {
            Node::Data(_) => Ok(()),
            Node::Restore(restore) => {
                let (source, into) = restore_pair(restore.pair).ok_or("an unknown restore pair")?;
                let high = self.get(Var::Reg(tracked(source).expect("tracked"), Half::High));
                self.set_word(into, high)
            }
            Node::Call(call) => self.call(&call.name, call.insn.at),
            Node::Opaque(one) => self.instruction(&one.insn, &one.semantics, &one.effects),
            Node::Long(one) => self.instruction(&one.insn, &one.semantics, &one.effects),
        }
    }

    fn instruction(&mut self, insn: &Insn, what: &Semantics, effects: &Effects) -> Emit<()> {
        let name = what.name.as_deref().unwrap_or("");
        match what.op {
            Operation::Nothing | Operation::Data => Ok(()),
            Operation::Move => {
                let value = self.read(&what.sources[0])?;
                self.write(&what.dests[0], value)
            }
            Operation::Exchange => {
                let (first, second) = (self.read(&what.dests[0])?, self.read(&what.dests[1])?);
                self.write(&what.dests[0], second)?;
                self.write(&what.dests[1], first)
            }
            Operation::Address => {
                let pointer = self.pointer(insn)?;
                let word = self.b.context.types.int(16);
                let value = self.cast(CastOp::PtrToInt, pointer, word);
                self.write(&what.dests[0], value)
            }
            Operation::Binary => self.binary_op(name, what, effects),
            Operation::Compare => {
                let (a, b) = (self.read(&what.sources[0])?, self.read(&what.sources[1])?);
                let b = self.fit(b, a);
                let (kind, op) = if name == "test" { (Kind::Logic, BinaryOp::And) } else { (Kind::Sub, BinaryOp::Sub) };
                let r = self.binary(op, a, b);
                self.flags(Some(Desc { kind, a, b, r, bits: self.bits_of(a) }), effects);
                Ok(())
            }
            Operation::Unary => {
                let a = self.read(&what.sources[0])?;
                let bits = self.bits_of(a);
                let (r, kind) = match name {
                    "neg" => {
                        let zero = self.b.int(bits, 0);
                        (self.binary(BinaryOp::Sub, zero, a), Some(Kind::Neg))
                    }
                    "not" => {
                        let ones = self.b.int(bits, -1);
                        (self.binary(BinaryOp::Xor, a, ones), None)
                    }
                    "inc" | "dec" => {
                        let one = self.b.int(bits, 1);
                        let op = if name == "inc" { BinaryOp::Add } else { BinaryOp::Sub };
                        (self.binary(op, a, one), Some(if name == "inc" { Kind::Inc } else { Kind::Dec }))
                    }
                    other => return Err(format!("unary {other}")),
                };
                self.write(&what.dests[0], r)?;
                let one = self.b.int(bits, 1);
                self.flags(kind.map(|kind| Desc { kind, a, b: one, r, bits }), effects);
                Ok(())
            }
            Operation::Extend => {
                let a = self.read(&what.sources[0])?;
                let bits = self.bits_of(a);
                let by = self.b.int(bits, i128::from(bits - 1));
                let sign = self.binary(BinaryOp::AShr, a, by);
                self.write(&what.dests[0], sign)
            }
            Operation::Multiply => self.multiply(what, effects),
            Operation::Divide => self.divide(what, effects),
            Operation::Push if matches!(what.sources[0], Loc::Reg(Reg { register: Register::BP, .. })) => Err("sets up its own BP frame".to_owned()),
            Operation::Push => {
                let value = self.read(&what.sources[0])?;
                self.push(value)
            }
            Operation::Pop => {
                let width = loc_width(&what.dests[0]).ok_or("a pop of no width")?;
                let value = self.pop(width)?;
                self.write(&what.dests[0], value)
            }
            Operation::Call => Err(if what.indirect || what.target.is_none() { "an indirect call".to_owned() } else { "a near call (GOSUB)".to_owned() }),
            Operation::Escape => Err("a far jump".to_owned()),
            Operation::Leave => Err("leave".to_owned()),
            Operation::Fill => Err("rep stosw".to_owned()),
            Operation::FloatLoad | Operation::FloatStore | Operation::FloatArith | Operation::FloatArithPop | Operation::FloatUnary => {
                Err(format!("x87 {name}"))
            }
            Operation::Barrier => Err(format!("{} is unmodelled", format!("{:?}", insn.insn.mnemonic()).to_lowercase())),
            other => Err(format!("{} in the middle of a block", other.as_str())),
        }
    }

    /// `b` at `a`'s width, as the machine sign-extends an immediate.
    fn fit(&mut self, b: Operand, a: Operand) -> Operand {
        let (want, have) = (self.bits_of(a), self.bits_of(b));
        let ty = self.b.context.types.int(want);
        if have < want {
            self.cast(CastOp::SExt, b, ty)
        } else if have > want {
            self.cast(CastOp::Trunc, b, ty)
        } else {
            b
        }
    }

    fn binary_op(&mut self, name: &str, what: &Semantics, effects: &Effects) -> Emit<()> {
        let a = self.read(&what.sources[0])?;
        let bits = self.bits_of(a);
        // `xor r,r` and `sub r,r` read nothing.
        if matches!(name, "xor" | "sub") && what.sources[0] == what.sources[1] && matches!(what.sources[0], Loc::Reg(_)) {
            let zero = self.b.int(bits, 0);
            self.write(&what.dests[0], zero)?;
            let kind = if name == "xor" { Kind::Logic } else { Kind::Sub };
            self.flags(Some(Desc { kind, a: zero, b: zero, r: zero, bits }), effects);
            return Ok(());
        }
        let b = self.read(&what.sources[1])?;
        if let "shl" | "shr" | "sar" = name {
            return self.shift(name, a, b, what, effects);
        }
        let b = self.fit(b, a);
        let (r, kind) = match name {
            "add" => (self.binary(BinaryOp::Add, a, b), Kind::Add),
            "sub" => (self.binary(BinaryOp::Sub, a, b), Kind::Sub),
            "and" => (self.binary(BinaryOp::And, a, b), Kind::Logic),
            "or" => (self.binary(BinaryOp::Or, a, b), Kind::Logic),
            "xor" => (self.binary(BinaryOp::Xor, a, b), Kind::Logic),
            "adc" | "sbb" => {
                let carry = self.bit(Bit::C)?;
                let ty = self.b.type_of(a);
                let wide = self.cast(CastOp::ZExt, carry, ty);
                let op = if name == "adc" { BinaryOp::Add } else { BinaryOp::Sub };
                let partial = self.binary(op, a, b);
                let r = self.binary(op, partial, wide);
                (r, if name == "adc" { Kind::Adc(carry) } else { Kind::Sbb(carry) })
            }
            other => return Err(format!("binary {other}")),
        };
        self.write(&what.dests[0], r)?;
        self.flags(Some(Desc { kind, a, b, r, bits }), effects);
        Ok(())
    }

    /// A shift: a constant count below the width is LLVM's own; a count in
    /// CL is masked to five bits as the 286 does, and one past the width
    /// fills.
    fn shift(&mut self, name: &str, a: Operand, count: Operand, what: &Semantics, effects: &Effects) -> Emit<()> {
        let bits = self.bits_of(a);
        let op = match name {
            "shl" => BinaryOp::Shl,
            "shr" => BinaryOp::LShr,
            _ => BinaryOp::AShr,
        };
        if let Some(n) = self.constant(count) {
            let n = (n & 31) as u32;
            if n == 0 {
                return Ok(());
            }
            let r = if n < bits {
                let by = self.b.int(bits, i128::from(n));
                self.binary(op, a, by)
            } else if op == BinaryOp::AShr {
                let by = self.b.int(bits, i128::from(bits - 1));
                self.binary(op, a, by)
            } else {
                self.b.int(bits, 0)
            };
            self.write(&what.dests[0], r)?;
            let kind = match op {
                BinaryOp::Shl => Kind::Shl(n),
                BinaryOp::LShr => Kind::Shr(n),
                _ => Kind::Sar(n),
            };
            let b = self.b.int(bits, i128::from(n));
            if n < bits {
                self.flags(Some(Desc { kind, a, b, r, bits }), effects);
            } else {
                self.unknown_flags(effects, "a shift by the whole width");
            }
            return Ok(());
        }
        let ty = self.b.type_of(a);
        let count = self.cast(CastOp::ZExt, count, ty);
        let five = self.b.int(bits, 31);
        let count = self.binary(BinaryOp::And, count, five);
        let width = self.b.int(bits, i128::from(bits));
        let inside = self.icmp(IntPredicate::Ult, count, width);
        let last = self.b.int(bits, i128::from(bits - 1));
        let safe = self.b.select(inside, count, last, "");
        self.note_pure();
        let shifted = self.binary(op, a, safe);
        let filled = if op == BinaryOp::AShr { shifted } else { self.b.int(bits, 0) };
        let r = self.b.select(inside, shifted, filled, "");
        self.note_pure();
        self.write(&what.dests[0], r)?;
        // A count of zero leaves the flags alone.
        self.unknown_flags(effects, "a shift by CL");
        Ok(())
    }

    fn multiply(&mut self, what: &Semantics, effects: &Effects) -> Emit<()> {
        let (a, b) = (self.read(&what.sources[0])?, self.read(&what.sources[1])?);
        let b = self.fit(b, a);
        let bits = self.bits_of(a);
        let wide = self.b.context.types.int(bits * 2);
        let (wa, wb) = (self.cast(CastOp::SExt, a, wide), self.cast(CastOp::SExt, b, wide));
        let product = self.binary(BinaryOp::Mul, wa, wb);
        let ty = self.b.context.types.int(bits);
        let low = self.cast(CastOp::Trunc, product, ty);
        let back = self.cast(CastOp::SExt, low, wide);
        let overflow = self.icmp(IntPredicate::Ne, back, product);
        self.write(&what.dests[0], low)?;
        if what.dests.len() == 2 {
            let by = self.b.int(bits * 2, i128::from(bits));
            let high = self.binary(BinaryOp::LShr, product, by);
            let high = self.cast(CastOp::Trunc, high, ty);
            self.write(&what.dests[1], high)?;
        }
        self.flags(Some(Desc { kind: Kind::Overflow(overflow), a, b, r: low, bits }), effects);
        Ok(())
    }

    /// `idiv`: DX:AX by the operand. The machine traps where the quotient
    /// does not fit; as C's division, that is left undefined.
    fn divide(&mut self, what: &Semantics, effects: &Effects) -> Emit<()> {
        let (high, low, divisor) = (self.read(&what.sources[0])?, self.read(&what.sources[1])?, self.read(&what.sources[2])?);
        let bits = self.bits_of(divisor);
        if bits != 16 {
            return Err(format!("a {}-bit dividend", bits * 2));
        }
        let dividend = self.join(low, high);
        let wide = self.b.context.types.int(32);
        let divisor = self.cast(CastOp::SExt, divisor, wide);
        let quotient = self.binary(BinaryOp::SDiv, dividend, divisor);
        let remainder = self.binary(BinaryOp::SRem, dividend, divisor);
        let ty = self.b.context.types.int(16);
        let (quotient, remainder) = (self.cast(CastOp::Trunc, quotient, ty), self.cast(CastOp::Trunc, remainder, ty));
        self.write(&what.dests[0], quotient)?;
        self.write(&what.dests[1], remainder)?;
        self.unknown_flags(effects, "idiv");
        Ok(())
    }

    // ------------------------------------------------------------------ flags

    /// Records what wrote the flags this instruction writes.
    pub fn flags(&mut self, desc: Option<Desc>, effects: &Effects) {
        let Some(desc) = desc else {
            return self.unknown_flags(effects, "an instruction");
        };
        let desc = Rc::new(desc);
        for (bit, flag) in BITS {
            if (effects.flags_written & flag).is_empty() {
                continue;
            }
            let written = match (&desc.kind, bit) {
                (Kind::Inc | Kind::Dec, Bit::C) => continue,
                (Kind::Shl(n) | Kind::Shr(n) | Kind::Sar(n), Bit::O) if *n != 1 => BitState::Unknown("reads OF after a shift by more than one".to_owned()),
                _ => BitState::Lazy(desc.clone()),
            };
            self.bits.insert(bit, written);
        }
    }

    /// Records every flag `effects` writes as one nothing may read.
    pub fn unknown_flags(&mut self, effects: &Effects, what: &str) {
        for (bit, flag) in BITS {
            if !(effects.flags_written & flag).is_empty() {
                self.bits.insert(bit, BitState::Unknown(format!("reads flags {what} leaves undefined")));
            }
        }
    }

    /// Sets every flag from one description, as a call answering in the flags does.
    pub fn set_flags(&mut self, desc: Desc) {
        let desc = Rc::new(desc);
        for (bit, _) in BITS {
            self.bits.insert(bit, BitState::Lazy(desc.clone()));
        }
    }

    /// Sets one flag to a computed value.
    pub fn set_bit(&mut self, bit: Bit, value: Operand) {
        self.bits.insert(bit, BitState::Value(value));
    }

    /// A flag's value here.
    pub fn bit(&mut self, bit: Bit) -> Emit<Operand> {
        match self.bits.get(&bit).cloned() {
            Some(BitState::Value(value)) => Ok(value),
            Some(BitState::Lazy(desc)) => {
                let value = self.compute(&desc, bit)?;
                self.bits.insert(bit, BitState::Value(value));
                Ok(value)
            }
            Some(BitState::Unknown(why)) => Err(why),
            None => Ok(self.get(Var::Bit(bit))),
        }
    }

    fn overflow_intrinsic(&mut self, name: &str, a: Operand, b: Operand) -> Emit<Operand> {
        let bits = self.bits_of(a);
        let full = format!("llvm.{name}.with.overflow.i{bits}");
        let &(callee, ty) = self.unit.intrinsics.get(&full).ok_or_else(|| format!("@{full} undeclared"))?;
        let pair = self.b.call(ty, Operand::Constant(callee), &[a, b], "").expect("a pair");
        self.note_pure();
        let flag = self.b.extract_value(pair, 1, "");
        self.note_pure();
        Ok(flag)
    }

    fn compute(&mut self, desc: &Desc, bit: Bit) -> Emit<Operand> {
        let Desc { kind, a, b, r, bits } = desc.clone();
        let zero = self.b.int(bits, 0);
        Ok(match (bit, kind) {
            (Bit::Z, _) => self.icmp(IntPredicate::Eq, r, zero),
            (Bit::S, _) => self.icmp(IntPredicate::Slt, r, zero),
            (Bit::C | Bit::O, Kind::Logic) => self.truth(false),
            (Bit::C | Bit::O, Kind::Overflow(flag)) => flag,
            (Bit::C, Kind::Add) => self.overflow_intrinsic("uadd", a, b)?,
            (Bit::C, Kind::Sub) => self.overflow_intrinsic("usub", a, b)?,
            (Bit::C, Kind::Adc(carry) | Kind::Sbb(carry)) => {
                let name = if matches!(desc.kind, Kind::Adc(_)) { "uadd" } else { "usub" };
                let op = if name == "uadd" { BinaryOp::Add } else { BinaryOp::Sub };
                let first = self.overflow_intrinsic(name, a, b)?;
                let partial = self.binary(op, a, b);
                let ty = self.b.type_of(a);
                let wide = self.cast(CastOp::ZExt, carry, ty);
                let second = self.overflow_intrinsic(name, partial, wide)?;
                self.binary(BinaryOp::Or, first, second)
            }
            (Bit::C, Kind::Neg) => self.icmp(IntPredicate::Ne, a, zero),
            (Bit::C, Kind::Shl(n)) => self.bit_of(a, bits - n),
            (Bit::C, Kind::Shr(n) | Kind::Sar(n)) => self.bit_of(a, n - 1),
            (Bit::O, Kind::Add | Kind::Adc(_)) => {
                let (ar, br) = (self.binary(BinaryOp::Xor, a, r), self.binary(BinaryOp::Xor, b, r));
                let both = self.binary(BinaryOp::And, ar, br);
                self.icmp(IntPredicate::Slt, both, zero)
            }
            (Bit::O, Kind::Sub | Kind::Sbb(_)) => {
                let (ab, ar) = (self.binary(BinaryOp::Xor, a, b), self.binary(BinaryOp::Xor, a, r));
                let both = self.binary(BinaryOp::And, ab, ar);
                self.icmp(IntPredicate::Slt, both, zero)
            }
            (Bit::O, Kind::Inc) => {
                let least = self.b.int(bits, 1i128 << (bits - 1));
                self.icmp(IntPredicate::Eq, r, least)
            }
            (Bit::O, Kind::Dec) => {
                let most = self.b.int(bits, (1i128 << (bits - 1)) - 1);
                self.icmp(IntPredicate::Eq, r, most)
            }
            (Bit::O, Kind::Neg) => {
                let least = self.b.int(bits, 1i128 << (bits - 1));
                self.icmp(IntPredicate::Eq, a, least)
            }
            (Bit::O, Kind::Shl(1)) => {
                let sign = self.icmp(IntPredicate::Slt, r, zero);
                let carry = self.bit_of(a, bits - 1);
                self.binary(BinaryOp::Xor, sign, carry)
            }
            (Bit::O, Kind::Shr(1)) => self.icmp(IntPredicate::Slt, a, zero),
            (Bit::O, Kind::Sar(1)) => self.truth(false),
            (bit, kind) => return Err(format!("reads {bit:?}F after {kind:?}")),
        })
    }

    /// Bit `index` of `value`, as an `i1`.
    fn bit_of(&mut self, value: Operand, index: u32) -> Operand {
        let bits = self.bits_of(value);
        let by = self.b.int(bits, i128::from(index));
        let shifted = self.binary(BinaryOp::LShr, value, by);
        let ty = self.b.context.types.int(1);
        self.cast(CastOp::Trunc, shifted, ty)
    }

    /// What a conditional branch tests.
    fn condition(&mut self, name: &str) -> Emit<Operand> {
        if let Some(value) = self.compared(name) {
            return Ok(value);
        }
        let negated = |me: &mut Self, value: Operand| {
            let one = me.truth(true);
            me.binary(BinaryOp::Xor, value, one)
        };
        let less = |me: &mut Self| -> Emit<Operand> {
            let (s, o) = (me.bit(Bit::S)?, me.bit(Bit::O)?);
            Ok(me.binary(BinaryOp::Xor, s, o))
        };
        Ok(match name {
            "je" => self.bit(Bit::Z)?,
            "jne" => {
                let z = self.bit(Bit::Z)?;
                negated(self, z)
            }
            "jb" => self.bit(Bit::C)?,
            "jae" => {
                let c = self.bit(Bit::C)?;
                negated(self, c)
            }
            "jbe" | "ja" => {
                let (c, z) = (self.bit(Bit::C)?, self.bit(Bit::Z)?);
                let either = self.binary(BinaryOp::Or, c, z);
                if name == "ja" { negated(self, either) } else { either }
            }
            "jl" => less(self)?,
            "jge" => {
                let l = less(self)?;
                negated(self, l)
            }
            "jle" | "jg" => {
                let l = less(self)?;
                let z = self.bit(Bit::Z)?;
                let either = self.binary(BinaryOp::Or, z, l);
                if name == "jg" { negated(self, either) } else { either }
            }
            "js" => self.bit(Bit::S)?,
            "jns" => {
                let s = self.bit(Bit::S)?;
                negated(self, s)
            }
            "jo" => self.bit(Bit::O)?,
            "jno" => {
                let o = self.bit(Bit::O)?;
                negated(self, o)
            }
            other => return Err(format!("{other} reads PF")),
        })
    }

    /// A branch on a comparison or logical operation still in this block,
    /// as the `icmp` it is.
    fn compared(&mut self, name: &str) -> Option<Operand> {
        let states: Vec<Rc<Desc>> = BITS
            .iter()
            .filter_map(|(bit, _)| match self.bits.get(bit) {
                Some(BitState::Lazy(desc)) => Some(desc.clone()),
                _ => None,
            })
            .collect();
        if states.len() != BITS.len() || !states.iter().all(|one| Rc::ptr_eq(one, &states[0])) {
            return None;
        }
        let desc = states[0].clone();
        let zero = self.b.int(desc.bits, 0);
        let (predicate, a, b) = match (&desc.kind, name) {
            (Kind::Sub, _) => {
                let predicate = match name {
                    "je" => IntPredicate::Eq,
                    "jne" => IntPredicate::Ne,
                    "jl" => IntPredicate::Slt,
                    "jge" => IntPredicate::Sge,
                    "jle" => IntPredicate::Sle,
                    "jg" => IntPredicate::Sgt,
                    "jb" => IntPredicate::Ult,
                    "jae" => IntPredicate::Uge,
                    "jbe" => IntPredicate::Ule,
                    "ja" => IntPredicate::Ugt,
                    "js" => return Some(self.icmp(IntPredicate::Slt, desc.r, zero)),
                    "jns" => return Some(self.icmp(IntPredicate::Sge, desc.r, zero)),
                    _ => return None,
                };
                (predicate, desc.a, desc.b)
            }
            (Kind::Logic, _) => {
                let predicate = match name {
                    "je" | "jbe" => IntPredicate::Eq,
                    "jne" | "ja" => IntPredicate::Ne,
                    "jl" | "js" => IntPredicate::Slt,
                    "jge" | "jns" => IntPredicate::Sge,
                    "jle" => IntPredicate::Sle,
                    "jg" => IntPredicate::Sgt,
                    _ => return None,
                };
                (predicate, desc.r, zero)
            }
            _ => return None,
        };
        Some(self.icmp(predicate, a, b))
    }

    // ----------------------------------------------------------------- memory

    /// The width of a location, and its value.
    pub fn read(&mut self, loc: &Loc) -> Emit<Operand> {
        match loc {
            Loc::Reg(reg) => match reg.register {
                register if self.unit.objects.names_data(register) => {
                    let selector = self.selector()?;
                    let word = self.b.context.types.int(16);
                    Ok(self.cast(CastOp::PtrToInt, selector, word))
                }
                Register::ES => {
                    let es = self.get(Var::Es);
                    let word = self.b.context.types.int(16);
                    Ok(self.cast(CastOp::PtrToInt, es, word))
                }
                Register::CS => {
                    let code = self.unit.objects.code().ok_or("reads cs, with no code segment")?;
                    let (segment, word) = (self.b.context.types.ptr(SEGMENT), self.b.context.types.int(16));
                    let selector = self.cast(CastOp::AddrSpaceCast, Operand::Constant(code), segment);
                    Ok(self.cast(CastOp::PtrToInt, selector, word))
                }
                register => self.register(register),
            },
            Loc::Imm(imm) => match imm.address {
                None => Ok(self.b.int(imm.width * 8, i128::from(imm.value))),
                Some(address) if imm.width == 2 && address.space == Space::Segment && !self.unit.objects.in_dgroup(address.index) => {
                    // An offset into a far segment: its far pointer's low word.
                    let far = self.unit.objects.far_address(self.b.context, address.index, address.disp).ok_or("an address outside DGROUP")?;
                    let (long, word) = (self.b.context.types.int(32), self.b.context.types.int(16));
                    let whole = self.cast(CastOp::PtrToInt, Operand::Constant(far), long);
                    Ok(self.cast(CastOp::Trunc, whole, word))
                }
                Some(address) if imm.width == 2 => {
                    if let Some(key) = (address.space == Space::Segment).then(|| self.unit.objects.key(address.index, address.disp)).flatten() {
                        return Ok(self.b.int(16, i128::from(key)));
                    }
                    let pointer = self.symbol(address)?;
                    let word = self.b.context.types.int(16);
                    Ok(self.cast(CastOp::PtrToInt, pointer, word))
                }
                Some(_) => Err(format!("a {}-byte address", imm.width)),
            },
            Loc::Mem(mem) => {
                let insn = self.insn.clone().ok_or("memory with no instruction")?;
                let pointer = self.pointer(&insn)?;
                let ty = self.b.context.types.int(mem.width * 8);
                Ok(self.b.load(ty, pointer, false, ""))
            }
            Loc::St(_) => Err("x87 registers".to_owned()),
            other => Err(format!("reads {other:?}")),
        }
    }

    pub fn write(&mut self, loc: &Loc, value: Operand) -> Emit<()> {
        match loc {
            Loc::Reg(reg) => match reg.register {
                Register::ES => {
                    let segment = self.b.context.types.ptr(SEGMENT);
                    let made = self.b.cast(CastOp::IntToPtr, value, segment, "");
                    self.set(Var::Es, made);
                    Ok(())
                }
                register => self.set_register(register, value),
            },
            Loc::Mem(_) => {
                let insn = self.insn.clone().ok_or("memory with no instruction")?;
                let pointer = self.pointer(&insn)?;
                self.b.store(value, pointer, false);
                Ok(())
            }
            other => Err(format!("writes {other:?}")),
        }
    }

    /// DGROUP's selector: the segment of any of its objects.
    fn selector(&mut self) -> Emit<Operand> {
        let object = self.unit.objects.any().ok_or("no DGROUP object to name DS by")?;
        let (far, segment) = (self.b.context.types.ptr(FAR), self.b.context.types.ptr(SEGMENT));
        let far = self.cast(CastOp::AddrSpaceCast, Operand::Constant(object.reference), far);
        Ok(self.cast(CastOp::AddrSpaceCast, far, segment))
    }

    /// Where a relocated address points.
    fn symbol(&mut self, address: Addr) -> Emit<Operand> {
        match address.space {
            Space::Segment => {
                if !self.unit.objects.in_dgroup(address.index) {
                    return Err("an address outside DGROUP".to_owned());
                }
                let object = self.unit.objects.at(address.index, address.disp).ok_or("an address past its segment")?.clone();
                Ok(self.offset(Operand::Constant(object.reference), address.disp - object.start))
            }
            Space::External => {
                let external = self.unit.objects.external(address.index).ok_or("an unknown external")?;
                Ok(self.offset(Operand::Constant(external), address.disp))
            }
            Space::Group => Err("a DGROUP-relative address".to_owned()),
            other => Err(format!("a {} address", other.name().to_lowercase())),
        }
    }

    /// The address of an instruction's memory operand.
    pub fn pointer(&mut self, insn: &Insn) -> Emit<Operand> {
        let raw = &insn.insn;
        let (base, index) = (raw.memory_base(), raw.memory_index());
        if base.size() == 4 || index.size() == 4 {
            return Err("32-bit addressing".to_owned());
        }
        let segment = match raw.segment_prefix() {
            Register::None if base == Register::BP => Register::SS,
            Register::None => Register::DS,
            one => one,
        };
        let disp = i64::from(raw.memory_displacement32() as u16 as i16);
        let resolved = insn.disp_at.map(|at| self.unit.facts.found.resolve(at as i64, disp)).filter(|one| one.space != Space::Literal);
        let mut sum: Option<Operand> = None;
        for register in [base, index] {
            if matches!(register, Register::None | Register::BP) {
                continue;
            }
            let value = self.register(register)?;
            sum = Some(match sum {
                None => value,
                Some(before) => self.binary(BinaryOp::Add, before, value),
            });
        }
        if base == Register::BP {
            if resolved.is_some() {
                return Err("a relocated BP-relative address".to_owned());
            }
            let pointer = self.frame_pointer(disp, raw.memory_size().size() as i64, sum.is_some())?;
            return Ok(match sum {
                Some(sum) => self.indexed(pointer, sum),
                None => pointer,
            });
        }
        match (segment, resolved) {
            (segment, Some(address)) if self.unit.objects.names_data(segment) => {
                let pointer = self.symbol(address)?;
                Ok(match sum {
                    Some(sum) => self.indexed(pointer, sum),
                    None => pointer,
                })
            }
            (Register::ES, Some(_)) => Err("es: with a relocated displacement".to_owned()),
            (segment, None) if segment == Register::ES || self.unit.objects.names_data(segment) => {
                let at = self.plus(sum, disp);
                self.segmented(segment, at)
            }
            (other, _) => Err(format!("{}: memory", name(other))),
        }
    }

    /// The address `segment:offset`, the offset a word.
    pub fn segmented(&mut self, segment: Register, offset: Operand) -> Emit<Operand> {
        match segment {
            segment if self.unit.objects.names_data(segment) => {
                let ptr = self.b.context.types.ptr(0);
                Ok(self.cast(CastOp::IntToPtr, offset, ptr))
            }
            Register::ES => {
                let es = self.get(Var::Es);
                let far = self.b.context.types.ptr(FAR);
                let base = self.cast(CastOp::AddrSpaceCast, es, far);
                Ok(self.indexed(base, offset))
            }
            other => Err(format!("{}: memory", name(other))),
        }
    }

    /// `sum + disp` as a word, either part absent.
    fn plus(&mut self, sum: Option<Operand>, disp: i64) -> Operand {
        let constant = self.b.int(16, i128::from(disp));
        match sum {
            Some(sum) if disp == 0 => sum,
            Some(sum) => self.binary(BinaryOp::Add, sum, constant),
            None => constant,
        }
    }

    /// Where `[bp + disp]` is in the frame, for an access `width` wide.
    fn frame_pointer(&mut self, disp: i64, width: i64, indexed: bool) -> Emit<Operand> {
        if self.frame != Frame::Active {
            return Err(format!("[bp{disp:+}] outside its frame"));
        }
        let layout = self.layout.ok_or("[bp] with no frame")?;
        if let Some((start, bytes, arguments)) = layout.arguments {
            if start <= disp && (indexed || disp + width <= start + bytes) {
                return Ok(self.offset(arguments, disp - start));
            }
        }
        if self.frame == Frame::Active && layout.low <= disp && (indexed || disp + width <= layout.high) {
            return Ok(self.offset(layout.locals, disp - layout.low));
        }
        Err(format!("[bp{disp:+}] outside its frame"))
    }

    /// Stores `value` below the pushed bytes.
    pub fn push(&mut self, value: Operand) -> Emit<()> {
        let bytes = i64::from(self.bits_of(value) / 8).max(2);
        let value = if self.bits_of(value) == 8 {
            let word = self.b.context.types.int(16);
            self.cast(CastOp::SExt, value, word)
        } else {
            value
        };
        self.depth += bytes;
        self.deepest = self.deepest.max(self.depth);
        let depth = self.depth;
        self.pushes.retain(|&at, &mut (_, width)| at - width >= depth || at <= depth - bytes);
        self.pushes.insert(depth, (value, bytes));
        let slot = self.slot(depth);
        self.b.store(value, slot, false);
        Ok(())
    }

    /// The pushed bytes above `depth` are gone.
    fn forget_above(&mut self, depth: i64) {
        self.pushes.retain(|&at, _| at <= depth);
    }

    /// The `width` bytes at `depth`: those of what a push stored, or a load.
    fn pushed(&mut self, depth: i64, width: u32) -> Operand {
        let found = self.pushes.iter().map(|(&at, &(value, bytes))| (at - depth, value, bytes)).find(|&(offset, _, bytes)| offset >= 0 && offset + i64::from(width) <= bytes);
        if let Some((offset, value, bytes)) = found {
            if offset == 0 && bytes == i64::from(width) {
                return value;
            }
            if let Some(constant) = self.constant(value) {
                return self.b.int(width * 8, i128::from(constant >> (8 * offset)));
            }
            let (by, ty) = (self.b.int(self.bits_of(value), i128::from(8 * offset)), self.b.context.types.int(width * 8));
            let shifted = self.binary(BinaryOp::LShr, value, by);
            return self.cast(CastOp::Trunc, shifted, ty);
        }
        let slot = self.slot(depth);
        let ty = self.b.context.types.int(width * 8);
        self.b.load(ty, slot, false, "")
    }

    pub fn pop(&mut self, width: u32) -> Emit<Operand> {
        let bytes = i64::from(width);
        if self.depth < bytes {
            return Err("pops what it did not push".to_owned());
        }
        let value = self.pushed(self.depth, width);
        self.depth -= bytes;
        self.forget_above(self.depth);
        Ok(value)
    }

    /// The pushed byte `depth` bytes below where the pushes began.
    fn slot(&mut self, depth: i64) -> Operand {
        self.offset(self.top, -depth)
    }

    /// The word at `depth` in the pushed bytes.
    pub fn stack_word(&mut self, depth: i64, width: u32) -> Emit<Operand> {
        if depth > self.depth || depth < i64::from(width) {
            return Err("reads arguments it did not push".to_owned());
        }
        Ok(self.pushed(depth, width))
    }

    pub fn depth(&self) -> i64 {
        self.depth
    }

    pub fn popped(&mut self, bytes: i64) -> Emit<()> {
        if self.depth < bytes {
            return Err("a call pops what was not pushed".to_owned());
        }
        self.depth -= bytes;
        self.forget_above(self.depth);
        Ok(())
    }

    /// An integer constant's value.
    pub fn constant(&self, value: Operand) -> Option<i64> {
        let Operand::Constant(id) = value else { return None };
        match self.b.context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits as i64),
            _ => None,
        }
    }

    // ------------------------------------------------------------------ calls

    fn call(&mut self, callee: &str, at: usize) -> Emit<()> {
        match callee {
            FRAME_ENTRY => return self.enter_frame(),
            FRAME_EXIT => return self.exit_frame(),
            _ => {}
        }
        if let Some(procedure) = self.unit.procedures.get(callee).cloned() {
            let (reference, ty, interface) = procedure.map_err(|why| format!("calls {callee}, whose interface is unknown: {why}"))?;
            let mut arguments = Vec::new();
            let words = interface.popped / 2;
            for index in 0..words {
                let depth = self.depth - interface.popped + 2 + 2 * index;
                arguments.push(self.stack_word(depth, 2)?);
            }
            let answered = self.b.call_as(llrm_mir::opcode::BASIC, ty, Operand::Constant(reference), &arguments, "");
            self.popped(interface.popped)?;
            let why = format!("{callee} clobbers it");
            self.clobber(&TRACKED, &why);
            let ty = self.var_type(Var::Es);
            let es = self.sentinel(ty, format!("reads es after {why}"));
            self.set(Var::Es, es);
            if let (Some(value), Answer::Registers(registers)) = (answered, &interface.answer) {
                self.answered(value, registers)?;
            }
            return Ok(());
        }
        self.runtime_call(callee, at, None)
    }

    /// A call of runtime routine `callee`, which pops `stack` bytes where
    /// its declaration does not say.
    pub fn runtime_call(&mut self, callee: &str, at: usize, stack: Option<i64>) -> Emit<()> {
        let contract = self.unit.facts.contract(at).ok_or_else(|| format!("{callee} has no contract"))?.clone();
        let spec = match self.unit.callees.named.get(callee) {
            Some(Ok(spec)) => spec.clone(),
            Some(Err(why)) => return Err(why.clone()),
            None => return Err(format!("{callee} is undeclared")),
        };
        let direct: Vec<Register> = llrm_bcmachine::abi::runtime::direct_slots(&contract).into_iter().filter_map(crate::machine::from_contract).collect();
        let mut arguments = Vec::new();
        for &root in &spec.inputs {
            arguments.push(if direct.contains(&root) { self.register(word(root))? } else { self.b.int(16, 0) });
        }
        let stack = stack.unwrap_or(spec.stack);
        for index in 0..stack / 2 {
            let depth = if spec.pops { self.depth - stack + 2 + 2 * index } else { self.depth - 2 * index };
            arguments.push(self.stack_word(depth, 2)?);
        }
        let answered = self.b.call_as(spec.convention, spec.ty, Operand::Constant(spec.reference), &arguments, "");
        if spec.pops {
            self.popped(stack)?;
        }
        let disturbed: Vec<Register> =
            llrm_bcmachine::abi::runtime::disturbs(&contract).into_iter().filter_map(crate::machine::from_contract).filter(|&one| one != crate::machine::FLAGS).collect();
        let why = format!("{callee} clobbers it");
        self.clobber(&disturbed, &why);
        if contract.clobbers.contains(&llrm_bcmachine::abi::runtime::Reg::Es) {
            let ty = self.var_type(Var::Es);
            let one = self.sentinel(ty, format!("reads es after {why}"));
            self.set(Var::Es, one);
        }
        match (&spec.answer, answered) {
            (Answer::Registers(registers), Some(value)) => self.answered(value, registers)?,
            (Answer::Flags, Some(value)) => {
                let zero = self.b.int(16, 0);
                self.set_flags(Desc { kind: Kind::Sub, a: value, b: zero, r: value, bits: 16 });
            }
            _ => {}
        }
        Ok(())
    }

    /// Puts a callee's answer in the registers it answers in.
    fn answered(&mut self, value: Operand, registers: &[Register]) -> Emit<()> {
        let word_ty = self.b.context.types.int(16);
        match registers {
            [one] => self.set_word(*one, value),
            [Register::EAX, Register::EDX] => {
                let low = self.cast(CastOp::Trunc, value, word_ty);
                let by = self.b.int(32, 16);
                let high = self.binary(BinaryOp::LShr, value, by);
                let high = self.cast(CastOp::Trunc, high, word_ty);
                self.set_word(Register::EAX, low)?;
                self.set_word(Register::EDX, high)
            }
            _ => {
                for (index, &one) in registers.iter().enumerate() {
                    let part = self.b.extract_value(value, index as u32, "");
                    self.set_word(one, part)?;
                }
                Ok(())
            }
        }
    }

    /// `B$ENRA`: CX bytes of locals below the runtime's header.
    fn enter_frame(&mut self) -> Emit<()> {
        if self.frame != Frame::Before || self.body.body.kind != BodyKind::Procedure {
            return Err(format!("{FRAME_ENTRY} twice, or outside a procedure"));
        }
        if self.depth != 0 {
            return Err(format!("{FRAME_ENTRY} with bytes pushed"));
        }
        let header = self.unit.header.ok_or("no frame header size for this compiler")?;
        let size = self.register(Register::CX)?;
        let size = self.constant(size).ok_or("a frame of variable size")?;
        // VBDOS's B$ENRA takes in BX the string temporaries it gives the
        // frame; QB 4.5's and PDS's code sets no BX for it.
        if self.unit.facts.family() == llrm_bcmachine::objectfile::module::Family::Vbdos {
            let temporaries = self.register(Register::BX)?;
            if self.constant(temporaries) != Some(0) {
                return Err(format!("{FRAME_ENTRY} with string temporaries"));
            }
        }
        let high = -header;
        self.layout = Some(self.frame_layout(high - size, high, header)?);
        self.frame = Frame::Active;
        self.clobber(&TRACKED, &format!("{FRAME_ENTRY} clobbers it"));
        Ok(())
    }

    /// `B$EXSA`: AX and DX pass through, the rest restored.
    fn exit_frame(&mut self) -> Emit<()> {
        if self.frame != Frame::Active {
            return Err(format!("{FRAME_EXIT} outside its frame"));
        }
        if self.depth != 0 {
            return Err(format!("{FRAME_EXIT} with bytes pushed"));
        }
        self.frame = Frame::After;
        self.clobber(&[Register::EBX, Register::ECX, Register::ESI, Register::EDI], &format!("{FRAME_EXIT} restores it"));
        Ok(())
    }

    // ------------------------------------------------------------ resolution

    /// Answers every placeholder with what reaches it.
    fn resolve(&mut self) -> Emit<()> {
        let mut replaced: BTreeMap<ValueId, Operand> = BTreeMap::new();
        let mut by_var: BTreeMap<Var, Vec<(BlockId, ValueId)>> = BTreeMap::new();
        for &(block, var, value) in &self.placeholders {
            by_var.entry(var).or_default().push((block, value));
        }
        let ends: Vec<(BlockId, HashMap<Var, Operand>, HashMap<Bit, BitState>)> =
            self.ends.iter().map(|(block, (vars, bits))| (*block, vars.clone(), bits.clone())).collect();
        for (var, readers) in by_var {
            let ty = self.var_type(var);
            let mut updater = SsaUpdater::new(ty, None);
            for (block, vars, bits) in &ends {
                let value = match var {
                    Var::Bit(bit) => match bits.get(&bit) {
                        Some(BitState::Value(value)) => Some(*value),
                        Some(BitState::Unknown(why)) => {
                            self.block = *block;
                            Some(self.sentinel(ty, why.clone()))
                        }
                        Some(BitState::Lazy(_)) => {
                            self.block = *block;
                            Some(self.sentinel(ty, "reads a flag its writer's block did not keep".to_owned()))
                        }
                        None => vars.get(&var).copied(),
                    },
                    _ => vars.get(&var).copied(),
                };
                if let Some(value) = value {
                    let value = provider(value, &replaced).map_err(|error| error.to_string())?;
                    updater.add_available_value(*block, value);
                }
            }
            for (block, placeholder) in readers {
                let value = updater.value_in_middle_of_block(self.b.context, self.b.function, block);
                let value = provider(value, &replaced).map_err(|error| error.to_string())?;
                self.b.function.replace_all_uses_with(placeholder, value);
                replaced.insert(placeholder, value);
                let inst = match self.b.function.value(placeholder).def {
                    llrm_mir::ValueDef::Instruction(inst) => inst,
                    _ => unreachable!("a placeholder is an instruction"),
                };
                self.b.function.set_operands(inst, Vec::new());
                self.b.function.erase(inst)?;
            }
        }
        Ok(())
    }

    /// Refuses a read of what the machine left undefined, sweeps what
    /// computes nothing read, and sizes the pushed bytes.
    fn finish(&mut self) -> Emit<()> {
        self.sweep();
        let mut sentinels: Vec<(ValueId, Sentinel)> = self.sentinels.drain().collect();
        sentinels.sort_by_key(|(value, _)| *value);
        for (value, sentinel) in sentinels {
            let users: Vec<llrm_mir::Use> = self.b.function.users(value).to_vec();
            let inst = match self.b.function.value(value).def {
                llrm_mir::ValueDef::Instruction(inst) => inst,
                _ => unreachable!("a sentinel is an instruction"),
            };
            if !users.is_empty() {
                // A partial write keeps the rest of an undefined word; only
                // a read of those bits reads it.
                if demanded(self.b.function, self.b.context, value, &mut HashMap::new()) != 0 {
                    return Err(sentinel.why);
                }
                let ty = self.b.function.value(value).ty;
                let zero = Operand::Constant(self.b.context.int(ty, 0));
                self.b.function.replace_all_uses_with(value, zero);
            }
            self.b.function.set_operands(inst, Vec::new());
            self.b.function.erase(inst)?;
        }
        self.sweep();
        let Operand::Value(top) = self.top else { unreachable!("a placeholder") };
        let inst = match self.b.function.value(top).def {
            llrm_mir::ValueDef::Instruction(inst) => inst,
            _ => unreachable!(),
        };
        if self.deepest > 0 {
            let byte = self.b.context.types.int(8);
            let ty = self.b.context.types.intern(Type::Array { element: byte, count: self.deepest as u64 });
            let stack = self.b.alloca(ty, "stack");
            let index = self.b.int(16, i128::from(self.deepest));
            let made = self.b.function.create_instruction(Opcode::GetElementPtr { source: byte }, self.b.type_of(stack), vec![stack, index], Flags::default(), Some("pushed"));
            self.b.function.insert(made, Position::Before(inst)).expect("the entry");
            let value = Operand::Value(self.b.function.instruction(made).result.expect("a pointer"));
            self.b.function.replace_all_uses_with(top, value);
        }
        self.b.function.set_operands(inst, Vec::new());
        self.b.function.erase(inst)?;
        Ok(())
    }

    /// Erases what this emitter made to compute a value nothing reads.
    fn sweep(&mut self) {
        let mut changed = true;
        while changed {
            changed = false;
            for inst in self.pure.clone() {
                if self.b.function.is_erased(inst) {
                    continue;
                }
                let Some(result) = self.b.function.instruction(inst).result else { continue };
                if self.b.function.users(result).is_empty() {
                    self.b.function.set_operands(inst, Vec::new());
                    let _ = self.b.function.erase(inst);
                    changed = true;
                }
            }
        }
    }
}

/// The bits of `value` anything observes, as LLVM's DemandedBits finds
/// them, for the few operations a register's partial write is built of.
fn demanded(function: &llrm_mir::Function, context: &llrm_mir::Context, value: ValueId, memo: &mut HashMap<ValueId, u64>) -> u64 {
    let width = context.types.int_bits(function.value(value).ty).unwrap_or(64);
    let all = if width >= 64 { u64::MAX } else { (1u64 << width) - 1 };
    if let Some(&known) = memo.get(&value) {
        return known;
    }
    // A cycle through a phi observes everything until proven otherwise.
    memo.insert(value, all);
    let constant = |operand: &Operand| match operand {
        Operand::Constant(id) => match context.get(*id).kind {
            ConstantKind::Int(bits) => Some(bits as u64),
            _ => None,
        },
        _ => None,
    };
    let mut out = 0u64;
    for one in function.users(value).to_vec() {
        let user = function.instruction(one.user);
        let result = |memo: &mut HashMap<ValueId, u64>| user.result.map_or(all, |result| demanded(function, context, result, memo));
        let other = user.operands.get(1 - one.index as usize).filter(|_| one.index < 2);
        out |= match &user.opcode {
            Opcode::Binary(BinaryOp::And) => result(memo) & other.and_then(constant).unwrap_or(u64::MAX),
            Opcode::Binary(BinaryOp::Or | BinaryOp::Xor) => result(memo),
            Opcode::Binary(BinaryOp::Shl) if one.index == 0 => match other.and_then(constant) {
                Some(by) if by < 64 => result(memo) >> by,
                _ => all,
            },
            Opcode::Binary(BinaryOp::LShr) if one.index == 0 => match other.and_then(constant) {
                Some(by) if by < 64 => result(memo) << by,
                _ => all,
            },
            Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul) => {
                let observed = result(memo);
                if observed == 0 { 0 } else { u64::MAX >> observed.leading_zeros() }
            }
            Opcode::Cast(CastOp::Trunc | CastOp::ZExt) => result(memo),
            Opcode::Phi | Opcode::Freeze => result(memo),
            Opcode::Select if one.index > 0 => result(memo),
            _ => all,
        } & all;
        if out == all {
            break;
        }
    }
    memo.insert(value, out);
    out
}

/// Whether a node ends its block's control: a branch, jump or return.
fn is_transfer(node: &Node) -> bool {
    matches!(node.semantics().op, Operation::Jump | Operation::Branch | Operation::Return | Operation::Escape)
}

fn insn_of(node: &Node) -> Option<&Insn> {
    match node {
        Node::Opaque(one) => Some(&one.insn),
        Node::Long(one) => Some(&one.insn),
        Node::Call(one) => Some(&one.insn),
        _ => None,
    }
}

fn loc_width(loc: &Loc) -> Option<u32> {
    match loc {
        Loc::Reg(one) => Some(one.width),
        Loc::Mem(one) => Some(one.width),
        _ => None,
    }
}

fn is_high_byte(register: Register) -> bool {
    matches!(register, Register::AH | Register::BH | Register::CH | Register::DH)
}

/// A root's 16-bit register.
fn word(root: Register) -> Register {
    match root {
        Register::EAX => Register::AX,
        Register::EBX => Register::BX,
        Register::ECX => Register::CX,
        Register::EDX => Register::DX,
        Register::ESI => Register::SI,
        Register::EDI => Register::DI,
        other => other,
    }
}

fn half_name(index: usize, half: Half) -> String {
    match half {
        Half::Low => name(word(TRACKED[index])),
        Half::High => format!("the high word of {}", name(TRACKED[index])),
    }
}

pub fn name(register: Register) -> String {
    format!("{register:?}").to_lowercase()
}
