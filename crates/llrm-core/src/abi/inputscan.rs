//! Port of `qbopt/abi/inputscan.py`: register inputs and preserved registers
//! discovered from linked OMF definitions. The Python module docstring is the
//! full account.
//!
//! A `frozenset` of entry lanes is `Lanes`, a bit per `root:byte`; nothing
//! iterates one into output.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;
use std::ops::{BitAnd, BitOr, BitOrAssign, Sub};
use std::rc::Rc;

use iced_x86::{
    Decoder, DecoderOptions, FlowControl, Instruction, InstructionInfoFactory, MemorySize,
    Mnemonic, OpKind, Register, UsedMemory, UsedRegister,
};

use crate::abi::runtime::{self, Reg};
use crate::frontends::bc::declen::{READS, WRITES, instruction_info_factory};
use crate::objectfile::omf::{self, Fixup, Record, ValueError};
use crate::support::hash::{HashSet, IndexMap, IndexSet};

pub type Address = (usize, i64, i64);

/// A library routine cannot be decoded into a conservative graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unrecognized(pub String);

impl fmt::Display for Unrecognized {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Unrecognized {}

/// `Unrecognized` subclasses `ValueError`.
impl From<Unrecognized> for ValueError {
    fn from(error: Unrecognized) -> ValueError {
        ValueError(error.0)
    }
}

const _ROOTS: [&str; 6] = ["ax", "bx", "cx", "dx", "si", "di"];

/// A frozenset of `root:byte` lanes, bit `2 * _ROOTS.index(root) + byte`.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
struct Lanes(u16);

impl Lanes {
    const EMPTY: Lanes = Lanes(0);

    fn is_empty(self) -> bool {
        self.0 == 0
    }

    fn has(self, root: &str, byte: usize) -> bool {
        let lane = _lane(root, byte);
        !lane.is_empty() && lane & self == lane
    }

    /// Each lane as `lane.split(":")`.
    fn iter(self) -> impl Iterator<Item = (&'static str, usize)> {
        (0..12)
            .filter(move |bit| self.0 >> bit & 1 != 0)
            .map(|bit| (_ROOTS[bit / 2], bit % 2))
    }
}

impl BitOr for Lanes {
    type Output = Lanes;
    fn bitor(self, other: Lanes) -> Lanes {
        Lanes(self.0 | other.0)
    }
}

impl BitOrAssign for Lanes {
    fn bitor_assign(&mut self, other: Lanes) {
        self.0 |= other.0;
    }
}

impl BitAnd for Lanes {
    type Output = Lanes;
    fn bitand(self, other: Lanes) -> Lanes {
        Lanes(self.0 & other.0)
    }
}

impl Sub for Lanes {
    type Output = Lanes;
    fn sub(self, other: Lanes) -> Lanes {
        Lanes(self.0 & !other.0)
    }
}

/// `{f"{root}:{byte}"}`; no lane names bp, so its set matches nothing.
fn _lane(root: &str, byte: usize) -> Lanes {
    match _ROOTS.iter().position(|one| *one == root) {
        Some(index) => Lanes(1 << (2 * index + byte)),
        None => Lanes::EMPTY,
    }
}

/// `_LANES[name]`.
fn _lanes(name: &str) -> Lanes {
    _lane(name, 0) | _lane(name, 1)
}

/// `_PARTS.get(register, ())`.
fn _parts(register: Register) -> Lanes {
    match register {
        Register::EAX | Register::AX => _lanes("ax"),
        Register::AL => _lane("ax", 0),
        Register::AH => _lane("ax", 1),
        Register::EBX | Register::BX => _lanes("bx"),
        Register::BL => _lane("bx", 0),
        Register::BH => _lane("bx", 1),
        Register::ECX | Register::CX => _lanes("cx"),
        Register::CL => _lane("cx", 0),
        Register::CH => _lane("cx", 1),
        Register::EDX | Register::DX => _lanes("dx"),
        Register::DL => _lane("dx", 0),
        Register::DH => _lane("dx", 1),
        Register::ESI | Register::SI => _lanes("si"),
        Register::EDI | Register::DI => _lanes("di"),
        _ => Lanes::EMPTY,
    }
}

const _ALL: Lanes = Lanes(0x0FFF);

/// `name.casefold()` over OMF's latin-1 names, where it differs from
/// `lower()` only at `ß` and `µ`.
pub fn _casefold(name: &str) -> String {
    name.to_lowercase()
        .replace('ß', "ss")
        .replace('µ', "\u{3BC}")
}

fn _symbol(name: &str) -> String {
    _casefold(name)
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Input {
    pub registers: BTreeSet<Reg>,
    pub evidence: String,
}

pub struct _Module {
    pub label: String,
    pub records: Vec<Rc<Record>>,
    pub definitions: IndexMap<String, (i64, i64)>,
    pub code_segments: BTreeSet<i64>,
    pub use32_segments: BTreeSet<i64>,
    pub code: IndexMap<i64, Vec<u8>>,
    pub covered: IndexMap<i64, HashSet<i64>>,
    pub fixups: Vec<Fixup>,
    pub externals: Vec<String>,
}

struct _Routine {
    address: Address,
    instructions: IndexMap<i64, Instruction>,
    successors: IndexMap<i64, Vec<i64>>,
    calls: IndexMap<i64, Option<Address>>,
}

/// The definitions visible to one LINK invocation.
pub struct Scanner {
    pub modules: Vec<_Module>,
    pub symbols: IndexMap<String, Vec<Address>>,
    pub names: IndexMap<Address, String>,
    _summarised: IndexMap<Address, _Summary>,
    _routines: IndexMap<Address, Rc<_Routine>>,
}

impl Scanner {
    pub fn new(members: &[(String, Vec<Rc<Record>>)]) -> Result<Scanner, ValueError> {
        let mut scanner = Scanner {
            modules: Vec::new(),
            symbols: IndexMap::default(),
            names: IndexMap::default(),
            _summarised: IndexMap::default(),
            _routines: IndexMap::default(),
        };
        for (module_index, (label, records)) in members.iter().enumerate() {
            let definitions = omf::public_definitions(records)?;
            let (code_segments, use32_segments) = _segment_kinds(records)?;
            let (code, covered) = _segments(records);
            let found = _Module {
                label: label.clone(),
                records: records.clone(),
                definitions,
                code_segments,
                use32_segments,
                code,
                covered,
                fixups: omf::fixups(records),
                externals: omf::externals(records),
            };
            for (name, &(segment, offset)) in &found.definitions {
                if !found.code_segments.contains(&segment) {
                    continue;
                }
                let address = (module_index, segment, offset);
                scanner
                    .symbols
                    .entry(_symbol(name))
                    .or_default()
                    .push(address);
                scanner.names.entry(address).or_insert_with(|| name.clone());
            }
            scanner.modules.push(found);
        }
        Ok(scanner)
    }

    /// Inputs of `name`, or every GP input when its graph is incomplete.
    pub fn inputs(
        &mut self,
        name: &str,
        chosen: Option<(&str, i64, i64)>,
    ) -> Result<Input, Unrecognized> {
        let root = match self._root(name, chosen) {
            Ok(root) => root,
            Err(reason) => {
                return Ok(Input {
                    registers: _registers(_ALL),
                    evidence: reason,
                });
            }
        };
        let (summary, count) = self._summary(root)?;
        Ok(Input {
            registers: _registers(summary.inputs),
            evidence: format!(
                "transitive OMF entry-value scan of {count} routine(s) rooted at {}:{}:{:#x}, following values \
                 through registers and stack slots; recursive and unresolved edges consume all GP inputs",
                self.modules[root.0].label, root.1, root.2
            ),
        })
    }

    /// Registers `name` returns as it found them on every path, or none.
    pub fn kept(
        &mut self,
        name: &str,
        chosen: Option<(&str, i64, i64)>,
    ) -> Result<Input, Unrecognized> {
        let root = match self._root(name, chosen) {
            Ok(root) => root,
            Err(reason) => {
                return Ok(Input {
                    registers: BTreeSet::new(),
                    evidence: reason,
                });
            }
        };
        let (summary, count) = self._summary(root)?;
        Ok(Input {
            registers: summary
                .kept
                .iter()
                .map(|one| Reg::from_value(one).unwrap())
                .collect(),
            evidence: format!(
                "transitive OMF save/restore scan of {count} routine(s): every return restores these; \
                 recursive, unresolved and unmodelled stack edges preserve nothing"
            ),
        })
    }

    fn _summary(&mut self, root: Address) -> Result<(_Summary, usize), Unrecognized> {
        let graph = self._graph(root)?;
        for address in _cycles(&graph) {
            self._summarised
                .entry(address)
                .or_insert_with(|| _UNKNOWN.clone());
        }
        let mut pending: IndexMap<Address, Rc<_Routine>> = graph
            .iter()
            .filter(|(address, _routine)| !self._summarised.contains_key(*address))
            .map(|(address, routine)| (*address, routine.clone()))
            .collect();
        while !pending.is_empty() {
            let ready: Vec<Address> = pending
                .iter()
                .filter(|(_address, routine)| {
                    routine.calls.values().all(|target| match target {
                        None => true,
                        Some(target) => {
                            self._summarised.contains_key(target) || !graph.contains_key(target)
                        }
                    })
                })
                .map(|(address, _routine)| *address)
                .collect();
            if ready.is_empty() {
                return Err(Unrecognized(
                    "call graph remains cyclic after recursive components were isolated".to_owned(),
                ));
            }
            for address in ready {
                let routine = pending.shift_remove(&address).unwrap();
                let summary = self._summarise(&routine)?;
                self._summarised.insert(address, summary);
            }
        }
        Ok((self._summarised[&root].clone(), graph.len()))
    }

    fn _summarise(&self, routine: &_Routine) -> Result<_Summary, Unrecognized> {
        let mut found = match _flow(routine, &self._summarised) {
            Ok(found) => found,
            Err(_Opaque(_)) => _Summary::new(_inputs(routine, &self._summarised)?),
        };
        let name = self.names.get(&routine.address);
        if let Some(name) = name {
            if let Some(documented) = runtime::ERROR_FUNNEL_INPUTS.get(name.to_uppercase().as_str())
            {
                found = _Summary::new(documented.iter().fold(Lanes::EMPTY, |lanes, register| {
                    lanes | _lanes(register.value())
                }));
            }
            if runtime::never_returns(name) {
                // A path into it ends. The error machinery walks frames and
                // return addresses, never a caller's saved register.
                return Ok(_Summary {
                    cleanup: Some(0),
                    reach: Some(found.reach.unwrap_or(0)),
                    never: true,
                    ..(_Summary::new(found.inputs))
                });
            }
        }
        Ok(found)
    }

    /// The one matching definition, or why there is none.
    fn _root(&self, name: &str, chosen: Option<(&str, i64, i64)>) -> Result<Address, String> {
        let mut roots: Vec<Address> = self
            .symbols
            .get(&_symbol(name))
            .cloned()
            .unwrap_or_default();
        if let Some((label, segment, offset)) = chosen {
            roots.retain(|address| {
                self.modules[address.0].label == label
                    && (address.1, address.2) == (segment, offset)
            });
        }
        if roots.len() == 1 {
            Ok(roots[0])
        } else {
            Err(format!(
                "definition scan found {} matching entries",
                roots.len()
            ))
        }
    }

    fn _definition(&self, name: &str) -> Option<Address> {
        let found = self.symbols.get(&_symbol(name))?;
        if found.len() == 1 {
            Some(found[0])
        } else {
            None
        }
    }

    fn _target(&self, address: Address, insn: &Instruction) -> Option<Address> {
        let (module_index, segment, _offset) = address;
        let found = &self.modules[module_index];
        if matches!(
            insn.flow_control(),
            FlowControl::IndirectCall | FlowControl::IndirectBranch
        ) {
            // A relocation in an indirect transfer identifies the pointer
            // cell, not the code address held in that cell.
            return None;
        }
        let (ip, next_ip) = (insn.ip() as i64, insn.next_ip() as i64);
        let fixes: Vec<&Fixup> = found
            .fixups
            .iter()
            .filter(|fix| fix.seg == Some(segment) && ip <= fix.offset && fix.offset < next_ip)
            .collect();
        if fixes.is_empty() {
            if insn.op0_kind() == OpKind::NearBranch16 {
                return Some((module_index, segment, insn.near_branch_target() as i64));
            }
            return None;
        }
        if fixes.len() != 1 {
            return None;
        }
        let fix = fixes[0];
        let code = &found.code[&segment];
        let mut decoder = Decoder::with_ip(16, _from(code, ip), ip as u64, DecoderOptions::NONE);
        let decoded = decoder.decode();
        let constants = decoder.get_constant_offsets(&decoded);
        let width = if fix.loc == 3 { 4 } else { 2 };
        if fix.offset != ip + constants.immediate_offset() as i64
            || constants.immediate_size() != 2
            || !(fix.loc == 1 || fix.loc == 3)
            || _slice(code, fix.offset, fix.offset + width)
                .iter()
                .any(|byte| *byte != 0)
        {
            return None;
        }
        if fix.target == "segment" {
            if found.code_segments.contains(&fix.index) {
                return Some((module_index, fix.index, fix.disp));
            }
            return None;
        }
        if fix.target == "external" && 0 < fix.index && (fix.index as usize) < found.externals.len()
        {
            let definition = self._definition(&found.externals[fix.index as usize]);
            if let Some((module, target_segment, target_offset)) = definition {
                return Some((module, target_segment, target_offset + fix.disp));
            }
        }
        None
    }

    fn _decode(&self, address: Address) -> Result<_Routine, Unrecognized> {
        let (module_index, segment, start) = address;
        let found = &self.modules[module_index];
        if !found.code_segments.contains(&segment) {
            return Err(Unrecognized(format!(
                "{}:{segment}:{start:#x}: control target is not code",
                found.label
            )));
        }
        if found.use32_segments.contains(&segment) {
            return Err(Unrecognized(format!(
                "{}:{segment}:{start:#x}: USE32 code is not supported",
                found.label
            )));
        }
        let code: &[u8] = found.code.get(&segment).map_or(&[], |code| code.as_slice());
        let empty = HashSet::default();
        let covered = found.covered.get(&segment).unwrap_or(&empty);
        let mut instructions: IndexMap<i64, Instruction> = IndexMap::default();
        let mut successors: IndexMap<i64, Vec<i64>> = IndexMap::default();
        let mut calls: IndexMap<i64, Option<Address>> = IndexMap::default();
        let mut pending = vec![start];
        while let Some(at) = pending.pop() {
            if instructions.contains_key(&at) {
                continue;
            }
            if instructions.len() >= 4000 {
                return Err(Unrecognized(format!(
                    "{}:{segment}:{start:#x}: instruction budget exceeded",
                    found.label
                )));
            }
            let insn = _decoded(code, at);
            if insn.is_invalid() || !(at..insn.next_ip() as i64).all(|byte| covered.contains(&byte))
            {
                return Err(Unrecognized(format!(
                    "{}:{segment}:{at:#x}: invalid or missing code",
                    found.label
                )));
            }
            // A branch may deliberately enter bytes which another path
            // decodes as an immediate. VBDOS lmove.asm does this at 0254/0255
            // (`cmp ax,imm16` versus `mov bx,si`). x86 defines both paths;
            // keying decoded instructions by their entry address represents
            // them without choosing one linear disassembly as authoritative.
            instructions.insert(at, insn);
            let next_ip = insn.next_ip() as i64;
            let mut next_at: Vec<i64> = Vec::new();
            match insn.flow_control() {
                FlowControl::Next => next_at.push(next_ip),
                FlowControl::Return => {}
                FlowControl::Call | FlowControl::IndirectCall => {
                    calls.insert(at, self._target(address, &insn));
                    next_at.push(next_ip);
                }
                flow @ (FlowControl::ConditionalBranch | FlowControl::UnconditionalBranch) => {
                    match self._target(address, &insn) {
                        None => {
                            calls.insert(at, None);
                        }
                        Some(target)
                            if (target.0, target.1) != (address.0, address.1)
                                || self.names.contains_key(&target) && target != address =>
                        {
                            calls.insert(at, Some(target));
                        }
                        Some(target) => next_at.push(target.2),
                    }
                    if flow == FlowControl::ConditionalBranch {
                        next_at.push(next_ip);
                    }
                }
                FlowControl::IndirectBranch => {
                    calls.insert(at, None);
                }
                FlowControl::Interrupt => {
                    calls.insert(at, None);
                    next_at.push(next_ip);
                }
                _ => {
                    return Err(Unrecognized(format!(
                        "{}:{segment}:{at:#x}: unrecognized control transfer",
                        found.label
                    )));
                }
            }
            let unique: IndexSet<i64> = next_at.into_iter().collect();
            let unique: Vec<i64> = unique.into_iter().collect();
            pending.extend(unique.iter().copied());
            successors.insert(at, unique);
        }
        Ok(_Routine {
            address,
            instructions,
            successors,
            calls,
        })
    }

    fn _graph(&mut self, root: Address) -> Result<IndexMap<Address, Rc<_Routine>>, Unrecognized> {
        let mut graph: IndexMap<Address, Rc<_Routine>> = IndexMap::default();
        let mut pending = vec![root];
        while let Some(address) = pending.pop() {
            if graph.contains_key(&address) {
                continue;
            }
            if graph.len() >= 2048 {
                return Err(Unrecognized("function budget exceeded".to_owned()));
            }
            if !self._routines.contains_key(&address) {
                let routine = self._decode(address)?;
                self._routines.insert(address, Rc::new(routine));
            }
            let routine = self._routines[&address].clone();
            graph.insert(address, routine.clone());
            pending.extend(
                routine
                    .calls
                    .values()
                    .filter_map(|target| *target)
                    .filter(|target| !graph.contains_key(target)),
            );
        }
        Ok(graph)
    }
}

/// `Decoder(16, code[at:], ip=at).decode()`.
fn _decoded(code: &[u8], at: i64) -> Instruction {
    Decoder::with_ip(16, _from(code, at), at as u64, DecoderOptions::NONE).decode()
}

/// `code[at:]`.
fn _from(code: &[u8], at: i64) -> &[u8] {
    code.get(at as usize..).unwrap_or(&[])
}

/// `code[low:high]`.
fn _slice(code: &[u8], low: i64, high: i64) -> &[u8] {
    let high = (high as usize).min(code.len());
    code.get(low as usize..high).unwrap_or(&[])
}

/// CODE-class and USE32 segment indices declared by SEGDEF records.
fn _segment_kinds(records: &[Rc<Record>]) -> Result<(BTreeSet<i64>, BTreeSet<i64>), Unrecognized> {
    let names = omf::names(records);
    let mut code = BTreeSet::new();
    let mut use32 = BTreeSet::new();
    let mut segment = 0;
    for record in records {
        if record.r#type & 0xFE != omf::SEGDEF {
            continue;
        }
        segment += 1;
        if record.body.is_empty() {
            return Err(Unrecognized(format!("segment {segment}: empty SEGDEF")));
        }
        let attribute = record.body[0];
        if attribute & 1 != 0 {
            use32.insert(segment);
        }
        let at = 1
            + (if record.r#type & 1 != 0 { 4 } else { 2 })
            + (if attribute >> 5 == 0 { 3 } else { 0 });
        let Some(kind) = omf::_index_checked(&record.body, at)
            .and_then(|(_, at)| omf::_index_checked(&record.body, at))
            .map(|(kind, _)| kind)
        else {
            return Err(Unrecognized(format!("segment {segment}: malformed SEGDEF")));
        };
        if kind as usize >= names.len() {
            return Err(Unrecognized(format!(
                "segment {segment}: SEGDEF names missing class index {kind}"
            )));
        }
        if _casefold(&names[kind as usize]) == "code" {
            code.insert(segment);
        }
    }
    Ok((code, use32))
}

#[allow(clippy::type_complexity)]
fn _segments(records: &[Rc<Record>]) -> (IndexMap<i64, Vec<u8>>, IndexMap<i64, HashSet<i64>>) {
    let mut pieces: IndexMap<i64, Vec<(i64, Vec<u8>)>> = IndexMap::default();
    for (_record, segment, offset, payload) in omf::ledata(records) {
        pieces.entry(segment).or_default().push((offset, payload));
    }
    let mut code = IndexMap::default();
    let mut covered = IndexMap::default();
    for (segment, chunks) in pieces {
        let end = chunks
            .iter()
            .map(|(offset, payload)| offset + payload.len() as i64)
            .max()
            .unwrap();
        let mut image = vec![0u8; end as usize];
        let mut seen = HashSet::default();
        // OMF permits a later LEDATA record to backpatch bytes emitted by an
        // earlier one. Apply file order exactly as LINK does; overlap is not
        // an ambiguity and the covered-byte union remains the same.
        for (offset, payload) in &chunks {
            let offset = *offset as usize;
            image[offset..offset + payload.len()].copy_from_slice(payload);
            seen.extend(offset as i64..(offset + payload.len()) as i64);
        }
        code.insert(segment, image);
        covered.insert(segment, seen);
    }
    (code, covered)
}

thread_local! {
    static INFO: RefCell<InstructionInfoFactory> = RefCell::new(instruction_info_factory());
}

/// `INFO.info(insn).used_registers()`.
fn _used_registers(insn: &Instruction) -> Vec<UsedRegister> {
    INFO.with(|info| info.borrow_mut().info(insn).used_registers().to_vec())
}

/// `INFO.info(insn).used_memory()`.
fn _used_memory(insn: &Instruction) -> Vec<UsedMemory> {
    INFO.with(|info| info.borrow_mut().info(insn).used_memory().to_vec())
}

/// Entry lanes read on any path before that lane is overwritten.
fn _inputs(
    routine: &_Routine,
    summaries: &IndexMap<Address, _Summary>,
) -> Result<Lanes, Unrecognized> {
    let start = routine.address.2;
    let mut incoming: IndexMap<i64, Lanes> = IndexMap::from_iter([(start, _ALL)]);
    let mut required = Lanes::EMPTY;
    let mut pending = vec![start];
    let mut visits = 0;
    while !pending.is_empty() {
        visits += 1;
        if visits > routine.instructions.len().max(1) * 100 {
            return Err(Unrecognized(format!(
                "{start:#x}: entry-value dataflow did not converge"
            )));
        }
        let at = pending.pop().unwrap();
        let mut live = incoming[&at];
        let insn = &routine.instructions[&at];
        let breaking = matches!(insn.mnemonic(), Mnemonic::Xor | Mnemonic::Sub)
            && insn.op_count() == 2
            && insn.op0_kind() == insn.op1_kind()
            && insn.op1_kind() == OpKind::Register
            && insn.op0_register() == insn.op1_register();
        for used in _used_registers(insn) {
            let parts = _parts(used.register());
            let broken = breaking && !parts.is_empty();
            if READS.contains(&used.access()) && !broken {
                required |= live & parts;
            }
            if WRITES.contains(&used.access()) {
                live = live - parts;
            }
        }
        if let Some(target) = routine.calls.get(&at) {
            let wanted = match target.and_then(|target| summaries.get(&target)) {
                Some(summary) => summary.inputs,
                None => _ALL,
            };
            required |= live & wanted;
            // Preservation is intentionally not inferred. Leaving lanes live
            // can only add later entry requirements; clearing them here would
            // silently assume the callee clobbered a value it may preserve.
        }
        for successor in routine.successors.get(&at).into_iter().flatten() {
            let merged = incoming.get(successor).copied().unwrap_or_default() | live;
            if Some(merged) != incoming.get(successor).copied() {
                incoming.insert(*successor, merged);
                pending.push(*successor);
            }
        }
    }
    Ok(required)
}

/// What a routine does with its caller's registers and stack.
///
/// `inputs` are entry lanes read, or moved anywhere but back into their own
/// place, on some path. `kept` are the roots every return restores.
/// `cleanup` is the argument bytes every return pops, `returns` the words
/// of return address it pops, and `reach` the words above its entry stack
/// pointer it reads or writes, return address included; None when unknown.
/// `passes` are the entry lanes that may still be in place at a return.
#[derive(Clone, Debug, Eq, PartialEq)]
struct _Summary {
    inputs: Lanes,
    passes: Lanes,
    kept: BTreeSet<&'static str>,
    cleanup: Option<i64>,
    returns: Option<i64>,
    reach: Option<i64>,
    never: bool,
}

impl _Summary {
    /// `_Summary(inputs)`, every other field at its default.
    fn new(inputs: Lanes) -> _Summary {
        _Summary {
            inputs,
            ..(_UNKNOWN.clone())
        }
    }
}

// Recursive components and unresolved targets.
static _UNKNOWN: _Summary = _Summary {
    inputs: _ALL,
    passes: _ALL,
    kept: BTreeSet::new(),
    cleanup: None,
    returns: None,
    reach: None,
    never: false,
};
const _TRACKED: [&str; 7] = ["ax", "bx", "cx", "dx", "si", "di", "bp"];

/// `_WORDS.get(register)`.
fn _words(register: Register) -> Option<&'static str> {
    match register {
        Register::AX => Some("ax"),
        Register::BX => Some("bx"),
        Register::CX => Some("cx"),
        Register::DX => Some("dx"),
        Register::SI => Some("si"),
        Register::DI => Some("di"),
        Register::BP => Some("bp"),
        _ => None,
    }
}

/// `_DWORDS.get(register)`.
fn _dwords(register: Register) -> Option<&'static str> {
    match register {
        Register::EAX => Some("ax"),
        Register::EBX => Some("bx"),
        Register::ECX => Some("cx"),
        Register::EDX => Some("dx"),
        Register::ESI => Some("si"),
        Register::EDI => Some("di"),
        Register::EBP => Some("bp"),
        _ => None,
    }
}

/// `_PLACES.get(register)`: its root and the bytes of it the register names.
fn _places(register: Register) -> Option<(&'static str, &'static [usize])> {
    if matches!(register, Register::BP | Register::EBP) {
        return Some(("bp", &[0, 1]));
    }
    let parts = _parts(register);
    let (root, _byte) = parts.iter().next()?;
    let bytes: &'static [usize] = match (parts.has(root, 0), parts.has(root, 1)) {
        (true, true) => &[0, 1],
        (true, false) => &[0],
        _ => &[1],
    };
    Some((root, bytes))
}

const _STACK: [Register; 2] = [Register::SP, Register::ESP];
const _BASES: [Register; 2] = [Register::BP, Register::EBP];

/// The stack or a register moves in a way the model does not follow.
#[derive(Debug)]
struct _Opaque(#[allow(dead_code)] String);

/// `str(insn)` in an `_Opaque` message, which nothing reads.
fn _text(insn: &Instruction) -> String {
    format!("{:?}@{:#x}", insn.code(), insn.ip())
}

/// What `_Value.token` holds: a root's name or `_FRAME`, or `("high", token)`.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Name(&'static str),
    High(Option<Rc<Token>>),
}

const _FRAME: Token = Token::Name("frame");

#[derive(Clone, Debug, Eq, PartialEq)]
struct _Value {
    // What the value certainly is -- a root's entry value, the frame -- or
    // None; and per byte, the entry lanes it may still hold.
    token: Option<Token>,
    lanes: [Lanes; 2],
}

impl _Value {
    fn meet(&self, other: &_Value) -> _Value {
        _Value {
            token: if self.token == other.token {
                self.token.clone()
            } else {
                None
            },
            lanes: [
                self.lanes[0] | other.lanes[0],
                self.lanes[1] | other.lanes[1],
            ],
        }
    }

    fn forget(&self) -> _Value {
        _Value {
            token: None,
            lanes: self.lanes,
        }
    }

    fn lanes_in(&self, places: &[usize]) -> Lanes {
        places
            .iter()
            .fold(Lanes::EMPTY, |lanes, place| lanes | self.lanes[*place])
    }
}

const _JUNK: _Value = _Value {
    token: None,
    lanes: [Lanes::EMPTY, Lanes::EMPTY],
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct _State {
    // Tracked registers in _TRACKED order; word slots pushed since entry;
    // `frame` is the stack depth when bp was set from sp.
    registers: Vec<_Value>,
    stack: Vec<_Value>,
    frame: Option<usize>,
}

impl _State {
    fn get(&self, root: &str) -> &_Value {
        &self.registers[_TRACKED.iter().position(|one| *one == root).unwrap()]
    }

    fn set(&self, root: &str, value: _Value) -> _State {
        let mut values = self.registers.clone();
        let frame = if root != "bp" || value.token == Some(_FRAME) {
            self.frame
        } else {
            None
        };
        values[_TRACKED.iter().position(|one| *one == root).unwrap()] = value;
        // bp holds the frame only while its token says so.
        _State {
            registers: values,
            stack: self.stack.clone(),
            frame,
        }
    }

    fn framed(&self) -> bool {
        self.frame.is_some() && self.get("bp").token == Some(_FRAME)
    }

    fn meet(&self, other: &_State) -> Result<_State, _Opaque> {
        if self.stack.len() != other.stack.len() {
            return Err(_Opaque("stack depth differs where paths join".to_owned()));
        }
        Ok(_State {
            registers: self
                .registers
                .iter()
                .zip(&other.registers)
                .map(|(one, two)| one.meet(two))
                .collect(),
            stack: self
                .stack
                .iter()
                .zip(&other.stack)
                .map(|(one, two)| one.meet(two))
                .collect(),
            frame: if self.frame == other.frame {
                self.frame
            } else {
                None
            },
        })
    }
}

/// `stack[:count]`.
fn _head(stack: &[_Value], count: usize) -> Vec<_Value> {
    stack[..count.min(stack.len())].to_vec()
}

/// `(*stack, *values)`.
fn _pushed(stack: &[_Value], values: impl IntoIterator<Item = _Value>) -> Vec<_Value> {
    stack.iter().cloned().chain(values).collect()
}

/// `(_JUNK,) * count`.
fn _junk(count: i64) -> impl Iterator<Item = _Value> {
    std::iter::repeat_n(_JUNK, count.max(0) as usize)
}

/// Entry lanes read, and words above entry reached, over one routine.
struct _Walk {
    inputs: Lanes,
    reach: Option<i64>,
}

impl _Walk {
    fn new() -> _Walk {
        _Walk {
            inputs: Lanes::EMPTY,
            reach: Some(0),
        }
    }

    fn read(&mut self, value: &_Value, places: &[usize]) {
        self.inputs |= value.lanes_in(places);
    }

    fn above(&mut self, words: Option<i64>) {
        self.reach = match (words, self.reach) {
            (Some(words), Some(reach)) => Some(reach.max(words)),
            _ => None,
        };
    }

    /// Stack indices a bp-framed operand covers; negative is above entry.
    fn slots(
        &self,
        state: &_State,
        base: Register,
        index: Register,
        displacement: u64,
        size: usize,
    ) -> Result<Option<Vec<i64>>, _Opaque> {
        if !_BASES.contains(&base) || !state.framed() {
            return Ok(None);
        }
        if index != Register::None {
            return Err(_Opaque("indexed frame access".to_owned()));
        }
        let mut displacement = (displacement & 0xFFFF) as i64;
        displacement -= if displacement & 0x8000 != 0 {
            0x10000
        } else {
            0
        };
        let size = size.max(1) as i64;
        let frame = state.frame.unwrap() as i64;
        let found: Vec<i64> = (displacement.div_euclid(2)
            ..(displacement + size - 1).div_euclid(2) + 1)
            .map(|word| frame - 1 - word)
            .collect();
        if found.iter().any(|index| *index >= state.stack.len() as i64) {
            return Err(_Opaque("frame access below sp".to_owned()));
        }
        Ok(Some(found))
    }

    /// Frame slots `insn` reads or writes through a bp-framed operand.
    fn memory(&mut self, state: &_State, insn: &Instruction) -> Result<_State, _Opaque> {
        let mut stack = state.stack.clone();
        for memory in _used_memory(insn) {
            if _STACK.contains(&memory.base()) {
                // Push, pop, call and return are modelled by their callers.
                if !matches!(insn.mnemonic(), Mnemonic::Push | Mnemonic::Pop) {
                    return Err(_Opaque(format!(
                        "{}: addresses the stack through sp",
                        _text(insn)
                    )));
                }
                continue;
            }
            let Some(found) = self.slots(
                state,
                memory.base(),
                memory.index(),
                memory.displacement(),
                memory.memory_size().size(),
            )?
            else {
                continue;
            };
            for index in found {
                if index < 0 {
                    self.above(Some(-index));
                    continue;
                }
                if READS.contains(&memory.access()) {
                    self.read(&stack[index as usize], &[0, 1]);
                }
                if WRITES.contains(&memory.access()) {
                    stack[index as usize] = _JUNK;
                }
            }
        }
        Ok(_State {
            registers: state.registers.clone(),
            stack,
            frame: state.frame,
        })
    }

    /// A transfer into `summary`, having pushed `pushed` return words.
    fn callee(&mut self, state: &_State, summary: &_Summary, pushed: i64) -> _State {
        for (root, byte) in summary.inputs.iter() {
            self.read(state.get(root), &[byte]);
        }
        // No lane names bp; a callee may still read it.
        self.read(state.get("bp"), &[0, 1]);
        let mut stack = state.stack.clone();
        let Some(reach) = summary.reach else {
            for value in &stack {
                self.read(value, &[0, 1]);
            }
            self.above(None);
            return _State {
                registers: state.registers.clone(),
                stack: stack.iter().map(_Value::forget).collect(),
                frame: state.frame,
            };
        };
        for word in 0..reach - pushed {
            let index = stack.len() as i64 - 1 - word;
            if index < 0 {
                self.above(Some(-index));
            } else {
                self.read(&stack[index as usize], &[0, 1]);
                stack[index as usize] = stack[index as usize].forget();
            }
        }
        _State {
            registers: state.registers.clone(),
            stack,
            frame: state.frame,
        }
    }
}

fn _whole(insn: &Instruction, operand: u32) -> Register {
    if insn.op_count() > operand && insn.op_kind(operand) == OpKind::Register {
        return insn.op_register(operand);
    }
    Register::None
}

/// One non-transfer instruction's effect on the registers and stack.
fn _step(state: &_State, insn: &Instruction, walk: &mut _Walk) -> Result<_State, _Opaque> {
    let (mnemonic, register, source) = (insn.mnemonic(), _whole(insn, 0), _whole(insn, 1));
    if state.framed()
        && (_BASES.contains(&source) && !(mnemonic == Mnemonic::Mov && register == Register::SP)
            || _BASES.contains(&register)
                && !matches!(mnemonic, Mnemonic::Push | Mnemonic::Pop | Mnemonic::Mov))
    {
        return Err(_Opaque(format!(
            "{}: the frame's address escapes",
            _text(insn)
        )));
    }
    if mnemonic == Mnemonic::Lea {
        if let Some(found) = walk.slots(
            state,
            insn.memory_base(),
            insn.memory_index(),
            insn.memory_displacement64(),
            2,
        )? {
            // A pointer to the caller's arguments reaches an unknown extent of
            // them; one into this routine's own slots is not followed.
            if *found.iter().max().unwrap() >= 0 {
                return Err(_Opaque(format!(
                    "{}: the frame's address escapes",
                    _text(insn)
                )));
            }
            walk.above(None);
        }
    }
    let stack = &state.stack;
    match mnemonic {
        Mnemonic::Push => {
            if let Some(root) = _dwords(register) {
                let value = state.get(root).clone();
                let high = _Value {
                    token: Some(Token::High(value.token.clone().map(Rc::new))),
                    lanes: _JUNK.lanes,
                };
                return Ok(_State {
                    registers: state.registers.clone(),
                    stack: _pushed(stack, [high, value]),
                    frame: state.frame,
                });
            }
            if let Some(root) = _words(register) {
                return Ok(_State {
                    registers: state.registers.clone(),
                    stack: _pushed(stack, [state.get(root).clone()]),
                    frame: state.frame,
                });
            }
            let state = walk.memory(state, insn)?;
            let words = (-(insn.stack_pointer_increment() as i64)).div_euclid(2);
            return Ok(_State {
                stack: _pushed(&state.stack, _junk(words)),
                ..state
            });
        }
        Mnemonic::Pop => {
            let words = (insn.stack_pointer_increment() as i64).div_euclid(2) as usize;
            if stack.len() < words {
                return Err(_Opaque("pop below entry".to_owned()));
            }
            let (rest, popped) = stack.split_at(stack.len() - words);
            let after = _State {
                registers: state.registers.clone(),
                stack: rest.to_vec(),
                frame: state.frame,
            };
            if let Some(root) = _dwords(register) {
                let [high, low] = popped else {
                    panic!("not enough values to unpack")
                };
                let whole = high.token == Some(Token::High(low.token.clone().map(Rc::new)));
                return Ok(after.set(root, if whole { low.clone() } else { low.forget() }));
            }
            if let Some(root) = _words(register) {
                return Ok(after.set(root, popped.last().unwrap().clone()));
            }
            if _STACK.contains(&register) {
                return Err(_Opaque("pop sp".to_owned()));
            }
            if insn.op0_kind() != OpKind::Register {
                if walk
                    .slots(
                        state,
                        insn.memory_base(),
                        insn.memory_index(),
                        insn.memory_displacement64(),
                        2,
                    )?
                    .is_some()
                {
                    return Err(_Opaque(format!("{}: pops into the frame", _text(insn))));
                }
                for base in [insn.memory_base(), insn.memory_index()] {
                    if let Some((root, places)) = _places(base) {
                        walk.read(state.get(root), places);
                    }
                }
            }
            // Into memory or a segment register: the value is used.
            for value in popped {
                walk.read(value, &[0, 1]);
            }
            return Ok(after);
        }
        Mnemonic::Pusha | Mnemonic::Pushad => {
            let slots: Vec<_Value> = ["ax", "cx", "dx", "bx"]
                .iter()
                .map(|root| state.get(root).clone())
                .chain([_JUNK])
                .chain(
                    ["bp", "si", "di"]
                        .iter()
                        .map(|root| state.get(root).clone()),
                )
                .collect();
            let width = if mnemonic == Mnemonic::Pushad { 2 } else { 1 };
            return Ok(_State {
                registers: state.registers.clone(),
                stack: _pushed(
                    stack,
                    slots
                        .into_iter()
                        .flat_map(|one| _junk(width - 1).chain([one])),
                ),
                frame: state.frame,
            });
        }
        Mnemonic::Popa | Mnemonic::Popad => {
            let width = if mnemonic == Mnemonic::Popad { 2 } else { 1 };
            if stack.len() < 8 * width {
                return Err(_Opaque("popa below entry".to_owned()));
            }
            let words: Vec<_Value> = stack[stack.len() - 8 * width..]
                .iter()
                .skip(width - 1)
                .step_by(width)
                .cloned()
                .collect();
            let mut after = _State {
                registers: state.registers.clone(),
                stack: stack[..stack.len() - 8 * width].to_vec(),
                frame: state.frame,
            };
            for (root, value) in [
                Some("di"),
                Some("si"),
                Some("bp"),
                None,
                Some("bx"),
                Some("dx"),
                Some("cx"),
                Some("ax"),
            ]
            .into_iter()
            .zip(words)
            {
                if let Some(root) = root {
                    after = after.set(root, value);
                }
            }
            return Ok(after);
        }
        Mnemonic::Pushf | Mnemonic::Pushfd => {
            return Ok(_State {
                registers: state.registers.clone(),
                stack: _pushed(
                    stack,
                    _junk(if mnemonic == Mnemonic::Pushfd { 2 } else { 1 }),
                ),
                frame: state.frame,
            });
        }
        Mnemonic::Popf | Mnemonic::Popfd => {
            let count = if mnemonic == Mnemonic::Popfd { 2 } else { 1 };
            if stack.len() < count {
                return Err(_Opaque("popf below entry".to_owned()));
            }
            for value in &stack[stack.len() - count..] {
                walk.read(value, &[0, 1]);
            }
            return Ok(_State {
                registers: state.registers.clone(),
                stack: stack[..stack.len() - count].to_vec(),
                frame: state.frame,
            });
        }
        Mnemonic::Leave => {
            if !state.framed() {
                return Err(_Opaque("leave without a known frame".to_owned()));
            }
            let unwound = _State {
                registers: state.registers.clone(),
                stack: _head(stack, state.frame.unwrap()),
                frame: None,
            };
            return _step(&unwound, &_decoded(&[0x5D], 0), walk);
        }
        Mnemonic::Enter if insn.immediate8_2nd() == 0 => {
            let pushed = _step(state, &_decoded(&[0x55], 0), walk)?;
            let framed = _State {
                registers: pushed
                    .set(
                        "bp",
                        _Value {
                            token: Some(_FRAME),
                            lanes: _JUNK.lanes,
                        },
                    )
                    .registers,
                frame: Some(pushed.stack.len()),
                stack: pushed.stack,
            };
            return Ok(_State {
                registers: framed.registers,
                stack: _pushed(&framed.stack, _junk(insn.immediate16() as i64 / 2)),
                frame: framed.frame,
            });
        }
        Mnemonic::Mov if register == Register::BP && source == Register::SP => {
            return Ok(_State {
                registers: state
                    .set(
                        "bp",
                        _Value {
                            token: Some(_FRAME),
                            lanes: _JUNK.lanes,
                        },
                    )
                    .registers,
                stack: stack.clone(),
                frame: Some(stack.len()),
            });
        }
        Mnemonic::Mov if register == Register::SP && source == Register::BP => {
            if !state.framed() {
                return Err(_Opaque("sp restored from an unknown frame".to_owned()));
            }
            return Ok(_State {
                registers: state.registers.clone(),
                stack: _head(stack, state.frame.unwrap()),
                frame: state.frame,
            });
        }
        Mnemonic::Add | Mnemonic::Sub
            if register == Register::SP
                && matches!(
                    insn.op1_kind(),
                    OpKind::Immediate8to16 | OpKind::Immediate16
                ) =>
        {
            let mut amount = (insn.immediate(1) & 0xFFFF) as i64;
            amount = if amount & 0x8000 != 0 {
                amount - 0x10000
            } else {
                amount
            };
            let moved = if mnemonic == Mnemonic::Sub {
                amount
            } else {
                -amount
            };
            let (words, odd) = (moved.div_euclid(2), moved.rem_euclid(2));
            if odd != 0 {
                return Err(_Opaque("sp moved by an odd amount".to_owned()));
            }
            if words >= 0 {
                return Ok(_State {
                    registers: state.registers.clone(),
                    stack: _pushed(stack, _junk(words)),
                    frame: state.frame,
                });
            }
            if (stack.len() as i64) < -words {
                return Err(_Opaque("sp released below entry".to_owned()));
            }
            return Ok(_State {
                registers: state.registers.clone(),
                stack: stack[..(stack.len() as i64 + words) as usize].to_vec(),
                frame: state.frame,
            });
        }
        Mnemonic::Mov if _words(register).is_some() && _words(source).is_some() => {
            return Ok(state.set(
                _words(register).unwrap(),
                state.get(_words(source).unwrap()).clone(),
            ));
        }
        Mnemonic::Xchg if _words(register).is_some() && _words(source).is_some() => {
            let (one, two) = (_words(register).unwrap(), _words(source).unwrap());
            return Ok(state
                .set(one, state.get(two).clone())
                .set(two, state.get(one).clone()));
        }
        Mnemonic::Mov
            if (_words(register).is_some() || _words(source).is_some())
                && insn.memory_size() == MemorySize::UInt16 =>
        {
            // A word moved between a register and its own frame slot.
            let found = walk.slots(
                state,
                insn.memory_base(),
                insn.memory_index(),
                insn.memory_displacement64(),
                2,
            )?;
            if let Some(found) = found {
                if found.len() == 1 && found[0] >= 0 {
                    let index = found[0] as usize;
                    if let Some(root) = _words(register) {
                        return Ok(state.set(root, stack[index].clone()));
                    }
                    let mut stack = stack.clone();
                    stack[index] = state.get(_words(source).unwrap()).clone();
                    return Ok(_State {
                        registers: state.registers.clone(),
                        stack,
                        frame: state.frame,
                    });
                }
            }
        }
        _ => {}
    }
    let breaking = matches!(mnemonic, Mnemonic::Xor | Mnemonic::Sub)
        && insn.op_count() == 2
        && register != Register::None
        && register == source;
    let used = _used_registers(insn);
    for one in &used {
        if _STACK.contains(&one.register()) {
            return Err(_Opaque(format!("{}: uses sp", _text(insn))));
        }
        if READS.contains(&one.access()) && !breaking {
            if let Some((root, places)) = _places(one.register()) {
                walk.read(state.get(root), places);
            }
        }
    }
    let mut after = walk.memory(state, insn)?;
    for one in &used {
        if WRITES.contains(&one.access()) {
            if let Some((root, places)) = _places(one.register()) {
                let old = after.get(root).lanes;
                let lanes = [0, 1].map(|byte| {
                    if places.contains(&byte) {
                        Lanes::EMPTY
                    } else {
                        old[byte]
                    }
                });
                after = after.set(root, _Value { token: None, lanes });
            }
        }
    }
    Ok(after)
}

/// `finish` inside `_flow`: one return's registers and stack effect.
#[allow(clippy::too_many_arguments)]
fn _finish(
    state: &_State,
    cleanup: Option<i64>,
    returns: Option<i64>,
    walk: &mut _Walk,
    preserved: &mut Option<BTreeSet<&'static str>>,
    passes: &mut Lanes,
    exits: &mut IndexSet<(Option<i64>, Option<i64>)>,
) -> Result<(), _Opaque> {
    if !state.stack.is_empty() {
        return Err(_Opaque(
            "returns with its own words still on the stack".to_owned(),
        ));
    }
    for root in _TRACKED {
        let value = state.get(root);
        for byte in 0..2 {
            walk.inputs |= value.lanes[byte] - _lane(root, byte);
            *passes |= value.lanes[byte] & _lane(root, byte);
        }
    }
    let here: BTreeSet<&'static str> = _TRACKED
        .into_iter()
        .filter(|root| state.get(root).token == Some(Token::Name(root)))
        .collect();
    *preserved = Some(match preserved.take() {
        None => here,
        Some(preserved) => preserved.intersection(&here).copied().collect(),
    });
    exits.insert((cleanup, returns));
    Ok(())
}

/// Follow entry values through registers and stack slots to every exit.
fn _flow(routine: &_Routine, summaries: &IndexMap<Address, _Summary>) -> Result<_Summary, _Opaque> {
    let start = routine.address.2;
    let entry = _ROOTS.map(|root| _Value {
        token: Some(Token::Name(root)),
        lanes: [_lane(root, 0), _lane(root, 1)],
    });
    // Keyed by depth too: paths may meet at different depths where what
    // follows never returns (B$STALC's two ways into B$ERR_OS).
    let mut registers = entry.to_vec();
    registers.push(_Value {
        token: Some(Token::Name("bp")),
        lanes: _JUNK.lanes,
    });
    let mut states: IndexMap<(i64, usize), _State> = IndexMap::from_iter([(
        (start, 0),
        _State {
            registers,
            stack: Vec::new(),
            frame: None,
        },
    )]);
    let mut walk = _Walk::new();
    let mut preserved: Option<BTreeSet<&'static str>> = None;
    let mut passes = Lanes::EMPTY;
    let mut exits: IndexSet<(Option<i64>, Option<i64>)> = IndexSet::default();
    let mut pending = vec![(start, 0)];
    let mut visits = 0;
    while !pending.is_empty() {
        visits += 1;
        if visits > routine.instructions.len().max(1) * 100 {
            return Err(_Opaque("did not converge".to_owned()));
        }
        let key = pending.pop().unwrap();
        let (at, mut state) = (key.0, states[&key].clone());
        let insn = &routine.instructions[&at];
        let flow = insn.flow_control();
        if flow == FlowControl::Return {
            if !matches!(insn.mnemonic(), Mnemonic::Ret | Mnemonic::Retf) {
                return Err(_Opaque(format!("{}: not a plain return", _text(insn))));
            }
            let cleanup = if insn.op_count() != 0 {
                insn.immediate16() as i64
            } else {
                0
            };
            let returns = if insn.mnemonic() == Mnemonic::Retf {
                2
            } else {
                1
            };
            _finish(
                &state,
                Some(cleanup),
                Some(returns),
                &mut walk,
                &mut preserved,
                &mut passes,
                &mut exits,
            )?;
            continue;
        }
        if let Some(target) = routine.calls.get(&at) {
            let summary = match target {
                Some(target) => summaries.get(target).unwrap_or(&_UNKNOWN),
                None => &_UNKNOWN,
            };
            if flow == FlowControl::Interrupt {
                for value in &state.registers {
                    walk.read(value, &[0, 1]);
                }
                state = _State {
                    registers: state.registers.iter().map(_Value::forget).collect(),
                    stack: state.stack,
                    frame: None,
                };
            } else {
                let far = matches!(insn.op0_kind(), OpKind::FarBranch16 | OpKind::FarBranch32);
                let pushed = if matches!(flow, FlowControl::Call | FlowControl::IndirectCall) {
                    if far { 2 } else { 1 }
                } else {
                    0
                };
                let called = walk.callee(&state, summary, pushed);
                let registers: Vec<_Value> = _TRACKED
                    .iter()
                    .zip(&called.registers)
                    .map(|(root, value)| {
                        if summary.kept.contains(root) {
                            value.clone()
                        } else {
                            _Value {
                                token: None,
                                lanes: [0, 1].map(|byte| {
                                    if *root == "bp" || summary.passes.has(root, byte) {
                                        value.lanes[byte]
                                    } else {
                                        Lanes::EMPTY
                                    }
                                }),
                            }
                        }
                    })
                    .collect();
                let frame = if registers.last().unwrap().token == Some(_FRAME) {
                    called.frame
                } else {
                    None
                };
                let after = _State {
                    registers,
                    stack: called.stack,
                    frame,
                };
                if summary.never {
                    // Only a conditional branch has a path past it.
                    if flow != FlowControl::ConditionalBranch {
                        continue;
                    }
                } else if pushed != 0 {
                    let (Some(cleanup), Some(returns)) = (summary.cleanup, summary.returns) else {
                        return Err(_Opaque(format!(
                            "{}: callee's stack effect is not known",
                            _text(insn)
                        )));
                    };
                    if cleanup % 2 != 0 {
                        return Err(_Opaque(format!(
                            "{}: callee's stack effect is not known",
                            _text(insn)
                        )));
                    }
                    let words = cleanup / 2 + returns - pushed;
                    if words < 0 || (after.stack.len() as i64) < words {
                        return Err(_Opaque(format!(
                            "{}: callee pops more than was pushed",
                            _text(insn)
                        )));
                    }
                    let kept = after.stack.len() - words as usize;
                    state = _State {
                        registers: after.registers,
                        stack: after.stack[..kept].to_vec(),
                        frame: after.frame,
                    };
                } else {
                    // A jump into another routine returns from there.
                    _finish(
                        &after,
                        summary.cleanup,
                        summary.returns,
                        &mut walk,
                        &mut preserved,
                        &mut passes,
                        &mut exits,
                    )?;
                }
            }
        } else {
            state = _step(&state, insn, &mut walk)?;
        }
        for &successor in routine.successors.get(&at).into_iter().flatten() {
            let key = (successor, state.stack.len());
            let merged = match states.get(&key) {
                None => state.clone(),
                Some(old) => old.meet(&state)?,
            };
            if Some(&merged) != states.get(&key) {
                states.insert(key, merged);
                pending.push(key);
            }
        }
    }
    let (cleanup, returns) = if exits.len() == 1 {
        exits[0]
    } else {
        (None, None)
    };
    Ok(_Summary {
        inputs: walk.inputs,
        passes,
        kept: preserved.unwrap_or_default(),
        cleanup,
        returns,
        reach: walk.reach,
        never: false,
    })
}

/// Every node in a recursive component; those components stay opaque.
fn _cycles(graph: &IndexMap<Address, Rc<_Routine>>) -> Vec<Address> {
    let mut reachable: IndexMap<Address, HashSet<Address>> = IndexMap::default();
    for (start, routine) in graph {
        let mut seen: HashSet<Address> = HashSet::default();
        let mut pending: Vec<Address> = routine
            .calls
            .values()
            .filter_map(|target| *target)
            .filter(|target| graph.contains_key(target))
            .collect();
        while let Some(target) = pending.pop() {
            if seen.contains(&target) {
                continue;
            }
            seen.insert(target);
            pending.extend(
                graph[&target]
                    .calls
                    .values()
                    .filter_map(|child| *child)
                    .filter(|child| graph.contains_key(child) && !seen.contains(child)),
            );
        }
        reachable.insert(*start, seen);
    }
    reachable
        .into_iter()
        .filter(|(address, seen)| seen.contains(address))
        .map(|(address, _)| address)
        .collect()
}

fn _registers(lanes: Lanes) -> BTreeSet<Reg> {
    _ROOTS
        .iter()
        .filter(|name| !(lanes & _lanes(name)).is_empty())
        .map(|name| Reg::from_value(name).unwrap())
        .collect()
}
