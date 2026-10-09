//! What the raise knows of a module's machine code before any MIR exists:
//! its bodies, blocks, decoded nodes and call contracts, and which registers
//! each node defines and reads. Adapted from llrm-core's `model/mir/raise.rs`
//! (`touched`, `call_touches`, `bodies`' preamble) and
//! `frontends/bc/raising_carried.rs`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use iced_x86::{Code, Register};
use llrm_qbruntime::{self as runtime, Contract, Control, Reg};
use llrm_target::machine::Machine;
use llrm_x86_bcmachine::abi::handlers;
use llrm_x86_bcmachine::frontends::bc::blocks::{self, Block};
use llrm_x86_bcmachine::frontends::bc::declen::{READS, WRITES, instruction_info_factory};
use llrm_x86_bcmachine::frontends::bc::extent::{Body, BodyKind};
use llrm_x86_bcmachine::frontends::bc::raising_control;
use llrm_x86_bcmachine::model::ir::decode;
use llrm_x86_bcmachine::model::ir::nodes::{Node, span};
use llrm_x86_bcmachine::model::ir::{Loc, Operation, root};
use llrm_x86_bcmachine::objectfile::cvinfo;
use llrm_x86_bcmachine::objectfile::module::{self, Module};
use llrm_x86_bcmachine::support::hash::IndexMap;

use crate::pairs::{self, Pair};

/// The registers that become values, rooted.
pub const TRACKED: [Register; 6] =
    [Register::EAX, Register::EBX, Register::ECX, Register::EDX, Register::ESI, Register::EDI];

/// The flags as one tracked name, where register sets hold them.
pub const FLAGS: Register = Register::None;

/// Registers as sets, FLAGS first.
pub type Registers = BTreeSet<Register>;

/// A contract register's root, or FLAGS; None for one MIR holds no value for.
pub fn from_contract(one: Reg) -> Option<Register> {
    Some(match one {
        Reg::Ax => Register::EAX,
        Reg::Bx => Register::EBX,
        Reg::Cx => Register::ECX,
        Reg::Dx => Register::EDX,
        Reg::Si => Register::ESI,
        Reg::Di => Register::EDI,
        Reg::Flags => FLAGS,
        Reg::Bp | Reg::Sp | Reg::Ds | Reg::Es => return None,
    })
}

/// The runtime routines that set up and tear down a BASIC frame: the raise
/// gives the function its frame instead of calling them.
pub const FRAME_ENTRY: &str = "B$ENRA";
pub const FRAME_EXIT: &str = "B$EXSA";

/// Which root a restore reads, and which it writes the high half into, by
/// `ir.FIXUP`'s pair numbering.
pub fn restore_pair(pair: usize) -> Option<(Register, Register)> {
    match pair {
        0 => Some((Register::EAX, Register::EDX)),
        1 => Some((Register::ECX, Register::EBX)),
        _ => None,
    }
}

/// A tracked root's low or high word, or the flags: what liveness tracks,
/// since a write of AX leaves EAX's high word as it was.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Word {
    Low(Register),
    High(Register),
    Flags,
}

pub type Words = BTreeSet<Word>;

/// The words a register is made of.
fn words(register: Register) -> Vec<Word> {
    if register == FLAGS {
        return vec![Word::Flags];
    }
    let Some(index) = tracked(register) else { return Vec::new() };
    let rooted = TRACKED[index];
    if register.size() == 4 { vec![Word::Low(rooted), Word::High(rooted)] } else { vec![Word::Low(rooted)] }
}

/// Both words of each root.
fn whole(roots: impl IntoIterator<Item = Register>) -> Words {
    roots
        .into_iter()
        .flat_map(|one| if one == FLAGS { vec![Word::Flags] } else { vec![Word::Low(one), Word::High(one)] })
        .collect()
}

/// What a call disturbs and reads, where its contract establishes it: it
/// disturbs whole roots and reads its inputs as words.
pub fn call_touches(routine: &Contract) -> Option<(Words, Words)> {
    if !routine.established && !runtime::established_inputs(routine) {
        return None;
    }
    let changed: Registers = runtime::disturbs(routine).into_iter().filter_map(from_contract).collect();
    let mut disturbed = whole(TRACKED.into_iter().filter(|one| changed.contains(one)));
    disturbed.insert(Word::Flags);
    // A contract names 16-bit registers: an input is its root's low word.
    let reads = runtime::direct_slots(routine)
        .into_iter()
        .filter_map(from_contract)
        .map(|one| if one == FLAGS { Word::Flags } else { Word::Low(one) })
        .collect();
    Some((disturbed, reads))
}

/// (defines, uses) of one node as words. A return reads what its function
/// answers in, `answer`.
pub fn touched(
    node: &Node,
    contracts: &IndexMap<i64, Contract>,
    answer: &[Register],
) -> (Words, Words) {
    if node.semantics().op == Operation::Return {
        return (Words::new(), answer.iter().map(|&one| Word::Low(one)).collect());
    }
    if matches!(
        node,
        Node::Opaque(opaque) if opaque.insn.insn.code() == Code::Into
    ) {
        return (Words::new(), Words::from([Word::Flags]));
    }
    // The frame's exit hands AX and DX through and restores the rest.
    if matches!(node, Node::Call(call) if call.name == FRAME_EXIT) {
        return (whole([Register::EBX, Register::ECX, Register::ESI, Register::EDI, FLAGS]), Words::new());
    }
    if node.semantics().op == Operation::Call {
        if let Some(known) = contracts.get(&(span(node).0 as i64)).and_then(call_touches) {
            return known;
        }
    }
    if let Node::Restore(restore) = node {
        if let Some((source, into)) = restore_pair(restore.pair) {
            return (Words::from([Word::Low(into)]), Words::from([Word::High(source)]));
        }
    }
    let effects = node.effects();
    let every = || whole(TRACKED.into_iter().chain([FLAGS]));
    let insn = match node {
        Node::Opaque(one) => &one.insn,
        Node::Long(one) => &one.insn,
        Node::Call(one) => &one.insn,
        _ => return (Words::new(), Words::new()),
    };
    let (mut defines, mut uses) = (Words::new(), Words::new());
    let mut factory = instruction_info_factory();
    for used in factory.info(&insn.insn).used_registers() {
        let register = used.register();
        let made = words(register);
        if WRITES.contains(&used.access()) {
            defines.extend(made.iter().copied());
            // A byte write keeps the other byte of its word.
            if register.size() == 1 {
                uses.extend(made.iter().copied());
            }
        }
        if READS.contains(&used.access()) {
            uses.extend(made);
        }
    }
    if effects.defs.is_none() {
        defines = every();
    }
    if effects.uses.is_none() {
        uses = every();
    }
    if !effects.flags_written.is_empty() {
        defines.insert(Word::Flags);
    }
    if !effects.flags_read.is_empty() {
        uses.insert(Word::Flags);
    }
    (defines, uses)
}

/// One body, as the raise reads it.
pub struct BodyFacts {
    pub body: Body,
    /// Its blocks reachable from its seed, the seed's first.
    pub blocks: Vec<Block>,
    /// Its nodes by where they start.
    pub nodes: IndexMap<i64, Arc<Node>>,
    /// A procedure's interface, or why it has none; None for another body.
    pub interface: Option<Result<Interface, String>>,
    /// Its longs' pairs of nodes, by the first's address.
    pub pairs: BTreeMap<i64, Pair>,
    /// The module's error handler, whose code runs on this body's frame.
    pub handler: Option<Handler>,
}

/// An error handler folded into the body it serves: its seed and blocks.
#[derive(Clone, Debug)]
pub struct Handler {
    pub seed: usize,
    pub blocks: BTreeSet<usize>,
}

impl BodyFacts {
    /// The node each block's instructions hold, in order.
    pub fn nodes_of<'a>(
        &'a self,
        block: &'a Block,
    ) -> impl Iterator<Item = &'a Arc<Node>> + 'a {
        block.insns.iter().filter_map(|insn| self.nodes.get(&(insn.at as i64)))
    }

    /// The tracked registers read after each call, before they are written:
    /// what a call must hand back of what it disturbs.
    pub fn live_after(
        &self,
        contracts: &IndexMap<i64, Contract>,
    ) -> IndexMap<i64, Words> {
        live_after(&self.blocks, &self.nodes, contracts, &self.answer())
    }

    /// The registers its returns read.
    pub fn answer(&self) -> Vec<Register> {
        match &self.interface {
            Some(Ok(Interface { answer: Answer::Registers(registers), .. })) => registers.clone(),
            _ => Vec::new(),
        }
    }
}

/// The registers live after each node, by the node's address.
pub fn live_after(
    blocks: &[Block],
    nodes: &IndexMap<i64, Arc<Node>>,
    contracts: &IndexMap<i64, Contract>,
    answer: &[Register],
) -> IndexMap<i64, Words> {
    let known: BTreeSet<usize> = blocks.iter().map(|block| block.at).collect();
    let mut live_in: IndexMap<usize, Words> = blocks.iter().map(|block| (block.at, Words::new())).collect();
    let mut after: IndexMap<i64, Words> = IndexMap::default();
    let mut moving = true;
    while moving {
        moving = false;
        for block in blocks.iter().rev() {
            let mut live: Words = block
                .succ
                .iter()
                .filter(|one| known.contains(one))
                .flat_map(|one| live_in[one].iter().copied())
                .collect();
            for insn in block.insns.iter().rev() {
                let Some(node) = nodes.get(&(insn.at as i64)) else { continue };
                after.insert(insn.at as i64, live.clone());
                let (defines, uses) = touched(node, contracts, answer);
                live = live.difference(&defines).copied().chain(uses).collect();
            }
            if live != live_in[&block.at] {
                live_in.insert(block.at, live);
                moving = true;
            }
        }
    }
    after
}

/// Each call site whose caller reads, after it, a register cmacros'
/// convention keeps but its contract lists as clobbered: the value before
/// the call is then an input. `raising_carried`'s rule.
fn carried(
    body: &BodyFacts,
    contracts: &IndexMap<i64, Contract>,
) -> IndexMap<i64, Contract> {
    let candidates: BTreeSet<Reg> =
        BTreeSet::from([Reg::Si, Reg::Di]).difference(&runtime::PER_CONVENTION).copied().collect();
    let mut chosen = contracts.clone();
    let mut changed: IndexMap<i64, Contract> = IndexMap::default();
    loop {
        let after = body.live_after(&chosen);
        let mut grown = false;
        for index in 0..chosen.len() {
            let (&at, routine) = chosen.get_index(index).expect("in range");
            if !after.contains_key(&at) || !runtime::established_inputs(routine) {
                continue;
            }
            let inputs = routine.inputs.clone().unwrap_or_default();
            let extra: BTreeSet<Reg> = runtime::disturbs(routine)
                .difference(&inputs)
                .filter(|one| candidates.contains(one))
                .filter(|&&one| {
                    from_contract(one).is_some_and(|root| {
                        after[&at].contains(&Word::Low(root)) || after[&at].contains(&Word::High(root))
                    })
                })
                .copied()
                .collect();
            if !extra.is_empty() {
                let mut made = routine.clone();
                made.inputs = Some(inputs.union(&extra).copied().collect());
                chosen.insert(at, made.clone());
                changed.insert(at, made);
                grown = true;
            }
        }
        if !grown {
            return changed;
        }
    }
}

/// A module's code, decoded, with a contract for every call.
pub struct Facts<'m> {
    pub found: &'m Module,
    /// The address spaces of the target the objects are recompiled for: the machine's layout's.
    pub spaces: llrm_mir::spaces::Spaces,
    pub bodies: Vec<BodyFacts>,
    pub contracts: IndexMap<i64, Contract>,
    /// Each procedure's CodeView record, by its entry.
    pub procedures: IndexMap<usize, cvinfo::Procedure>,
    /// Where /V and /W's event-poll adapter is, which each statement calls.
    pub event_stub: Option<usize>,
    /// Whether the runtime enters this module's code other than by a call:
    /// an error or event handler, or a RESUME target.
    pub handlers: bool,
    /// The handler each ON ERROR GOTO registers, by its call.
    pub registrations: BTreeMap<i64, i64>,
    /// The label each RESUME label continues at, by its call.
    pub resumptions: BTreeMap<i64, i64>,
    /// Where each statement starts, in address order, and its ERL: from the
    /// statement table, whose rows for statements without code share the
    /// next one's start, which keeps the last's line.
    pub statements: Vec<(usize, i64)>,
}

impl<'m> Facts<'m> {
    pub fn new(
        found: &'m Module,
        machine: &Machine,
    ) -> Result<Self, String> {
        let decoded = decode::decode_module(found, machine)?;
        let mapped = blocks::code_map(found)?;
        let all = blocks::partition(found, &mapped);
        let mut contracts =
            llrm_x86_bcmachine::abi::callsite::for_module(found, None).map_err(|error| error.to_string())?;
        let header = blocks::has_header(found);
        let procedures: IndexMap<usize, cvinfo::Procedure> = if header {
            cvinfo::parse(&found.records).procedures.into_iter().map(|one| (one.offset as usize, one)).collect()
        } else {
            IndexMap::default()
        };
        // A call to the event-poll adapter is a call to B$EVCK.
        let event_stub = blocks::event_stub(found);
        let polls: IndexMap<i64, String> = decoded
            .iter()
            .flat_map(|one| one.nodes.iter())
            .filter(|node| polls(node, event_stub))
            .map(|node| (span(node).0 as i64, crate::runtime::EVENT_POLL.to_owned()))
            .collect();
        contracts.extend(runtime::per_call(&polls, module::family(&found.records).value(), &BTreeSet::new()));
        let mut bodies = Vec::new();
        for one in decoded {
            let nodes: IndexMap<i64, Arc<Node>> =
                one.nodes.iter().map(|node| (span(node).0 as i64, node.clone())).collect();
            let mine: Vec<Block> = all
                .iter()
                .filter(|block| one.body.ranges.iter().any(|&(lo, hi)| lo <= block.at && block.at < hi))
                .cloned()
                .collect();
            let seeds: Vec<usize> = std::iter::once(one.body.seed).chain(one.body.entries.iter().copied()).collect();
            let mine = reachable(raising_control::terminal_edges(mine, &contracts), &seeds);
            let interface =
                (one.body.kind == BodyKind::Procedure).then(|| interface(&nodes, procedures.get(&one.body.seed)));
            let pairs = pairs::found(&mine, &nodes);
            bodies.push((
                BodyFacts { body: one.body, blocks: mine, nodes, interface: None, pairs, handler: None },
                interface,
            ));
        }
        // Without CodeView, a procedure answers in what its callers read after it.
        let mut read: BTreeMap<String, Words> = BTreeMap::new();
        for (body, _) in &bodies {
            let after = body.live_after(&contracts);
            for node in body.nodes.values() {
                if let Node::Call(call) = &**node {
                    let live = after.get(&(call.insn.at as i64)).cloned().unwrap_or_default();
                    read.entry(call.name.clone()).or_default().extend(live);
                }
            }
        }
        let bodies: Vec<BodyFacts> = bodies
            .into_iter()
            .map(|(mut body, interface)| {
                body.interface = interface.map(|made| {
                    made.map(|(popped, answer)| {
                        let answer = answer.unwrap_or_else(|| {
                            let read =
                                body.body.name.as_ref().and_then(|name| read.get(name)).cloned().unwrap_or_default();
                            match (read.contains(&Word::Low(Register::EAX)), read.contains(&Word::Low(Register::EDX))) {
                                (_, true) => Answer::Registers(vec![Register::EAX, Register::EDX]),
                                (true, false) => Answer::Registers(vec![Register::EAX]),
                                _ => Answer::None,
                            }
                        });
                        Interface { popped, answer }
                    })
                });
                body
            })
            .collect();
        for body in &bodies {
            contracts.extend(carried(body, &contracts));
        }
        let bodies = folded(bodies);
        let handlers =
            bodies.iter().any(|body| matches!(
                body.body.kind,
                BodyKind::ErrorHandler | BodyKind::EventHandler
            ));
        let registrations = handlers::error_registrations(found);
        let resumptions = handlers::resumptions(found);
        let mut statements = blocks::statements(found);
        statements.sort_by_key(|&(at, _)| at);
        statements.reverse();
        statements.dedup_by_key(|&mut (at, _)| at);
        statements.reverse();
        Ok(Facts {
            found,
            spaces: machine.layout().spaces.roles,
            bodies,
            contracts,
            procedures,
            event_stub,
            handlers,
            registrations,
            resumptions,
            statements,
        })
    }

    /// Whether `node` calls the event-poll adapter.
    pub fn event_poll(
        &self,
        node: &Node,
    ) -> bool {
        polls(node, self.event_stub)
    }

    /// The contract of the call at `at`.
    pub fn contract(
        &self,
        at: usize,
    ) -> Option<&Contract> {
        self.contracts.get(&(at as i64))
    }

    pub fn family(&self) -> module::Family {
        module::family(&self.found.records)
    }
}

fn polls(
    node: &Node,
    stub: Option<usize>,
) -> bool {
    let what = node.semantics();
    what.op == Operation::Call && !what.indirect && stub.is_some() && what.target.map(|one| one as usize) == stub
}

/// How a callee answers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Answer {
    None,
    /// Registers, in slot order: one is `i16`, AX and DX an `i32`, more a struct.
    Registers(Vec<Register>),
    /// The flags, as `cmp result, 0` leaves them.
    Flags,
}

/// A procedure's own interface: the bytes of arguments it pops, and what it answers.
#[derive(Clone, Debug)]
pub struct Interface {
    pub popped: i64,
    pub answer: Answer,
}

/// A procedure's interface: what its `retf n` pops and its CodeView
/// return type, None without a record.
fn interface(
    nodes: &IndexMap<i64, Arc<Node>>,
    procedure: Option<&cvinfo::Procedure>,
) -> Result<(i64, Option<Answer>), String> {
    let mut popped = None;
    for node in nodes.values() {
        let what = node.semantics();
        if what.op != Operation::Return {
            continue;
        }
        if what.name.as_deref() != Some("retf") {
            return Err("a near return".to_owned());
        }
        let bytes = what
            .sources
            .iter()
            .find_map(
                |one| match one {
                    Loc::Imm(imm) => Some(imm.value),
                    _ => None,
                },
            );
        let bytes = bytes.unwrap_or(0);
        if popped.is_some_and(|one| one != bytes) {
            return Err("its returns pop different byte counts".to_owned());
        }
        popped = Some(bytes);
    }
    let popped = popped.ok_or("no far return")?;
    if popped % 2 != 0 {
        return Err("pops an odd number of bytes".to_owned());
    }
    let Some(procedure) = procedure else { return Ok((popped, None)) };
    // A SUB's signature carries the type an INTEGER FUNCTION's does; such a
    // FUNCTION has a local of its own name. Any other type is a FUNCTION's.
    let named = match procedure.return_type() {
        Some(one) => Some(one.to_owned()),
        None => {
            let signed = procedure
                .signature()
                .and_then(|signature| cvinfo::type_name(signature.return_type, Some(&procedure.types)));
            let local = procedure.locals.iter().any(|local| local.name.eq_ignore_ascii_case(&procedure.name));
            signed.filter(|one| local || one != "INTEGER")
        }
    };
    let answer = match named.as_deref() {
        None => Answer::None,
        Some("INTEGER") => Answer::Registers(vec![Register::EAX]),
        Some("LONG") => Answer::Registers(vec![Register::EAX, Register::EDX]),
        // Stored through a hidden last argument, whose address AX answers.
        Some("SINGLE" | "DOUBLE") => Answer::Registers(vec![Register::EAX]),
        Some(other) => return Err(format!("a FUNCTION returning {other}")),
    };
    Ok((popped, Some(answer)))
}

/// `blocks` reachable from `seed`, the seed's first; a body whose seed is no
/// block's start has none.
/// The one error handler folded into the main body, whose frame it runs on;
/// a second is left a body of its own, which the raise refuses.
fn folded(mut bodies: Vec<BodyFacts>) -> Vec<BodyFacts> {
    let handlers: Vec<usize> = bodies
        .iter()
        .enumerate()
        .filter(|(_, one)| one.body.kind == BodyKind::ErrorHandler)
        .map(|(index, _)| index)
        .collect();
    let main = bodies.iter().position(|one| one.body.kind == BodyKind::Main);
    let (&[index], Some(main)) = (&handlers[..], main) else { return bodies };
    let handler = bodies.remove(index);
    let main = &mut bodies[if main > index { main - 1 } else { main }];
    main.handler =
        Some(Handler { seed: handler.body.seed, blocks: handler.blocks.iter().map(|block| block.at).collect() });
    main.blocks.extend(handler.blocks);
    main.nodes.extend(handler.nodes);
    main.pairs.extend(handler.pairs);
    bodies
}

fn reachable(
    blocks: Vec<Block>,
    seeds: &[usize],
) -> Vec<Block> {
    let by_at: BTreeMap<usize, &Block> = blocks.iter().map(|block| (block.at, block)).collect();
    let Some(&seed) = seeds.first().filter(|seed| by_at.contains_key(seed)) else {
        return Vec::new();
    };
    let mut seen: BTreeSet<usize> = seeds.iter().copied().filter(|one| by_at.contains_key(one)).collect();
    let mut pending: Vec<usize> = seen.iter().copied().collect();
    while let Some(at) = pending.pop() {
        for &next in &by_at[&at].succ {
            if by_at.contains_key(&next) && seen.insert(next) {
                pending.push(next);
            }
        }
    }
    let mut out: Vec<Block> = blocks.into_iter().filter(|block| seen.contains(&block.at)).collect();
    out.sort_by_key(|block| (block.at != seed, block.at));
    out
}

/// A body's MIR name: a procedure's own, `main`, or its kind and entry.
pub fn function_name(body: &Body) -> String {
    match (body.kind, &body.name) {
        (BodyKind::Main, _) => "main".to_owned(),
        (BodyKind::Procedure, Some(name)) => name.clone(),
        (kind, _) => format!("{}.{:04x}", kind.value(), body.seed),
    }
}

/// Whether a routine never comes back, by its contract.
pub fn never_returns(contract: &Contract) -> bool {
    contract.established && contract.control == Control::Never
}

/// The root a register is part of, when the raise tracks it.
pub fn tracked(register: Register) -> Option<usize> {
    let rooted = root(register);
    TRACKED.iter().position(|&one| one == rooted)
}
